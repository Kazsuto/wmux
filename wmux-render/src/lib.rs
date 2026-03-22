pub mod error;
pub mod gpu;
pub mod pane;
pub mod quad;
pub mod terminal;
pub mod text;

pub use error::RenderError;
pub use gpu::GpuContext;
pub use pane::{PaneRenderer, PaneViewport, SurfaceType};
pub use quad::QuadPipeline;
pub use terminal::{TerminalMetrics, TerminalRenderer};
pub use text::GlyphonRenderer;

/// Fallback foreground text color before theme is loaded.
/// Matches the wmux-default theme foreground (#e6edf3).
/// After `set_palette()` is called, TerminalRenderer uses the theme's foreground.
pub(crate) const DEFAULT_TEXT_COLOR: glyphon::Color = glyphon::Color::rgb(0xe6, 0xed, 0xf3);
