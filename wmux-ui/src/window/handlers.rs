use tokio::sync::mpsc;
use winit::{
    event::KeyEvent,
    event_loop::EventLoopProxy,
    keyboard::{Key, ModifiersState, NamedKey},
};
use wmux_core::{
    surface::SplitDirection,
    surface_manager::{Surface, SurfaceManager},
    AppStateHandle, FocusDirection, PaneId, PaneState, Terminal,
};
use wmux_pty::{PtyActorHandle, PtyEvent, PtyManager, SpawnConfig};

use crate::event::WmuxEvent;
use crate::shortcuts::ShortcutAction;
use crate::sidebar::SidebarInteraction;

use super::UiState;

pub(super) fn spawn_pane_pty(
    pane_id: PaneId,
    cols: u16,
    rows: u16,
    app_state: &AppStateHandle,
    rt_handle: &tokio::runtime::Handle,
) {
    spawn_pane_pty_inner(pane_id, cols, rows, None, None, app_state, rt_handle);
}

/// Spawn a PTY for a pane during session restore, optionally injecting scrollback text
/// into the terminal before registration so it appears in the scrollback buffer.
pub(super) fn spawn_pane_pty_for_restore(
    pane_id: PaneId,
    cols: u16,
    rows: u16,
    cwd: Option<&str>,
    scrollback_text: Option<&str>,
    app_state: &AppStateHandle,
    rt_handle: &tokio::runtime::Handle,
) {
    spawn_pane_pty_inner(
        pane_id,
        cols,
        rows,
        cwd,
        scrollback_text,
        app_state,
        rt_handle,
    );
}

fn spawn_pane_pty_inner(
    pane_id: PaneId,
    cols: u16,
    rows: u16,
    cwd: Option<&str>,
    scrollback_text: Option<&str>,
    app_state: &AppStateHandle,
    rt_handle: &tokio::runtime::Handle,
) {
    // Create terminal with event channel (owned by actor via PaneState).
    let (mut terminal, terminal_event_rx) = Terminal::with_event_channel(cols, rows);

    // Inject saved scrollback before registration — this populates the terminal
    // grid/scrollback so content appears immediately on session restore.
    // SECURITY: sanitize to printable chars + CR/LF only — strip escape sequences
    // to prevent VTE injection from a tampered session.json.
    if let Some(text) = scrollback_text {
        let sanitized = sanitize_scrollback(text);
        terminal.process(sanitized.as_bytes());
    }

    // Bounded bridge channels between PTY actor and AppState actor.
    let (write_tx, mut write_rx) = mpsc::channel::<Vec<u8>>(256);
    let (resize_tx, mut resize_rx) = mpsc::channel::<(u16, u16)>(4);

    // Register pane with the actor before spawning the PTY so that any early
    // output events can be delivered to an already-registered pane.
    let pane_state = PaneState {
        terminal,
        terminal_event_rx,
        pty_write_tx: write_tx,
        pty_resize_tx: resize_tx,
        process_exited: false,
        surfaces: SurfaceManager::new(Surface::new("shell", pane_id)),
        child_pid: None,
    };
    app_state.register_pane(pane_id, pane_state);

    // Spawn PTY process.
    // SECURITY: validate CWD is a local absolute path — reject UNC paths
    // to prevent NTLM relay attacks via crafted session.json.
    let validated_cwd = cwd.and_then(|c| {
        if c.starts_with("\\\\") || c.starts_with("//") || c.contains("..") {
            tracing::warn!(cwd = c, "session restore: rejected suspicious CWD path");
            None
        } else {
            let p = std::path::Path::new(c);
            if p.is_absolute() {
                Some(c)
            } else {
                None
            }
        }
    });
    let working_directory = validated_cwd.map(std::path::PathBuf::from);
    // Keep a copy for set_pane_initial_cwd (working_directory is consumed by SpawnConfig).
    let initial_cwd = working_directory.clone();
    let manager = PtyManager::new();

    let config = SpawnConfig {
        cols,
        rows,
        working_directory,
        ..Default::default()
    };
    let handle = match manager.spawn(config) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!(error = %e, pane_id = %pane_id, "failed to spawn PTY for pane");
            return;
        }
    };

    // Capture the child shell PID before the actor consumes the handle.
    let child_pid = handle.child_pid();
    app_state.set_child_pid(pane_id, child_pid);

    // Set initial CWD in the actor for restored panes without a shell to emit OSC 7.
    if let Some(cwd) = initial_cwd {
        app_state.set_pane_initial_cwd(pane_id, cwd);
    }

    // PTY bridge task: PTY output → AppState actor, AppState input → PTY.
    let app_state_clone = app_state.clone();
    rt_handle.spawn(async move {
        let mut actor = PtyActorHandle::spawn(handle);
        loop {
            tokio::select! {
                event = actor.next_event() => {
                    match event {
                        Some(PtyEvent::Output(data)) => {
                            app_state_clone.process_pty_output(pane_id, data);
                        }
                        Some(PtyEvent::Exited { success }) => {
                            app_state_clone.mark_exited(pane_id, success);
                            break;
                        }
                        None => break,
                    }
                }
                data = write_rx.recv() => {
                    match data {
                        Some(bytes) => {
                            if actor.write(bytes).await.is_err() {
                                break;
                            }
                        }
                        None => break,
                    }
                }
                size = resize_rx.recv() => {
                    match size {
                        Some((new_rows, new_cols)) => {
                            let _ = actor.resize(new_rows, new_cols).await;
                        }
                        None => break,
                    }
                }
            }
        }
        tracing::info!(pane_id = %pane_id, "PTY bridge task ended");
    });
}

