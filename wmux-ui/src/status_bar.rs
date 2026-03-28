use glyphon::{Buffer, Family, Metrics, Shaping};
use wmux_config::UiChrome;

use crate::{animation::MOTION_PULSE, f32_to_glyphon_color, typography};

/// Height of the status bar in logical pixels.
pub const STATUS_BAR_HEIGHT: f32 = 34.0;

/// Status bar text — uses Caption token.
const FONT_SIZE: f32 = typography::CAPTION_FONT_SIZE;
const LINE_HEIGHT: f32 = typography::CAPTION_LINE_HEIGHT;
const CONNECTION_DOT_SIZE: f32 = 7.0;

/// Connection state for the status bar indicator dot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnectionStatus {
    #[default]
    Connected,
    Reconnecting,
    Disconnected,
}

/// Data required to render a single status bar frame.
#[derive(Debug, Clone, Default)]
pub struct StatusBarData {
    pub workspace_name: String,
    pub pane_count: usize,
    pub connection: ConnectionStatus,
    pub branch: Option<String>,
    pub shell: String,
}

/// Status bar component — 28px strip at the bottom of the window.
///
/// Displays workspace name, pane count, connection status, git branch, and shell.
pub struct StatusBar {
    text_buffer: Buffer,
    last_text: String,
    last_width: f32,
}

impl StatusBar {
    /// Create a new status bar with a pre-allocated text buffer.
    ///
    /// `width` must be in **logical pixels** so that `Align::Center` computes
    /// the correct center when `TextArea.scale` is applied.
    pub fn new(font_system: &mut glyphon::FontSystem, width: f32) -> Self {
        let mut buf = Buffer::new(font_system, Metrics::new(FONT_SIZE, LINE_HEIGHT));
        buf.set_size(font_system, Some(width), Some(STATUS_BAR_HEIGHT));
        Self {
            text_buffer: buf,
            last_text: String::new(),
            last_width: width,
        }
    }

    /// Update text content when data or width changes.
    ///
    /// `width` must be in **logical pixels**.
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
            "{} · {} panes{} · {}",
            data.workspace_name, data.pane_count, branch_part, data.shell,
        );

        let width_changed = (width - self.last_width).abs() > 0.5;

        if text != self.last_text || width_changed {
            self.text_buffer
                .set_size(font_system, Some(width), Some(STATUS_BAR_HEIGHT));
            self.text_buffer.set_text(
                font_system,
                &text,
                &glyphon::Attrs::new()
                    .family(Family::Name("Segoe UI"))
                    .weight(glyphon::Weight::NORMAL),
                Shaping::Advanced,
                Some(glyphon::cosmic_text::Align::Center),
            );
            self.text_buffer.shape_until_scroll(font_system, false);
            self.last_text = text;
            self.last_width = width;
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
        scale_factor: f32,
    ) {
        let height = STATUS_BAR_HEIGHT * scale_factor;
        let dot_size = CONNECTION_DOT_SIZE * scale_factor;

        // Background
        quads.push_quad(x, y, width, height, ui_chrome.surface_1);

        // Top border — subtle separator line
        quads.push_quad(x, y, width, 1.0 * scale_factor, ui_chrome.border_subtle);

        // Connection dot — positioned relative to centered text.
        let text_width = self
            .text_buffer
            .layout_runs()
            .next()
            .map_or(0.0, |run| run.line_w);
        let text_start_x = x + (width / scale_factor - text_width) / 2.0 * scale_factor;
        let dot_gap = 6.0 * scale_factor;
        let dot_x = text_start_x - dot_size - dot_gap;
        let dot_y = y + (height - dot_size) / 2.0;
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
        quads.push_rounded_quad(dot_x, dot_y, dot_size, dot_size, dot_color, dot_size / 2.0);
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
        let height = STATUS_BAR_HEIGHT * scale_factor;

        glyphon::TextArea {
            buffer: &self.text_buffer,
            left: x,
            top: y + (height - LINE_HEIGHT * scale_factor) / 2.0,
            scale: scale_factor,
            bounds: glyphon::TextBounds {
                left: x as i32,
                top: y as i32,
                right: (x + width) as i32,
                bottom: (y + height) as i32,
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
