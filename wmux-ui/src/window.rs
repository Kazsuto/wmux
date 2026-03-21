use crate::event::WmuxEvent;
use crate::input::InputHandler;
use crate::mouse::{MouseAction, MouseButton, MouseHandler};
use crate::shortcuts::{ShortcutAction, ShortcutMap};
use crate::toast::{self, ToastService};
use crate::UiError;
use std::sync::Arc;
use tokio::sync::mpsc;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    keyboard::ModifiersState,
    window::{Window, WindowAttributes, WindowId},
};
use wmux_core::surface::SplitDirection;
use wmux_core::surface_manager::{Surface, SurfaceManager};
use wmux_core::{
    AppEvent, AppStateHandle, FocusDirection, PaneId, PaneState, Terminal, TerminalMode,
};
use wmux_pty::{PtyActorHandle, PtyEvent, PtyManager, SpawnConfig};
use wmux_render::{GlyphonRenderer, GpuContext, QuadPipeline, TerminalMetrics, TerminalRenderer};

/// UI-thread state created during window initialization.
///
/// Contains only rendering and input state. All terminal/pane state
/// lives in the AppState actor and is accessed via snapshots.
struct UiState<'window> {
    // Rendering
    window: Arc<Window>,
    gpu: GpuContext<'window>,
    quads: QuadPipeline,
    glyphon: GlyphonRenderer,
    terminal_renderer: TerminalRenderer,
    metrics: TerminalMetrics,

    // Input
    input: InputHandler,
    mouse: MouseHandler,
    shortcuts: ShortcutMap,
    modifiers: ModifiersState,
    cursor_pos: (f64, f64),

    // Notifications
    toast_service: ToastService,

    // Active pane tracking
    focused_pane: PaneId,
    cols: u16,
    rows: u16,
    process_exited: bool,
    /// Cached terminal modes from the last render snapshot.
    terminal_modes: TerminalMode,
}

impl UiState<'_> {
    /// Render a frame: get layout from actor, draw borders, render focused pane.
    fn render(
        &mut self,
        app_state: &AppStateHandle,
        rt: &tokio::runtime::Handle,
    ) -> Result<(), UiError> {
        let surface_viewport =
            wmux_core::rect::Rect::new(0.0, 0.0, self.gpu.width() as f32, self.gpu.height() as f32);

        // Get pane layout from the actor (blocks briefly).
        let layout = rt.block_on(app_state.get_layout(surface_viewport));

        // Build PaneViewport descriptors for border rendering.
        let viewports: Vec<wmux_render::PaneViewport> = layout
            .iter()
            .map(|(id, rect)| wmux_render::PaneViewport {
                pane_id: *id,
                rect: *rect,
                focused: *id == self.focused_pane,
                tab_count: 1,
                tab_titles: vec![],
                active_tab: 0,
                zoomed: false,
            })
            .collect();

        // Draw 1px borders for all panes (focused pane gets accent colour).
        let pane_renderer = wmux_render::PaneRenderer::new();
        pane_renderer.render_pane_borders(
            &mut self.quads,
            &viewports,
            [0.3, 0.3, 0.35, 1.0],
            [0.2, 0.6, 0.9, 1.0],
        );

        // Dark background fill for non-focused panes.
        for vp in &viewports {
            if !vp.focused {
                self.quads.push_quad(
                    vp.rect.x,
                    vp.rect.y,
                    vp.rect.width,
                    vp.rect.height,
                    [0.08, 0.08, 0.1, 1.0],
                );
            }
        }

        // Determine the focused pane rect (falls back to full surface for single-pane).
        let focused_rect = viewports
            .iter()
            .find(|vp| vp.focused)
            .map(|vp| vp.rect)
            .unwrap_or(surface_viewport);

        // Request render data for the focused pane only.
        let render_data = rt.block_on(app_state.get_render_data(self.focused_pane));

        if let Some(data) = render_data {
            self.process_exited = data.process_exited;
            self.terminal_modes = data.modes;

            // Render terminal content at the focused pane's origin.
            self.terminal_renderer.update_from_snapshot(
                &data.grid,
                &data.dirty_rows,
                data.viewport_offset,
                &data.scrollback_visible_rows,
                self.glyphon.font_system(),
                &mut self.quads,
                (focused_rect.x, focused_rect.y),
            );

            // Selection highlight overlay, offset by the pane's origin.
            if let Some(sel) = self.mouse.selection() {
                let (start, end) = sel.normalized();
                let cols = self.cols as usize;
                for row in start.row..=end.row {
                    let col_start = if row == start.row { start.col } else { 0 };
                    let col_end = if row == end.row { end.col + 1 } else { cols };
                    let x = focused_rect.x + col_start as f32 * self.metrics.cell_width;
                    let y = focused_rect.y + row as f32 * self.metrics.cell_height;
                    let w = col_end.saturating_sub(col_start) as f32 * self.metrics.cell_width;
                    let h = self.metrics.cell_height;
                    self.quads.push_quad(x, y, w, h, [0.3, 0.5, 0.8, 0.3]);
                }
            }
        }

        // Upload GPU data.
        self.quads.prepare(&self.gpu.queue);
        self.terminal_renderer.prepare(
            &self.gpu.device,
            &self.gpu.queue,
            &mut self.glyphon,
            self.gpu.width(),
            self.gpu.height(),
            (focused_rect.x, focused_rect.y),
            focused_rect,
        )?;

        // Render pass.
        let output = self.gpu.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("wmux_encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("wmux_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.12,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            // Backgrounds + borders + cursor + selection first, then text on top.
            self.quads.render(&mut render_pass);
            self.terminal_renderer
                .render(&mut render_pass, &self.glyphon)?;
        }

        self.gpu.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        self.quads.clear();

        Ok(())
    }

    /// Convert the current cursor pixel position to cell coordinates.
    fn cursor_cell(&self) -> (usize, usize) {
        let max_col = (self.cols as usize).saturating_sub(1);
        let max_row = (self.rows as usize).saturating_sub(1);
        let col = (self.cursor_pos.0.max(0.0) as f32 / self.metrics.cell_width) as usize;
        let row = (self.cursor_pos.1.max(0.0) as f32 / self.metrics.cell_height) as usize;
        (col.min(max_col), row.min(max_row))
    }

    /// Process a mouse action returned by the mouse handler.
    fn handle_mouse_action(&mut self, action: MouseAction, app_state: &AppStateHandle) {
        match action {
            MouseAction::None => {}
            MouseAction::SelectionStarted
            | MouseAction::SelectionUpdated
            | MouseAction::SelectionFinished => {
                self.window.request_redraw();
            }
            MouseAction::Report(bytes) => {
                app_state.send_input(self.focused_pane, bytes);
            }
            MouseAction::Scroll(_) => {
                // Scroll is handled directly in mouse wheel event handler.
                self.window.request_redraw();
            }
        }
    }
}

