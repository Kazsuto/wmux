use crate::divider::{self, DividerOrientation};
use crate::event::WmuxEvent;
use crate::mouse::MouseButton;
use crate::sidebar::SidebarInteraction;
use crate::toast;
use crate::UiError;
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseScrollDelta, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::ModifiersState,
    window::{WindowAttributes, WindowId},
};
use wmux_config::derive_ui_chrome;
use wmux_core::{AppEvent, PaneId, TerminalMode};
use wmux_render::GpuContext;

use super::{handlers, App, TabDragState, UiState};

impl<'window> ApplicationHandler<WmuxEvent> for App<'window> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let attrs = WindowAttributes::default()
            .with_title("wmux")
            .with_inner_size(winit::dpi::LogicalSize::new(1200, 800));

        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("failed to create window"),
        );

        let gpu =
            pollster::block_on(GpuContext::new(window.clone())).expect("failed to initialize GPU");

        let quads = wmux_render::QuadPipeline::new(
            &gpu.device,
            &gpu.queue,
            gpu.format,
            gpu.width(),
            gpu.height(),
        );

        let mut glyphon = wmux_render::GlyphonRenderer::new(&gpu.device, &gpu.queue, gpu.format);
        glyphon.resize(&gpu.queue, gpu.width(), gpu.height());

        // Compute terminal dimensions from window size and font metrics
        let metrics = wmux_render::TerminalMetrics::new(glyphon.font_system());
        let cols = ((gpu.width() as f32) / metrics.cell_width).floor().max(1.0) as u32;
        let rows = ((gpu.height() as f32) / metrics.cell_height)
            .floor()
            .max(1.0) as u32;
        let cols = cols.min(u16::MAX as u32) as u16;
        let rows = rows.min(u16::MAX as u32) as u16;

        // Per-pane renderers are created lazily in the render loop.
        let renderers = std::collections::HashMap::new();

        // Initialize Windows Toast notification support.
        toast::init_aumid();
        let toast_service = crate::toast::ToastService::new();

        // Generate pane ID and delegate PTY spawn + registration to shared helper.
        let pane_id = PaneId::new();
        handlers::spawn_pane_pty(pane_id, cols, rows, &self.app_state, &self.rt_handle);

        // Event forwarding task: reads AppEvent → sends WmuxEvent via EventLoopProxy.
        if let Some(mut event_rx) = self.app_event_rx.take() {
            let proxy_fwd = self.proxy.clone();
            self.rt_handle.spawn(async move {
                while let Some(event) = event_rx.recv().await {
                    match event {
                        AppEvent::PaneNeedsRedraw(_) => {
                            let _ = proxy_fwd.send_event(WmuxEvent::PtyOutput);
                        }
                        AppEvent::NotificationAdded {
                            notification,
                            suppressed,
                        } => {
                            if !suppressed {
                                let _ = proxy_fwd.send_event(WmuxEvent::ShowToast(notification));
                            }
                        }
                        AppEvent::PaneExited { success, .. } => {
                            let _ = proxy_fwd.send_event(WmuxEvent::PtyExited { success });
                        }
                        AppEvent::FocusChanged { pane_id } => {
                            let _ = proxy_fwd.send_event(WmuxEvent::FocusPane(pane_id));
                        }
                        // Workspace events are handled by the sidebar (Task L2_08).
                        AppEvent::WorkspaceCreated { .. }
                        | AppEvent::WorkspaceSwitched { .. }
                        | AppEvent::WorkspaceClosed { .. } => {}
                    }
                }
                tracing::info!("event forwarding task ended");
            });
        }

        // Load theme from config, apply overrides, then derive UI chrome.
        let config = wmux_config::Config::default();
        let mut theme_engine = wmux_config::ThemeEngine::new();
        if let Err(e) = theme_engine.set_theme(&config.theme) {
            tracing::warn!(theme = %config.theme, error = %e, "failed to load configured theme, using default");
        }
        let mut palette = theme_engine.current_theme().palette.clone();
        // Apply per-color config overrides on top of the loaded theme.
        if let Some(ref bg) = config.background {
            if let Ok(c) = wmux_config::parse_hex_color_public(bg) {
                palette.background = c;
            }
        }
        if let Some(ref fg) = config.foreground {
            if let Ok(c) = wmux_config::parse_hex_color_public(fg) {
                palette.foreground = c;
            }
        }
        for (i, slot) in config.palette.iter().enumerate() {
            if let Some(ref hex) = slot {
                if let Ok(c) = wmux_config::parse_hex_color_public(hex) {
                    palette.ansi[i] = c;
                }
            }
        }
        let ui_chrome = derive_ui_chrome(&palette);
        let theme_ansi = palette.ansi;
        let theme_cursor = palette.cursor;
        let theme_foreground = palette.foreground;

        let dark_mode = wmux_config::ThemeEngine::is_dark_mode();
        let title_colors = crate::effects::TitleBarColors {
            background: palette.background,
            text: palette.foreground,
            border: palette.background, // seamless border
        };
        let effect_result = crate::effects::apply_window_effects(&window, dark_mode, &title_colors);

        tracing::info!(
            cols,
            rows,
            width = gpu.width(),
            height = gpu.height(),
            format = ?gpu.format,
            pane_id = %pane_id,
            "terminal initialized (actor pattern)",
        );

        self.state = Some(UiState {
            window,
            gpu,
            quads,
            glyphon,
            renderers,
            metrics,
            input: crate::input::InputHandler::new(),
            mouse: crate::mouse::MouseHandler::new(),
            shortcuts: crate::shortcuts::ShortcutMap::new(),
            modifiers: ModifiersState::default(),
            cursor_pos: (0.0, 0.0),
            toast_service,
            sidebar: crate::sidebar::SidebarState::new(220),
            workspace_cache: Vec::new(),
            dividers: Vec::new(),
            drag_state: None,
            focused_pane: pane_id,
            cols,
            rows,
            process_exited: false,
            terminal_modes: TerminalMode::empty(),
            last_layout: Vec::new(),
            search: crate::search::SearchState::new(),
            last_search_rows: Vec::new(),
            last_total_visible_rows: 0,
            tab_title_buffers: std::collections::HashMap::new(),
            last_viewports: Vec::new(),
            tab_drag: TabDragState::None,
            ui_chrome,
            effect_result,
            theme_ansi,
            theme_cursor,
            theme_foreground,
            inactive_pane_opacity: wmux_config::Config::default().inactive_pane_opacity,
        });
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: WmuxEvent) {
        match event {
            WmuxEvent::PtyOutput => {
                if let Some(state) = self.state.as_ref() {
                    state.window.request_redraw();
                }
            }
            WmuxEvent::PtyExited { success } => {
                if let Some(state) = self.state.as_mut() {
                    state.process_exited = true;
                    state.window.request_redraw();
                    tracing::info!(success, "shell process exited");
                }
            }
            WmuxEvent::ShowToast(notification) => {
                if let Some(state) = self.state.as_ref() {
                    state.toast_service.show(&notification);
                }
            }
            WmuxEvent::FocusPane(pane_id) => {
                if let Some(state) = self.state.as_mut() {
                    state.focused_pane = pane_id;
                    state.window.request_redraw();
                }
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = self.state.as_mut() else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => {
                tracing::info!("window close requested");
                self.app_state.shutdown();
                event_loop.exit();
            }

            WindowEvent::Resized(physical_size) => {
                let w = physical_size.width;
                let h = physical_size.height;
                if w > 0 && h > 0 {
                    // GPU resize
                    state.gpu.resize(w, h);
                    state.quads.resize(&state.gpu.queue, w, h);
                    state.glyphon.resize(&state.gpu.queue, w, h);

                    // Terminal resize
                    let new_cols = ((w as f32) / state.metrics.cell_width).floor().max(1.0) as u32;
                    let new_rows = ((h as f32) / state.metrics.cell_height).floor().max(1.0) as u32;
                    let new_cols = new_cols.min(u16::MAX as u32) as u16;
                    let new_rows = new_rows.min(u16::MAX as u32) as u16;

                    if new_cols != state.cols || new_rows != state.rows {
                        state.cols = new_cols;
                        state.rows = new_rows;
                        // Per-pane renderer + PTY resizing is handled in render()
                        // based on each pane's actual rect dimensions.
                    }

                    state.window.request_redraw();
                }
            }

            WindowEvent::RedrawRequested => match state.render(&self.app_state, &self.rt_handle) {
                Ok(()) => {}
                Err(UiError::Surface(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated)) => {
                    let w = state.gpu.width();
                    let h = state.gpu.height();
                    state.gpu.resize(w, h);
                    state.window.request_redraw();
                }
                Err(e) => {
                    tracing::error!(error = %e, "render failed");
                }
            },

            WindowEvent::ModifiersChanged(modifiers) => {
                state.modifiers = modifiers.state();
            }

            WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } => {
                if event.state != ElementState::Pressed {
                    return;
                }

                // Priority 0: sidebar inline editing — intercept all keys when renaming.
                if state.sidebar.is_editing() {
                    handlers::handle_sidebar_edit_key(state, &event, &self.app_state);
                    state.window.request_redraw();
                    return;
                }

                // Priority 1: global shortcuts — intercepted before terminal input.
                // Shortcuts must work even when the focused pane's process has exited
                // (so the user can close panes, switch workspaces, etc.).
                // Match the shortcut regardless of repeat state, but only execute
                // the action on the first press. Repeated keys are consumed (return)
                // to prevent shortcut key combos from leaking to the PTY as raw
                // control bytes (e.g. Ctrl+D → 0x04 EOF sent to the shell).
                if let Some(action) = state.shortcuts.match_shortcut(
                    &event.logical_key,
                    event.physical_key,
                    &state.modifiers,
                ) {
                    if !event.repeat {
                        handlers::handle_shortcut(
                            action,
                            state,
                            &self.app_state,
                            &self.rt_handle,
                            &self.proxy,
                        );
                    }
                    return;
                }

                // Don't send input to a dead process.
                if state.process_exited {
                    return;
                }

                // Priority 1.5: search overlay input — intercepted when search is active.
                if state.search.active {
                    handlers::handle_search_key(state, &event);
                    state.window.request_redraw();
                    return;
                }

                // Priority 2: regular key input → actor → PTY
                if let Some(bytes) =
                    state
                        .input
                        .handle_key_event(&event, &state.modifiers, state.terminal_modes)
                {
                    self.app_state.reset_viewport(state.focused_pane);
                    state.mouse.clear_selection();
                    self.app_state.send_input(state.focused_pane, bytes);
                    state.window.request_redraw();
                }
            }

            WindowEvent::MouseInput {
                state: elem_state,
                button,
                ..
            } => {
                let px = state.cursor_pos.0 as f32;
                let py = state.cursor_pos.1 as f32;

                // Sidebar mouse interaction — click to select, drag to reorder, double-click to rename.
                if state.sidebar.visible && px < state.sidebar.effective_width() {
                    if elem_state == ElementState::Pressed
                        && button == winit::event::MouseButton::Left
                    {
                        // Track click count for double-click detection.
                        let mouse_mode =
                            state.terminal_modes.contains(TerminalMode::MOUSE_REPORTING);
                        let shift = state.modifiers.shift_key();
                        let (col, row) = state.cursor_cell();
                        let _ = state.mouse.handle_mouse_press(
                            col,
                            row,
                            MouseButton::Left,
                            shift,
                            mouse_mode,
                        );
                        // We only needed click counting — discard the terminal
                        // selection that handle_mouse_press created.
                        state.mouse.clear_selection();

                        if let Some(row_index) =
                            state.sidebar.hit_test_row(py, state.workspace_cache.len())
                        {
                            if state.mouse.click_count() >= 2 {
                                // Double-click: start inline editing.
                                let name = state
                                    .workspace_cache
                                    .get(row_index)
                                    .map(|ws| ws.name.clone())
                                    .unwrap_or_default();
                                let cursor = name.chars().count();
                                state.sidebar.interaction = SidebarInteraction::Editing {
                                    index: row_index,
                                    text: name,
                                    cursor,
                                };
                                tracing::debug!(row_index, "sidebar: started inline editing");
                            } else {
                                // Single press: start tracking for click vs drag.
                                state.sidebar.interaction = SidebarInteraction::Pressing {
                                    row: row_index,
                                    start_y: py,
                                };
                            }
                        }
                    } else if elem_state == ElementState::Released
                        && button == winit::event::MouseButton::Left
                    {
                        match state.sidebar.interaction.clone() {
                            SidebarInteraction::Pressing { row, .. } => {
                                // Click completed without drag → switch workspace.
                                self.app_state.switch_workspace(row);
                                tracing::debug!(row, "sidebar: workspace selected via click");
                                state.sidebar.interaction = SidebarInteraction::Idle;
                            }
                            SidebarInteraction::Dragging {
                                from_row,
                                current_y,
                            } => {
                                // Drag completed → reorder workspace.
                                let target = state
                                    .sidebar
                                    .drag_target_index(current_y, state.workspace_cache.len());
                                if target != from_row {
                                    self.app_state.reorder_workspace(from_row, target);
                                    tracing::debug!(
                                        from_row,
                                        target,
                                        "sidebar: workspace reordered via drag"
                                    );
                                }
                                state.sidebar.interaction = SidebarInteraction::Idle;
                            }
                            _ => {
                                // Release in other states (Editing, Idle, Hover) — no-op.
                            }
                        }
                    }

                    // Click in sidebar area — cancel editing if clicking outside edit row.
                    if elem_state == ElementState::Pressed {
                        if let SidebarInteraction::Editing {
                            index, ref text, ..
                        } = state.sidebar.interaction
                        {
                            if let Some(row_index) =
                                state.sidebar.hit_test_row(py, state.workspace_cache.len())
                            {
                                if row_index != index {
                                    // Commit the edit on click-away.
                                    if let Some(ws) = state.workspace_cache.get(index) {
                                        if !text.is_empty() && *text != ws.name {
                                            self.app_state.rename_workspace(ws.id, text.clone());
                                        }
                                    }
                                    state.sidebar.interaction = SidebarInteraction::Idle;
                                }
                            }
                        }
                    }

                    state.window.request_redraw();
                    return;
                }

                // Click outside sidebar — cancel any sidebar editing.
                if elem_state == ElementState::Pressed {
                    if let SidebarInteraction::Editing {
                        index, ref text, ..
                    } = state.sidebar.interaction
                    {
                        if let Some(ws) = state.workspace_cache.get(index) {
                            if !text.is_empty() && *text != ws.name {
                                self.app_state.rename_workspace(ws.id, text.clone());
                            }
                        }
                        state.sidebar.interaction = SidebarInteraction::Idle;
                        state.window.request_redraw();
                    }
                }

                // Left-button press on a divider: start drag or reset on double-click.
                if elem_state == ElementState::Pressed && button == winit::event::MouseButton::Left
                {
                    if let Some(div) = divider::hit_test(&state.dividers, px, py) {
                        // Detect double-click via the mouse handler's click count.
                        // We call handle_mouse_press so it tracks timing; then inspect count.
                        let mouse_mode =
                            state.terminal_modes.contains(TerminalMode::MOUSE_REPORTING);
                        let shift = state.modifiers.shift_key();
                        let (col, row) = state.cursor_cell();
                        let _ = state.mouse.handle_mouse_press(
                            col,
                            row,
                            MouseButton::Left,
                            shift,
                            mouse_mode,
                        );

                        if state.mouse.click_count() >= 2 {
                            // Double-click: reset split ratio to equal halves.
                            tracing::debug!(
                                pane_id = %div.pane_id,
                                "divider double-click — resetting ratio to 0.5"
                            );
                            self.app_state.resize_split(div.pane_id, 0.5);
                            state.window.request_redraw();
                        } else {
                            // Single press: start drag.
                            // Compute actual container dimension from adjacent pane rects
                            // for correct ratio calculation in multi-level splits.
                            let (split_start, split_dimension) = {
                                let pos = div.position;
                                let layout = &state.last_layout;
                                match div.orientation {
                                    DividerOrientation::Vertical => {
                                        // Find panes immediately left and right of divider
                                        let left = layout.iter().find(|(_, r)| {
                                            (r.x + r.width - pos).abs() < 4.0
                                                && r.y < div.end
                                                && (r.y + r.height) > div.start
                                        });
                                        let right = layout.iter().find(|(_, r)| {
                                            (r.x - pos).abs() < 4.0
                                                && r.y < div.end
                                                && (r.y + r.height) > div.start
                                        });
                                        match (left, right) {
                                            (Some((_, l)), Some((_, r))) => {
                                                (l.x, l.width + r.width)
                                            }
                                            _ => {
                                                let sw = state.sidebar.effective_width();
                                                (sw, state.gpu.width() as f32 - sw)
                                            }
                                        }
                                    }
                                    DividerOrientation::Horizontal => {
                                        let above = layout.iter().find(|(_, r)| {
                                            (r.y + r.height - pos).abs() < 4.0
                                                && r.x < div.end
                                                && (r.x + r.width) > div.start
                                        });
                                        let below = layout.iter().find(|(_, r)| {
                                            (r.y - pos).abs() < 4.0
                                                && r.x < div.end
                                                && (r.x + r.width) > div.start
                                        });
                                        match (above, below) {
                                            (Some((_, a)), Some((_, b))) => {
                                                (a.y, a.height + b.height)
                                            }
                                            _ => (0.0, state.gpu.height() as f32),
                                        }
                                    }
                                }
                            };
                            let start_cursor = match div.orientation {
                                DividerOrientation::Vertical => px,
                                DividerOrientation::Horizontal => py,
                            };
                            // Derive start_ratio from current divider position.
                            let start_ratio = if split_dimension > 0.0 {
                                (div.position - split_start) / split_dimension
                            } else {
                                0.5
                            };
                            state.drag_state = Some(crate::divider::DragState {
                                pane_id: div.pane_id,
                                orientation: div.orientation,
                                split_dimension,
                                split_start,
                                start_cursor,
                                start_ratio,
                            });
                            tracing::debug!(
                                pane_id = %div.pane_id,
                                start_ratio,
                                "divider drag started"
                            );
                        }
                        return;
                    }
                }

                // Tab drag release: reorder tabs if dragging.
                if elem_state == ElementState::Released && button == winit::event::MouseButton::Left
                {
                    if let TabDragState::Dragging {
                        pane_id,
                        from_index,
                        current_x,
                    } = state.tab_drag
                    {
                        if let Some(vp) = state.last_viewports.iter().find(|v| v.pane_id == pane_id)
                        {
                            let mut to_index = from_index;
                            for i in 0..vp.tab_count {
                                let (tw, tx) = wmux_render::pane::PaneRenderer::tab_metrics(vp, i);
                                if current_x >= tx && current_x < tx + tw {
                                    to_index = i;
                                    break;
                                }
                            }
                            if from_index != to_index {
                                self.app_state
                                    .reorder_surface(pane_id, from_index, to_index);
                            }
                        }
                        state.tab_drag = TabDragState::None;
                        state.window.set_cursor(winit::window::CursorIcon::Default);
                        state.window.request_redraw();
                        return;
                    }
                    state.tab_drag = TabDragState::None;
                }

                // Left-button release: end any active divider drag.
                if elem_state == ElementState::Released
                    && button == winit::event::MouseButton::Left
                    && state.drag_state.is_some()
                {
                    tracing::debug!("divider drag ended");
                    state.drag_state = None;
                    state.window.request_redraw();
                    return;
                }

                // Tab bar: click to switch + initiate drag.
                if elem_state == ElementState::Pressed && button == winit::event::MouseButton::Left
                {
                    let mut tab_clicked = false;
                    for vp in &state.last_viewports {
                        if vp.tab_count <= 1 {
                            continue;
                        }
                        let tab_bar_bottom = vp.rect.y + wmux_render::pane::TAB_BAR_HEIGHT;
                        if px >= vp.rect.x
                            && px < vp.rect.x + vp.rect.width
                            && py >= vp.rect.y
                            && py < tab_bar_bottom
                        {
                            let mut tab_index = None;
                            for i in 0..vp.tab_count {
                                let (tw, tx) = wmux_render::pane::PaneRenderer::tab_metrics(vp, i);
                                if px >= tx && px < tx + tw {
                                    tab_index = Some(i);
                                    break;
                                }
                            }
                            let Some(tab_index) = tab_index else {
                                continue;
                            };

                            if vp.pane_id != state.focused_pane {
                                state.focused_pane = vp.pane_id;
                                self.app_state.focus_pane(vp.pane_id);
                            }
                            if tab_index != vp.active_tab {
                                self.app_state.cycle_surface_to_index(vp.pane_id, tab_index);
                            }
                            state.tab_drag = TabDragState::Pressing {
                                pane_id: vp.pane_id,
                                tab_index,
                                start_x: px,
                            };
                            tab_clicked = true;
                            state.window.request_redraw();
                            break;
                        }
                    }
                    if tab_clicked {
                        return;
                    }
                }

                // Click-to-focus: on any press, check if the click landed in a
                // different pane and switch focus to it.
                if elem_state == ElementState::Pressed {
                    // Use cached layout from the last render frame instead of
                    // blocking on the actor, which could cause UI freezes.
                    for (pane_id, rect) in &state.last_layout {
                        if rect.contains_point(px, py) && *pane_id != state.focused_pane {
                            state.focused_pane = *pane_id;
                            self.app_state.focus_pane(*pane_id);
                            state.window.request_redraw();
                            break;
                        }
                    }
                }

                let btn = match button {
                    winit::event::MouseButton::Left => MouseButton::Left,
                    winit::event::MouseButton::Right => MouseButton::Right,
                    winit::event::MouseButton::Middle => MouseButton::Middle,
                    _ => return,
                };

                let mouse_mode = state.terminal_modes.contains(TerminalMode::MOUSE_REPORTING);
                let shift = state.modifiers.shift_key();
                let (col, row) = state.cursor_cell();

                let action = match elem_state {
                    ElementState::Pressed => state
                        .mouse
                        .handle_mouse_press(col, row, btn, shift, mouse_mode),
                    ElementState::Released => {
                        state.mouse.handle_mouse_release(col, row, btn, mouse_mode)
                    }
                };

                state.handle_mouse_action(action, &self.app_state);
            }

            WindowEvent::CursorMoved { position, .. } => {
                state.cursor_pos = (position.x, position.y);
                let px = position.x as f32;
                let py = position.y as f32;

                // Tab drag: transition Pressing → Dragging on threshold.
                match state.tab_drag {
                    TabDragState::Pressing {
                        pane_id,
                        tab_index,
                        start_x,
                    } => {
                        if (px - start_x).abs() > 5.0 {
                            state.tab_drag = TabDragState::Dragging {
                                pane_id,
                                from_index: tab_index,
                                current_x: px,
                            };
                            state.window.set_cursor(winit::window::CursorIcon::Grabbing);
                            state.window.request_redraw();
                        }
                    }
                    TabDragState::Dragging {
                        ref mut current_x, ..
                    } => {
                        *current_x = px;
                        state.window.request_redraw();
                    }
                    TabDragState::None => {}
                }

                // Sidebar interactions: hover highlighting and drag-to-reorder.
                if state.sidebar.visible {
                    let in_sidebar = px < state.sidebar.effective_width();
                    let ws_count = state.workspace_cache.len();

                    // Check if we should transition from Pressing to Dragging.
                    if state.sidebar.should_start_drag(py) {
                        if let SidebarInteraction::Pressing { row, .. } = state.sidebar.interaction
                        {
                            state.sidebar.interaction = SidebarInteraction::Dragging {
                                from_row: row,
                                current_y: py,
                            };
                            state.window.set_cursor(winit::window::CursorIcon::Grabbing);
                            state.window.request_redraw();
                            return;
                        }
                    }

                    // Update drag position.
                    if let SidebarInteraction::Dragging {
                        ref mut current_y, ..
                    } = state.sidebar.interaction
                    {
                        *current_y = py;
                        state.window.set_cursor(winit::window::CursorIcon::Grabbing);
                        state.window.request_redraw();
                        return;
                    }

                    // Hover: update when inside sidebar and not dragging/editing/pressing.
                    if in_sidebar
                        && !matches!(
                            state.sidebar.interaction,
                            SidebarInteraction::Dragging { .. }
                                | SidebarInteraction::Editing { .. }
                                | SidebarInteraction::Pressing { .. }
                        )
                    {
                        let new_hover = state.sidebar.hit_test_row(py, ws_count);
                        let old_hover =
                            if let SidebarInteraction::Hover(h) = state.sidebar.interaction {
                                Some(h)
                            } else {
                                None
                            };
                        if new_hover != old_hover {
                            state.sidebar.interaction = match new_hover {
                                Some(idx) => SidebarInteraction::Hover(idx),
                                None => SidebarInteraction::Idle,
                            };
                            state.window.request_redraw();
                        }
                        // Pointer cursor in sidebar over workspace rows.
                        if new_hover.is_some() {
                            state.window.set_cursor(winit::window::CursorIcon::Pointer);
                        }
                        return;
                    } else if !in_sidebar
                        && matches!(state.sidebar.interaction, SidebarInteraction::Hover(_))
                    {
                        // Moved out of sidebar — clear hover.
                        state.sidebar.interaction = SidebarInteraction::Idle;
                        state.window.request_redraw();
                    }
                }

                // If a divider drag is active, compute the new ratio and resize.
                if let Some(ref drag) = state.drag_state {
                    let cursor = match drag.orientation {
                        DividerOrientation::Vertical => px,
                        DividerOrientation::Horizontal => py,
                    };
                    let new_ratio = divider::compute_ratio(drag, cursor);
                    self.app_state.resize_split(drag.pane_id, new_ratio);
                    state.window.request_redraw();
                    return;
                }

                // Change cursor icon based on divider hover (skip during tab drag).
                if matches!(state.tab_drag, TabDragState::None) {
                    let icon = match divider::hit_test(&state.dividers, px, py)
                        .map(|d| d.orientation)
                    {
                        Some(DividerOrientation::Vertical) => winit::window::CursorIcon::EwResize,
                        Some(DividerOrientation::Horizontal) => winit::window::CursorIcon::NsResize,
                        None => winit::window::CursorIcon::Default,
                    };
                    state.window.set_cursor(icon);
                }

                let (col, row) = state.cursor_cell();
                let mouse_mode = state.terminal_modes.contains(TerminalMode::MOUSE_REPORTING);
                let action = state.mouse.handle_mouse_motion(col, row, mouse_mode);
                state.handle_mouse_action(action, &self.app_state);
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let mouse_mode = state.terminal_modes.contains(TerminalMode::MOUSE_REPORTING);

                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y as f64,
                    MouseScrollDelta::PixelDelta(pos) => pos.y / state.metrics.cell_height as f64,
                };

                if mouse_mode {
                    let (col, row) = state.cursor_cell();
                    let button: u8 = if lines > 0.0 { 64 } else { 65 };
                    let report = {
                        use std::io::Write;
                        let mut buf = Vec::with_capacity(16);
                        let _ = write!(buf, "\x1b[<{};{};{}M", button, col + 1, row + 1);
                        buf
                    };
                    self.app_state.send_input(state.focused_pane, report);
                } else {
                    // Scroll viewport via actor (3 lines per scroll notch).
                    const SCROLL_LINES: i32 = 3;
                    let delta = if lines > 0.0 {
                        (lines.ceil() as i32) * SCROLL_LINES
                    } else {
                        (lines.floor() as i32) * SCROLL_LINES
                    };
                    if delta != 0 {
                        self.app_state.scroll_viewport(state.focused_pane, delta);
                        state.window.request_redraw();
                    }
                }
            }

            _ => {}
        }
    }
}
