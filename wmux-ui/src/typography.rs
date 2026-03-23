//! Typography tokens for wmux UI chrome.
//!
//! Defines a consistent type scale used across sidebar, tab bar, status bar,
//! command palette, and overlays. Terminal monospace sizing is separate
//! (in `wmux-render::terminal`).

/// Title — sidebar section headers, overlay titles.
pub const TITLE_FONT_SIZE: f32 = 18.0;
pub const TITLE_LINE_HEIGHT: f32 = 24.0;

/// Body — tab titles, sidebar workspace names, labels.
pub const BODY_FONT_SIZE: f32 = 15.0;
pub const BODY_LINE_HEIGHT: f32 = 20.0;

/// Caption — status bar, sidebar subtitles, search bar, secondary text.
pub const CAPTION_FONT_SIZE: f32 = 13.0;
pub const CAPTION_LINE_HEIGHT: f32 = 18.0;

/// Badge — notification counts, small indicators.
pub const BADGE_FONT_SIZE: f32 = 11.0;
pub const BADGE_LINE_HEIGHT: f32 = 14.0;