/// Spawn a new terminal + PTY for `pane_id`, register it with the AppState actor,
/// and start the PTY bridge task.
///
/// This is a fire-and-forget helper: it spawns blocking work inside
/// `rt_handle` and returns immediately. Any spawn failure is logged.
fn spawn_pane_pty(
    pane_id: PaneId,
    cols: u16,
    rows: u16,
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
        surfaces: SurfaceManager::new(Surface::new("shell")),
    };
    app_state.register_pane(pane_id, pane_state);

    // Spawn PTY process.
    let manager = PtyManager::new();
    let config = SpawnConfig {
        cols,
        rows,
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

/// Main application — owns the winit event loop and AppState handle.
pub struct App<'window> {
    state: Option<UiState<'window>>,
    app_state: AppStateHandle,
    app_event_rx: Option<mpsc::Receiver<AppEvent>>,
    rt_handle: tokio::runtime::Handle,
    proxy: EventLoopProxy<WmuxEvent>,
}

impl<'window> App<'window> {
    /// Create the event loop and run the application.
    pub fn run(
        rt_handle: tokio::runtime::Handle,
        app_state: AppStateHandle,
        app_event_rx: mpsc::Receiver<AppEvent>,
    ) -> Result<(), UiError> {
        let event_loop = EventLoop::<WmuxEvent>::with_user_event().build()?;
        let proxy = event_loop.create_proxy();
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
        let mut app = App {
            state: None,
            app_state,
            app_event_rx: Some(app_event_rx),
            rt_handle,
            proxy,
        };
        event_loop.run_app(&mut app)?;
        Ok(())
    }
}

