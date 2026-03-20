use crate::UiError;
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowAttributes, WindowId},
};
use wmux_render::{GlyphonRenderer, GpuContext, QuadPipeline};

struct AppState<'window> {
    window: Arc<Window>,
    gpu: GpuContext<'window>,
    quads: QuadPipeline,
    text: GlyphonRenderer,
}

#[derive(Default)]
pub struct App<'window> {
    state: Option<AppState<'window>>,
}

impl<'window> App<'window> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn run() -> Result<(), UiError> {
        let event_loop = EventLoop::new()?;
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
        let mut app = App::new();
        event_loop.run_app(&mut app)?;
        Ok(())
    }

    fn render(&mut self) -> Result<(), UiError> {
        let state = self
            .state
            .as_mut()
            .expect("render called before window initialization");

        // Demo quads for visual verification (remove after terminal grid integration)
        state
            .quads
            .push_quad(50.0, 50.0, 200.0, 100.0, [0.8, 0.2, 0.2, 1.0]);
        state
            .quads
            .push_quad(300.0, 50.0, 200.0, 100.0, [0.2, 0.8, 0.2, 1.0]);
        state
            .quads
            .push_quad(550.0, 50.0, 200.0, 100.0, [0.2, 0.2, 0.8, 1.0]);
        state
            .quads
            .push_quad(175.0, 200.0, 200.0, 100.0, [0.8, 0.8, 0.2, 0.5]);

        // Upload GPU data before render pass
        state.quads.prepare(&state.gpu.queue);
        state.text.prepare(&state.gpu.device, &state.gpu.queue)?;

        let output = state.gpu.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder =
            state
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

            // Backgrounds first, then text on top
            state.quads.render(&mut render_pass);
            state.text.render(&mut render_pass)?;
        }

        state.gpu.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        state.quads.clear();

        Ok(())
    }
}

impl<'window> ApplicationHandler for App<'window> {
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

        let text = GlyphonRenderer::new(
            &gpu.device,
            &gpu.queue,
            gpu.format,
            gpu.width(),
            gpu.height(),
        );

        tracing::info!(
            width = gpu.width(),
            height = gpu.height(),
            format = ?gpu.format,
            "window created",
        );

        self.state = Some(AppState {
            window,
            gpu,
            quads,
            text,
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                tracing::info!("window close requested");
                event_loop.exit();
            }
            WindowEvent::Resized(physical_size) => {
                if let Some(state) = self.state.as_mut() {
                    let w = physical_size.width;
                    let h = physical_size.height;
                    if w > 0 && h > 0 {
                        state.gpu.resize(w, h);
                        state.quads.resize(&state.gpu.queue, w, h);
                        state.text.resize(&state.gpu.queue, w, h);
                        state.window.request_redraw();
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if self.state.is_some() {
                    if let Err(e) = self.render() {
                        tracing::error!(error = %e, "render failed");
                    }
                }
            }
            _ => {}
        }
    }
}
