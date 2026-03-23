use glyphon::{Buffer, Family, Metrics, Shaping};
use wmux_config::UiChrome;

use crate::animation::MOTION_PULSE;
use crate::f32_to_glyphon_color;
use crate::typography;

/// Height of the status bar in logical pixels.
pub const STATUS_BAR_HEIGHT: f32 = 34.0;

/// Status bar text — uses Caption token.
const FONT_SIZE: f32 = typography::CAPTION_FONT_SIZE;
const LINE_HEIGHT: f32 = typography::CAPTION_LINE_HEIGHT;
const PADDING_X: f32 = 14.0;
const CONNECTION_DOT_SIZE: f32 = 7.0;

/// Connection state for the status bar indicator dot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Connected,
    Reconnecting,
    Disconnected,
}

/// Data required to render a single status bar frame.
#[derive(Debug, Clone)]
pub struct StatusBarData {
    pub workspace_name: String,
    pub pane_count: usize,
    pub connection: ConnectionStatus,
    pub branch: Option<String>,
    pub shell: String,
}

impl Default for StatusBarData {
    fn default() -> Self {
        Self {
            workspace_name: String::new(),
            pane_count: 0,
            connection: ConnectionStatus::Connected,
            branch: None,
            shell: String::new(),
        }
    }
}

/// Status bar component — 28px strip at the bottom of the window.
///
/// Displays workspace name, pane count, connection status, git branch, and shell.
pub struct StatusBar {
    text_buffer: Buffer,
    last_text: String,
}

impl StatusBar {
    /// Create a new status bar with a pre-allocated text buffer.
    pub fn new(font_system: &mut glyphon::FontSystem, width: f32) -> Self {
        let mut buf = Buffer::new(font_system, Metrics::new(FONT_SIZE, LINE_HEIGHT));
        buf.set_size(font_system, Some(width), Some(STATUS_BAR_HEIGHT));
        Self {
            text_buffer: buf,
            last_text: String::new(),
        }
    }

    /// Update text content when data changes.
    pub fn update_text(
        &mut self,
        font_system: &mut glyphon::FontSystem,
        data: &StatusBarData,
        width: f32,
    ) {
        let branch_part = data
            .branch
            .as_deref()
            .map(|b| format!(" · {b}"))
            .unwrap_or_default();
        let text = format!(
            "  {}  ·  {} panes{}  ·  {}",
            data.workspace_name, data.pane_count, branch_part, data.shell,
        );

        if text != self.last_text {
            self.text_buffer
                .set_size(font_system, Some(width), Some(STATUS_BAR_HEIGHT));
            self.text_buffer.set_text(
                font_system,
                &text,
                &glyphon::Attrs::new()
                    .family(Family::Name("Segoe UI"))
                    .weight(glyphon::Weight::NORMAL),
                Shaping::Advanced,
                None,
            );
            self.last_text = text;
        }
    }

    /// Resize the text buffer when the window width changes.
    pub fn resize(&mut self, font_system: &mut glyphon::FontSystem, width: f32) {
        self.text_buffer
            .set_size(font_system, Some(width), Some(STATUS_BAR_HEIGHT));
    }

    /// Push status bar background quads into the pipeline.
    #[expect(
        clippy::too_many_arguments,
        reason = "render method needs quad pipeline, chrome, position, size, time, and data"
    )]
    pub fn render_quads(
        &self,
        quads: &mut wmux_render::QuadPipeline,
        ui_chrome: &UiChrome,
        x: f32,
        y: f32,
        width: f32,
        time_secs: f32,
        data: &StatusBarData,
    ) {
        // Background
        quads.push_quad(x, y, width, STATUS_BAR_HEIGHT, ui_chrome.surface_1);

        // Connection dot
        let dot_x = x + PADDING_X;
        let dot_y = y + (STATUS_BAR_HEIGHT - CONNECTION_DOT_SIZE) / 2.0;
        let dot_color = match data.connection {
            ConnectionStatus::Connected => ui_chrome.success,
            ConnectionStatus::Reconnecting => {
                // Pulse animation
                let alpha = 0.5
                    + 0.5 * (time_secs * std::f32::consts::PI / MOTION_PULSE.as_secs_f32()).sin();
                let c = ui_chrome.warning;
                [c[0], c[1], c[2], alpha]
            }
            ConnectionStatus::Disconnected => ui_chrome.error,
        };
        quads.push_rounded_quad(
            dot_x,
            dot_y,
            CONNECTION_DOT_SIZE,
            CONNECTION_DOT_SIZE,
            dot_color,
            CONNECTION_DOT_SIZE / 2.0,
        );
    }

    /// Return a text area descriptor for glyphon rendering.
    pub fn text_area(
        &self,
        x: f32,
        y: f32,
        width: f32,
        ui_chrome: &UiChrome,
        scale_factor: f32,
    ) -> glyphon::TextArea<'_> {
        // Offset text past the connection dot
        let text_x = x + PADDING_X + CONNECTION_DOT_SIZE + 8.0;

        glyphon::TextArea {
            buffer: &self.text_buffer,
            left: text_x,
            top: y + (STATUS_BAR_HEIGHT - LINE_HEIGHT) / 2.0,
            scale: scale_factor,
            bounds: glyphon::TextBounds {
                left: text_x as i32,
                top: y as i32,
                right: (x + width) as i32,
                bottom: (y + STATUS_BAR_HEIGHT) as i32,
            },
            default_color: f32_to_glyphon_color(ui_chrome.text_secondary),
            custom_glyphs: &[],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_bar_height_is_34() {
        assert_eq!(STATUS_BAR_HEIGHT, 34.0);
    }

    #[test]
    fn status_bar_data_default() {
        let data = StatusBarData::default();
        assert!(data.workspace_name.is_empty());
        assert_eq!(data.pane_count, 0);
        assert_eq!(data.connection, ConnectionStatus::Connected);
        assert!(data.branch.is_none());
    }

    #[test]
    fn connection_status_eq() {
        assert_eq!(ConnectionStatus::Connected, ConnectionStatus::Connected);
        assert_ne!(ConnectionStatus::Connected, ConnectionStatus::Disconnected);
    }
}
