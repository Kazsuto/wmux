use crate::divider::{self, DividerOrientation};

/// Build a CustomGlyph for an SVG icon (used at init to pre-build glyph arrays).
fn svg_cg(icon: wmux_render::icons::Icon, size: f32) -> glyphon::CustomGlyph {
    glyphon::CustomGlyph {
        id: icon.svg_id(),
        left: 0.0,
        top: 0.0,
        width: size,
        height: size,
        color: None,
        snap_to_physical_pixel: true,
        metadata: 0,
    }
}
use crate::event::WmuxEvent;
use crate::mouse::MouseButton;
use crate::shortcuts::ShortcutAction;
use crate::sidebar::SidebarInteraction;
use crate::toast;
use crate::UiError;
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseScrollDelta, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{Key, ModifiersState, NamedKey},
    window::{WindowAttributes, WindowId},
};
use wmux_config::derive_ui_chrome;
use wmux_core::{
    AppEvent, AppStateHandle, PaneId, PaneTreeSnapshot, SessionState, SplitDirection, TerminalMode,
};
use wmux_render::GpuContext;

use super::{handlers, App, TabDragState, UiState};

impl<'window> ApplicationHandler<WmuxEvent> for App<'window> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        // Apply saved window geometry from session, or use default size.
        let attrs = if let Some(ref session) = self.pending_session {
            if let Some(ref geom) = session.window {
                WindowAttributes::default()
                    .with_title("wmux")
                    .with_inner_size(winit::dpi::PhysicalSize::new(geom.width, geom.height))
                    .with_position(winit::dpi::PhysicalPosition::new(geom.x, geom.y))
            } else {
                WindowAttributes::default()
                    .with_title("wmux")
                    .with_inner_size(winit::dpi::LogicalSize::new(1200, 800))
            }
        } else {
            WindowAttributes::default()
                .with_title("wmux")
                .with_inner_size(winit::dpi::LogicalSize::new(1200, 800))
        };

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

        let shadows = wmux_render::ShadowPipeline::new(
            &gpu.device,
            &gpu.queue,
            gpu.format,
            gpu.width(),
            gpu.height(),
        );

        let mut glyphon = wmux_render::GlyphonRenderer::new(&gpu.device, &gpu.queue, gpu.format);
        glyphon.resize(&gpu.queue, gpu.width(), gpu.height());

        // Load config early so font metrics use the configured font size.
        // Config::load() uses std::fs which is fine here — one-time init on the UI thread.
        let config = wmux_config::Config::load().unwrap_or_else(|e| {
            tracing::warn!(error = %e, "failed to load config, using defaults");
            wmux_config::Config::default()
        });

        // Capture DPI scale factor for physical pixel calculations.
        let initial_scale_factor = window.scale_factor() as f32;

        // Compute terminal dimensions from window size and DPI-scaled font metrics.
        // The font size is multiplied by the OS scale factor so cell dimensions
        // are in physical pixels — matching the wgpu surface coordinate space.
        let metrics = wmux_render::TerminalMetrics::new(
            glyphon.font_system(),
            Some(config.font_family.as_str()),
            Some(config.font_size * initial_scale_factor),
        );
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

        // Initialize BrowserManager on the UI/STA thread (WebView2 requires STA).
        let browser_manager = match wmux_browser::BrowserManager::new() {
            Ok(mgr) => {
                if mgr.is_runtime_available() {
                    tracing::info!("WebView2 runtime available, browser integration enabled");
                    Some(mgr)
                } else {
                    tracing::warn!("WebView2 runtime not installed, browser integration disabled");
                    None
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "BrowserManager init failed, browser integration disabled");
                None
            }
        };

        // Extract main window HWND for WebView2 child HWND creation.
        let main_hwnd = {
            use windows::Win32::Foundation::HWND;
            use winit::raw_window_handle::HasWindowHandle;
            use winit::raw_window_handle::RawWindowHandle;
            match window.window_handle() {
                Ok(handle) => match handle.as_raw() {
                    RawWindowHandle::Win32(h) => {
                        let hwnd = HWND(h.hwnd.get() as *mut _);
                        tracing::info!("main window HWND acquired for WebView2");
                        hwnd
                    }
                    _ => {
                        tracing::warn!("non-Win32 window handle, browser features disabled");
                        HWND::default()
                    }
                },
                Err(e) => {
                    tracing::warn!(error = %e, "failed to get window handle, browser features disabled");
                    HWND::default()
                }
            }
        };

        // Restore session or create a fresh default pane.
        let session = self.pending_session.take();
        let restored_sidebar_width = session.as_ref().and_then(|s| {
            if s.sidebar_width > 0 {
                Some(s.sidebar_width)
            } else {
                None
            }
        });
        let pane_id = if let Some(session) = session {
            restore_session(&session, cols, rows, &self.app_state, &self.rt_handle)
        } else {
            let id = PaneId::new();
            handlers::spawn_pane_pty(id, cols, rows, &self.app_state, &self.rt_handle);
            id
        };

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

        // Browser command forwarding: IPC → EventLoopProxy → UI thread.
        if let Some(mut browser_rx) = self.browser_cmd_rx.take() {
            let proxy_browser = self.proxy.clone();
            self.rt_handle.spawn(async move {
                while let Some(cmd) = browser_rx.recv().await {
                    let _ = proxy_browser.send_event(WmuxEvent::BrowserCommand(cmd));
                }
                tracing::info!("browser command forwarding task ended");
            });
        }

        // Config was already loaded earlier (before metrics computation).
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

        // Initialize status bar.
        let status_bar =
            crate::status_bar::StatusBar::new(glyphon.font_system(), gpu.width() as f32);
        let status_bar_data = crate::status_bar::StatusBarData::default();

        // Pre-allocate search bar text buffers (query + match count).
        let search_text_metrics = glyphon::Metrics::new(
            crate::typography::CAPTION_FONT_SIZE,
            crate::typography::CAPTION_LINE_HEIGHT,
        );
        let search_query_buffer = glyphon::Buffer::new(glyphon.font_system(), search_text_metrics);
        let search_count_buffer = glyphon::Buffer::new(glyphon.font_system(), search_text_metrics);

        tracing::info!(
            cols,
            rows,
            width = gpu.width(),
            height = gpu.height(),
            format = ?gpu.format,
            pane_id = %pane_id,
            "terminal initialized (actor pattern)",
        );

        // initial_scale_factor was captured earlier (before metrics computation).

        // Empty buffer for SVG CustomGlyph TextAreas (no text, only custom_glyphs).
        let icon_empty_buffer = {
            let m = glyphon::Metrics::new(16.0, 20.0);
            let mut b = glyphon::Buffer::new(glyphon.font_system(), m);
            b.set_size(glyphon.font_system(), Some(1.0), Some(1.0));
            b
        };
        let _has_icon_font = glyphon.has_icon_font();

        // Status icon CustomGlyphs — one per Icon variant that from_name() can return.
        let status_icon_cgs: std::collections::HashMap<
            wmux_render::icons::Icon,
            [glyphon::CustomGlyph; 1],
        > = [
            wmux_render::icons::Icon::Info,
            wmux_render::icons::Icon::Warning,
            wmux_render::icons::Icon::Error,
            wmux_render::icons::Icon::Settings,
            wmux_render::icons::Icon::Terminal,
            wmux_render::icons::Icon::Globe,
            wmux_render::icons::Icon::Search,
            wmux_render::icons::Icon::Workspace,
        ]
        .into_iter()
        .map(|icon| (icon, [svg_cg(icon, 14.0)]))
        .collect();

        // Split menu text buffers.
        let menu_labels = ["Split Right", "Split Left", "Split Up", "Split Down"];
        let menu_hints = ["Ctrl-K Right", "Ctrl-K Left", "Ctrl-K Up", "Ctrl-K Down"];
        let menu_m = glyphon::Metrics::new(13.0, 18.0);
        let menu_attrs = glyphon::Attrs::new().family(glyphon::Family::Name("Segoe UI"));
        let hint_attrs = glyphon::Attrs::new()
            .family(glyphon::Family::Name("Segoe UI"))
            .weight(glyphon::Weight::LIGHT);
        let split_menu_buffers = std::array::from_fn(|i| {
            let mut b = glyphon::Buffer::new(glyphon.font_system(), menu_m);
            b.set_size(glyphon.font_system(), Some(120.0), Some(28.0));
            b.set_text(
                glyphon.font_system(),
                menu_labels[i],
                &menu_attrs,
                glyphon::Shaping::Advanced,
                None,
            );
            b.shape_until_scroll(glyphon.font_system(), false);
            b
        });
        let ws_menu_labels = ["Rename Workspace", "Close Workspace"];
        let ws_menu_buffers: [glyphon::Buffer; super::WORKSPACE_MENU_ITEMS] =
            std::array::from_fn(|i| {
                let mut b = glyphon::Buffer::new(glyphon.font_system(), menu_m);
                b.set_size(glyphon.font_system(), Some(180.0), Some(28.0));
                b.set_text(
                    glyphon.font_system(),
                    ws_menu_labels[i],
                    &menu_attrs,
                    glyphon::Shaping::Advanced,
                    None,
                );
                b.shape_until_scroll(glyphon.font_system(), false);
                b
            });

        let split_menu_hint_buffers = std::array::from_fn(|i| {
            let mut b = glyphon::Buffer::new(glyphon.font_system(), menu_m);
            b.set_size(glyphon.font_system(), Some(120.0), Some(28.0));
            b.set_text(
                glyphon.font_system(),
                menu_hints[i],
                &hint_attrs,
                glyphon::Shaping::Advanced,
                None,
            );
            b.shape_until_scroll(glyphon.font_system(), false);
            b
        });

        self.state = Some(UiState {
            window,
            gpu,
            quads,
            shadows,
            glyphon,
            renderers,
            metrics,
            input: crate::input::InputHandler::new(),
            mouse: crate::mouse::MouseHandler::new(),
            shortcuts: crate::shortcuts::ShortcutMap::new(),
            modifiers: ModifiersState::default(),
            cursor_pos: (0.0, 0.0),
            toast_service,
            sidebar: crate::sidebar::SidebarState::new(config.sidebar_width),
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
            search_query_buffer,
            search_count_buffer,
            tab_title_buffers: std::collections::HashMap::new(),
            last_viewports: Vec::new(),
            tab_drag: TabDragState::None,
            tab_close_hover: None,
            tab_edit: super::TabEditState::None,
            tab_edit_buffer: None,
            icon_empty_buffer,
            cg_close: [svg_cg(wmux_render::icons::Icon::Close, 14.0)],
            cg_add: [svg_cg(wmux_render::icons::Icon::Add, 16.0)],
            cg_terminal: [svg_cg(wmux_render::icons::Icon::Terminal, 16.0)],
            cg_globe: [svg_cg(wmux_render::icons::Icon::Globe, 16.0)],
            cg_split: [svg_cg(wmux_render::icons::Icon::Split, 16.0)],
            cg_search: [svg_cg(wmux_render::icons::Icon::Search, 14.0)],
            cg_arrows: [
                [svg_cg(wmux_render::icons::Icon::ArrowRight, 14.0)],
                [svg_cg(wmux_render::icons::Icon::ArrowLeft, 14.0)],
                [svg_cg(wmux_render::icons::Icon::ArrowUp, 14.0)],
                [svg_cg(wmux_render::icons::Icon::ArrowDown, 14.0)],
            ],
            cg_workspace: [svg_cg(wmux_render::icons::Icon::Workspace, 16.0)],
            status_icon_cgs,
            browser_manager,
            main_hwnd,
            focused_surface_kind: wmux_core::PanelKind::Terminal,
            status_bar,
            status_bar_data,
            start_instant: std::time::Instant::now(),
            chord_state: super::ChordState::default(),
            split_menu: super::SplitMenuState::default(),
            split_menu_buffers,
            split_menu_hint_buffers,
            split_menu_hover: None,
            workspace_menu: super::WorkspaceMenuState::default(),
            workspace_menu_buffers: ws_menu_buffers,
            workspace_menu_hover: None,
            animation: crate::animation::AnimationEngine::default(),
            focus_glow_anim: None,
            tab_hover: None,
            tab_hover_anim: None,
            divider_hover: None,
            ui_chrome,
            effect_result,
            theme_ansi,
            theme_cursor,
            theme_foreground,
            inactive_pane_opacity: config.inactive_pane_opacity,
            scale_factor: initial_scale_factor,
            terminal_font_family: config.font_family.clone(),
            terminal_font_size: config.font_size,
        });

        // Override sidebar width from saved session (takes priority over config default).
        if let Some(sw) = restored_sidebar_width {
            if let Some(ref mut state) = self.state {
                state.sidebar.width = (sw as f32).clamp(
                    crate::sidebar::MIN_SIDEBAR_WIDTH,
                    crate::sidebar::MAX_SIDEBAR_WIDTH,
                );
            }
        }
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
                    state.set_focused_pane(pane_id);
                    state.window.request_redraw();
                }
            }
            WmuxEvent::BrowserCommand(cmd) => {
                if let Some(state) = self.state.as_mut() {
                    let result = handle_browser_command(
                        state,
                        &self.app_state,
                        &self.rt_handle,
                        &self.proxy,
                        &cmd.method,
                        &cmd.params,
                    );
                    let _ = cmd.reply.send(result);
                    state.window.request_redraw();
                } else {
                    let _ = cmd.reply.send(Err("UI not initialized yet".to_owned()));
                }
            }
            WmuxEvent::CreateBrowserPanel { surface_id, url } => {
                tracing::info!(surface_id = %surface_id, url = %url, "CreateBrowserPanel event received");
                if let Some(state) = self.state.as_mut() {
                    if let Some(ref mut mgr) = state.browser_manager {
                        // Use the actual pane viewport if available, else a reasonable fallback.
                        let rect = state
                            .last_viewports
                            .iter()
                            .find(|vp| vp.surface_ids.contains(&surface_id))
                            .map(wmux_render::PaneRenderer::terminal_viewport)
                            .unwrap_or_else(|| {
                                // Fallback: use the focused pane's viewport, or a default.
                                state
                                    .last_viewports
                                    .iter()
                                    .find(|vp| vp.focused)
                                    .map(wmux_render::PaneRenderer::terminal_viewport)
                                    .unwrap_or_else(|| {
                                        wmux_core::rect::Rect::new(0.0, 0.0, 800.0, 600.0)
                                    })
                            });
                        match mgr.create_panel(surface_id, state.main_hwnd, &rect) {
                            Ok(_) => {
                                if let Some(panel) = mgr.get_panel(surface_id) {
                                    let _ = panel.navigate(&url);
                                    let _ = panel.focus_webview();
                                }
                                tracing::info!(
                                    surface_id = %surface_id,
                                    url = %url,
                                    "browser panel created and navigated"
                                );
                            }
                            Err(e) => {
                                tracing::error!(error = %e, "failed to create browser panel");
                            }
                        }
                    }
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
                    state.shadows.resize(&state.gpu.queue, w, h);
                    state.glyphon.resize(&state.gpu.queue, w, h);
                    state
                        .status_bar
                        .resize(state.glyphon.font_system(), w as f32);

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

                    // Update session persistence with current window geometry.
                    send_ui_state(state, &self.app_state);

                    state.window.request_redraw();
                }
            }

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let new_scale = scale_factor as f32;
                // Guard against degenerate values from drivers/platform quirks.
                if !new_scale.is_finite() || new_scale < 0.1 {
                    tracing::warn!(scale_factor = new_scale, "ignoring invalid scale factor");
                } else if (new_scale - state.scale_factor).abs() > 1e-4 {
                    tracing::info!(
                        old = state.scale_factor,
                        new = new_scale,
                        "DPI scale factor changed"
                    );
                    state.scale_factor = new_scale;
                    // Recompute cell metrics with the updated DPI scale.
                    state.metrics = wmux_render::TerminalMetrics::new(
                        state.glyphon.font_system(),
                        Some(state.terminal_font_family.as_str()),
                        Some(state.terminal_font_size * new_scale),
                    );
                    // Recalculate global cols/rows from the new metrics in case no
                    // Resized event follows (e.g., same physical size, different DPI).
                    let w = state.gpu.width();
                    let h = state.gpu.height();
                    let new_cols = ((w as f32) / state.metrics.cell_width).floor().max(1.0) as u32;
                    let new_rows = ((h as f32) / state.metrics.cell_height).floor().max(1.0) as u32;
                    state.cols = new_cols.min(u16::MAX as u32) as u16;
                    state.rows = new_rows.min(u16::MAX as u32) as u16;
                    // Force all per-pane renderers to be recreated with new font size.
                    state.renderers.clear();
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

                // Close split menu on any keypress.
                if matches!(state.split_menu, super::SplitMenuState::Open { .. }) {
                    state.split_menu = super::SplitMenuState::Closed;
                    state.split_menu_hover = None;
                    if matches!(event.logical_key, Key::Named(NamedKey::Escape)) {
                        state.window.request_redraw();
                        return;
                    }
                }

                // Close workspace context menu on any keypress.
                if matches!(state.workspace_menu, super::WorkspaceMenuState::Open { .. }) {
                    state.workspace_menu = super::WorkspaceMenuState::Closed;
                    state.workspace_menu_hover = None;
                    if matches!(event.logical_key, Key::Named(NamedKey::Escape)) {
                        state.window.request_redraw();
                        return;
                    }
                }

                // Priority 0a: sidebar inline editing — intercept all keys when renaming.
                if state.sidebar.is_editing() {
                    handlers::handle_sidebar_edit_key(state, &event, &self.app_state);
                    state.window.request_redraw();
                    return;
                }

                // Priority 0b: tab inline editing — intercept all keys when renaming a tab.
                if matches!(state.tab_edit, super::TabEditState::Editing { .. }) {
                    handlers::handle_tab_edit_key(state, &event, &self.app_state);
                    state.window.request_redraw();
                    return;
                }

                // Priority 0.9: Chord completion — if a Ctrl+K chord is pending,
                // check the second key for split direction arrows.
                if state.chord_state.is_pending() {
                    state.chord_state = super::ChordState::Idle;
                    let chord_action = match &event.logical_key {
                        Key::Named(NamedKey::ArrowRight) => Some(ShortcutAction::SplitRight),
                        Key::Named(NamedKey::ArrowLeft) => Some(ShortcutAction::SplitLeft),
                        Key::Named(NamedKey::ArrowUp) => Some(ShortcutAction::SplitUp),
                        Key::Named(NamedKey::ArrowDown) => Some(ShortcutAction::SplitDown),
                        _ => None,
                    };
                    if let Some(action) = chord_action {
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
                    // Not a recognized chord completion — fall through to normal handling.
                    // The Ctrl+K prefix is consumed (not forwarded to terminal).
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
                    // Ctrl+K chord prefix: enter pending state instead of dispatching.
                    if action == ShortcutAction::ChordPrefix {
                        if !event.repeat {
                            state.chord_state =
                                super::ChordState::Pending(std::time::Instant::now());
                            tracing::debug!("chord prefix Ctrl+K — waiting for second key");
                        }
                        return;
                    }
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

                // Priority 1.5: search overlay input — intercepted when search is active.
                // Must be before the process_exited check so search works even
                // after the shell process has exited.
                if state.search.active {
                    handlers::handle_search_key(state, &event);
                    state.window.request_redraw();
                    return;
                }

                // Don't send input to a dead process.
                if state.process_exited {
                    return;
                }

                // Skip terminal input when a browser surface has focus —
                // WebView2 receives keyboard events via its own child HWND.
                if state.focused_surface_kind == wmux_core::PanelKind::Browser {
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

                // Split menu click handling — highest priority when menu is open.
                if let super::SplitMenuState::Open {
                    pane_id: menu_pane,
                    menu_x,
                    menu_y,
                } = state.split_menu
                {
                    let item_h = 32.0;
                    let menu_w = 240.0;
                    let menu_h = item_h * 4.0 + 8.0;

                    if elem_state == ElementState::Pressed
                        && button == winit::event::MouseButton::Left
                    {
                        if px >= menu_x
                            && px < menu_x + menu_w
                            && py >= menu_y
                            && py < menu_y + menu_h
                        {
                            // Determine which item was clicked (skip 4px top padding).
                            let raw = py - menu_y - 4.0;
                            if raw < 0.0 {
                                // Click in top padding — not on any item.
                                state.split_menu = super::SplitMenuState::Closed;
                                state.split_menu_hover = None;
                                state.window.request_redraw();
                                return;
                            }
                            let item_idx = (raw / item_h).floor() as usize;
                            if item_idx < 4 {
                                let action = match item_idx {
                                    0 => ShortcutAction::SplitRight,
                                    1 => ShortcutAction::SplitLeft,
                                    2 => ShortcutAction::SplitUp,
                                    3 => ShortcutAction::SplitDown,
                                    _ => unreachable!(),
                                };
                                // Focus the menu's target pane.
                                if menu_pane != state.focused_pane {
                                    state.set_focused_pane(menu_pane);
                                    self.app_state.focus_pane(menu_pane);
                                }
                                state.split_menu = super::SplitMenuState::Closed;
                                state.split_menu_hover = None;
                                handlers::handle_shortcut(
                                    action,
                                    state,
                                    &self.app_state,
                                    &self.rt_handle,
                                    &self.proxy,
                                );
                            }
                            state.window.request_redraw();
                            return;
                        }
                        // Click outside menu — close it.
                        state.split_menu = super::SplitMenuState::Closed;
                        state.split_menu_hover = None;
                        state.window.request_redraw();
                        // Don't return — let the click propagate to other handlers.
                    }
                }

                // Workspace context menu click handling — highest priority when open.
                if let super::WorkspaceMenuState::Open {
                    workspace_index,
                    menu_x,
                    menu_y,
                } = state.workspace_menu
                {
                    let item_h = 32.0;
                    let menu_w = 200.0;
                    let menu_items = super::WORKSPACE_MENU_ITEMS;
                    let menu_h = item_h * menu_items as f32 + 8.0;

                    if elem_state == ElementState::Pressed
                        && button == winit::event::MouseButton::Left
                    {
                        if px >= menu_x
                            && px < menu_x + menu_w
                            && py >= menu_y
                            && py < menu_y + menu_h
                        {
                            let raw = py - menu_y - 4.0;
                            if raw < 0.0 {
                                state.workspace_menu = super::WorkspaceMenuState::Closed;
                                state.workspace_menu_hover = None;
                                state.window.request_redraw();
                                return;
                            }
                            let item_idx = (raw / item_h).floor() as usize;
                            if item_idx < menu_items {
                                state.workspace_menu = super::WorkspaceMenuState::Closed;
                                state.workspace_menu_hover = None;
                                match item_idx {
                                    0 => {
                                        // Rename Workspace — start inline editing.
                                        if let Some(ws) = state.workspace_cache.get(workspace_index)
                                        {
                                            let name = ws.name.clone();
                                            let cursor = name.chars().count();
                                            state.sidebar.interaction =
                                                SidebarInteraction::Editing {
                                                    index: workspace_index,
                                                    text: name,
                                                    cursor,
                                                };
                                        }
                                    }
                                    1 => {
                                        // Close Workspace.
                                        if let Some(ws) = state.workspace_cache.get(workspace_index)
                                        {
                                            let ws_id = ws.id;
                                            self.app_state.close_workspace(ws_id);
                                            tracing::info!(
                                                workspace_id = %ws_id,
                                                "workspace closed via context menu"
                                            );
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            state.window.request_redraw();
                            return;
                        }
                        // Click outside menu — close it.
                        state.workspace_menu = super::WorkspaceMenuState::Closed;
                        state.workspace_menu_hover = None;
                        state.window.request_redraw();
                        // Don't return — let click propagate.
                    }
                }

                // Sidebar resize: start on left-click when hovering the resize edge.
                if state.sidebar.visible
                    && matches!(state.sidebar.interaction, SidebarInteraction::ResizeHover)
                    && elem_state == ElementState::Pressed
                    && button == winit::event::MouseButton::Left
                {
                    state.sidebar.interaction = SidebarInteraction::Resizing {
                        start_x: px,
                        start_width: state.sidebar.width,
                    };
                    state.window.set_cursor(winit::window::CursorIcon::EwResize);
                    return;
                }

                // Sidebar resize: end on release.
                if matches!(
                    state.sidebar.interaction,
                    SidebarInteraction::Resizing { .. }
                ) && elem_state == ElementState::Released
                    && button == winit::event::MouseButton::Left
                {
                    state.sidebar.interaction = SidebarInteraction::Idle;
                    state.window.set_cursor(winit::window::CursorIcon::Default);
                    // Persist new sidebar width for session save.
                    send_ui_state(state, &self.app_state);
                    state.window.request_redraw();
                    return;
                }

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

                    // Right-click on sidebar row — open workspace context menu.
                    if elem_state == ElementState::Pressed
                        && button == winit::event::MouseButton::Right
                    {
                        if let Some(row_index) =
                            state.sidebar.hit_test_row(py, state.workspace_cache.len())
                        {
                            state.workspace_menu = super::WorkspaceMenuState::Open {
                                workspace_index: row_index,
                                menu_x: px,
                                menu_y: py,
                            };
                            state.workspace_menu_hover = None;
                            tracing::debug!(row_index, "sidebar: opened workspace context menu");
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
                                split_id = %div.split_id,
                                "divider double-click — resetting ratio to 0.5"
                            );
                            self.app_state.resize_split_by_id(div.split_id, 0.5);
                            state.window.request_redraw();
                        } else {
                            // Single press: start drag.
                            // Split metadata comes directly from the tree-based divider.
                            let start_cursor = match div.orientation {
                                DividerOrientation::Vertical => px,
                                DividerOrientation::Horizontal => py,
                            };
                            state.drag_state = Some(crate::divider::DragState {
                                split_id: div.split_id,
                                orientation: div.orientation,
                                split_dimension: div.split_dimension,
                                split_start: div.split_start,
                                start_cursor,
                                start_ratio: div.current_ratio,
                            });
                            tracing::debug!(
                                split_id = %div.split_id,
                                start_ratio = div.current_ratio,
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
                    // Clear mouse state so the stale press doesn't trigger
                    // selection on subsequent mouse moves.
                    let (col, row) = state.cursor_cell();
                    let mouse_mode = state.terminal_modes.contains(TerminalMode::MOUSE_REPORTING);
                    let _ =
                        state
                            .mouse
                            .handle_mouse_release(col, row, MouseButton::Left, mouse_mode);
                    state.window.request_redraw();
                    return;
                }

                // Tab bar: "+" button, close button, double-click rename, click to switch, drag.
                if elem_state == ElementState::Pressed && button == winit::event::MouseButton::Left
                {
                    let mut tab_clicked = false;
                    // Clone viewports to avoid borrow issues with state.
                    let viewports = state.last_viewports.clone();
                    for vp in &viewports {
                        let tab_bar_bottom = vp.rect.y + vp.tab_bar_height();
                        if px >= vp.rect.x
                            && px < vp.rect.x + vp.rect.width
                            && py >= vp.rect.y
                            && py < tab_bar_bottom
                        {
                            // Split button hit-test (priority over "+" and tabs).
                            if let Some((sbx, sby, sbw, sbh)) =
                                wmux_render::pane::PaneRenderer::split_button_rect(vp)
                            {
                                if px >= sbx && px < sbx + sbw && py >= sby && py < sby + sbh {
                                    if vp.pane_id != state.focused_pane {
                                        state.set_focused_pane(vp.pane_id);
                                        self.app_state.focus_pane(vp.pane_id);
                                    }
                                    // Toggle split menu.
                                    if matches!(
                                        state.split_menu,
                                        super::SplitMenuState::Open { pane_id, .. }
                                        if pane_id == vp.pane_id
                                    ) {
                                        state.split_menu = super::SplitMenuState::Closed;
                                    } else {
                                        state.split_menu = super::SplitMenuState::Open {
                                            pane_id: vp.pane_id,
                                            menu_x: sbx,
                                            menu_y: sby + sbh + 4.0,
                                        };
                                    }
                                    state.split_menu_hover = None;
                                    state.window.request_redraw();
                                    tab_clicked = true;
                                    break;
                                }
                            }

                            // Globe button hit-test — open new browser surface.
                            if state.browser_manager.is_some() {
                                if let Some((gbx, gby, gbw, gbh)) =
                                    wmux_render::pane::PaneRenderer::globe_button_rect(vp)
                                {
                                    if px >= gbx && px < gbx + gbw && py >= gby && py < gby + gbh {
                                        if vp.pane_id != state.focused_pane {
                                            state.set_focused_pane(vp.pane_id);
                                            self.app_state.focus_pane(vp.pane_id);
                                        }
                                        // Create a new browser surface (same as Ctrl+Shift+L).
                                        handlers::handle_shortcut(
                                            ShortcutAction::NewBrowserSurface,
                                            state,
                                            &self.app_state,
                                            &self.rt_handle,
                                            &self.proxy,
                                        );
                                        tab_clicked = true;
                                        break;
                                    }
                                }
                            }

                            // "+" button hit-test (priority over tab switch).
                            if let Some((pbx, pby, pbw, pbh)) =
                                wmux_render::pane::PaneRenderer::plus_button_rect(vp)
                            {
                                if px >= pbx && px < pbx + pbw && py >= pby && py < pby + pbh {
                                    // Focus this pane first if needed.
                                    if vp.pane_id != state.focused_pane {
                                        state.set_focused_pane(vp.pane_id);
                                        self.app_state.focus_pane(vp.pane_id);
                                    }
                                    // Create a new terminal surface (same as Ctrl+T).
                                    handlers::handle_shortcut(
                                        ShortcutAction::NewSurface,
                                        state,
                                        &self.app_state,
                                        &self.rt_handle,
                                        &self.proxy,
                                    );
                                    tab_clicked = true;
                                    break;
                                }
                            }

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

                            // Close button hit-test (priority over switch/drag).
                            if let Some((bx, by, bw, bh)) =
                                wmux_render::pane::PaneRenderer::close_button_rect(vp, tab_index)
                            {
                                if px >= bx && px < bx + bw && py >= by && py < by + bh {
                                    if let Some(sid) = vp.surface_ids.get(tab_index).copied() {
                                        tracing::info!(
                                            pane_id = %vp.pane_id,
                                            surface_id = %sid,
                                            "surface closed via tab button"
                                        );
                                        // Cancel any ongoing tab edit.
                                        state.tab_edit = super::TabEditState::None;
                                        self.app_state.close_surface(vp.pane_id, sid);
                                        state.window.request_redraw();
                                    }
                                    tab_clicked = true;
                                    break;
                                }
                            }

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
                            state.mouse.clear_selection();

                            // Double-click: start inline rename.
                            if state.mouse.click_count() >= 2 {
                                if let Some(sid) = vp.surface_ids.get(tab_index).copied() {
                                    let title =
                                        vp.tab_titles.get(tab_index).cloned().unwrap_or_default();
                                    let cursor = title.chars().count();
                                    state.tab_edit = super::TabEditState::Editing {
                                        pane_id: vp.pane_id,
                                        tab_index,
                                        surface_id: sid,
                                        text: title,
                                        cursor,
                                    };
                                    tracing::debug!(
                                        pane_id = %vp.pane_id,
                                        tab_index,
                                        "tab: started inline editing"
                                    );
                                }
                                tab_clicked = true;
                                state.window.request_redraw();
                                break;
                            }

                            // Cancel any active tab edit before switching/dragging.
                            if matches!(state.tab_edit, super::TabEditState::Editing { .. }) {
                                state.tab_edit = super::TabEditState::None;
                            }

                            if vp.pane_id != state.focused_pane {
                                state.set_focused_pane(vp.pane_id);
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
                    let hit_pane = state
                        .last_layout
                        .iter()
                        .find(|(pid, rect)| {
                            rect.contains_point(px, py) && *pid != state.focused_pane
                        })
                        .map(|(pid, _)| *pid);
                    if let Some(pid) = hit_pane {
                        state.set_focused_pane(pid);
                        self.app_state.focus_pane(pid);
                        state.window.request_redraw();
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

                // Split menu hover tracking.
                if let super::SplitMenuState::Open { menu_x, menu_y, .. } = state.split_menu {
                    let item_h = 32.0;
                    let menu_w = 240.0;
                    let menu_h = item_h * 4.0 + 8.0;
                    let new_hover = if px >= menu_x
                        && px < menu_x + menu_w
                        && py >= menu_y
                        && py < menu_y + menu_h
                    {
                        let raw = py - menu_y - 4.0;
                        if raw >= 0.0 {
                            let idx = (raw / item_h).floor() as usize;
                            if idx < 4 {
                                Some(idx)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    if new_hover != state.split_menu_hover {
                        state.split_menu_hover = new_hover;
                        state.window.request_redraw();
                    }
                }

                // Workspace context menu hover tracking.
                if let super::WorkspaceMenuState::Open { menu_x, menu_y, .. } = state.workspace_menu
                {
                    let item_h = 32.0;
                    let menu_w = 200.0;
                    let menu_items = super::WORKSPACE_MENU_ITEMS;
                    let menu_h = item_h * menu_items as f32 + 8.0;
                    let new_hover = if px >= menu_x
                        && px < menu_x + menu_w
                        && py >= menu_y
                        && py < menu_y + menu_h
                    {
                        let raw = py - menu_y - 4.0;
                        if raw >= 0.0 {
                            let idx = (raw / item_h).floor() as usize;
                            if idx < menu_items {
                                Some(idx)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    if new_hover != state.workspace_menu_hover {
                        state.workspace_menu_hover = new_hover;
                        state.window.request_redraw();
                    }
                }

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

                // Tab close button hover tracking.
                {
                    let mut new_hover = None;
                    for vp in &state.last_viewports {
                        if vp.tab_count <= 1 {
                            continue;
                        }
                        let tab_bar_bottom = vp.rect.y + vp.tab_bar_height();
                        if px >= vp.rect.x
                            && px < vp.rect.x + vp.rect.width
                            && py >= vp.rect.y
                            && py < tab_bar_bottom
                        {
                            for i in 0..vp.tab_count {
                                if let Some((bx, by, bw, bh)) =
                                    wmux_render::pane::PaneRenderer::close_button_rect(vp, i)
                                {
                                    if px >= bx && px < bx + bw && py >= by && py < by + bh {
                                        new_hover = Some((vp.pane_id, i));
                                        break;
                                    }
                                }
                            }
                            break;
                        }
                    }
                    if new_hover != state.tab_close_hover {
                        state.tab_close_hover = new_hover;
                        state.window.request_redraw();
                    }
                }

                // General tab hover tracking (for background highlight animation).
                {
                    let mut new_tab_hover = None;
                    for vp in &state.last_viewports {
                        let tab_bar_bottom = vp.rect.y + vp.tab_bar_height();
                        if px >= vp.rect.x
                            && px < vp.rect.x + vp.rect.width
                            && py >= vp.rect.y
                            && py < tab_bar_bottom
                        {
                            for i in 0..vp.tab_count {
                                let (tw, tx) = wmux_render::pane::PaneRenderer::tab_metrics(vp, i);
                                if px >= tx && px < tx + tw {
                                    new_tab_hover = Some((vp.pane_id, i));
                                    break;
                                }
                            }
                            break;
                        }
                    }
                    if new_tab_hover != state.tab_hover {
                        if let Some(old) = state.tab_hover_anim.take() {
                            state.animation.cancel(old);
                        }
                        state.tab_hover = new_tab_hover;
                        if new_tab_hover.is_some() {
                            state.tab_hover_anim = Some(state.animation.start(
                                0.0,
                                1.0,
                                crate::animation::MOTION_FAST,
                                crate::animation::Easing::CubicOut,
                            ));
                        }
                        state.window.request_redraw();
                    }
                }

                // Sidebar resize: update width while dragging.
                if let SidebarInteraction::Resizing {
                    start_x,
                    start_width,
                } = state.sidebar.interaction
                {
                    let delta = px - start_x;
                    state.sidebar.width = start_width + delta;
                    state.sidebar.clamp_width();
                    state.window.set_cursor(winit::window::CursorIcon::EwResize);
                    state.window.request_redraw();
                    return;
                }

                // Sidebar resize edge hover detection.
                if state.sidebar.visible
                    && state.sidebar.hit_test_resize_edge(px)
                    && !matches!(
                        state.sidebar.interaction,
                        SidebarInteraction::Dragging { .. }
                            | SidebarInteraction::Editing { .. }
                            | SidebarInteraction::Pressing { .. }
                    )
                {
                    if !matches!(state.sidebar.interaction, SidebarInteraction::ResizeHover) {
                        state.sidebar.interaction = SidebarInteraction::ResizeHover;
                    }
                    state.window.set_cursor(winit::window::CursorIcon::EwResize);
                    return;
                } else if matches!(state.sidebar.interaction, SidebarInteraction::ResizeHover) {
                    state.sidebar.interaction = SidebarInteraction::Idle;
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
                    self.app_state.resize_split_by_id(drag.split_id, new_ratio);
                    state.window.request_redraw();
                    return;
                }

                // Change cursor icon and track divider hover (skip during tab drag).
                if matches!(state.tab_drag, TabDragState::None) {
                    let hit = divider::hit_test(&state.dividers, px, py);
                    let new_div_hover = hit.map(|d| {
                        state
                            .dividers
                            .iter()
                            .position(|dd| std::ptr::eq(dd, d))
                            .unwrap_or(0)
                    });
                    if new_div_hover != state.divider_hover {
                        state.divider_hover = new_div_hover;
                        state.window.request_redraw();
                    }
                    let icon = match hit.map(|d| d.orientation) {
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

/// Restore a saved session: recreate workspaces, pane trees, and PTYs.
///
/// Returns the focused pane ID (first leaf of the active workspace).
/// Falls back to a fresh default pane on any error.
fn restore_session(
    session: &SessionState,
    cols: u16,
    rows: u16,
    app_state: &AppStateHandle,
    rt_handle: &tokio::runtime::Handle,
) -> PaneId {
    // The actor already has a default workspace (index 0).
    // We'll rename it for the first session workspace,
    // then create additional workspaces for the rest.
    let mut first_pane_id = None;

    for (i, ws_snapshot) in session.workspaces.iter().enumerate() {
        if i == 0 {
            // Rename the existing default workspace.
            let existing = rt_handle.block_on(app_state.list_workspaces());
            if let Some(ws) = existing.first() {
                app_state.rename_workspace(ws.id, ws_snapshot.name.clone());
            }
        } else {
            // Create additional workspaces.
            let _ = rt_handle.block_on(app_state.create_workspace(ws_snapshot.name.clone()));
        }

        // Switch to this workspace so panes are added to the correct one.
        app_state.switch_workspace(i);

        // Restore pane tree for this workspace.
        if let Some(ref tree) = ws_snapshot.pane_tree {
            let leaf_id = restore_pane_tree(tree, cols, rows, app_state, rt_handle);
            if i == session.active_workspace_index {
                first_pane_id = Some(leaf_id);
            }
        } else {
            // Workspace has no pane tree — create a default pane.
            let id = PaneId::new();
            handlers::spawn_pane_pty(id, cols, rows, app_state, rt_handle);
            if i == session.active_workspace_index {
                first_pane_id = Some(id);
            }
        }
    }

    // Switch to the active workspace from the saved session.
    app_state.switch_workspace(session.active_workspace_index);

    first_pane_id.unwrap_or_else(|| {
        // Fallback: create a fresh pane if nothing was restored.
        let id = PaneId::new();
        handlers::spawn_pane_pty(id, cols, rows, app_state, rt_handle);
        id
    })
}

/// Restore an entire pane tree from a snapshot at arbitrary depth.
///
/// **Algorithm**: the first (leftmost) leaf is created and registered, which
/// initialises the workspace's pane_tree. Then `fill_splits` recursively
/// splits that leaf to build the full tree structure, and `fill_second`
/// spawns PTYs for each second-child created by those splits.
///
/// Returns the PaneId of the first leaf (used as the focused pane).
fn restore_pane_tree(
    tree: &PaneTreeSnapshot,
    cols: u16,
    rows: u16,
    app_state: &AppStateHandle,
    rt_handle: &tokio::runtime::Handle,
) -> PaneId {
    // Create the root pane from the leftmost leaf of the snapshot.
    let root_id = PaneId::new();
    let leaf_data = wmux_core::first_leaf(tree);
    handlers::spawn_pane_pty_for_restore(
        root_id,
        cols,
        rows,
        leaf_data.cwd,
        leaf_data.scrollback_text,
        app_state,
        rt_handle,
    );

    // Build the tree structure by recursively splitting.
    fill_splits(tree, root_id, cols, rows, app_state, rt_handle);

    root_id
}

/// Recursively build the split structure for a snapshot subtree.
///
/// `pane_id` is the first (leftmost) leaf of this subtree — it was already
/// created by the caller. If `snapshot` is a Split, we split `pane_id` and
/// recurse into both children.
fn fill_splits(
    snapshot: &PaneTreeSnapshot,
    pane_id: PaneId,
    cols: u16,
    rows: u16,
    app_state: &AppStateHandle,
    rt_handle: &tokio::runtime::Handle,
) {
    let PaneTreeSnapshot::Split {
        direction,
        ratio,
        first,
        second,
    } = snapshot
    else {
        // Leaf — pane_id already has a PTY; nothing to do.
        return;
    };

    let split_dir = match direction.as_str() {
        "vertical" => SplitDirection::Vertical,
        _ => SplitDirection::Horizontal,
    };

    let second_id = match rt_handle.block_on(app_state.split_pane(pane_id, split_dir)) {
        Ok(id) => id,
        Err(e) => {
            tracing::warn!(error = %e, "session restore: split failed, skipping subtree");
            return;
        }
    };
    app_state.resize_split(pane_id, *ratio);

    // Recurse into first child (pane_id remains its first leaf).
    fill_splits(first, pane_id, cols, rows, app_state, rt_handle);

    // Handle second child — needs a PTY and possibly more splits.
    fill_second(second, second_id, cols, rows, app_state, rt_handle);
}

/// Spawn a PTY for a second-child pane and optionally continue splitting.
///
/// `pane_id` was created by `split_pane` and exists as a leaf in the tree
/// but has no PTY yet.
fn fill_second(
    snapshot: &PaneTreeSnapshot,
    pane_id: PaneId,
    cols: u16,
    rows: u16,
    app_state: &AppStateHandle,
    rt_handle: &tokio::runtime::Handle,
) {
    match snapshot {
        PaneTreeSnapshot::Leaf {
            cwd,
            scrollback_text,
            ..
        } => {
            handlers::spawn_pane_pty_for_restore(
                pane_id,
                cols,
                rows,
                cwd.as_deref(),
                scrollback_text.as_deref(),
                app_state,
                rt_handle,
            );
        }
        PaneTreeSnapshot::Split { .. } => {
            // pane_id is the first leaf of this sub-split — spawn its PTY.
            let leaf_data = wmux_core::first_leaf(snapshot);
            handlers::spawn_pane_pty_for_restore(
                pane_id,
                cols,
                rows,
                leaf_data.cwd,
                leaf_data.scrollback_text,
                app_state,
                rt_handle,
            );
            // Then build the split structure.
            fill_splits(snapshot, pane_id, cols, rows, app_state, rt_handle);
        }
    }
}

/// Send current UI state (sidebar width + window geometry) to the actor for session persistence.
fn send_ui_state(state: &super::UiState<'_>, app_state: &AppStateHandle) {
    let size = state.window.inner_size();
    let pos = state.window.outer_position().unwrap_or_default();
    app_state.update_ui_state(
        state.sidebar.width as u16,
        Some(wmux_core::WindowGeometry {
            x: pos.x,
            y: pos.y,
            width: size.width,
            height: size.height,
            maximized: state.window.is_maximized(),
        }),
    );
}

/// Process a browser command from the IPC handler on the UI/STA thread.
fn handle_browser_command(
    state: &mut super::UiState<'_>,
    app_state: &AppStateHandle,
    rt_handle: &tokio::runtime::Handle,
    proxy: &winit::event_loop::EventLoopProxy<WmuxEvent>,
    method: &str,
    params: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let mgr = state.browser_manager.as_mut().ok_or_else(|| {
        "browser integration not available (WebView2 runtime not installed)".to_owned()
    })?;

    match method {
        "open" => {
            let url = params
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("about:blank")
                .to_owned();

            // Create a backing pane (with PTY — needed by the surface system).
            let layout_pane_id = state.focused_pane;
            let backing_pane_id = PaneId::new();
            let cols = state.cols;
            let rows = state.rows;
            handlers::spawn_pane_pty(backing_pane_id, cols, rows, app_state, rt_handle);

            // Register browser surface in the actor, then defer panel creation
            // via WmuxEvent to avoid COM deadlocks.
            let app_clone = app_state.clone();
            let proxy_clone = proxy.clone();
            let url_clone = url.clone();
            rt_handle.spawn(async move {
                match app_clone
                    .create_browser_surface(layout_pane_id, backing_pane_id)
                    .await
                {
                    Ok(surface_id) => {
                        let _ = proxy_clone.send_event(WmuxEvent::CreateBrowserPanel {
                            surface_id,
                            url: url_clone,
                        });
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "failed to register browser surface");
                    }
                }
            });

            Ok(serde_json::json!({
                "status": "creating",
                "url": url,
            }))
        }

        "navigate" => {
            let url = params
                .get("url")
                .and_then(|v| v.as_str())
                .ok_or("missing required parameter 'url'")?;
            let surface_id_str = params
                .get("surface_id")
                .and_then(|v| v.as_str())
                .ok_or("missing required parameter 'surface_id'")?;
            let surface_id: wmux_core::SurfaceId = surface_id_str
                .parse()
                .map_err(|_| format!("invalid surface_id: {surface_id_str}"))?;

            let panel = mgr
                .get_panel(surface_id)
                .ok_or_else(|| format!("no browser panel for surface {surface_id}"))?;
            panel
                .navigate(url)
                .map_err(|e| format!("navigate failed: {e}"))?;

            Ok(serde_json::json!({"ok": true}))
        }

        "back" => {
            let sid = parse_surface_id(params)?;
            let panel = mgr.get_panel(sid).ok_or("panel not found")?;
            panel.back().map_err(|e| format!("{e}"))?;
            Ok(serde_json::json!({"ok": true}))
        }

        "forward" => {
            let sid = parse_surface_id(params)?;
            let panel = mgr.get_panel(sid).ok_or("panel not found")?;
            panel.forward().map_err(|e| format!("{e}"))?;
            Ok(serde_json::json!({"ok": true}))
        }

        "reload" => {
            let sid = parse_surface_id(params)?;
            let panel = mgr.get_panel(sid).ok_or("panel not found")?;
            panel.reload().map_err(|e| format!("{e}"))?;
            Ok(serde_json::json!({"ok": true}))
        }

        "url" => {
            let sid = parse_surface_id(params)?;
            let panel = mgr.get_panel(sid).ok_or("panel not found")?;
            let url = panel.current_url().map_err(|e| format!("{e}"))?;
            Ok(serde_json::json!({"url": url}))
        }

        "eval" => {
            let sid = parse_surface_id(params)?;
            let js = params
                .get("expression")
                .or_else(|| params.get("js"))
                .and_then(|v| v.as_str())
                .ok_or("missing 'expression' parameter")?;
            let panel = mgr.get_panel(sid).ok_or("panel not found")?;
            let result = panel.eval(js).map_err(|e| format!("{e}"))?;
            Ok(serde_json::json!({"result": result}))
        }

        "close" => {
            let sid = parse_surface_id(params)?;
            mgr.remove_panel(sid).map_err(|e| format!("{e}"))?;
            tracing::info!(surface_id = %sid, "browser panel removed");
            Ok(serde_json::json!({"ok": true}))
        }

        "identify" => Ok(serde_json::json!({
            "handler": "browser",
            "status": "active",
            "runtime_available": true,
        })),

        _ => Err(format!("browser.{method} not yet implemented")),
    }
}

/// Parse a surface_id from JSON-RPC params.
fn parse_surface_id(params: &serde_json::Value) -> Result<wmux_core::SurfaceId, String> {
    let s = params
        .get("surface_id")
        .and_then(|v| v.as_str())
        .ok_or("missing 'surface_id' parameter")?;
    s.parse().map_err(|_| format!("invalid surface_id: {s}"))
}
