use serde::{Deserialize, Serialize};

/// Terminal color representation.
///
/// Supports the three standard terminal color modes:
/// - Named ANSI colors (indices 0-15)
/// - 256-color palette (indices 0-255)
/// - 24-bit true color (RGB)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Color {
    /// Standard ANSI color. 0-7 normal, 8-15 bright variants.
    Named(u8),
    /// 256-color palette index.
    Indexed(u8),
    /// 24-bit true color.
    Rgb(u8, u8, u8),
}

impl Default for Color {
    /// Default foreground: white (Named 7).
    fn default() -> Self {
        Self::Named(7)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_named_white() {
        assert_eq!(Color::default(), Color::Named(7));
    }

    #[test]
    fn named_boundary_values() {
        let normal_black = Color::Named(0);
        let bright_white = Color::Named(15);
        assert_ne!(normal_black, bright_white);
    }

    #[test]
    fn indexed_boundary_values() {
        let first = Color::Indexed(0);
        let last = Color::Indexed(255);
        assert_ne!(first, last);
    }

    #[test]
    fn rgb_equality() {
        assert_eq!(Color::Rgb(255, 0, 128), Color::Rgb(255, 0, 128));
        assert_ne!(Color::Rgb(255, 0, 0), Color::Rgb(0, 255, 0));
    }

    #[test]
    fn color_is_copy() {
        let c = Color::Rgb(10, 20, 30);
        let c2 = c;
        assert_eq!(c, c2);
    }

    #[test]
    fn serde_roundtrip() {
        let colors = [
            Color::Named(0),
            Color::Indexed(128),
            Color::Rgb(255, 128, 0),
        ];
        for color in &colors {
            let json = serde_json::to_string(color).unwrap();
            let back: Color = serde_json::from_str(&json).unwrap();
            assert_eq!(*color, back);
        }
    }
}
