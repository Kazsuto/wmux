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

use crate::{
    divider::{self, DividerOrientation},
    event::WmuxEvent,
    mouse::MouseButton,
    shortcuts::ShortcutAction,
    sidebar::SidebarInteraction,
    toast, UiError,
};

use super::{handlers, App, TabDragState, UiState};

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

        // Resolve the best available terminal font: preferred Nerd Font → fallback → monospace.
        let resolved_font = glyphon
            .resolve_terminal_font(&config.font_family)
            .map(|s| s.to_string());

        // Compute terminal dimensions from window size and DPI-scaled font metrics.
        // The font size is multiplied by the OS scale factor so cell dimensions
        // are in physical pixels — matching the wgpu surface coordinate space.
        let metrics = wmux_render::TerminalMetrics::new(
            glyphon.font_system(),
            resolved_font.as_deref(),
            Some(config.font_size * initial_scale_factor),
        );
        // Subtract UI chrome from the window size to get the usable terminal area.
        // This ensures the initial PTY dimensions match what the first frame will render.
        //
        // Note: sidebar.effective_width() returns unscaled logical pixels that are used
        // directly in physical-pixel coordinate space (a pre-existing DPI inconsistency
        // in the sidebar). We match that behavior here to avoid a startup resize.
        let titlebar_h = crate::titlebar::TITLE_BAR_HEIGHT * initial_scale_factor;
        let tab_bar_h = wmux_render::pane::TAB_BAR_HEIGHT * initial_scale_factor;
        let status_bar_h = crate::status_bar::STATUS_BAR_HEIGHT * initial_scale_factor;
        let sidebar_w = config.sidebar_width as f32;
        let pad = wmux_render::pane::TERMINAL_PADDING * initial_scale_factor;

        let usable_w = (gpu.width() as f32 - sidebar_w - 2.0 * pad).max(1.0);
        let usable_h =
            (gpu.height() as f32 - titlebar_h - tab_bar_h - status_bar_h - 2.0 * pad).max(1.0);

        let cols = (usable_w / metrics.cell_width).floor().max(1.0) as u32;
        let rows = (usable_h / metrics.cell_height).floor().max(1.0) as u32;
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

        // Install custom title bar chrome (removes native title bar, enables drag/snap).
        let custom_chrome = unsafe { crate::titlebar::install_custom_chrome(main_hwnd) };
        if custom_chrome {
            crate::titlebar::update_metrics(initial_scale_factor);
        } else {
            tracing::warn!("custom chrome failed, keeping native title bar");
        }

        // Restore session or create a fresh default pane.
        let session = self.pending_session.take();
        let restored_sidebar_width = session.as_ref().and_then(|s| {
            if s.sidebar_width > 0 {
                Some(s.sidebar_width)
            } else {
                None
            }
        });
        let restored_sidebar_collapsed = session.as_ref().is_some_and(|s| s.sidebar_collapsed);
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
                        AppEvent::PaneExited { pane_id, success } => {
                            let _ = proxy_fwd.send_event(WmuxEvent::PtyExited { pane_id, success });
                        }
                        AppEvent::FocusChanged { pane_id } => {
                            let _ = proxy_fwd.send_event(WmuxEvent::FocusPane(pane_id));
                        }
                        // Workspace events: trigger a redraw so the UI picks up
                        // the new active workspace (empty or not).
                        AppEvent::WorkspaceCreated { .. } | AppEvent::WorkspaceClosed { .. } => {
                            let _ = proxy_fwd.send_event(WmuxEvent::PtyOutput);
                        }
                        AppEvent::WorkspaceSwitched { .. } => {
                            let _ = proxy_fwd.send_event(WmuxEvent::PtyOutput);
                        }
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

        {
            let (br, bg, bb) = palette.background;
            let (r1, g1, b1) = palette.ansi[1];
            let (cr, cg, cb) = palette.cursor;
            tracing::info!(
                theme = %theme_engine.current_theme().name,
                bg = format_args!("#{br:02x}{bg:02x}{bb:02x}"),
                ansi_red = format_args!("#{r1:02x}{g1:02x}{b1:02x}"),
                cursor = format_args!("#{cr:02x}{cg:02x}{cb:02x}"),
                "theme loaded"
            );
        }

        let dark_mode = wmux_config::ThemeEngine::is_dark_mode();
        let title_colors = crate::effects::TitleBarColors {
            background: palette.background,
            text: palette.foreground,
            border: palette.background, // seamless border
        };
        let effect_result = crate::effects::apply_window_effects(&window, dark_mode, &title_colors);

        // Initialize status bar.
        let status_bar = crate::status_bar::StatusBar::new(
            glyphon.font_system(),
            gpu.width() as f32 / initial_scale_factor,
        );
        let status_bar_data = crate::status_bar::StatusBarData::default();

        // Pre-allocate search bar text buffers (query + match count).
        let search_text_metrics = glyphon::Metrics::new(
            crate::typography::CAPTION_FONT_SIZE,
            crate::typography::CAPTION_LINE_HEIGHT,
        );
        let search_query_buffer = glyphon::Buffer::new(glyphon.font_system(), search_text_metrics);
        let search_count_buffer = glyphon::Buffer::new(glyphon.font_system(), search_text_metrics);

        // Pre-allocate address bar text buffer.
        let address_bar_buffer = glyphon::Buffer::new(glyphon.font_system(), search_text_metrics);

        // Pre-allocate command palette text buffers.
        let palette_text_metrics = glyphon::Metrics::new(
            crate::typography::CAPTION_FONT_SIZE,
            crate::typography::CAPTION_LINE_HEIGHT,
        );
        let palette_query_buffer =
            glyphon::Buffer::new(glyphon.font_system(), palette_text_metrics);
        let palette_filter_labels = ["All", "Commands", "Workspaces", "Surfaces"];
        let palette_ui_attrs = glyphon::Attrs::new().family(glyphon::Family::Name("Segoe UI"));
        let pill_h = crate::command_palette::FILTER_ROW_HEIGHT
            - 2.0 * crate::command_palette::FILTER_TAB_PAD_Y;
        // Shape filter labels with generous buffer width, then measure actual text widths.
        let mut measured_filter_widths = [0.0f32; 4];
        let palette_filter_buffers: [glyphon::Buffer; 4] = std::array::from_fn(|i| {
            let mut b = glyphon::Buffer::new(glyphon.font_system(), palette_text_metrics);
            // Use generous buffer width for shaping — we'll measure actual width after.
            b.set_size(glyphon.font_system(), Some(300.0), Some(pill_h));
            b.set_text(
                glyphon.font_system(),
                palette_filter_labels[i],
                &palette_ui_attrs,
                glyphon::Shaping::Advanced,
                None,
            );
            b.shape_until_scroll(glyphon.font_system(), false);
            // Measure actual rendered text width from glyphon layout.
            // line_w is in buffer coordinates (unscaled) — multiply by scale_factor
            // to get physical pixel width, since all quad/bounds positions use physical pixels.
            let text_w = b.layout_runs().next().map_or(0.0, |run| run.line_w);
            measured_filter_widths[i] =
                text_w * initial_scale_factor + 2.0 * crate::command_palette::FILTER_TAB_PAD_X;
            b
        });
        let palette_result_buffers: Vec<glyphon::Buffer> = (0
            ..crate::command_palette::MAX_VISIBLE_RESULTS)
            .map(|_| glyphon::Buffer::new(glyphon.font_system(), palette_text_metrics))
            .collect();
        let palette_shortcut_buffers: Vec<glyphon::Buffer> = (0
            ..crate::command_palette::MAX_VISIBLE_RESULTS)
            .map(|_| glyphon::Buffer::new(glyphon.font_system(), palette_text_metrics))
            .collect();

        // Pre-allocate notification panel text buffers.
        let notif_title_metrics = glyphon::Metrics::new(
            crate::typography::TITLE_FONT_SIZE,
            crate::typography::TITLE_LINE_HEIGHT,
        );
        let notif_body_metrics = glyphon::Metrics::new(
            crate::typography::BODY_FONT_SIZE,
            crate::typography::BODY_LINE_HEIGHT,
        );
        let notif_caption_metrics = glyphon::Metrics::new(
            crate::typography::CAPTION_FONT_SIZE,
            crate::typography::CAPTION_LINE_HEIGHT,
        );
        let notif_ui_attrs = glyphon::Attrs::new().family(glyphon::Family::Name("Segoe UI"));
        let notif_ui_bold = glyphon::Attrs::new()
            .family(glyphon::Family::Name("Segoe UI"))
            .weight(glyphon::Weight::BOLD);

        // Locale for i18n string lookups.
        let locale = wmux_config::Locale::new(&config.language);

        // Initialize custom title bar — buffer width in logical pixels for Align::Center.
        let logical_width = gpu.width() as f32 / initial_scale_factor;
        let mut titlebar =
            crate::titlebar::TitleBarState::new(glyphon.font_system(), logical_width, &locale);
        titlebar.custom_chrome_active = custom_chrome;

        // Header "Notifications" (title size)
        let mut notif_header_buffer =
            glyphon::Buffer::new(glyphon.font_system(), notif_title_metrics);
        notif_header_buffer.set_size(glyphon.font_system(), Some(250.0), Some(30.0));
        notif_header_buffer.set_text(
            glyphon.font_system(),
            locale.t("notification.notifications"),
            &notif_ui_bold,
            glyphon::Shaping::Advanced,
            None,
        );
        notif_header_buffer.shape_until_scroll(glyphon.font_system(), false);

        // "Clear all" (caption size)
        let mut notif_clear_all_buffer =
            glyphon::Buffer::new(glyphon.font_system(), notif_caption_metrics);
        notif_clear_all_buffer.set_size(glyphon.font_system(), Some(80.0), Some(20.0));
        notif_clear_all_buffer.set_text(
            glyphon.font_system(),
            locale.t("notification.clear_all"),
            &notif_ui_attrs,
            glyphon::Shaping::Advanced,
            None,
        );
        notif_clear_all_buffer.shape_until_scroll(glyphon.font_system(), false);

        // Empty state text
        let mut notif_empty_buffer =
            glyphon::Buffer::new(glyphon.font_system(), notif_caption_metrics);
        notif_empty_buffer.set_size(glyphon.font_system(), Some(200.0), Some(20.0));
        notif_empty_buffer.set_text(
            glyphon.font_system(),
            locale.t("notification.no_notifications"),
            &notif_ui_attrs,
            glyphon::Shaping::Advanced,
            None,
        );
        notif_empty_buffer.shape_until_scroll(glyphon.font_system(), false);

        // Item buffer pools (category=caption, title=body bold, body=caption, time=caption).
        let notif_category_buffers: Vec<glyphon::Buffer> = (0
            ..crate::notification_panel::MAX_VISIBLE_ITEMS)
            .map(|_| glyphon::Buffer::new(glyphon.font_system(), notif_caption_metrics))
            .collect();
        let notif_title_buffers: Vec<glyphon::Buffer> = (0
            ..crate::notification_panel::MAX_VISIBLE_ITEMS)
            .map(|_| glyphon::Buffer::new(glyphon.font_system(), notif_body_metrics))
            .collect();
        let notif_body_buffers: Vec<glyphon::Buffer> = (0
            ..crate::notification_panel::MAX_VISIBLE_ITEMS)
            .map(|_| glyphon::Buffer::new(glyphon.font_system(), notif_caption_metrics))
            .collect();
        let notif_time_buffers: Vec<glyphon::Buffer> = (0
            ..crate::notification_panel::MAX_VISIBLE_ITEMS)
            .map(|_| glyphon::Buffer::new(glyphon.font_system(), notif_caption_metrics))
            .collect();

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
        let menu_hints = ["Ctrl-D Right", "Ctrl-D Left", "Ctrl-D Up", "Ctrl-D Down"];
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

        let tab_menu_labels = [locale.t("tab.rename_tab"), locale.t("tab.close_tab")];
        let tab_menu_buffers: [glyphon::Buffer; super::TAB_MENU_ITEMS] = std::array::from_fn(|i| {
            let mut b = glyphon::Buffer::new(glyphon.font_system(), menu_m);
            b.set_size(glyphon.font_system(), Some(180.0), Some(28.0));
            b.set_text(
                glyphon.font_system(),
                tab_menu_labels[i],
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
            notification_panel: crate::notification_panel::NotificationPanel::new(),
            notification_cache: Vec::new(),
            notif_header_buffer,
            notif_clear_all_buffer,
            notif_empty_buffer,
            notif_category_buffers,
            notif_title_buffers,
            notif_body_buffers,
            notif_time_buffers,
            dividers: Vec::new(),
            drag_state: None,
            focused_pane: pane_id,
            cols,
            rows,
            process_exited: false,
            terminal_modes: TerminalMode::empty(),
            last_layout: Vec::new(),
            command_palette: {
                let mut cp = crate::command_palette::CommandPalette::new();
                cp.filter_pill_widths = measured_filter_widths;
                cp
            },
            command_registry: wmux_core::CommandRegistry::with_defaults(),
            palette_query_buffer,
            palette_filter_buffers,
            palette_result_buffers,
            palette_shortcut_buffers,
            palette_actions: Vec::new(),
            palette_last_query: String::new(),
            palette_last_filter: crate::command_palette::PaletteFilter::All,
            search: crate::search::SearchState::new(),
            last_search_rows: Vec::new(),
            last_total_visible_rows: 0,
            search_query_buffer,
            search_count_buffer,
            tab_title_buffers: std::collections::HashMap::new(),
            toggle_label_buffers: std::collections::HashMap::new(),
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
            status_icon_cgs,
            cg_chrome_buttons: [
                svg_cg(wmux_render::icons::Icon::ChromeMinimize, 10.0),
                svg_cg(wmux_render::icons::Icon::ChromeMaximize, 10.0),
                svg_cg(wmux_render::icons::Icon::ChromeClose, 10.0),
            ],
            browser_manager,
            main_hwnd,
            focused_surface_kind: wmux_core::PanelKind::Terminal,
            address_bar: crate::address_bar::AddressBarState::new(),
            address_bar_buffer,
            browser_urls: std::collections::HashMap::new(),
            browser_focus_target: None,
            browser_default_url: config.browser_default_url.clone(),
            titlebar,
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
            tab_menu: super::TabContextMenuState::default(),
            tab_menu_buffers,
            tab_menu_hover: None,
            animation: {
                let mut engine = crate::animation::AnimationEngine::default();
                if !crate::effects::is_animations_enabled() {
                    engine.set_reduced_motion(true);
                }
                engine
            },
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
            terminal_font_family: resolved_font,
            terminal_font_size: config.font_size,
            locale,
            live_browser_sids: std::collections::HashSet::new(),
        });

        // Override sidebar width and collapsed state from saved session.
        if let Some(ref mut state) = self.state {
            if let Some(sw) = restored_sidebar_width {
                state.sidebar.width = (sw as f32).clamp(
                    crate::sidebar::MIN_SIDEBAR_WIDTH,
                    crate::sidebar::MAX_SIDEBAR_WIDTH,
                );
            }
            state.sidebar.collapsed = restored_sidebar_collapsed;
            // Set sidebar top offset to title bar height when custom chrome is active.
            if custom_chrome {
                state.sidebar.top_offset = crate::titlebar::TITLE_BAR_HEIGHT * initial_scale_factor;
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
            WmuxEvent::PtyExited { pane_id, success } => {
                if let Some(state) = self.state.as_mut() {
                    // Only block input for the focused pane — other panes
                    // and UI elements (address bar, sidebar) must keep working.
                    if pane_id == state.focused_pane {
                        state.process_exited = true;
                    }
                    state.window.request_redraw();
                    tracing::info!(%pane_id, success, "shell process exited");
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
                        // Use browser_viewport (below tab bar + address bar) for initial bounds.
                        let rect = state
                            .last_viewports
                            .iter()
                            .find(|vp| vp.surface_ids.contains(&surface_id))
                            .map(wmux_render::PaneRenderer::browser_viewport)
                            .unwrap_or_else(|| {
                                state
                                    .last_viewports
                                    .iter()
                                    .find(|vp| vp.focused)
                                    .map(wmux_render::PaneRenderer::browser_viewport)
                                    .unwrap_or_else(|| {
                                        wmux_core::rect::Rect::new(0.0, 0.0, 800.0, 600.0)
                                    })
                            });
                        match mgr.create_panel(surface_id, state.main_hwnd, &rect) {
                            Ok(_) => {
                                if let Some(panel) = mgr.get_panel(surface_id) {
                                    if let Err(e) = panel.navigate(&url) {
                                        tracing::error!(error = %e, url = %url, "browser navigate failed");
                                    }
                                    let _ = panel.focus_webview();
                                    state.browser_focus_target = Some(surface_id);
                                }
                                // Track URL for address bar display.
                                state.browser_urls.insert(surface_id, url.clone());
                                state.address_bar.set_url(&url);
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
                    state
                        .titlebar
                        .resize(state.glyphon.font_system(), w as f32 / state.scale_factor);

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

            WindowEvent::Moved(_) => {
                // WS_POPUP browser panels use screen coordinates — they do NOT
                // move automatically with the parent. Trigger a redraw so
                // render() repositions them via set_bounds + ClientToScreen.
                state.window.request_redraw();
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
                        state.terminal_font_family.as_deref(),
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
                    // Update title bar metrics for the new DPI scale.
                    if state.titlebar.custom_chrome_active {
                        crate::titlebar::update_metrics(new_scale);
                        state.sidebar.top_offset = crate::titlebar::TITLE_BAR_HEIGHT * new_scale;
                        state.titlebar.resize(
                            state.glyphon.font_system(),
                            state.gpu.width() as f32 / new_scale,
                        );
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

                // Close tab context menu on any keypress.
                if matches!(state.tab_menu, super::TabContextMenuState::Open { .. }) {
                    state.tab_menu = super::TabContextMenuState::Closed;
                    state.tab_menu_hover = None;
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

                // Priority 0.9: Chord completion — if a Ctrl+D chord is pending,
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
                    // The Ctrl+D prefix is consumed (not forwarded to terminal).
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
                    // Ctrl+D chord prefix: enter pending state instead of dispatching.
                    if action == ShortcutAction::ChordPrefix {
                        if !event.repeat {
                            state.chord_state =
                                super::ChordState::Pending(std::time::Instant::now());
                            tracing::debug!("chord prefix Ctrl+D — waiting for second key");
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

                // Priority 1.25: command palette input — consumes all keys when open.
                // Must be after shortcuts (so Ctrl+Shift+P toggles) but before
                // search and terminal input.
                if state.command_palette.open {
                    handlers::handle_palette_key(
                        state,
                        &event,
                        &self.app_state,
                        &self.rt_handle,
                        &self.proxy,
                    );
                    state.window.request_redraw();
                    return;
                }

                // Priority 1.3: notification panel — Escape closes the panel.
                if state.notification_panel.open
                    && event.logical_key
                        == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape)
                {
                    state.notification_panel.toggle();
                    state.window.request_redraw();
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

                // Priority 1.6: address bar editing — intercepts keyboard when
                // the user is typing a URL in the browser address bar.
                // Must be before process_exited check so URL editing works
                // even when the shell in the same pane has exited.
                if state.address_bar.editing {
                    if let Some(url) = handlers::handle_address_bar_key(state, &event) {
                        // Navigate the focused browser panel to the entered URL.
                        if let Some(ref mut mgr) = state.browser_manager {
                            // Find the focused browser surface ID.
                            if let Some(focused_vp) =
                                state.last_viewports.iter().find(|vp| vp.focused)
                            {
                                let active_sid =
                                    focused_vp.surface_ids.get(focused_vp.active_tab).copied();
                                if let Some(sid) = active_sid {
                                    if let Some(panel) = mgr.get_panel(sid) {
                                        let _ = panel.navigate(&url);
                                    }
                                    state.browser_urls.insert(sid, url);
                                }
                            }
                        }
                    }
                    state.window.request_redraw();
                    return;
                }

                // Don't send terminal input to a dead process.
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

                // Title bar button click handling — highest priority.
                if elem_state == ElementState::Pressed && button == winit::event::MouseButton::Left
                {
                    if let Some(btn) = state.titlebar.hit_test_button(
                        px,
                        py,
                        state.gpu.width() as f32,
                        state.scale_factor,
                    ) {
                        use windows::Win32::UI::WindowsAndMessaging::{
                            ShowWindow, SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE,
                        };
                        // SAFETY: ShowWindow/close are safe with a valid HWND from winit.
                        unsafe {
                            match btn {
                                crate::titlebar::ChromeButton::Minimize => {
                                    let _ = ShowWindow(state.main_hwnd, SW_MINIMIZE);
                                }
                                crate::titlebar::ChromeButton::Maximize => {
                                    if state.titlebar.is_maximized {
                                        let _ = ShowWindow(state.main_hwnd, SW_RESTORE);
                                    } else {
                                        let _ = ShowWindow(state.main_hwnd, SW_MAXIMIZE);
                                    }
                                }
                                crate::titlebar::ChromeButton::Close => {
                                    // WM_CLOSE triggers winit's CloseRequested → clean shutdown.
                                    windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                                        Some(state.main_hwnd),
                                        0x0010, // WM_CLOSE
                                        windows::Win32::Foundation::WPARAM(0),
                                        windows::Win32::Foundation::LPARAM(0),
                                    )
                                    .ok();
                                }
                            }
                        }
                        return;
                    }
                }

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
                                                    selected_all: false,
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

                // Tab context menu click handling — highest priority when open.
                if let super::TabContextMenuState::Open {
                    pane_id,
                    tab_index,
                    surface_id,
                    menu_x,
                    menu_y,
                } = state.tab_menu.clone()
                {
                    let item_h = 32.0_f32;
                    let menu_w = 200.0_f32;
                    let menu_items = super::TAB_MENU_ITEMS;
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
                                state.tab_menu = super::TabContextMenuState::Closed;
                                state.tab_menu_hover = None;
                                state.window.request_redraw();
                                return;
                            }
                            let item_idx = (raw / item_h).floor() as usize;
                            if item_idx < menu_items {
                                state.tab_menu = super::TabContextMenuState::Closed;
                                state.tab_menu_hover = None;
                                match item_idx {
                                    0 => {
                                        // Rename Tab — start inline editing.
                                        let title = state
                                            .last_viewports
                                            .iter()
                                            .find(|vp| vp.pane_id == pane_id)
                                            .and_then(|vp| vp.tab_titles.get(tab_index).cloned())
                                            .unwrap_or_default();
                                        let cursor = title.chars().count();
                                        state.tab_edit = super::TabEditState::Editing {
                                            pane_id,
                                            tab_index,
                                            surface_id,
                                            text: title,
                                            cursor,
                                            selected_all: false,
                                        };
                                        tracing::debug!(
                                            pane_id = %pane_id,
                                            tab_index,
                                            "tab: started inline editing via context menu"
                                        );
                                    }
                                    1 => {
                                        // Close Tab.
                                        tracing::info!(
                                            pane_id = %pane_id,
                                            surface_id = %surface_id,
                                            "surface closed via tab context menu"
                                        );
                                        self.app_state.close_surface(pane_id, surface_id);
                                    }
                                    _ => {}
                                }
                            }
                            state.window.request_redraw();
                            return;
                        }
                        // Click outside menu — close it.
                        state.tab_menu = super::TabContextMenuState::Closed;
                        state.tab_menu_hover = None;
                        state.window.request_redraw();
                        // Don't return — let click propagate.
                    }
                }

                // Notification panel click handling.
                if state.notification_panel.open
                    && elem_state == ElementState::Pressed
                    && button == winit::event::MouseButton::Left
                {
                    let sw = state.gpu.width() as f32;
                    if state.notification_panel.contains_x(px, sw) {
                        // Header hit-test (close / clear all).
                        if let Some(action) = state.notification_panel.hit_test_header(px, py, sw) {
                            match action {
                                crate::notification_panel::HeaderAction::Close => {
                                    state.notification_panel.toggle();
                                }
                                crate::notification_panel::HeaderAction::ClearAll => {
                                    let app = self.app_state.clone();
                                    self.rt_handle.spawn(async move {
                                        app.clear_all_notifications().await;
                                    });
                                }
                            }
                            state.window.request_redraw();
                            return;
                        }
                        // Item click — focus source workspace.
                        let total = state.notification_cache.len();
                        if let Some(idx) = state.notification_panel.hit_test(py, total) {
                            if let Some(notif) = state.notification_cache.get(idx) {
                                if let Some(ws_id) = notif.source_workspace {
                                    let app = self.app_state.clone();
                                    self.rt_handle.spawn(async move {
                                        app.select_workspace_by_id(ws_id).await;
                                    });
                                }
                            }
                            state.notification_panel.toggle();
                            state.window.request_redraw();
                            return;
                        }
                        // Click in panel but not on header/item — consume event.
                        return;
                    }
                    // Click outside notification panel — close it.
                    state.notification_panel.toggle();
                    state.window.request_redraw();
                    // Don't return — let click propagate.
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
                            if !state.sidebar.collapsed && state.mouse.click_count() >= 2 {
                                // Double-click: start inline editing (expanded only).
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
                                    selected_all: false,
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

                    // Right-click on sidebar row — open workspace context menu (expanded only).
                    if !state.sidebar.collapsed
                        && elem_state == ElementState::Pressed
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
                            // Toggle mode: segmented control hit-test.
                            if vp.is_toggle_mode() {
                                // Close button hit-test (priority).
                                if let Some((bx, by, bw, bh)) =
                                    wmux_render::pane::PaneRenderer::toggle_close_button_rect(vp)
                                {
                                    if px >= bx && px < bx + bw && py >= by && py < by + bh {
                                        if let Some(sid) =
                                            vp.surface_ids.get(vp.active_tab).copied()
                                        {
                                            tracing::info!(
                                                pane_id = %vp.pane_id,
                                                surface_id = %sid,
                                                "surface closed via toggle close button"
                                            );
                                            self.app_state.close_surface(vp.pane_id, sid);
                                            state.window.request_redraw();
                                        }
                                        tab_clicked = true;
                                        break;
                                    }
                                }

                                for seg in 0..2usize {
                                    if let Some((sx, sy, sw, sh)) =
                                        wmux_render::pane::PaneRenderer::toggle_segment_rect(
                                            vp, seg,
                                        )
                                    {
                                        if px >= sx && px < sx + sw && py >= sy && py < sy + sh {
                                            if vp.pane_id != state.focused_pane {
                                                state.set_focused_pane(vp.pane_id);
                                                self.app_state.focus_pane(vp.pane_id);
                                            }
                                            if let Some(tab_idx) =
                                                wmux_render::pane::PaneRenderer::toggle_segment_to_tab(
                                                    vp, seg,
                                                )
                                            {
                                                if tab_idx != vp.active_tab {
                                                    self.app_state.cycle_surface_to_index(
                                                        vp.pane_id, tab_idx,
                                                    );
                                                }
                                            }
                                            tab_clicked = true;
                                            state.window.request_redraw();
                                            break;
                                        }
                                    }
                                }
                                if tab_clicked {
                                    break;
                                }
                                // Click in tab bar area but outside toggle segments —
                                // treat as focus-only click.
                                if vp.pane_id != state.focused_pane {
                                    state.set_focused_pane(vp.pane_id);
                                    self.app_state.focus_pane(vp.pane_id);
                                }
                                tab_clicked = true;
                                state.window.request_redraw();
                                break;
                            }

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
                                        selected_all: false,
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

                // Address bar click handling — only for browser panes.
                if elem_state == ElementState::Pressed && button == winit::event::MouseButton::Left
                {
                    let viewports = state.last_viewports.clone();
                    for vp in &viewports {
                        let active_type = vp
                            .surface_types
                            .get(vp.active_tab)
                            .copied()
                            .unwrap_or(wmux_render::SurfaceType::Terminal);
                        if active_type != wmux_render::SurfaceType::Browser {
                            continue;
                        }
                        let bar_rect = wmux_render::PaneRenderer::address_bar_rect(vp);
                        let hit = state.address_bar.hit_test(px, py, &bar_rect, vp.scale);
                        match hit {
                            crate::address_bar::AddressBarHit::Back => {
                                if let Some(sid) = vp.surface_ids.get(vp.active_tab).copied() {
                                    if let Some(ref mut mgr) = state.browser_manager {
                                        if let Some(panel) = mgr.get_panel(sid) {
                                            let _ = panel.back();
                                        }
                                    }
                                }
                                state.window.request_redraw();
                                return;
                            }
                            crate::address_bar::AddressBarHit::Forward => {
                                if let Some(sid) = vp.surface_ids.get(vp.active_tab).copied() {
                                    if let Some(ref mut mgr) = state.browser_manager {
                                        if let Some(panel) = mgr.get_panel(sid) {
                                            let _ = panel.forward();
                                        }
                                    }
                                }
                                state.window.request_redraw();
                                return;
                            }
                            crate::address_bar::AddressBarHit::UrlField => {
                                // Focus the pane if needed.
                                if vp.pane_id != state.focused_pane {
                                    state.set_focused_pane(vp.pane_id);
                                    self.app_state.focus_pane(vp.pane_id);
                                }
                                state.address_bar.start_editing();
                                state.window.request_redraw();
                                return;
                            }
                            crate::address_bar::AddressBarHit::None => {}
                        }
                    }
                }

                // Any click that wasn't consumed by the address bar URL field
                // dismisses the editing mode (standard click-away-to-dismiss).
                if elem_state == ElementState::Pressed && state.address_bar.editing {
                    state.address_bar.cancel_editing();
                    state.window.request_redraw();
                }

                // Right-click on tab bar — open tab context menu.
                if elem_state == ElementState::Pressed && button == winit::event::MouseButton::Right
                {
                    let viewports = state.last_viewports.clone();
                    for vp in &viewports {
                        let tab_bar_bottom = vp.rect.y + vp.tab_bar_height();
                        if px >= vp.rect.x
                            && px < vp.rect.x + vp.rect.width
                            && py >= vp.rect.y
                            && py < tab_bar_bottom
                        {
                            // Find which tab was right-clicked.
                            let mut clicked_tab = None;
                            if vp.is_toggle_mode() {
                                // Toggle mode: right-click on a segment.
                                for seg in 0..2usize {
                                    if let Some((sx, sy, sw, sh)) =
                                        wmux_render::pane::PaneRenderer::toggle_segment_rect(
                                            vp, seg,
                                        )
                                    {
                                        if px >= sx && px < sx + sw && py >= sy && py < sy + sh {
                                            clicked_tab =
                                                wmux_render::pane::PaneRenderer::toggle_segment_to_tab(
                                                    vp, seg,
                                                );
                                            break;
                                        }
                                    }
                                }
                            } else {
                                // Pill mode: right-click on a tab pill.
                                for i in 0..vp.tab_count {
                                    let (tw, tx) =
                                        wmux_render::pane::PaneRenderer::tab_metrics(vp, i);
                                    if px >= tx && px < tx + tw {
                                        clicked_tab = Some(i);
                                        break;
                                    }
                                }
                            }

                            if let Some(tab_index) = clicked_tab {
                                if let Some(sid) = vp.surface_ids.get(tab_index).copied() {
                                    state.tab_menu = super::TabContextMenuState::Open {
                                        pane_id: vp.pane_id,
                                        tab_index,
                                        surface_id: sid,
                                        menu_x: px,
                                        menu_y: py,
                                    };
                                    state.tab_menu_hover = None;
                                    tracing::debug!(
                                        pane_id = %vp.pane_id,
                                        tab_index,
                                        "tab: opened context menu"
                                    );
                                    state.window.request_redraw();
                                    return;
                                }
                            }
                            break;
                        }
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

                // Title bar button hover tracking.
                {
                    let new_hover = state.titlebar.hit_test_button(
                        px,
                        py,
                        state.gpu.width() as f32,
                        state.scale_factor,
                    );
                    if new_hover != state.titlebar.hovered_button {
                        state.titlebar.hovered_button = new_hover;
                        state.window.request_redraw();
                    }
                }

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

                // Tab context menu hover tracking.
                if let super::TabContextMenuState::Open { menu_x, menu_y, .. } = state.tab_menu {
                    let item_h = 32.0;
                    let menu_w = 200.0;
                    let menu_items = super::TAB_MENU_ITEMS;
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
                    if new_hover != state.tab_menu_hover {
                        state.tab_menu_hover = new_hover;
                        state.window.request_redraw();
                    }
                }

                // Notification panel hover tracking.
                if state.notification_panel.open {
                    let sw = state.gpu.width() as f32;
                    if state.notification_panel.contains_x(px, sw) {
                        let total = state.notification_cache.len();
                        let old = state.notification_panel.hovered_item;
                        state.notification_panel.update_hover(py, total);
                        if state.notification_panel.hovered_item != old {
                            state.window.request_redraw();
                        }
                    } else if state.notification_panel.hovered_item.is_some() {
                        state.notification_panel.hovered_item = None;
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
                            if vp.is_toggle_mode() {
                                // Toggle mode: single close button.
                                if let Some((bx, by, bw, bh)) =
                                    wmux_render::pane::PaneRenderer::toggle_close_button_rect(vp)
                                {
                                    if px >= bx && px < bx + bw && py >= by && py < by + bh {
                                        new_hover = Some((vp.pane_id, vp.active_tab));
                                    }
                                }
                            } else {
                                // Pill mode: per-tab close buttons.
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
                            }
                            break;
                        }
                    }
                    if new_hover != state.tab_close_hover {
                        state.tab_close_hover = new_hover;
                        state.window.request_redraw();
                    }
                }

                // General tab hover tracking (for background highlight animation, skip toggle mode).
                {
                    let mut new_tab_hover = None;
                    for vp in &state.last_viewports {
                        if vp.is_toggle_mode() {
                            continue;
                        }
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

                    // Check if we should transition from Pressing to Dragging (expanded only).
                    if !state.sidebar.collapsed && state.sidebar.should_start_drag(py) {
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
                // Notification panel scroll — intercept when cursor is over the panel.
                if state.notification_panel.open {
                    let sw = state.gpu.width() as f32;
                    let px = state.cursor_pos.0 as f32;
                    if state.notification_panel.contains_x(px, sw) {
                        let pixel_delta = match delta {
                            MouseScrollDelta::LineDelta(_, y) => -y * 40.0,
                            MouseScrollDelta::PixelDelta(pos) => -pos.y as f32,
                        };
                        let sh = state.gpu.height() as f32;
                        let total = state.notification_cache.len();
                        state.notification_panel.scroll(pixel_delta, total, sh);
                        state.window.request_redraw();
                        return;
                    }
                }

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

/// Send current UI state (sidebar width, collapsed, window geometry) to the actor for session persistence.
fn send_ui_state(state: &super::UiState<'_>, app_state: &AppStateHandle) {
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
                .unwrap_or(state.browser_default_url.as_str())
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

            // Track URL for address bar display.
            state.browser_urls.insert(surface_id, url.to_owned());
            state.address_bar.set_url(url);

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
