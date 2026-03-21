use crate::event::WmuxEvent;
use crate::input::InputHandler;
use crate::mouse::{MouseAction, MouseButton, MouseHandler};
use crate::UiError;
use std::sync::Arc;
use tokio::sync::mpsc;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    keyboard::{KeyCode, ModifiersState, PhysicalKey},
    window::{Window, WindowAttributes, WindowId},
};
use wmux_core::{Terminal, TerminalMode};
use wmux_pty::{PtyActorHandle, PtyEvent, PtyManager, SpawnConfig};
use wmux_render::{GlyphonRenderer, GpuContext, QuadPipeline, TerminalMetrics, TerminalRenderer};

/// All state created during window initialization.
struct AppState<'window> {
    // Rendering
    window: Arc<Window>,
    gpu: GpuContext<'window>,
    quads: QuadPipeline,
    glyphon: GlyphonRenderer,
    terminal_renderer: TerminalRenderer,
    metrics: TerminalMetrics,

    // Terminal
    terminal: Terminal,
    terminal_event_rx: tokio::sync::mpsc::Receiver<wmux_core::TerminalEvent>,

    // Input
    input: InputHandler,
    mouse: MouseHandler,
    modifiers: ModifiersState,
    cursor_pos: (f64, f64),

    // PTY bridge channels
    pty_write_tx: mpsc::Sender<Vec<u8>>,
    pty_resize_tx: mpsc::Sender<(u16, u16)>,
    pty_output_rx: mpsc::Receiver<Vec<u8>>,

    // Process state
    process_exited: bool,
}

impl AppState<'_> {
    /// Drain all buffered PTY output into the terminal state machine,
    /// then forward any terminal write-back responses (DSR, DA1) to the PTY.
    fn drain_pty_output(&mut self) {
        while let Ok(data) = self.pty_output_rx.try_recv() {
            self.terminal.process(&data);
        }

        while let Ok(event) = self.terminal_event_rx.try_recv() {
            if let wmux_core::TerminalEvent::PtyWrite(bytes) = event {
                let _ = self.pty_write_tx.try_send(bytes);
            }
        }
    }

    /// Render a frame: drain PTY output, update terminal, draw.
    fn render(&mut self) -> Result<(), UiError> {
        self.drain_pty_output();

        // Update terminal renderer — dirty rows → glyphon buffers + background quads
        let (grid, scrollback) = self.terminal.grid_and_scrollback();
        self.terminal_renderer.update(
            grid,
            scrollback,
            self.glyphon.font_system(),
            &mut self.quads,
        );

        // Selection highlight overlay
        if let Some(sel) = self.mouse.selection() {
            let (start, end) = sel.normalized();
            let cols = self.terminal.cols() as usize;
            for row in start.row..=end.row {
                let col_start = if row == start.row { start.col } else { 0 };
                let col_end = if row == end.row { end.col + 1 } else { cols };
                let x = col_start as f32 * self.metrics.cell_width;
                let y = row as f32 * self.metrics.cell_height;
                let w = col_end.saturating_sub(col_start) as f32 * self.metrics.cell_width;
                let h = self.metrics.cell_height;
                self.quads.push_quad(x, y, w, h, [0.3, 0.5, 0.8, 0.3]);
            }
        }

        // Upload GPU data
        self.quads.prepare(&self.gpu.queue);
        self.terminal_renderer.prepare(
            &self.gpu.device,
            &self.gpu.queue,
            &mut self.glyphon,
            self.gpu.width(),
            self.gpu.height(),
        )?;

        // Render pass
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

            // Backgrounds + cursor + selection first, then text on top
            self.quads.render(&mut render_pass);
            self.terminal_renderer
                .render(&mut render_pass, &self.glyphon)?;
        }

        self.gpu.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        self.quads.clear();

        Ok(())
    }

    /// Convert the current cursor pixel position to cell coordinates,
    /// clamped to valid terminal bounds.
    fn cursor_cell(&self) -> (usize, usize) {
        let max_col = (self.terminal.cols() as usize).saturating_sub(1);
        let max_row = (self.terminal.rows() as usize).saturating_sub(1);
        let col = (self.cursor_pos.0.max(0.0) as f32 / self.metrics.cell_width) as usize;
        let row = (self.cursor_pos.1.max(0.0) as f32 / self.metrics.cell_height) as usize;
        (col.min(max_col), row.min(max_row))
    }

    /// Process a mouse action returned by the mouse handler.
    fn handle_mouse_action(&mut self, action: MouseAction) {
        match action {
            MouseAction::None => {}
            MouseAction::SelectionStarted
            | MouseAction::SelectionUpdated
            | MouseAction::SelectionFinished => {
                self.window.request_redraw();
            }
            MouseAction::Report(bytes) => {
                let _ = self.pty_write_tx.try_send(bytes);
            }
            MouseAction::Scroll(new_offset) => {
                let current = self.terminal.viewport_offset();
                if new_offset > current {
                    self.terminal.scroll_viewport_up(new_offset - current);
                } else {
                    self.terminal.scroll_viewport_down(current - new_offset);
                }
                self.window.request_redraw();
            }
        }
    }
}

