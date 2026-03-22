pub mod animation;
pub mod command_palette;
pub mod divider;
pub mod effects;
pub mod error;
pub mod event;
pub mod input;
pub mod mouse;
pub mod notification_panel;
pub mod search;
pub mod shortcuts;
pub mod sidebar;
pub mod status_bar;
pub mod toast;
pub mod window;

pub use error::UiError;
pub use event::WmuxEvent;
pub use input::InputHandler;
pub use mouse::{MouseAction, MouseHandler};
pub use shortcuts::{ShortcutAction, ShortcutMap};
pub use toast::ToastService;
pub use window::App;

/// Convert `[f32; 4]` RGBA (0.0–1.0) to `glyphon::Color` (0–255).
///
/// Shared helper used by sidebar, notification panel, and status bar.
pub(crate) fn f32_to_glyphon_color(c: [f32; 4]) -> glyphon::Color {
    glyphon::Color::rgba(
        (c[0] * 255.0) as u8,
        (c[1] * 255.0) as u8,
        (c[2] * 255.0) as u8,
        (c[3] * 255.0) as u8,
    )
}