/// Apply a text editing key to a string buffer with cursor position tracking.
///
/// Handles Backspace, Delete, ArrowLeft, ArrowRight, Home, End, and character
/// insertion. Returns `true` if the key was handled, `false` if not (the caller
/// should handle Escape, Enter, and other keys).
/// Check if the key event is Ctrl+A (select all).
///
/// Uses `logical_key` (not `physical_key`) because Ctrl+A follows the key label,
/// not the physical position. On AZERTY, "A" is at QWERTY "Q" position —
/// `physical_key` would be `KeyCode::KeyQ`, but `logical_key` is correctly `"a"`.
///
/// Winit on Windows sends `logical_key: Character("\x01")` (SOH) for Ctrl+A,
/// which is the most reliable cross-platform signal.
fn is_ctrl_a(event: &KeyEvent, modifiers: &ModifiersState) -> bool {
    // Strategy 1: Ctrl+A produces SOH control character (U+0001) — works on all layouts.
    if matches!(&event.logical_key, Key::Character(ch) if ch.as_str() == "\x01") {
        return true;
    }

    // Strategy 2: logical key "a" with Ctrl modifier.
    if modifiers.control_key()
        && matches!(&event.logical_key, Key::Character(ch) if ch.as_str() == "a")
    {
        return true;
    }

    false
}

/// Check if a key event matches Ctrl+`letter` (e.g. Ctrl+V, Ctrl+C).
///
/// Uses two strategies for layout independence:
/// 1. Control character detection (Ctrl+V → SYN U+0016, Ctrl+C → ETX U+0003)
/// 2. Logical key + Ctrl modifier
fn is_ctrl_key(event: &KeyEvent, modifiers: &ModifiersState, letter: &str, ctrl_char: u8) -> bool {
    if let Key::Character(ch) = &event.logical_key {
        if ch.as_str().as_bytes() == [ctrl_char] {
            return true;
        }
    }
    modifiers.control_key()
        && matches!(&event.logical_key, Key::Character(ch) if ch.as_str().eq_ignore_ascii_case(letter))
}

/// Check if a key event is a Ctrl+letter combination that should NOT be inserted as text.
///
/// Returns true for control characters (Ctrl+A→SOH, Ctrl+C→ETX, etc.) so the caller
/// can skip text insertion. Named keys (Backspace, Delete, arrows, Home, End) always
/// pass through — they are editing keys, not Ctrl+letter combos.
fn is_ctrl_combo(event: &KeyEvent, modifiers: &ModifiersState) -> bool {
    // Named keys are never Ctrl+letter combos — always allow them through
    // so Backspace, Delete, Home, End, arrows work in all edit fields.
    if matches!(event.logical_key, Key::Named(_)) {
        return false;
    }
    if modifiers.control_key() {
        return true;
    }
    // Detect via logical_key: Ctrl+letter produces control characters (U+0001–U+001A).
    if let Key::Character(ch) = &event.logical_key {
        if ch.as_str().bytes().all(|b| b < 0x20) && !ch.is_empty() {
            return true;
        }
    }
    false
}

