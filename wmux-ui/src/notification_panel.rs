use wmux_core::Notification;
use wmux_render::quad::QuadPipeline;

/// Width of the notification panel overlay in pixels.
const PANEL_WIDTH: f32 = 350.0;

/// Height of each notification item in the panel.
const ITEM_HEIGHT: f32 = 72.0;

/// Padding inside each notification item.
const ITEM_PADDING: f32 = 8.0;

/// Background color for the notification panel.
const PANEL_BG: [f32; 4] = [0.08, 0.08, 0.12, 0.95];

/// Background color for individual notification items on hover.
const ITEM_HOVER_BG: [f32; 4] = [0.15, 0.15, 0.2, 0.9];

/// Border color for the panel left edge.
const PANEL_BORDER: [f32; 4] = [0.3, 0.5, 1.0, 0.4];

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
    ) {
        if !self.open {
            return;
        }

        let panel_x = surface_width - PANEL_WIDTH;

        // Panel background
        quads.push_quad(panel_x, 0.0, PANEL_WIDTH, surface_height, PANEL_BG);

        // Left border accent
        quads.push_quad(panel_x, 0.0, 2.0, surface_height, PANEL_BORDER);

        // Render each visible notification item
        for (i, _notif) in notifications.iter().enumerate() {
            let item_y = i as f32 * ITEM_HEIGHT - self.scroll_offset;

            // Skip items outside visible area
            if item_y + ITEM_HEIGHT < 0.0 || item_y > surface_height {
                continue;
            }

            // Hover highlight
            if self.hovered_item == Some(i) {
                quads.push_quad(
                    panel_x + 2.0,
                    item_y,
                    PANEL_WIDTH - 2.0,
                    ITEM_HEIGHT,
                    ITEM_HOVER_BG,
                );
            }

            // Bottom separator
            quads.push_quad(
                panel_x + ITEM_PADDING,
                item_y + ITEM_HEIGHT - 1.0,
                PANEL_WIDTH - 2.0 * ITEM_PADDING,
                1.0,
                [1.0, 1.0, 1.0, 0.05],
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
        self.open && x >= (surface_width - PANEL_WIDTH)
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
