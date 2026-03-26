use glyphon::{TextArea, TextBounds};
use wmux_config::UiChrome;
use wmux_core::Notification;
use wmux_render::quad::QuadPipeline;

use crate::f32_to_glyphon_color;

/// Width of the notification panel overlay in pixels.
const PANEL_WIDTH: f32 = 360.0;

/// Height of each notification item in the panel.
const ITEM_HEIGHT: f32 = 82.0;

/// Width of the severity indicator stripe on the left edge.
const SEVERITY_STRIPE_WIDTH: f32 = 4.0;

/// Padding inside each notification item.
const ITEM_PADDING: f32 = 8.0;

/// State for the notification panel overlay.
#[derive(Debug)]
pub struct NotificationPanel {
    /// Whether the panel is currently visible.
    pub open: bool,
    /// Scroll offset in pixels (0 = top).
    pub scroll_offset: f32,
    /// Index of the hovered item (if any).
    pub hovered_item: Option<usize>,
}

impl Default for NotificationPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl NotificationPanel {
    /// Create a new hidden notification panel.
    #[must_use]
    pub fn new() -> Self {
        Self {
            open: false,
            scroll_offset: 0.0,
            hovered_item: None,
        }
    }

    /// Toggle panel visibility.
    pub fn toggle(&mut self) {
        self.open = !self.open;
        if self.open {
            self.scroll_offset = 0.0;
            self.hovered_item = None;
            tracing::debug!("notification panel opened");
        } else {
            tracing::debug!("notification panel closed");
        }
    }

    /// Scroll the panel by `delta` pixels (positive = scroll down).
    pub fn scroll(&mut self, delta: f32, total_items: usize, visible_height: f32) {
        let max_scroll = (total_items as f32 * ITEM_HEIGHT - visible_height).max(0.0);
        self.scroll_offset = (self.scroll_offset + delta).clamp(0.0, max_scroll);
    }

    /// Hit-test: given a click Y coordinate relative to the panel top,
    /// return the notification index that was clicked (if any).
    #[must_use]
    pub fn hit_test(&self, y: f32, total_items: usize) -> Option<usize> {
        if !self.open {
            return None;
        }
        let adjusted_y = y + self.scroll_offset;
        if adjusted_y < 0.0 {
            return None;
        }
        let index = (adjusted_y / ITEM_HEIGHT) as usize;
        if index < total_items {
            Some(index)
        } else {
            None
        }
    }

    /// Update hover state from mouse Y coordinate relative to panel top.
    pub fn update_hover(&mut self, y: f32, total_items: usize) {
        self.hovered_item = self.hit_test(y, total_items);
    }

    /// Render the notification panel background and item backgrounds.
    ///
    /// Call this during the quad accumulation phase (before `quads.prepare()`).
    /// Text rendering is handled separately via `text_areas()`.
    pub fn render_quads(
        &self,
        quads: &mut QuadPipeline,
        notifications: &[&Notification],
        surface_width: f32,
        surface_height: f32,
        ui_chrome: &UiChrome,
    ) {
        if !self.open {
            return;
        }

        let panel_x = (surface_width - PANEL_WIDTH).max(0.0);
        let effective_panel_w = PANEL_WIDTH.min(surface_width);
        // surface_overlay already has 95% alpha baked in.
        let panel_bg = ui_chrome.surface_overlay;

        // Panel background (rounded left corners)
        quads.push_rounded_quad(
            panel_x,
            0.0,
            effective_panel_w,
            surface_height,
            panel_bg,
            8.0,
        );

        // Empty state: show "All caught up" when no notifications
        if notifications.is_empty() {
            // Centered text will be rendered separately via text_areas()
            return;
        }

        // Render each visible notification item
        for (i, notif) in notifications.iter().enumerate() {
            let item_y = i as f32 * ITEM_HEIGHT - self.scroll_offset;

            // Skip items outside visible area
            if item_y + ITEM_HEIGHT < 0.0 || item_y > surface_height {
                continue;
            }

            // Severity-specific stripe color and background tint.
            let (stripe_color, tint_color) = match notif.severity {
                wmux_core::NotificationSeverity::Info => (ui_chrome.accent, ui_chrome.info_muted),
                wmux_core::NotificationSeverity::Warning => {
                    (ui_chrome.warning, ui_chrome.warning_muted)
                }
                wmux_core::NotificationSeverity::Error => (ui_chrome.error, ui_chrome.error_muted),
                wmux_core::NotificationSeverity::Success => {
                    (ui_chrome.success, ui_chrome.success_muted)
                }
            };

            // Left severity stripe
            quads.push_quad(
                panel_x,
                item_y,
                SEVERITY_STRIPE_WIDTH,
                ITEM_HEIGHT,
                stripe_color,
            );

            // Severity background tint (subtle muted background)
            quads.push_rounded_quad(
                panel_x + ITEM_PADDING,
                item_y + 2.0,
                PANEL_WIDTH - 2.0 * ITEM_PADDING,
                ITEM_HEIGHT - 4.0,
                tint_color,
                6.0,
            );

            // Hover highlight (rounded)
            if self.hovered_item == Some(i) {
                quads.push_rounded_quad(
                    panel_x + ITEM_PADDING,
                    item_y + 2.0,
                    PANEL_WIDTH - 2.0 * ITEM_PADDING,
                    ITEM_HEIGHT - 4.0,
                    ui_chrome.surface_1,
                    6.0,
                );
            }

            // Bottom separator
            quads.push_quad(
                panel_x + ITEM_PADDING,
                item_y + ITEM_HEIGHT - 1.0,
                PANEL_WIDTH - 2.0 * ITEM_PADDING,
                1.0,
                ui_chrome.border_default,
            );
        }
    }