/// Main application — owns the winit event loop and terminal state.
pub struct App<'window> {
    state: Option<AppState<'window>>,
    rt_handle: tokio::runtime::Handle,
    proxy: EventLoopProxy<WmuxEvent>,
}

impl<'window> App<'window> {
    /// Create the event loop and run the application.
    ///
    /// `rt_handle` must come from a tokio runtime created before the
    /// event loop — winit owns the main thread.
    pub fn run(rt_handle: tokio::runtime::Handle) -> Result<(), UiError> {
        let event_loop = EventLoop::<WmuxEvent>::with_user_event().build()?;
        let proxy = event_loop.create_proxy();
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
        let mut app = App {
            state: None,
            rt_handle,
            proxy,
        };
        event_loop.run_app(&mut app)?;
        Ok(())
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
        let (terminal, terminal_event_rx) = Terminal::with_event_channel(cols, rows);

        // Spawn PTY
        let manager = PtyManager::new();
        let config = SpawnConfig {
            cols,
            rows,
            ..Default::default()
        };
        let handle = manager.spawn(config).expect("failed to spawn PTY");

        // Bridge channels between tokio (PTY) and winit (main thread)
        let (output_tx, output_rx) = mpsc::channel::<Vec<u8>>(256);
        let (write_tx, mut write_rx) = mpsc::channel::<Vec<u8>>(256);
        let (resize_tx, mut resize_rx) = mpsc::channel::<(u16, u16)>(4);

        let proxy = self.proxy.clone();
        self.rt_handle.spawn(async move {
            let mut actor = PtyActorHandle::spawn(handle);
            loop {
                tokio::select! {
                    event = actor.next_event() => {
                        match event {
                            Some(PtyEvent::Output(data)) => {
                                if output_tx.send(data).await.is_err() {
                                    break;
                                }
                                let _ = proxy.send_event(WmuxEvent::PtyOutput);
                            }
                            Some(PtyEvent::Exited { success }) => {
                                let _ = proxy.send_event(WmuxEvent::PtyExited { success });
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
            tracing::info!("PTY bridge task ended");
        });

        tracing::info!(
            cols,
            rows,
            width = gpu.width(),
            height = gpu.height(),
            format = ?gpu.format,
            "terminal initialized",
        );

        self.state = Some(AppState {
            window,
            gpu,
            quads,
            glyphon,
            terminal_renderer,
            metrics,
            terminal,
            terminal_event_rx,
            input: InputHandler::new(),
            mouse: MouseHandler::new(),
            modifiers: ModifiersState::default(),
            cursor_pos: (0.0, 0.0),
            pty_write_tx: write_tx,
            pty_resize_tx: resize_tx,
            pty_output_rx: output_rx,
            process_exited: false,
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
                    // Write exit message into terminal
                    let msg = if success {
                        "\r\n[Process exited]\r\n"
                    } else {
                        "\r\n[Process exited with error]\r\n"
                    };
                    state.terminal.process(msg.as_bytes());
                    state.window.request_redraw();
                    tracing::info!(success, "shell process exited");
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

                    if new_cols != state.terminal.cols() || new_rows != state.terminal.rows() {
                        state.terminal.resize(new_cols, new_rows);
                        state.terminal_renderer.resize(
                            new_cols,
                            new_rows,
                            state.glyphon.font_system(),
                        );
                        let _ = state.pty_resize_tx.try_send((new_rows, new_cols));
                    }

                    state.window.request_redraw();
                }
            }

            WindowEvent::RedrawRequested => {
                match state.render() {
                    Ok(()) => {}
                    Err(UiError::Surface(
                        wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated,
                    )) => {
                        // Reconfigure surface and retry
                        let w = state.gpu.width();
                        let h = state.gpu.height();
                        state.gpu.resize(w, h);
                        state.window.request_redraw();
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "render failed");
                    }
                }
            }

            WindowEvent::ModifiersChanged(modifiers) => {
                state.modifiers = modifiers.state();
            }

            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }

                // Don't accept keyboard input after process exit (except scrolling)
                if state.process_exited {
                    return;
                }

                // Copy: Ctrl+Shift+C
                if state.modifiers.control_key() && state.modifiers.shift_key() {
                    if event.physical_key == PhysicalKey::Code(KeyCode::KeyC) {
                        state
                            .mouse
                            .copy_selection(state.terminal.grid(), state.terminal.scrollback());
                        return;
                    }
                    // Paste: Ctrl+Shift+V
                    if event.physical_key == PhysicalKey::Code(KeyCode::KeyV) {
                        if let Some(text) = state.mouse.paste_from_clipboard() {
                            let bytes = state
                                .input
                                .wrap_bracketed_paste(&text, state.terminal.modes());
                            let _ = state.pty_write_tx.try_send(bytes);
                            state.window.request_redraw();
                        }
                        return;
                    }
                }

                // Regular key input → PTY
                if let Some(bytes) =
                    state
                        .input
                        .handle_key_event(&event, &state.modifiers, state.terminal.modes())
                {
                    state.terminal.reset_viewport();
                    state.mouse.clear_selection();
                    let _ = state.pty_write_tx.try_send(bytes);
                    state.window.request_redraw();
                }
            }

            WindowEvent::MouseInput {
                state: elem_state,
                button,
                ..
            } => {
                let btn = match button {
                    winit::event::MouseButton::Left => MouseButton::Left,
                    winit::event::MouseButton::Right => MouseButton::Right,
                    winit::event::MouseButton::Middle => MouseButton::Middle,
                    _ => return,
                };

                let mouse_mode = state
                    .terminal
                    .modes()
                    .contains(TerminalMode::MOUSE_REPORTING);
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

                state.handle_mouse_action(action);
            }

            WindowEvent::CursorMoved { position, .. } => {
                state.cursor_pos = (position.x, position.y);
                let (col, row) = state.cursor_cell();
                let mouse_mode = state
                    .terminal
                    .modes()
                    .contains(TerminalMode::MOUSE_REPORTING);
                let action = state.mouse.handle_mouse_motion(col, row, mouse_mode);
                state.handle_mouse_action(action);
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let mouse_mode = state
                    .terminal
                    .modes()
                    .contains(TerminalMode::MOUSE_REPORTING);

                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y as f64,
                    MouseScrollDelta::PixelDelta(pos) => pos.y / state.metrics.cell_height as f64,
                };

                if mouse_mode {
                    // In mouse reporting mode, send SGR wheel events
                    let (col, row) = state.cursor_cell();
                    let button: u8 = if lines > 0.0 { 64 } else { 65 };
                    let report = {
                        use std::io::Write;
                        let mut buf = Vec::with_capacity(16);
                        let _ = write!(buf, "\x1b[<{};{};{}M", button, col + 1, row + 1);
                        buf
                    };
                    let _ = state.pty_write_tx.try_send(report);
                } else {
                    // Normal scroll — adjust viewport
                    let viewport_offset = state.terminal.viewport_offset();
                    let scrollback_len = state.terminal.scrollback().len();
                    let action = state
                        .mouse
                        .handle_scroll(lines, viewport_offset, scrollback_len);
                    state.handle_mouse_action(action);
                }
            }

            _ => {}
        }
    }
}
