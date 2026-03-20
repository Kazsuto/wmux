use serde::{Deserialize, Serialize};

/// Terminal cursor shape.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CursorShape {
    #[default]
    Block,
    Underline,
    Bar,
}

/// Terminal cursor state: position, shape, and visibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CursorState {
    pub row: usize,
    pub col: usize,
    pub shape: CursorShape,
    pub visible: bool,
    pub blinking: bool,
}

impl Default for CursorState {
    fn default() -> Self {
        Self {
            row: 0,
            col: 0,
            shape: CursorShape::default(),
            visible: true,
            blinking: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_cursor_shape_is_block() {
        assert_eq!(CursorShape::default(), CursorShape::Block);
    }

    #[test]
    fn default_cursor_state() {
        let cursor = CursorState::default();
        assert_eq!(cursor.row, 0);
        assert_eq!(cursor.col, 0);
        assert_eq!(cursor.shape, CursorShape::Block);
        assert!(cursor.visible);
        assert!(cursor.blinking);
    }

    #[test]
    fn cursor_state_is_copy() {
        let c = CursorState::default();
        let c2 = c;
        assert_eq!(c, c2);
    }

    #[test]
    fn serde_roundtrip() {
        let cursor = CursorState {
            row: 10,
            col: 42,
            shape: CursorShape::Bar,
            visible: false,
            blinking: true,
        };
        let json = serde_json::to_string(&cursor).unwrap();
        let back: CursorState = serde_json::from_str(&json).unwrap();
        assert_eq!(cursor, back);
    }
}
