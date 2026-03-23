use bytemuck::{Pod, Zeroable};

/// Maximum shadows per frame. Shadows are few (tab bar, sidebar, overlays).
const MAX_SHADOWS: usize = 256;

/// Per-instance data for an analytical drop shadow (Evan Wallace technique).
///
/// The expanded quad covers the shadow blur extent. The caster rect
/// (the element casting the shadow) is described in local coordinates
/// so the fragment shader can compute the Gaussian integral analytically.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ShadowInstance {
    /// Expanded quad position (includes 3*sigma padding + offset).
    pub x: f32,
    pub y: f32,
    /// Expanded quad size.
    pub w: f32,
    pub h: f32,
    /// Shadow color (typically black with theme-adaptive alpha).
    pub color: [f32; 4],
    /// Caster rect origin in expanded quad local space.
    pub caster_x: f32,
    pub caster_y: f32,
    /// Caster rect size.
    pub caster_w: f32,
    pub caster_h: f32,
    /// Gaussian blur sigma (standard deviation in pixels).
    pub sigma: f32,
    /// Caster border radius (uniform, for future rounded shadow support).
    pub border_radius: f32,
    pub _pad: [f32; 2],
}

/// Viewport uniform, shared layout with QuadPipeline.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct ViewportUniform {
    width: f32,
    height: f32,
    _padding: [f32; 2],
}

/// GPU pipeline for analytical drop shadows rendered before content quads.
///
/// Usage each frame:
/// 1. `push_shadow()` to queue shadows
/// 2. `prepare(queue)` to upload (before render pass)
/// 3. `render(render_pass)` to draw (inside render pass, before quads)
/// 4. `clear()` to reset for next frame
pub struct ShadowPipeline {
    pipeline: wgpu::RenderPipeline,
    instance_buffer: wgpu::Buffer,
    viewport_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    shadows: Vec<ShadowInstance>,
    capacity: usize,
}

impl ShadowPipeline {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let width = width.max(1);
        let height = height.max(1);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("wmux_shadow_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shadow.wgsl").into()),
        });

        let viewport_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("wmux_shadow_viewport"),
            size: std::mem::size_of::<ViewportUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let viewport = ViewportUniform {
            width: width as f32,
            height: height as f32,
            _padding: [0.0; 2],
        };
        queue.write_buffer(&viewport_buffer, 0, bytemuck::bytes_of(&viewport));

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("wmux_shadow_bind_group_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("wmux_shadow_bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: viewport_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("wmux_shadow_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });

        let instance_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ShadowInstance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // pos: vec2<f32>
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                // size: vec2<f32>
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 8,
                    shader_location: 1,
                },
                // color: vec4<f32>
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 16,
                    shader_location: 2,
                },
                // caster_pos: vec2<f32>
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 32,
                    shader_location: 3,
                },
                // caster_size: vec2<f32>
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 40,
                    shader_location: 4,
                },
                // sigma: f32
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 48,
                    shader_location: 5,
                },
                // border_radius: f32
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 52,
                    shader_location: 6,
                },
            ],
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("wmux_shadow_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[instance_layout],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("wmux_shadow_instances"),
            size: (MAX_SHADOWS * std::mem::size_of::<ShadowInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            instance_buffer,
            viewport_buffer,
            bind_group,
            shadows: Vec::with_capacity(MAX_SHADOWS),
            capacity: MAX_SHADOWS,
        }
    }

    /// Queue an analytical drop shadow.
    ///
    /// The shadow is the Gaussian-blurred projection of the caster rect,
    /// offset by `(offset_x, offset_y)` and blurred with `sigma`.
    #[inline]
    #[expect(clippy::too_many_arguments)]
    pub fn push_shadow(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        border_radius: f32,
        sigma: f32,
        offset_x: f32,
        offset_y: f32,
        color: [f32; 4],
    ) {
        if self.shadows.len() >= self.capacity {
            return;
        }
        if !(w > 0.0 && h > 0.0 && sigma > 0.0 && x.is_finite() && y.is_finite()) {
            return;
        }
        // Expand quad by 3*sigma to capture 99.7% of Gaussian mass.
        let padding = 3.0 * sigma;
        let expanded_x = x + offset_x - padding;
        let expanded_y = y + offset_y - padding;
        let expanded_w = w + 2.0 * padding;
        let expanded_h = h + 2.0 * padding;

        // Caster rect position within expanded quad local space.
        let caster_x = padding;
        let caster_y = padding;

        self.shadows.push(ShadowInstance {
            x: expanded_x,
            y: expanded_y,
            w: expanded_w,
            h: expanded_h,
            color,
            caster_x,
            caster_y,
            caster_w: w,
            caster_h: h,
            sigma,
            border_radius,
            _pad: [0.0; 2],
        });
    }

    /// Upload queued shadow data to the GPU. Call before the render pass.
    #[inline]
    pub fn prepare(&self, queue: &wgpu::Queue) {
        if self.shadows.is_empty() {
            return;
        }
        queue.write_buffer(
            &self.instance_buffer,
            0,
            bytemuck::cast_slice(&self.shadows),
        );
    }

    /// Draw all queued shadows. Call inside render pass BEFORE quads.
    #[inline]
    pub fn render<'pass>(&'pass self, render_pass: &mut wgpu::RenderPass<'pass>) {
        let count = self.shadows.len() as u32;
        if count == 0 {
            return;
        }
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
        render_pass.draw(0..6, 0..count);
    }

    /// Update viewport dimensions after a window resize.
    #[inline]
    pub fn resize(&mut self, queue: &wgpu::Queue, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        let uniform = ViewportUniform {
            width: width as f32,
            height: height as f32,
            _padding: [0.0; 2],
        };
        queue.write_buffer(&self.viewport_buffer, 0, bytemuck::bytes_of(&uniform));
    }

    /// Clear all queued shadows. Call after frame submission.
    #[inline]
    pub fn clear(&mut self) {
        self.shadows.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shadow_instance_is_pod() {
        let zero = ShadowInstance::zeroed();
        assert_eq!(zero.x, 0.0);
        assert_eq!(zero.sigma, 0.0);
    }

    #[test]
    fn shadow_instance_size() {
        assert_eq!(std::mem::size_of::<ShadowInstance>(), 64);
    }

    #[test]
    fn pipeline_is_send_and_sync() {
        fn _assert_send<T: Send>() {}
        fn _assert_sync<T: Sync>() {}
        _assert_send::<ShadowPipeline>();
        _assert_sync::<ShadowPipeline>();
    }
}
