use crate::RenderError;
use glyphon::{
    Buffer, Cache, Color, FontSystem, Metrics, Resolution, SwashCache, TextArea, TextAtlas,
    TextBounds, TextRenderer, Viewport,
};
use wgpu::{Device, MultisampleState, Queue, TextureFormat};

pub struct GlyphonRenderer {
    font_system: FontSystem,
    swash_cache: SwashCache,
    /// Held alive for the atlas — not read directly.
    _cache: Cache,
    atlas: TextAtlas,
    renderer: TextRenderer,
    viewport: Viewport,
    buffer: Buffer,
    width: u32,
    height: u32,
}

/// Default monospace text attributes for terminal rendering.
#[inline]
fn default_attrs() -> glyphon::Attrs<'static> {
    glyphon::Attrs::new().family(glyphon::Family::Monospace)
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
            &default_attrs(),
            glyphon::Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(&mut font_system, false);

        GlyphonRenderer {
            font_system,
            swash_cache,
            _cache: cache,
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
            &default_attrs(),
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

    /// Prepare an arbitrary set of text areas using this renderer's shared GPU resources.
    ///
    /// Used by `TerminalRenderer` to prepare per-row glyphon buffers without exposing
    /// individual fields (which would require splitting the borrow).
    pub fn prepare_text_areas<'a>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        text_areas: impl IntoIterator<Item = glyphon::TextArea<'a>>,
    ) -> Result<(), crate::RenderError> {
        self.renderer.prepare(
            device,
            queue,
            &mut self.font_system,
            &mut self.atlas,
            &self.viewport,
            text_areas,
            &mut self.swash_cache,
        )?;
        Ok(())
    }

    pub fn font_system(&mut self) -> &mut glyphon::FontSystem {
        &mut self.font_system
    }

    pub fn swash_cache(&mut self) -> &mut glyphon::SwashCache {
        &mut self.swash_cache
    }

    pub fn atlas(&mut self) -> &mut glyphon::TextAtlas {
        &mut self.atlas
    }

    pub fn text_renderer(&mut self) -> &mut glyphon::TextRenderer {
        &mut self.renderer
    }

    pub fn viewport(&mut self) -> &mut glyphon::Viewport {
        &mut self.viewport
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
