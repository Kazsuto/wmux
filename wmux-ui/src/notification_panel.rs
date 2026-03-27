use glyphon::{TextArea, TextBounds};
use std::time::SystemTime;
use wmux_config::{Locale, UiChrome};
use wmux_core::Notification;
use wmux_render::quad::QuadPipeline;

use crate::f32_to_glyphon_color;

/// Width of the notification panel overlay in pixels.
pub(crate) const PANEL_WIDTH: f32 = 360.0;

/// Height of each notification item in the panel.
const ITEM_HEIGHT: f32 = 82.0;

/// Width of the severity indicator stripe on the left edge.
const SEVERITY_STRIPE_WIDTH: f32 = 4.0;

/// Padding inside each notification item.
const ITEM_PADDING: f32 = 8.0;

/// Height of the header area (title + clear all).
const HEADER_HEIGHT: f32 = 52.0;

/// Horizontal padding inside the header.
const HEADER_PAD_X: f32 = 16.0;

/// Size of the severity indicator circle.
const ICON_SIZE: f32 = 24.0;

/// Maximum notification items to pre-allocate buffers for.
pub const MAX_VISIBLE_ITEMS: usize = 12;

/// X offset where text content starts (after stripe + padding + icon + gap).
pub(crate) const TEXT_LEFT_OFFSET: f32 = SEVERITY_STRIPE_WIDTH + ITEM_PADDING + ICON_SIZE + 8.0;

/// Buffers for rendering notification panel text.
pub struct NotificationBuffers<'a> {
    pub header: &'a glyphon::Buffer,
    pub clear_all: &'a glyphon::Buffer,
    pub empty: &'a glyphon::Buffer,
    pub categories: &'a [glyphon::Buffer],
    pub titles: &'a [glyphon::Buffer],
    pub bodies: &'a [glyphon::Buffer],
    pub timestamps: &'a [glyphon::Buffer],
}

/// Header click target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderAction {
    Close,
    ClearAll,
}

