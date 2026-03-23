use super::UiState;
use crate::event::WmuxEvent;
use crate::shortcuts::ShortcutAction;
use crate::sidebar::SidebarInteraction;
use tokio::sync::mpsc;
use winit::event::KeyEvent;
use winit::event_loop::EventLoopProxy;
use winit::keyboard::{Key, NamedKey};
use wmux_core::surface::SplitDirection;
use wmux_core::surface_manager::{Surface, SurfaceManager};
use wmux_core::{AppStateHandle, FocusDirection, PaneId, PaneState, Terminal};
use wmux_pty::{PtyActorHandle, PtyEvent, PtyManager, SpawnConfig};

pub(super) fn spawn_pane_pty(
    pane_id: PaneId,
    cols: u16,
    rows: u16,
    app_state: &AppStateHandle,
    rt_handle: &tokio::runtime::Handle,
) {
    spawn_pane_pty_with_cwd(pane_id, cols, rows, None, app_state, rt_handle);
}

/// Spawn a PTY for a pane with an optional working directory (used by session restore).
pub(super) fn spawn_pane_pty_with_cwd(
    pane_id: PaneId,
    cols: u16,
    rows: u16,
    cwd: Option<&str>,
    app_state: &AppStateHandle,
    rt_handle: &tokio::runtime::Handle,
) {
    // Create terminal with event channel (owned by actor via PaneState).
    let (terminal, terminal_event_rx) = Terminal::with_event_channel(cols, rows);

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
    };
    app_state.register_pane(pane_id, pane_state);

    // Spawn PTY process.
    let manager = PtyManager::new();
    let config = SpawnConfig {
        cols,
        rows,
        working_directory: cwd.map(std::path::PathBuf::from),
        ..Default::default()
    };
    let handle = match manager.spawn(config) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!(error = %e, pane_id = %pane_id, "failed to spawn PTY for new pane");
            return;
        }
    };

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

pub(super) fn handle_sidebar_edit_key(
    state: &mut UiState<'_>,
    event: &KeyEvent,
    app_state: &AppStateHandle,
) {
    // Extract editing state; if not editing, do nothing.
    let (index, text, cursor) = match &mut state.sidebar.interaction {
        SidebarInteraction::Editing {
            index,
            text,
            cursor,
        } => (*index, text, cursor),
        _ => return,
    };

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
        Key::Named(NamedKey::Backspace) => {
            if *cursor > 0 {
                // Remove the character before the cursor (byte-aware).
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
        }
        Key::Named(NamedKey::Delete) => {
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
        Key::Named(NamedKey::ArrowLeft) => {
            if *cursor > 0 {
                *cursor -= 1;
            }
        }
        Key::Named(NamedKey::ArrowRight) => {
            let char_count = text.chars().count();
            if *cursor < char_count {
                *cursor += 1;
            }
        }
        Key::Named(NamedKey::Home) => {
            *cursor = 0;
        }
        Key::Named(NamedKey::End) => {
            *cursor = text.chars().count();
        }
        Key::Character(ch) => {
            let s = ch.as_str();
            // Filter out control characters.
            if s.chars().all(|c| !c.is_control()) {
                // Insert at cursor position (byte-aware).
                let byte_pos = text
                    .char_indices()
                    .nth(*cursor)
                    .map(|(pos, _)| pos)
                    .unwrap_or(text.len());
                text.insert_str(byte_pos, s);
                *cursor += s.chars().count();
            }
        }
        _ => {
            // Other named keys silently consumed.
        }
    }
}

const MAX_TAB_TITLE_LEN: usize = 256;

pub(super) fn handle_tab_edit_key(
    state: &mut UiState<'_>,
    event: &KeyEvent,
    app_state: &AppStateHandle,
) {
    let (pane_id, surface_id, text, cursor) = match &mut state.tab_edit {
        super::TabEditState::Editing {
            pane_id,
            surface_id,
            text,
            cursor,
            ..
        } => (*pane_id, *surface_id, text, cursor),
        super::TabEditState::None => return,
    };

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
        Key::Named(NamedKey::Backspace) => {
            if *cursor > 0 {
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
        }
        Key::Named(NamedKey::Delete) => {
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
        Key::Named(NamedKey::ArrowLeft) => {
            if *cursor > 0 {
                *cursor -= 1;
            }
        }
        Key::Named(NamedKey::ArrowRight) => {
            let char_count = text.chars().count();
            if *cursor < char_count {
                *cursor += 1;
            }
        }
        Key::Named(NamedKey::Home) => {
            *cursor = 0;
        }
        Key::Named(NamedKey::End) => {
            *cursor = text.chars().count();
        }
        Key::Character(ch) => {
            let s = ch.as_str();
            if s.chars().all(|c| !c.is_control())
                && text.chars().count() + s.chars().count() <= MAX_TAB_TITLE_LEN
            {
                let byte_pos = text
                    .char_indices()
                    .nth(*cursor)
                    .map(|(pos, _)| pos)
                    .unwrap_or(text.len());
                text.insert_str(byte_pos, s);
                *cursor += s.chars().count();
            }
        }
        _ => {}
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
            let layout = rt_handle.block_on(app_state.get_layout(viewport));
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
            rt_handle.spawn(async move {
                match app_clone
                    .create_browser_surface(layout_pane_id, backing_pane_id)
                    .await
                {
                    Ok(surface_id) => {
                        tracing::info!(surface_id = %surface_id, "browser surface registered, requesting panel creation");
                        let _ = proxy_clone.send_event(crate::event::WmuxEvent::CreateBrowserPanel {
                            surface_id,
                            url: "about:blank".to_owned(),
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
            state.sidebar.toggle();
            state.window.request_redraw();
        }

        // Placeholders for future tasks.
        ShortcutAction::CommandPalette => {
            tracing::debug!("CommandPalette shortcut (placeholder — Task L4_01)");
        }
        ShortcutAction::Find => {
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
            tracing::debug!("NotificationPanelToggle shortcut (placeholder — L3_09 UI wiring)");
        }
        ShortcutAction::JumpLastUnread => {
            tracing::debug!("JumpLastUnread shortcut (placeholder — L3_09 UI wiring)");
        }
    }
}