fn apply_text_edit_key(
    text: &mut String,
    cursor: &mut usize,
    key: &Key,
    max_len: Option<usize>,
    selected_all: &mut bool,
) -> bool {
    match key {
        Key::Named(NamedKey::Backspace) => {
            if *selected_all {
                // Delete all selected text.
                text.clear();
                *cursor = 0;
                *selected_all = false;
            } else if *cursor > 0 {
                let byte_pos = text
                    .char_indices()
                    .nth(*cursor - 1)
                    .map(|(pos, _)| pos)
                    .unwrap_or(0);
                let next_byte = text
                    .char_indices()
                    .nth(*cursor)
                    .map(|(pos, _)| pos)
                    .unwrap_or(text.len());
                text.replace_range(byte_pos..next_byte, "");
                *cursor -= 1;
            }
            true
        }
        Key::Named(NamedKey::Delete) => {
            if *selected_all {
                text.clear();
                *cursor = 0;
                *selected_all = false;
            } else {
                let char_count = text.chars().count();
                if *cursor < char_count {
                    let byte_pos = text
                        .char_indices()
                        .nth(*cursor)
                        .map(|(pos, _)| pos)
                        .unwrap_or(text.len());
                    let next_byte = text
                        .char_indices()
                        .nth(*cursor + 1)
                        .map(|(pos, _)| pos)
                        .unwrap_or(text.len());
                    text.replace_range(byte_pos..next_byte, "");
                }
            }
            true
        }
        Key::Named(NamedKey::ArrowLeft) => {
            *selected_all = false;
            if *cursor > 0 {
                *cursor -= 1;
            }
            true
        }
        Key::Named(NamedKey::ArrowRight) => {
            *selected_all = false;
            let char_count = text.chars().count();
            if *cursor < char_count {
                *cursor += 1;
            }
            true
        }
        Key::Named(NamedKey::Home) => {
            *selected_all = false;
            *cursor = 0;
            true
        }
        Key::Named(NamedKey::End) => {
            *selected_all = false;
            *cursor = text.chars().count();
            true
        }
        Key::Character(ch) => {
            let s = ch.as_str();
            if s.chars().all(|c| !c.is_control()) {
                if *selected_all {
                    // Replace all text with the typed character.
                    text.clear();
                    *cursor = 0;
                    *selected_all = false;
                }
                if let Some(max) = max_len {
                    if text.chars().count() + s.chars().count() > max {
                        return true;
                    }
                }
                let byte_pos = text
                    .char_indices()
                    .nth(*cursor)
                    .map(|(pos, _)| pos)
                    .unwrap_or(text.len());
                text.insert_str(byte_pos, s);
                *cursor += s.chars().count();
            }
            true
        }
        _ => false,
    }
}

pub(super) fn handle_sidebar_edit_key(
    state: &mut UiState<'_>,
    event: &KeyEvent,
    app_state: &AppStateHandle,
) {
    // Extract editing state; if not editing, do nothing.
    let (index, text, cursor, selected_all) = match &mut state.sidebar.interaction {
        SidebarInteraction::Editing {
            index,
            text,
            cursor,
            selected_all,
        } => (*index, text, cursor, selected_all),
        _ => return,
    };

    // Ctrl+A — select all text.
    if is_ctrl_a(event, &state.modifiers) {
        *selected_all = true;
        *cursor = text.chars().count();
        return;
    }

    // Block any other Ctrl+letter combo from inserting text.
    if is_ctrl_combo(event, &state.modifiers) {
        return;
    }

    match &event.logical_key {
        Key::Named(NamedKey::Escape) => {
            // Cancel editing — discard changes.
            state.sidebar.interaction = SidebarInteraction::Idle;
            tracing::debug!(index, "sidebar: editing cancelled");
        }
        Key::Named(NamedKey::Enter) => {
            // Commit the rename.
            let new_name = text.clone();
            if let Some(ws) = state.workspace_cache.get(index) {
                if !new_name.is_empty() && new_name != ws.name {
                    app_state.rename_workspace(ws.id, new_name);
                    tracing::debug!(index, "sidebar: workspace renamed");
                }
            }
            state.sidebar.interaction = SidebarInteraction::Idle;
        }
        other => {
            apply_text_edit_key(text, cursor, other, None, selected_all);
        }
    }
}

const MAX_TAB_TITLE_LEN: usize = 256;

