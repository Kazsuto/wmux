pub mod error;
pub mod gpu;
pub mod icons;
pub mod pane;
pub mod quad;
pub mod shadow;
pub mod svg_icons;
pub mod terminal;
pub mod text;

pub use error::RenderError;
pub use gpu::GpuContext;
pub use pane::{PaneRenderer, PaneViewport, SurfaceType};
pub use quad::QuadPipeline;
pub use shadow::ShadowPipeline;
pub use terminal::{TerminalMetrics, TerminalRenderer};
pub use text::GlyphonRenderer;

/// Fallback foreground text color before theme is loaded.
/// Matches the wmux-default theme foreground (#ffffff / cmux Apple System Colors).
/// After `set_palette()` is called, TerminalRenderer uses the theme's foreground.
pub(crate) const DEFAULT_TEXT_COLOR: glyphon::Color = glyphon::Color::rgb(0xff, 0xff, 0xff);
