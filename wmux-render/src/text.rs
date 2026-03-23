use crate::RenderError;
use glyphon::{Cache, FontSystem, Resolution, SwashCache, TextAtlas, TextRenderer, Viewport};
use wgpu::{Device, MultisampleState, Queue, TextureFormat};

/// The UI font family name used for non-terminal chrome (sidebar, tabs, status bar).
///
/// Resolves to "Segoe UI Variable" on Win11, falls back to "Segoe UI" on Win10,
/// then to system sans-serif via glyphon's font matching.
pub const UI_FONT_FAMILY: &str = "Segoe UI Variable";

/// Fallback UI font for Win10 where Segoe UI Variable is unavailable.
pub const UI_FONT_FAMILY_FALLBACK: &str = "Segoe UI";

/// Icon font family for UI chrome icons (close, add, split, globe, etc.).
///
/// Segoe Fluent Icons is pre-installed on Windows 11 — no need to embed.
/// `FontSystem::new()` loads system fonts automatically, so it's available
/// if installed. Use `has_icon_font()` to check at runtime.
pub const ICON_FONT_FAMILY: &str = "Segoe Fluent Icons";

pub struct GlyphonRenderer {
    font_system: FontSystem,
    swash_cache: SwashCache,
    /// Held alive for the atlas — not read directly.
    _cache: Cache,
    atlas: TextAtlas,
    renderer: TextRenderer,
    viewport: Viewport,
    /// Whether "Segoe UI Variable" (Win11) is available, else "Segoe UI" (Win10).
    ui_font_available: bool,
    /// Whether "Segoe Fluent Icons" (Win11) is available for UI icon rendering.
    icon_font_available: bool,
}

impl GlyphonRenderer {
    pub fn new(device: &Device, queue: &Queue, format: TextureFormat) -> Self {
        let font_system = FontSystem::new();

        // Probe whether Segoe UI Variable (Win11) or Segoe UI (Win10) is available.
        let ui_font_available = font_system.db().faces().any(|face| {
            face.families
                .iter()
                .any(|(name, _)| name == UI_FONT_FAMILY || name == UI_FONT_FAMILY_FALLBACK)
        });

        if ui_font_available {
            tracing::info!("UI sans-serif font loaded for chrome rendering");
        } else {
            tracing::warn!("Segoe UI not found — UI chrome will use system sans-serif fallback");
        }

        // Probe whether Segoe Fluent Icons (Win11) is available for icon rendering.
        let icon_font_available = font_system.db().faces().any(|face| {
            face.families
                .iter()
                .any(|(name, _)| name == ICON_FONT_FAMILY)
        });

        if icon_font_available {
            tracing::info!("Segoe Fluent Icons available for UI icon rendering");
        } else {
            tracing::warn!(
                "Segoe Fluent Icons not found — icon rendering will fall back to text glyphs"
            );
        }

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
            ui_font_available,
            icon_font_available,
        }
    }

    /// Whether Segoe UI Variable or Segoe UI is available for UI chrome.
    #[inline]
    pub fn has_ui_font(&self) -> bool {
        self.ui_font_available
    }

    /// Whether Segoe Fluent Icons is available for UI icon rendering.
    #[inline]
    pub fn has_icon_font(&self) -> bool {
        self.icon_font_available
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
    /// Uses `prepare_with_custom()` to support both regular text glyphs and
    /// SVG-based custom glyphs in the same draw call. Custom glyphs are
    /// rasterized on demand via [`crate::svg_icons::rasterize_svg_icon`]
    /// and cached in glyphon's LRU atlas.
    pub fn prepare_text_areas<'a>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        text_areas: impl IntoIterator<Item = glyphon::TextArea<'a>>,
    ) -> Result<(), crate::RenderError> {
        self.renderer.prepare_with_custom(
            device,
            queue,
            &mut self.font_system,
            &mut self.atlas,
            &self.viewport,
            text_areas,
            &mut self.swash_cache,
            crate::svg_icons::rasterize_svg_icon,
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

    /// Evict unused glyphs from the GPU atlas cache.
    /// Call once per frame after rendering to prevent memory leaks.
    #[inline]
    pub fn trim_atlas(&mut self) {
        self.atlas.trim();
    }
}
