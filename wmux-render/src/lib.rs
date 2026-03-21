pub mod error;
pub mod gpu;
pub mod pane;
pub mod quad;
pub mod terminal;
pub mod text;

pub use error::RenderError;
pub use gpu::GpuContext;
pub use pane::{PaneRenderer, PaneViewport};
pub use quad::QuadPipeline;
pub use terminal::{TerminalMetrics, TerminalRenderer};
pub use text::GlyphonRenderer;

/// Default foreground text color used by terminal grid and overlays.
pub const DEFAULT_TEXT_COLOR: glyphon::Color = glyphon::Color::rgb(204, 204, 204);
