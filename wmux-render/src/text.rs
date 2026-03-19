use crate::RenderError;
use glyphon::{
    Buffer, Cache, Color, FontSystem, Metrics, Resolution, SwashCache, TextArea, TextAtlas,
    TextBounds, TextRenderer, Viewport,
};
use wgpu::{Device, MultisampleState, Queue, TextureFormat};

pub struct GlyphonRenderer {
    pub font_system: FontSystem,
    pub swash_cache: SwashCache,
    pub cache: Cache,
    pub atlas: TextAtlas,
    pub renderer: TextRenderer,
    pub viewport: Viewport,
    pub buffer: Buffer,
    width: u32,
    height: u32,
}

impl GlyphonRenderer {
    pub fn new(
        device: &Device,
        queue: &Queue,
        format: TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let mut font_system = FontSystem::new();
        let swash_cache = SwashCache::new();
        let cache = Cache::new(device);
        let mut atlas = TextAtlas::new(device, queue, &cache, format);
        let renderer = TextRenderer::new(&mut atlas, device, MultisampleState::default(), None);
        let viewport = Viewport::new(device, &cache);

        let mut buffer = Buffer::new(&mut font_system, Metrics::new(16.0, 20.0));
        buffer.set_size(&mut font_system, Some(width as f32), Some(height as f32));
        buffer.set_text(
            &mut font_system,
            "wmux - Windows Terminal Multiplexer\n\nGPU rendering with wgpu + glyphon is working!",
            &glyphon::Attrs::new().family(glyphon::Family::Monospace),
            glyphon::Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(&mut font_system, false);

        GlyphonRenderer {
            font_system,
            swash_cache,
            cache,
            atlas,
            renderer,
            viewport,
            buffer,
            width,
            height,
        }
    }

    #[inline]
    pub fn resize(&mut self, queue: &Queue, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.width = width;
            self.height = height;
            self.viewport.update(queue, Resolution { width, height });
            self.buffer.set_size(
                &mut self.font_system,
                Some(width as f32),
                Some(height as f32),
            );
            self.buffer.shape_until_scroll(&mut self.font_system, false);
        }
    }

    #[inline]
    pub fn set_text(&mut self, text: &str) {
        self.buffer.set_text(
            &mut self.font_system,
            text,
            &glyphon::Attrs::new().family(glyphon::Family::Monospace),
            glyphon::Shaping::Advanced,
            None,
        );
        self.buffer.shape_until_scroll(&mut self.font_system, false);
    }

    #[inline]
    pub fn prepare(&mut self, device: &Device, queue: &Queue) -> Result<(), RenderError> {
        let text_area = TextArea {
            buffer: &self.buffer,
            left: 10.0,
            top: 10.0,
            scale: 1.0,
            bounds: TextBounds {
                left: 0,
                top: 0,
                right: self.width as i32,
                bottom: self.height as i32,
            },
            default_color: Color::rgb(204, 204, 204),
            custom_glyphs: &[],
        };

        self.renderer.prepare(
            device,
            queue,
            &mut self.font_system,
            &mut self.atlas,
            &self.viewport,
            [text_area],
            &mut self.swash_cache,
        )?;

        Ok(())
    }

    #[inline]
    pub fn render<'pass>(
        &'pass self,
        render_pass: &mut wgpu::RenderPass<'pass>,
    ) -> Result<(), RenderError> {
        self.renderer
            .render(&self.atlas, &self.viewport, render_pass)?;
        Ok(())
    }
}