pub(super) fn handle_tab_edit_key(
    state: &mut UiState<'_>,
    event: &KeyEvent,
    app_state: &AppStateHandle,
) {
    let (pane_id, surface_id, text, cursor, selected_all) = match &mut state.tab_edit {
        super::TabEditState::Editing {
            pane_id,
            surface_id,
            text,
            cursor,
            selected_all,
            ..
        } => (*pane_id, *surface_id, text, cursor, selected_all),
        super::TabEditState::None => return,
    };

    // Ctrl+A — select all text.
    if is_ctrl_a(event, &state.modifiers) {
        *selected_all = true;
        *cursor = text.chars().count();
        return;
    }

    // Block any other Ctrl+letter combo from inserting text.
    if is_ctrl_combo(event, &state.modifiers) {
        return;
    }

    match &event.logical_key {
        Key::Named(NamedKey::Escape) => {
            state.tab_edit = super::TabEditState::None;
            tracing::debug!("tab: editing cancelled");
        }
        Key::Named(NamedKey::Enter) => {
            let new_name = text.clone();
            if !new_name.is_empty() {
                app_state.rename_surface(pane_id, surface_id, new_name);
                tracing::debug!("tab: surface renamed");
            }
            state.tab_edit = super::TabEditState::None;
        }
        other => {
            apply_text_edit_key(text, cursor, other, Some(MAX_TAB_TITLE_LEN), selected_all);
        }
    }
}

pub(super) fn handle_search_key(state: &mut UiState<'_>, event: &KeyEvent) {
    match &event.logical_key {
        Key::Named(NamedKey::Escape) => {
            state.search.close();
            tracing::debug!("search closed via Escape");
        }
        Key::Named(NamedKey::Backspace) => {
            state.search.query.pop();
            if state.search.query.is_empty() {
                state.search.matches.clear();
                state.search.current_match = 0;
            }
        }
        Key::Named(NamedKey::Enter) => {
            if state.modifiers.shift_key() {
                state.search.prev_match();
            } else {
                state.search.next_match();
            }
        }
        Key::Named(NamedKey::Space) => {
            state.search.query.push(' ');
        }
        Key::Character(ch) => {
            let s = ch.as_str();
            // Filter out control characters (ASCII < 0x20) to avoid injecting
            // non-printable bytes into the search query.
            if s.chars().all(|c| !c.is_control()) {
                state.search.query.push_str(s);
            }
        }
        _ => {
            // Named keys (arrows, Tab, F-keys) are silently consumed.
        }
    }
}

/// Handle keyboard input when the browser address bar is in editing mode.
///
/// Returns `Some(url)` when the user presses Enter to navigate.
pub(super) fn handle_address_bar_key(state: &mut UiState<'_>, event: &KeyEvent) -> Option<String> {
    // Ctrl+A — select all URL text.
    if is_ctrl_a(event, &state.modifiers) {
        state.address_bar.selected_all = true;
        state.address_bar.cursor_pos = state.address_bar.url.chars().count();
        return None;
    }

    // Ctrl+V — paste from clipboard.
    if is_ctrl_key(event, &state.modifiers, "v", b'\x16') {
        if let Some(text) = state.mouse.paste_from_clipboard() {
            // Strip newlines — URL bar is single-line.
            let clean: String = text.chars().filter(|c| *c != '\n' && *c != '\r').collect();
            if state.address_bar.selected_all {
                state.address_bar.url = clean.clone();
                state.address_bar.selected_all = false;
            } else {
                let pos = state.address_bar.cursor_pos;
                let byte_pos: usize = state
                    .address_bar
                    .url
                    .chars()
                    .take(pos)
                    .map(char::len_utf8)
                    .sum();
                state.address_bar.url.insert_str(byte_pos, &clean);
            }
            state.address_bar.cursor_pos += clean.chars().count();
        }
        return None;
    }

    // Ctrl+C — copy URL to clipboard.
    if is_ctrl_key(event, &state.modifiers, "c", b'\x03') {
        state.mouse.copy_text_to_clipboard(&state.address_bar.url);
        return None;
    }

    // Ctrl+X — cut URL to clipboard.
    if is_ctrl_key(event, &state.modifiers, "x", b'\x18') {
        state.mouse.copy_text_to_clipboard(&state.address_bar.url);
        state.address_bar.url.clear();
        state.address_bar.cursor_pos = 0;
        state.address_bar.selected_all = false;
        return None;
    }

    // Block any other Ctrl+letter combo from inserting text.
    if is_ctrl_combo(event, &state.modifiers) {
        return None;
    }

    match &event.logical_key {
        Key::Named(NamedKey::Escape) => {
            state.address_bar.cancel_editing();
            tracing::debug!("address bar edit cancelled via Escape");
            None
        }
        Key::Named(NamedKey::Enter) => {
            let url = state.address_bar.confirm_editing();
            tracing::debug!(url = %url, "address bar navigation confirmed");
            Some(url)
        }
        Key::Named(NamedKey::Space) => {
            // Space is a NamedKey, not Key::Character — insert explicitly.
            apply_text_edit_key(
                &mut state.address_bar.url,
                &mut state.address_bar.cursor_pos,
                &Key::Character(" ".into()),
                None,
                &mut state.address_bar.selected_all,
            );
            None
        }
        key => {
            apply_text_edit_key(
                &mut state.address_bar.url,
                &mut state.address_bar.cursor_pos,
                key,
                None,
                &mut state.address_bar.selected_all,
            );
            None
        }
    }
}

