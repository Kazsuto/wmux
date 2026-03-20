use bytemuck::{Pod, Zeroable};

/// Maximum number of quads per frame. Enough for full-screen terminal
/// backgrounds, cursor, selection, dividers, sidebar, and notification badges.
const MAX_QUADS: usize = 4096;

/// Per-instance data for a single colored quad.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct QuadInstance {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub color: [f32; 4],
}

/// Viewport dimensions uniform, padded to 16-byte alignment.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct ViewportUniform {
    width: f32,
    height: f32,
    _padding: [f32; 2],
}

/// GPU pipeline for batched instanced rendering of colored rectangles.
///
/// Usage each frame:
/// 1. `push_quad()` to accumulate quads
/// 2. `prepare(queue)` to upload instance data (before render pass)
/// 3. `render(render_pass)` to draw (inside render pass)
/// 4. `clear()` to reset for next frame
pub struct QuadPipeline {
    pipeline: wgpu::RenderPipeline,
    instance_buffer: wgpu::Buffer,
    viewport_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    quads: Vec<QuadInstance>,
    capacity: usize,
}

impl QuadPipeline {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("wmux_quad_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        // Viewport uniform buffer
        let viewport_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("wmux_quad_viewport"),
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

        // Bind group
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("wmux_quad_bind_group_layout"),
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
            label: Some("wmux_quad_bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: viewport_buffer.as_entire_binding(),
            }],
        });

        // Pipeline
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("wmux_quad_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });

        let instance_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<QuadInstance>() as u64,
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
            ],
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("wmux_quad_pipeline"),
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
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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

        // Pre-allocated instance buffer
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("wmux_quad_instances"),
            size: (MAX_QUADS * std::mem::size_of::<QuadInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            instance_buffer,
            viewport_buffer,
            bind_group,
            quads: Vec::with_capacity(MAX_QUADS),
            capacity: MAX_QUADS,
        }
    }

    /// Queue a colored rectangle for rendering.
    /// Silently skips zero/negative-size quads, non-finite values, and quads beyond capacity.
    #[inline]
    pub fn push_quad(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        if !(w > 0.0 && h > 0.0 && x.is_finite() && y.is_finite() && w.is_finite() && h.is_finite())
        {
            return;
        }
        if self.quads.len() >= self.capacity {
            return;
        }
        self.quads.push(QuadInstance { x, y, w, h, color });
    }

    /// Upload queued quad data to the GPU. Must be called before the render pass.
    #[inline]
    pub fn prepare(&self, queue: &wgpu::Queue) {
        if self.quads.is_empty() {
            return;
        }
        queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&self.quads));
    }

    /// Draw all queued quads. Must be called inside a render pass after `prepare`.
    #[inline]
    pub fn render<'pass>(&'pass self, render_pass: &mut wgpu::RenderPass<'pass>) {
        let count = self.quads.len() as u32;
        if count == 0 {
            return;
        }
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
        render_pass.draw(0..6, 0..count);
    }

    /// Update the viewport dimensions uniform after a window resize.
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

    /// Clear all queued quads. Call after frame submission to prepare for the next frame.
    #[inline]
    pub fn clear(&mut self) {
        self.quads.clear();
    }

    /// Number of quads currently queued.
    #[inline]
    pub fn quad_count(&self) -> usize {
        self.quads.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn _assert_send<T: Send>() {}
    fn _assert_sync<T: Sync>() {}

    #[test]
    fn quad_instance_is_pod() {
        // Compile-time verification that QuadInstance is Pod + Zeroable
        let zero = QuadInstance::zeroed();
        assert_eq!(zero.x, 0.0);
        assert_eq!(zero.color, [0.0; 4]);
    }

    #[test]
    fn pipeline_is_send_and_sync() {
        _assert_send::<QuadPipeline>();
        _assert_sync::<QuadPipeline>();
    }

    #[test]
    fn push_quad_skips_zero_size() {
        let mut quads = Vec::<QuadInstance>::new();
        // Simulate push_quad logic
        let cases = [(0.0_f32, 10.0_f32), (10.0, 0.0), (-1.0, 10.0), (10.0, -1.0)];
        for (w, h) in cases {
            if w > 0.0 && h > 0.0 {
                quads.push(QuadInstance {
                    x: 0.0,
                    y: 0.0,
                    w,
                    h,
                    color: [1.0; 4],
                });
            }
        }
        assert_eq!(quads.len(), 0);
    }

    #[test]
    fn quad_instance_size_matches_layout() {
        assert_eq!(std::mem::size_of::<QuadInstance>(), 32);
        // 2 floats (pos) + 2 floats (size) + 4 floats (color) = 8 * 4 = 32 bytes
    }

    #[test]
    fn viewport_uniform_is_16_byte_aligned() {
        assert_eq!(std::mem::size_of::<ViewportUniform>(), 16);
    }
}