/// Format a timestamp as a relative "time ago" string using locale keys.
///
/// Uses `notification.time_just_now`, `notification.time_minutes_ago`,
/// `notification.time_hours_ago`, `notification.time_days_ago` from the
/// locale files. Templates use `{n}` as the number placeholder.
pub fn format_time_ago(timestamp: SystemTime, locale: &Locale) -> String {
    let elapsed = SystemTime::now()
        .duration_since(timestamp)
        .unwrap_or_default();
    let secs = elapsed.as_secs();
    if secs < 60 {
        return locale.t("notification.time_just_now").to_string();
    }
    let mins = secs / 60;
    if mins < 60 {
        return locale
            .t("notification.time_minutes_ago")
            .replace("{n}", &mins.to_string());
    }
    let hours = mins / 60;
    if hours < 24 {
        return locale
            .t("notification.time_hours_ago")
            .replace("{n}", &hours.to_string());
    }
    let days = hours / 24;
    locale
        .t("notification.time_days_ago")
        .replace("{n}", &days.to_string())
}

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
        let content_height = total_items as f32 * ITEM_HEIGHT;
        let scrollable = (visible_height - HEADER_HEIGHT).max(0.0);
        let max_scroll = (content_height - scrollable).max(0.0);
        self.scroll_offset = (self.scroll_offset + delta).clamp(0.0, max_scroll);
    }

    /// Hit-test: given a click Y coordinate (absolute screen Y),
    /// return the notification index that was clicked (if any).
    #[must_use]
    pub fn hit_test(&self, y: f32, total_items: usize) -> Option<usize> {
        if !self.open || y < HEADER_HEIGHT {
            return None;
        }
        let adjusted_y = y - HEADER_HEIGHT + self.scroll_offset;
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

    /// Update hover state from mouse Y coordinate.
    pub fn update_hover(&mut self, y: f32, total_items: usize) {
        self.hovered_item = self.hit_test(y, total_items);
    }

    /// Hit-test the header area for close button or "clear all" link.
    #[must_use]
    pub fn hit_test_header(&self, x: f32, y: f32, surface_width: f32) -> Option<HeaderAction> {
        if !self.open || y >= HEADER_HEIGHT {
            return None;
        }
        let panel_x = (surface_width - PANEL_WIDTH).max(0.0);
        let rel_x = x - panel_x;

        // Close button (X) — top-right corner, 32x32 region
        if rel_x >= PANEL_WIDTH - 40.0 && y < 40.0 {
            return Some(HeaderAction::Close);
        }
        // "Clear all" — right side of header, below close
        if rel_x >= PANEL_WIDTH - 100.0 && (28.0..HEADER_HEIGHT).contains(&y) {
            return Some(HeaderAction::ClearAll);
        }
        None
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
        // Force opaque background — surface_overlay has 95% alpha which lets terminal bleed through.
        let bg = ui_chrome.surface_overlay;
        let panel_bg = [bg[0], bg[1], bg[2], 1.0];

        // Panel background (rounded left corners)
        quads.push_rounded_quad(
            panel_x,
            0.0,
            effective_panel_w,
            surface_height,
            panel_bg,
            8.0,
        );

        // Header separator line
        quads.push_quad(
            panel_x + HEADER_PAD_X,
            HEADER_HEIGHT - 1.0,
            PANEL_WIDTH - 2.0 * HEADER_PAD_X,
            1.0,
            ui_chrome.border_subtle,
        );

        // Empty state
        if notifications.is_empty() {
            return;
        }

        // Render each visible notification item
        for (i, notif) in notifications.iter().enumerate() {
            let item_y = HEADER_HEIGHT + i as f32 * ITEM_HEIGHT - self.scroll_offset;

            // Skip items outside visible area
            if item_y + ITEM_HEIGHT < HEADER_HEIGHT || item_y > surface_height {
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

            // Severity circle icon (left side of item)
            let icon_x = panel_x + SEVERITY_STRIPE_WIDTH + ITEM_PADDING;
            let icon_y = item_y + (ITEM_HEIGHT - ICON_SIZE) / 2.0;
            quads.push_rounded_quad(
                icon_x,
                icon_y,
                ICON_SIZE,
                ICON_SIZE,
                stripe_color,
                ICON_SIZE / 2.0,
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
    /// Returns text areas for the header, notification items, or an empty-state
    /// message depending on the current state.
    #[must_use]
    pub fn text_areas<'a>(
        &'a self,
        notifications: &[&Notification],
        surface_width: f32,
        surface_height: f32,
        scale_factor: f32,
        ui_chrome: &UiChrome,
        buffers: &'a NotificationBuffers<'a>,
    ) -> Vec<TextArea<'a>> {
        if !self.open {
            return Vec::new();
        }

        let panel_x = (surface_width - PANEL_WIDTH).max(0.0);
        let panel_right = panel_x + PANEL_WIDTH;
        let panel_bounds = TextBounds {
            left: panel_x as i32,
            top: 0,
            right: panel_right as i32,
            bottom: surface_height as i32,
        };

        let mut areas = Vec::with_capacity(4 + notifications.len() * 4);

        // Header title: "Notifications"
        areas.push(TextArea {
            buffer: buffers.header,
            left: panel_x + HEADER_PAD_X,
            top: 14.0,
            scale: scale_factor,
            bounds: panel_bounds,
            default_color: f32_to_glyphon_color(ui_chrome.text_primary),
            custom_glyphs: &[],
        });

        // "Clear all" text — right-aligned in header
        areas.push(TextArea {
            buffer: buffers.clear_all,
            left: panel_right - 80.0,
            top: 32.0,
            scale: scale_factor,
            bounds: panel_bounds,
            default_color: f32_to_glyphon_color(ui_chrome.text_muted),
            custom_glyphs: &[],
        });

        // Empty state
        if notifications.is_empty() {
            areas.push(TextArea {
                buffer: buffers.empty,
                left: panel_x + PANEL_WIDTH / 2.0 - 60.0,
                top: surface_height / 2.0,
                scale: scale_factor,
                bounds: panel_bounds,
                default_color: f32_to_glyphon_color(ui_chrome.text_muted),
                custom_glyphs: &[],
            });
            return areas;
        }

        // Item clipping bounds (below header)
        let item_bounds = TextBounds {
            left: panel_x as i32,
            top: HEADER_HEIGHT as i32,
            right: panel_right as i32,
            bottom: surface_height as i32,
        };

        // Per-item text areas
        for (i, notif) in notifications.iter().enumerate() {
            if i >= buffers.categories.len() {
                break;
            }

            let item_y = HEADER_HEIGHT + i as f32 * ITEM_HEIGHT - self.scroll_offset;

            // Skip items fully outside visible area
            if item_y + ITEM_HEIGHT < HEADER_HEIGHT || item_y > surface_height {
                continue;
            }

            let text_x = panel_x + TEXT_LEFT_OFFSET;
            let severity_color = match notif.severity {
                wmux_core::NotificationSeverity::Info => ui_chrome.accent,
                wmux_core::NotificationSeverity::Warning => ui_chrome.warning,
                wmux_core::NotificationSeverity::Error => ui_chrome.error,
                wmux_core::NotificationSeverity::Success => ui_chrome.success,
            };

            // Category label (colored, e.g. "Success", "Warning")
            areas.push(TextArea {
                buffer: &buffers.categories[i],
                left: text_x,
                top: item_y + 8.0,
                scale: scale_factor,
                bounds: item_bounds,
                default_color: f32_to_glyphon_color(severity_color),
                custom_glyphs: &[],
            });

            // Timestamp (right-aligned on category row)
            if i < buffers.timestamps.len() {
                areas.push(TextArea {
                    buffer: &buffers.timestamps[i],
                    left: panel_right - ITEM_PADDING - 70.0,
                    top: item_y + 8.0,
                    scale: scale_factor,
                    bounds: item_bounds,
                    default_color: f32_to_glyphon_color(ui_chrome.text_faint),
                    custom_glyphs: &[],
                });
            }

            // Title (bold)
            if i < buffers.titles.len() {
                areas.push(TextArea {
                    buffer: &buffers.titles[i],
                    left: text_x,
                    top: item_y + 28.0,
                    scale: scale_factor,
                    bounds: item_bounds,
                    default_color: f32_to_glyphon_color(ui_chrome.text_primary),
                    custom_glyphs: &[],
                });
            }

            // Body (secondary)
            if i < buffers.bodies.len() {
                areas.push(TextArea {
                    buffer: &buffers.bodies[i],
                    left: text_x,
                    top: item_y + 50.0,
                    scale: scale_factor,
                    bounds: item_bounds,
                    default_color: f32_to_glyphon_color(ui_chrome.text_secondary),
                    custom_glyphs: &[],
                });
            }
        }

        areas
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
        // Items start at HEADER_HEIGHT — first item at y=HEADER_HEIGHT+5 → index 0
        assert_eq!(panel.hit_test(HEADER_HEIGHT + 5.0, 5), Some(0));
        // Second item
        assert_eq!(
            panel.hit_test(HEADER_HEIGHT + ITEM_HEIGHT + 5.0, 5),
            Some(1)
        );
        // Beyond items
        assert_eq!(panel.hit_test(HEADER_HEIGHT + ITEM_HEIGHT * 10.0, 5), None);
    }

    #[test]
    fn hit_test_when_closed() {
        let panel = NotificationPanel::new();
        assert_eq!(panel.hit_test(HEADER_HEIGHT + 10.0, 5), None);
    }

    #[test]
    fn hit_test_in_header_returns_none() {
        let mut panel = NotificationPanel::new();
        panel.open = true;
        assert_eq!(panel.hit_test(10.0, 5), None);
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

    #[test]
    fn format_time_ago_works() {
        let locale = Locale::new("en");
        let now = SystemTime::now();
        assert_eq!(format_time_ago(now, &locale), "just now");
    }
}