    /// Return the panel width (for layout calculations).
    #[must_use]
    pub fn panel_width(&self) -> f32 {
        if self.open {
            PANEL_WIDTH
        } else {
            0.0
        }
    }

    /// Check if a screen X coordinate falls within the panel area.
    #[must_use]
    pub fn contains_x(&self, x: f32, surface_width: f32) -> bool {
        self.open && x >= (surface_width - PANEL_WIDTH).max(0.0)
    }

    /// Produce TextArea descriptors for the notification panel.
    ///
    /// When no notifications exist, returns a centered "All caught up" message.
    /// Otherwise returns empty (notification text rendered elsewhere).
    #[must_use]
    pub fn text_areas<'a>(
        &'a self,
        notifications: &[&Notification],
        surface_width: f32,
        surface_height: f32,
        ui_chrome: &UiChrome,
        buffer: &'a glyphon::Buffer,
    ) -> Vec<TextArea<'a>> {
        if !self.open || !notifications.is_empty() {
            return Vec::new();
        }

        let panel_x = surface_width - PANEL_WIDTH;
        let text_color = f32_to_glyphon_color(ui_chrome.text_muted);

        vec![TextArea {
            buffer,
            left: panel_x + PANEL_WIDTH / 2.0,
            top: surface_height / 2.0,
            scale: 1.0,
            bounds: TextBounds {
                left: panel_x as i32,
                top: 0,
                right: surface_width as i32,
                bottom: surface_height as i32,
            },
            default_color: text_color,
            custom_glyphs: &[],
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_panel_is_closed() {
        let panel = NotificationPanel::new();
        assert!(!panel.open);
        assert!(panel.scroll_offset.abs() < f32::EPSILON);
    }

    #[test]
    fn toggle_opens_and_closes() {
        let mut panel = NotificationPanel::new();
        panel.toggle();
        assert!(panel.open);
        panel.toggle();
        assert!(!panel.open);
    }

    #[test]
    fn scroll_clamps_to_bounds() {
        let mut panel = NotificationPanel::new();
        panel.open = true;
        panel.scroll(-100.0, 5, 400.0);
        assert!(panel.scroll_offset.abs() < f32::EPSILON);
    }

    #[test]
    fn hit_test_returns_index() {
        let mut panel = NotificationPanel::new();
        panel.open = true;
        // Item at y=0 should be index 0
        assert_eq!(panel.hit_test(10.0, 5), Some(0));
        // Item at y=ITEM_HEIGHT should be index 1
        assert_eq!(panel.hit_test(ITEM_HEIGHT + 5.0, 5), Some(1));
        // Beyond items
        assert_eq!(panel.hit_test(ITEM_HEIGHT * 10.0, 5), None);
    }

    #[test]
    fn hit_test_when_closed() {
        let panel = NotificationPanel::new();
        assert_eq!(panel.hit_test(10.0, 5), None);
    }

    #[test]
    fn hit_test_negative_y_returns_none() {
        let mut panel = NotificationPanel::new();
        panel.open = true;
        assert_eq!(panel.hit_test(-10.0, 5), None);
    }

    #[test]
    fn contains_x_when_open() {
        let mut panel = NotificationPanel::new();
        panel.open = true;
        assert!(panel.contains_x(1920.0 - 100.0, 1920.0));
        assert!(!panel.contains_x(100.0, 1920.0));
    }

    #[test]
    fn panel_width_depends_on_state() {
        let mut panel = NotificationPanel::new();
        assert!(panel.panel_width().abs() < f32::EPSILON);
        panel.open = true;
        assert!((panel.panel_width() - PANEL_WIDTH).abs() < f32::EPSILON);
    }
}