/// Handle keyboard input when the command palette is open.
pub(super) fn handle_palette_key(
    state: &mut UiState<'_>,
    event: &KeyEvent,
    app_state: &AppStateHandle,
    rt_handle: &tokio::runtime::Handle,
    proxy: &winit::event_loop::EventLoopProxy<crate::event::WmuxEvent>,
) {
    match &event.logical_key {
        Key::Named(NamedKey::Escape) => {
            state.command_palette.close();
        }
        Key::Named(NamedKey::Backspace) => {
            state.command_palette.query.pop();
            state.command_palette.selected = 0;
        }
        Key::Named(NamedKey::ArrowDown) => {
            // Skip over section-header rows so the user lands on selectable items.
            let count = state.command_palette.result_count;
            for _ in 0..count.max(1) {
                state.command_palette.select_next();
                let idx = state.command_palette.selected_index();
                let is_section = idx
                    .and_then(|i| state.palette_row_sections.get(i))
                    .is_some_and(|s| s.is_some());
                if !is_section {
                    break;
                }
            }
        }
        Key::Named(NamedKey::ArrowUp) => {
            let count = state.command_palette.result_count;
            for _ in 0..count.max(1) {
                state.command_palette.select_prev();
                let idx = state.command_palette.selected_index();
                let is_section = idx
                    .and_then(|i| state.palette_row_sections.get(i))
                    .is_some_and(|s| s.is_some());
                if !is_section {
                    break;
                }
            }
        }
        Key::Named(NamedKey::Tab) => {
            if state.modifiers.shift_key() {
                state.command_palette.prev_filter();
            } else {
                state.command_palette.next_filter();
            }
        }
        Key::Named(NamedKey::Enter) => {
            // Use palette_actions populated during the last render frame.
            if let Some(idx) = state.command_palette.selected_index() {
                if let Some(action) = state.palette_actions.get(idx).cloned() {
                    state.command_palette.close();
                    match action {
                        crate::command_palette::PaletteAction::Command(ref id) => {
                            if let Some(sa) = crate::command_palette::command_id_to_action(id) {
                                handle_shortcut(sa, state, app_state, rt_handle, proxy);
                            } else {
                                tracing::warn!(command_id = %id, "unhandled command ID from palette — missing mapping in command_id_to_action");
                            }
                        }
                        crate::command_palette::PaletteAction::SwitchWorkspace(n) => {
                            handle_shortcut(
                                ShortcutAction::SwitchWorkspace(n),
                                state,
                                app_state,
                                rt_handle,
                                proxy,
                            );
                        }
                        crate::command_palette::PaletteAction::FocusSurface(pane_id, tab_idx) => {
                            app_state.cycle_surface_to_index(pane_id, tab_idx);
                            state.set_focused_pane(pane_id);
                            state.window.request_redraw();
                        }
                    }
                } else {
                    state.command_palette.close();
                }
            } else {
                state.command_palette.close();
            }
        }
        Key::Named(NamedKey::Space) => {
            state.command_palette.query.push(' ');
            state.command_palette.selected = 0;
        }
        Key::Character(ch) => {
            let s = ch.as_str();
            if s.chars().all(|c| !c.is_control()) {
                state.command_palette.query.push_str(s);
                state.command_palette.selected = 0;
            }
        }
        _ => {
            // All other keys consumed silently (don't leak to terminal).
        }
    }
}

