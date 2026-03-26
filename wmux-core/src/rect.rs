use serde::{Deserialize, Serialize};

/// Minimum pane width in pixels.
pub const MIN_PANE_WIDTH: f32 = 80.0;
/// Minimum pane height in pixels.
pub const MIN_PANE_HEIGHT: f32 = 40.0;
/// Width of the divider between split panes in pixels.
/// 8px gives the focus glow halo room to show between adjacent panes.
pub const DIVIDER_WIDTH: f32 = 8.0;

/// Axis-aligned rectangle used for pane layout calculations.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    /// Create a new rectangle with the given position and size.
    #[must_use]
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Check whether a point is inside this rectangle (inclusive).
    #[inline]
    #[must_use]
    pub fn contains_point(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.width && py >= self.y && py <= self.y + self.height
    }

    /// Split this rectangle horizontally (left/right) at the given ratio.
    ///
    /// The `gap` is the pixel width of the divider placed between the two halves.
    /// The first rectangle gets the left portion, the second gets the right.
    #[inline]
    #[must_use]
    pub fn split_horizontal(&self, ratio: f32, gap: f32) -> (Rect, Rect) {
        let first_width = (self.width * ratio - gap / 2.0).max(0.0);
        let second_width = (self.width * (1.0 - ratio) - gap / 2.0).max(0.0);
        let first = Rect {
            x: self.x,
            y: self.y,
            width: first_width,
            height: self.height,
        };
        let second = Rect {
            x: self.x + first_width + gap,
            y: self.y,
            width: second_width,
            height: self.height,
        };
        (first, second)
    }

    /// Split this rectangle vertically (top/bottom) at the given ratio.
    ///
    /// The `gap` is the pixel height of the divider placed between the two halves.
    /// The first rectangle gets the top portion, the second gets the bottom.
    #[inline]
    #[must_use]
    pub fn split_vertical(&self, ratio: f32, gap: f32) -> (Rect, Rect) {
        let first_height = (self.height * ratio - gap / 2.0).max(0.0);
        let second_height = (self.height * (1.0 - ratio) - gap / 2.0).max(0.0);
        let first = Rect {
            x: self.x,
            y: self.y,
            width: self.width,
            height: first_height,
        };
        let second = Rect {
            x: self.x,
            y: self.y + first_height + gap,
            width: self.width,
            height: second_height,
        };
        (first, second)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construction_and_field_access() {
        let r = Rect::new(10.0, 20.0, 300.0, 200.0);
        assert_eq!(r.x, 10.0);
        assert_eq!(r.y, 20.0);
        assert_eq!(r.width, 300.0);
        assert_eq!(r.height, 200.0);
    }

    #[test]
    fn split_horizontal_half() {
        let r = Rect::new(0.0, 0.0, 100.0, 50.0);
        let (left, right) = r.split_horizontal(0.5, 4.0);

        assert_eq!(left.x, 0.0);
        assert_eq!(left.width, 48.0); // 100*0.5 - 4/2
        assert_eq!(left.height, 50.0);

        assert_eq!(right.x, 52.0); // 48 + 4
        assert_eq!(right.width, 48.0); // 100*0.5 - 4/2
        assert_eq!(right.height, 50.0);
    }

    #[test]
    fn split_vertical_half() {
        let r = Rect::new(0.0, 0.0, 100.0, 100.0);
        let (top, bottom) = r.split_vertical(0.5, 4.0);

        assert_eq!(top.y, 0.0);
        assert_eq!(top.height, 48.0); // 100*0.5 - 4/2
        assert_eq!(top.width, 100.0);

        assert_eq!(bottom.y, 52.0); // 48 + 4
        assert_eq!(bottom.height, 48.0);
        assert_eq!(bottom.width, 100.0);
    }

    #[test]
    fn split_with_custom_ratio() {
        let r = Rect::new(0.0, 0.0, 100.0, 100.0);
        let (left, right) = r.split_horizontal(0.3, 4.0);

        assert!((left.width - 28.0).abs() < 1e-4); // 100*0.3 - 2
        assert!((right.width - 68.0).abs() < 1e-4); // 100*0.7 - 2
        assert!((right.x - 32.0).abs() < 1e-4); // 28 + 4
    }

    #[test]
    fn contains_point_true() {
        let r = Rect::new(10.0, 10.0, 50.0, 30.0);
        assert!(r.contains_point(10.0, 10.0)); // top-left corner
        assert!(r.contains_point(60.0, 40.0)); // bottom-right corner
        assert!(r.contains_point(35.0, 25.0)); // center
    }

    #[test]
    fn contains_point_false() {
        let r = Rect::new(10.0, 10.0, 50.0, 30.0);
        assert!(!r.contains_point(9.9, 10.0)); // left of rect
        assert!(!r.contains_point(10.0, 9.9)); // above rect
        assert!(!r.contains_point(60.1, 10.0)); // right of rect
        assert!(!r.contains_point(10.0, 40.1)); // below rect
    }

    #[test]
    fn zero_size_rect() {
        let r = Rect::new(5.0, 5.0, 0.0, 0.0);
        assert!(r.contains_point(5.0, 5.0)); // exact point is inside
        assert!(!r.contains_point(5.1, 5.0));

        let (left, right) = r.split_horizontal(0.5, 0.0);
        assert_eq!(left.width, 0.0);
        assert_eq!(right.width, 0.0);
    }

    #[test]
    fn serde_roundtrip() {
        let r = Rect::new(1.0, 2.0, 3.0, 4.0);
        let json = serde_json::to_string(&r).unwrap();
        let back: Rect = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }
}