/// Dispatch a matched global shortcut action.
///
/// Takes the app_state handle and rt_handle by reference to avoid borrow
/// conflicts with the mutable UiState borrow in the event handler.
fn handle_shortcut(
    action: ShortcutAction,
    state: &mut UiState<'_>,
    app_state: &AppStateHandle,
    rt_handle: &tokio::runtime::Handle,
    proxy: &EventLoopProxy<WmuxEvent>,
) {
    match action {
        ShortcutAction::SplitRight => {
            let pane_id = state.focused_pane;
            let cols = state.cols;
            let rows = state.rows;
            let app_state_clone = app_state.clone();
            let rt_clone = rt_handle.clone();
            let proxy_clone = proxy.clone();
            rt_handle.spawn(async move {
                match app_state_clone
                    .split_pane(pane_id, SplitDirection::Horizontal)
                    .await
                {
                    Ok(new_id) => {
                        spawn_pane_pty(new_id, cols, rows, &app_state_clone, &rt_clone);
                        app_state_clone.focus_pane(new_id);
                        let _ = proxy_clone.send_event(WmuxEvent::FocusPane(new_id));
                        tracing::info!(
                            pane_id = %pane_id,
                            new_pane = %new_id,
                            "pane split right"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "SplitRight failed");
                    }
                }
            });
            state.window.request_redraw();
        }

        ShortcutAction::SplitDown => {
            let pane_id = state.focused_pane;
            let cols = state.cols;
            let rows = state.rows;
            let app_state_clone = app_state.clone();
            let rt_clone = rt_handle.clone();
            let proxy_clone = proxy.clone();
            rt_handle.spawn(async move {
                match app_state_clone
                    .split_pane(pane_id, SplitDirection::Vertical)
                    .await
                {
                    Ok(new_id) => {
                        spawn_pane_pty(new_id, cols, rows, &app_state_clone, &rt_clone);
                        app_state_clone.focus_pane(new_id);
                        let _ = proxy_clone.send_event(WmuxEvent::FocusPane(new_id));
                        tracing::info!(
                            pane_id = %pane_id,
                            new_pane = %new_id,
                            "pane split down"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "SplitDown failed");
                    }
                }
            });
            state.window.request_redraw();
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
                state.focused_pane = *next_id;
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
                    .create_workspace("New Workspace".to_string())
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
            let pane_id = state.focused_pane;
            let app_clone = app_state.clone();
            rt_handle.spawn(async move {
                match app_clone.create_surface(pane_id).await {
                    Ok(id) => tracing::info!(surface_id = %id, "new surface created"),
                    Err(e) => tracing::warn!(error = %e, "create surface failed"),
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

        // Placeholders for future tasks.
        ShortcutAction::CommandPalette => {
            tracing::debug!("CommandPalette shortcut (placeholder — Task L4_01)");
        }
        ShortcutAction::Find => {
            tracing::debug!("Find shortcut (placeholder)");
        }
        ShortcutAction::ToggleDevTools => {
            tracing::debug!("ToggleDevTools shortcut (placeholder)");
        }
    }
}

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

        let quads = QuadPipeline::new(
            &gpu.device,
            &gpu.queue,
            gpu.format,
            gpu.width(),
            gpu.height(),
        );

        let mut glyphon = GlyphonRenderer::new(
            &gpu.device,
            &gpu.queue,
            gpu.format,
            gpu.width(),
            gpu.height(),
        );

        // Compute terminal dimensions from window size and font metrics
        let metrics = TerminalMetrics::new(glyphon.font_system());
        let cols = ((gpu.width() as f32) / metrics.cell_width).floor() as u16;
        let rows = ((gpu.height() as f32) / metrics.cell_height).floor() as u16;
        let cols = cols.max(1);
        let rows = rows.max(1);

        let terminal_renderer = TerminalRenderer::new(glyphon.font_system(), cols, rows);

        // Initialize Windows Toast notification support.
        toast::init_aumid();
        let toast_service = ToastService::new();

        // Generate pane ID and delegate PTY spawn + registration to shared helper.
        let pane_id = PaneId::new();
        spawn_pane_pty(pane_id, cols, rows, &self.app_state, &self.rt_handle);

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
            terminal_renderer,
            metrics,
            input: InputHandler::new(),
            mouse: MouseHandler::new(),
            shortcuts: ShortcutMap::new(),
            modifiers: ModifiersState::default(),
            cursor_pos: (0.0, 0.0),
            toast_service,
            focused_pane: pane_id,
            cols,
            rows,
            process_exited: false,
            terminal_modes: TerminalMode::empty(),
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
        let state = match self.state.as_mut() {
            Some(s) => s,
            None => return,
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
                    let new_cols = ((w as f32) / state.metrics.cell_width).floor() as u16;
                    let new_rows = ((h as f32) / state.metrics.cell_height).floor() as u16;
                    let new_cols = new_cols.max(1);
                    let new_rows = new_rows.max(1);

                    if new_cols != state.cols || new_rows != state.rows {
                        state.cols = new_cols;
                        state.rows = new_rows;
                        state.terminal_renderer.resize(
                            new_cols,
                            new_rows,
                            state.glyphon.font_system(),
                        );
                        // Resize via actor (handles terminal + PTY).
                        self.app_state
                            .resize_pane(state.focused_pane, new_cols, new_rows);
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

                if state.process_exited {
                    return;
                }

                // Priority 1: global shortcuts — intercepted before terminal input.
                if let Some(action) = state.shortcuts.match_shortcut(
                    &event.logical_key,
                    event.physical_key,
                    &state.modifiers,
                ) {
                    handle_shortcut(action, state, &self.app_state, &self.rt_handle, &self.proxy);
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
                // Click-to-focus: on any press, check if the click landed in a
                // different pane and switch focus to it.
                if elem_state == ElementState::Pressed {
                    let px = state.cursor_pos.0 as f32;
                    let py = state.cursor_pos.1 as f32;
                    let viewport = wmux_core::rect::Rect::new(
                        0.0,
                        0.0,
                        state.gpu.width() as f32,
                        state.gpu.height() as f32,
                    );
                    let layout = self.rt_handle.block_on(self.app_state.get_layout(viewport));
                    for (pane_id, rect) in &layout {
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