/// Spawn a new split pane. When `before` is true, the new pane is placed
/// before the original (left for horizontal, above for vertical) by swapping
/// the two panes after the split.
pub(super) fn spawn_split(
    direction: SplitDirection,
    before: bool,
    state: &mut UiState<'_>,
    app_state: &AppStateHandle,
    rt_handle: &tokio::runtime::Handle,
    proxy: &EventLoopProxy<WmuxEvent>,
) {
    let pane_id = state.focused_pane;
    let cols = state.cols;
    let rows = state.rows;
    let app_state_clone = app_state.clone();
    let rt_clone = rt_handle.clone();
    let proxy_clone = proxy.clone();
    rt_handle.spawn(async move {
        match app_state_clone.split_pane(pane_id, direction).await {
            Ok(new_id) => {
                // For "before" splits (left/up), swap so new pane occupies the first position.
                if before {
                    app_state_clone.swap_panes(pane_id, new_id);
                }
                spawn_pane_pty(new_id, cols, rows, &app_state_clone, &rt_clone);
                app_state_clone.focus_pane(new_id);
                let _ = proxy_clone.send_event(WmuxEvent::FocusPane(new_id));
                tracing::info!(
                    pane_id = %pane_id,
                    new_pane = %new_id,
                    ?direction,
                    before,
                    "pane split"
                );
            }
            Err(e) => {
                tracing::warn!(error = %e, ?direction, "split failed");
            }
        }
    });
    state.window.request_redraw();
}

