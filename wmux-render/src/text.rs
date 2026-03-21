use crate::RenderError;
use glyphon::{Cache, FontSystem, Resolution, SwashCache, TextAtlas, TextRenderer, Viewport};
use wgpu::{Device, MultisampleState, Queue, TextureFormat};

pub struct GlyphonRenderer {
    font_system: FontSystem,
    swash_cache: SwashCache,
    /// Held alive for the atlas — not read directly.
    _cache: Cache,
    atlas: TextAtlas,
    renderer: TextRenderer,
    viewport: Viewport,
}

impl GlyphonRenderer {
    pub fn new(device: &Device, queue: &Queue, format: TextureFormat) -> Self {
        let font_system = FontSystem::new();
        let swash_cache = SwashCache::new();
        let cache = Cache::new(device);
        let mut atlas = TextAtlas::new(device, queue, &cache, format);
        let renderer = TextRenderer::new(&mut atlas, device, MultisampleState::default(), None);
        let viewport = Viewport::new(device, &cache);

        GlyphonRenderer {
            font_system,
            swash_cache,
            _cache: cache,
            atlas,
            renderer,
            viewport,
        }
    }

    /// Update the viewport resolution after a surface resize.
    #[inline]
    pub fn resize(&mut self, queue: &Queue, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.viewport.update(queue, Resolution { width, height });
        }
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
