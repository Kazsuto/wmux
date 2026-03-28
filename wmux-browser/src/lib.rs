pub mod automation;
pub mod com;
pub mod error;
pub mod manager;
pub mod panel;

pub use automation::{NavigationState, WaitCondition};
pub use com::{recv_with_pump, ComGuard};
pub use error::BrowserError;
pub use manager::BrowserManager;
pub use panel::BrowserPanel;

// Re-export the core types needed by callers working with browser panels.
pub use wmux_core::rect::Rect;
pub use wmux_core::types::SurfaceId;