/// Takes the app_state handle and rt_handle by reference to avoid borrow
/// conflicts with the mutable UiState borrow in the event handler.
pub(super) fn handle_shortcut(
    action: ShortcutAction,
    state: &mut UiState<'_>,
    app_state: &AppStateHandle,
    rt_handle: &tokio::runtime::Handle,
    proxy: &EventLoopProxy<WmuxEvent>,
) {
    match action {
        ShortcutAction::SplitRight => {
            spawn_split(
                SplitDirection::Horizontal,
                false,
                state,
                app_state,
                rt_handle,
                proxy,
            );
        }

        ShortcutAction::SplitLeft => {
            spawn_split(
                SplitDirection::Horizontal,
                true,
                state,
                app_state,
                rt_handle,
                proxy,
            );
        }

        ShortcutAction::SplitDown => {
            spawn_split(
                SplitDirection::Vertical,
                false,
                state,
                app_state,
                rt_handle,
                proxy,
            );
        }

        ShortcutAction::SplitUp => {
            spawn_split(
                SplitDirection::Vertical,
                true,
                state,
                app_state,
                rt_handle,
                proxy,
            );
        }

        ShortcutAction::ChordPrefix => {
            // Handled by the chord state machine in event_loop — should not reach here.
            tracing::debug!("ChordPrefix action reached handler (no-op)");
        }

        ShortcutAction::ClosePane => {
            let closing = state.focused_pane;
            app_state.close_pane(closing);

            // After closing, get the updated layout to find another pane to focus.
            let viewport = wmux_core::rect::Rect::new(
                0.0,
                0.0,
                state.gpu.width() as f32,
                state.gpu.height() as f32,
            );
            let (layout, _) = rt_handle.block_on(app_state.get_layout(viewport));
            if let Some((next_id, _)) = layout.first() {
                state.set_focused_pane(*next_id);
                app_state.focus_pane(*next_id);
            } else {
                // Last pane closed — exit the application.
                tracing::info!("last pane closed, shutting down");
                app_state.shutdown();
                state.window.request_redraw();
            }
            state.window.request_redraw();
        }

        ShortcutAction::ZoomToggle => {
            app_state.toggle_zoom(state.focused_pane);
            state.window.request_redraw();
        }

        ShortcutAction::FocusUp => {
            app_state.navigate_focus(FocusDirection::Up);
            state.window.request_redraw();
        }

        ShortcutAction::FocusDown => {
            app_state.navigate_focus(FocusDirection::Down);
            state.window.request_redraw();
        }

        ShortcutAction::FocusLeft => {
            app_state.navigate_focus(FocusDirection::Left);
            state.window.request_redraw();
        }

        ShortcutAction::FocusRight => {
            app_state.navigate_focus(FocusDirection::Right);
            state.window.request_redraw();
        }

        ShortcutAction::NewWorkspace => {
            let cols = state.cols;
            let rows = state.rows;
            let app_state_clone = app_state.clone();
            let rt_clone = rt_handle.clone();
            let proxy_clone = proxy.clone();
            rt_handle.spawn(async move {
                // 1. Create workspace (actor auto-switches to it)
                let _ws_id = app_state_clone
                    // TODO(L2_16): route through i18n system when wmux-config i18n is implemented.
                    .create_workspace("New Workspace".to_owned())
                    .await;

                // 2. Spawn a pane with PTY in the new (now active) workspace
                let pane_id = PaneId::new();
                spawn_pane_pty(pane_id, cols, rows, &app_state_clone, &rt_clone);

                // 3. Focus the new pane
                app_state_clone.focus_pane(pane_id);
                let _ = proxy_clone.send_event(WmuxEvent::FocusPane(pane_id));

                tracing::info!(pane_id = %pane_id, "new workspace with pane created");
            });
        }

        ShortcutAction::CloseWorkspace => {
            let app_clone = app_state.clone();
            let proxy_clone = proxy.clone();
            rt_handle.spawn(async move {
                if let Some(ws) = app_clone.get_current_workspace().await {
                    app_clone.close_workspace(ws.id);
                    tracing::info!(workspace_id = %ws.id, "workspace closed via shortcut");
                    // After closing, the actor switches to an adjacent workspace.
                    // Query the new layout to update focused_pane so the UI doesn't
                    // reference a dead pane ID (stale focus → input lost, no cursor).
                    let viewport = wmux_core::rect::Rect::new(0.0, 0.0, 1920.0, 1080.0);
                    let (layout, _) = app_clone.get_layout(viewport).await;
                    if let Some((first_pane, _)) = layout.first() {
                        let _ = proxy_clone.send_event(WmuxEvent::FocusPane(*first_pane));
                    }
                }
            });
            state.window.request_redraw();
        }

        ShortcutAction::SwitchWorkspace(n) => {
            // n is 1-based; switch_workspace takes 0-based index.
            let index = (n as usize).saturating_sub(1);
            app_state.switch_workspace(index);
            state.window.request_redraw();
        }

        ShortcutAction::NewSurface => {
            let layout_pane_id = state.focused_pane;
            let new_pane_id = PaneId::new();
            let cols = state.cols;
            let rows = state.rows;
            let app_clone = app_state.clone();
            let rt_clone = rt_handle.clone();
            rt_handle.spawn(async move {
                // 1. Spawn PTY (registers backing PaneState in actor).
                spawn_pane_pty(new_pane_id, cols, rows, &app_clone, &rt_clone);
                // 2. Register as surface in the layout pane.
                match app_clone.create_surface(layout_pane_id, new_pane_id).await {
                    Ok(sid) => tracing::info!(
                        surface_id = %sid,
                        backing = %new_pane_id,
                        "new surface created",
                    ),
                    Err(e) => tracing::warn!(error = %e, "create surface failed"),
                }
            });
            state.window.request_redraw();
        }
        ShortcutAction::NewBrowserSurface => {
            if state.browser_manager.is_none() {
                tracing::warn!(
                    "browser integration not available (WebView2 runtime not installed)"
                );
                state.window.request_redraw();
                return;
            }

            // Create a browser tab: backing pane + browser surface.
            // The actual WebView2 panel creation is DEFERRED — it happens in
            // user_event(WmuxEvent::CreateBrowserPanel) to avoid deadlocking
            // the Win32 message pump (COM callbacks need the message loop).
            let layout_pane_id = state.focused_pane;
            let backing_pane_id = PaneId::new();
            let cols = state.cols;
            let rows = state.rows;

            // Spawn backing PTY (needed by the surface system).
            spawn_pane_pty(backing_pane_id, cols, rows, app_state, rt_handle);

            // Create browser surface in the actor, then trigger panel creation.
            let app_clone = app_state.clone();
            let proxy_clone = proxy.clone();
            let default_url = state.browser_default_url.clone();
            rt_handle.spawn(async move {
                match app_clone
                    .create_browser_surface(layout_pane_id, backing_pane_id)
                    .await
                {
                    Ok(surface_id) => {
                        tracing::info!(surface_id = %surface_id, "browser surface registered, requesting panel creation");
                        let _ = proxy_clone.send_event(crate::event::WmuxEvent::CreateBrowserPanel {
                            surface_id,
                            url: default_url,
                        });
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "failed to create browser surface");
                    }
                }
            });

            state.window.request_redraw();
        }
        ShortcutAction::CycleSurfaceForward => {
            app_state.cycle_surface(state.focused_pane, true);
            state.window.request_redraw();
        }
        ShortcutAction::CycleSurfaceBackward => {
            app_state.cycle_surface(state.focused_pane, false);
            state.window.request_redraw();
        }

        ShortcutAction::Copy => {
            if let Some(sel) = state.mouse.selection() {
                let sel_clone = sel.clone();
                let text =
                    rt_handle.block_on(app_state.extract_selection(state.focused_pane, sel_clone));
                if let Some(text) = text {
                    state.mouse.copy_text_to_clipboard(&text);
                }
            }
        }

        ShortcutAction::Paste => {
            if let Some(text) = state.mouse.paste_from_clipboard() {
                let bytes = state
                    .input
                    .wrap_bracketed_paste(&text, state.terminal_modes);
                app_state.send_input(state.focused_pane, bytes);
                state.window.request_redraw();
            }
        }

        ShortcutAction::ToggleSidebar => {
            state.sidebar.toggle_collapsed();
            // Persist collapsed state for session save.
            let size = state.window.inner_size();
            let pos = state.window.outer_position().unwrap_or_default();
            app_state.update_ui_state(
                state.sidebar.width as u16,
                state.sidebar.collapsed,
                Some(wmux_core::WindowGeometry {
                    x: pos.x,
                    y: pos.y,
                    width: size.width,
                    height: size.height,
                    maximized: state.window.is_maximized(),
                }),
            );
            state.window.request_redraw();
        }

        // Placeholders for future tasks.
        ShortcutAction::CommandPalette => {
            if state.command_palette.open {
                state.command_palette.close();
            } else {
                // Close other overlays — only one at a time.
                if state.search.active {
                    state.search.close();
                }
                if state.notification_panel.open {
                    state.notification_panel.toggle();
                }
                state.command_palette.open();
            }
            state.window.request_redraw();
        }
        ShortcutAction::Find => {
            // Close other overlays — only one at a time.
            if state.command_palette.open {
                state.command_palette.close();
            }
            if state.notification_panel.open {
                state.notification_panel.toggle();
            }
            if state.search.active {
                state.search.close();
                tracing::debug!("search closed via Ctrl+F toggle");
            } else {
                state.search.open();
                tracing::debug!("search opened via Ctrl+F");
            }
            state.window.request_redraw();
        }
        ShortcutAction::ToggleDevTools => {
            tracing::debug!("ToggleDevTools shortcut (placeholder)");
        }
        ShortcutAction::NotificationPanelToggle => {
            // Close other overlays for mutual exclusion.
            if state.command_palette.open {
                state.command_palette.close();
            }
            if state.search.active {
                state.search.close();
            }
            state.notification_panel.toggle();
            state.window.request_redraw();
        }
        ShortcutAction::JumpLastUnread => {
            // Fetch notifications directly from actor (cache may be empty when panel is closed).
            let notifs = rt_handle.block_on(app_state.list_notifications(50));
            if let Some(notif) = notifs.iter().find(|n| {
                n.state == wmux_core::NotificationState::Received
                    || n.state == wmux_core::NotificationState::Unread
            }) {
                if let Some(ws_id) = notif.source_workspace {
                    let app = app_state.clone();
                    rt_handle.spawn(async move {
                        app.select_workspace_by_id(ws_id).await;
                    });
                    tracing::debug!(%ws_id, "jumped to last unread notification workspace");
                }
            }
            state.window.request_redraw();
        }
    }
}

/// Sanitize scrollback text for safe VTE injection during session restore.
///
/// Strips all control characters except `\r` and `\n`, and normalizes bare `\n`
/// to `\r\n` so lines render left-aligned. This prevents escape sequence
/// injection from a tampered session.json.
fn sanitize_scrollback(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut prev_cr = false;
    for ch in text.chars() {
        if ch == '\r' {
            out.push('\r');
            prev_cr = true;
        } else if ch == '\n' {
            if !prev_cr {
                out.push('\r');
            }
            out.push('\n');
            prev_cr = false;
        } else if ch.is_control() {
            // Strip all other control characters (ESC, BEL, etc.)
            prev_cr = false;
        } else {
            out.push(ch);
            prev_cr = false;
        }
    }
    out
}
