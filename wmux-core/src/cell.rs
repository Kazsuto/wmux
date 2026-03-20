use std::sync::Arc;

use bitflags::bitflags;
use compact_str::CompactString;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::color::Color;
use crate::event::Hyperlink;

bitflags! {
    /// Terminal cell attribute flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct CellFlags: u16 {
        const BOLD          = 1 << 0;
        const ITALIC        = 1 << 1;
        const UNDERLINE     = 1 << 2;
        const STRIKETHROUGH = 1 << 3;
        const INVERSE       = 1 << 4;
        const HIDDEN        = 1 << 5;
        const DIM           = 1 << 6;
        const BLINK         = 1 << 7;
        /// Marks the trailing cell of a wide (double-width) character.
        const WIDE_SPACER   = 1 << 8;
    }
}

impl Default for CellFlags {
    fn default() -> Self {
        Self::empty()
    }
}

impl Serialize for CellFlags {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.bits().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CellFlags {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let bits = u16::deserialize(deserializer)?;
        Ok(Self::from_bits_truncate(bits))
    }
}

/// A single terminal cell storing a grapheme cluster with styling.
///
/// Wide characters span two cells: the first holds the content, the second
/// has `CellFlags::WIDE_SPACER` set with an empty grapheme.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    /// The grapheme cluster displayed in this cell.
    pub grapheme: CompactString,
    /// Foreground color.
    pub fg: Color,
    /// Background color.
    pub bg: Color,
    /// Attribute flags (bold, italic, etc.).
    pub flags: CellFlags,
    /// Hyperlink target (OSC 8). Shared across cells in the same link span.
    #[serde(skip)]
    pub hyperlink: Option<Arc<Hyperlink>>,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            grapheme: CompactString::const_new(" "),
            fg: Color::Named(7), // white
            bg: Color::Named(0), // black
            flags: CellFlags::empty(),
            hyperlink: None,
        }
    }
}

/// A single row in the terminal grid.
///
/// **Note:** The grid itself must NOT be `Vec<Row>`. Use a flat contiguous
/// buffer with stride-based indexing for cache efficiency. See terminal-vte
/// architecture rules.
pub type Row = Vec<Cell>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_cell_is_space() {
        let cell = Cell::default();
        assert_eq!(cell.grapheme, " ");
        assert_eq!(cell.fg, Color::Named(7));
        assert_eq!(cell.bg, Color::Named(0));
        assert_eq!(cell.flags, CellFlags::empty());
    }

    #[test]
    fn cell_with_empty_grapheme() {
        let cell = Cell {
            grapheme: CompactString::default(),
            ..Cell::default()
        };
        assert!(cell.grapheme.is_empty());
    }

    #[test]
    fn cell_with_multibyte_grapheme() {
        let cell = Cell {
            grapheme: CompactString::from("\u{1F600}"), // emoji
            ..Cell::default()
        };
        assert_eq!(cell.grapheme.as_str(), "\u{1F600}");
    }

    #[test]
    fn cell_with_cjk_grapheme() {
        let cell = Cell {
            grapheme: CompactString::from("\u{4E16}"), // CJK character
            ..Cell::default()
        };
        assert_eq!(cell.grapheme.as_str(), "\u{4E16}");
    }

    #[test]
    fn wide_spacer_cell() {
        let cell = Cell {
            grapheme: CompactString::default(),
            flags: CellFlags::WIDE_SPACER,
            ..Cell::default()
        };
        assert!(cell.flags.contains(CellFlags::WIDE_SPACER));
    }

    #[test]
    fn combine_cell_flags() {
        let flags = CellFlags::BOLD | CellFlags::ITALIC | CellFlags::UNDERLINE;
        assert!(flags.contains(CellFlags::BOLD));
        assert!(flags.contains(CellFlags::ITALIC));
        assert!(flags.contains(CellFlags::UNDERLINE));
        assert!(!flags.contains(CellFlags::STRIKETHROUGH));
    }

    #[test]
    fn row_type_is_vec_cell() {
        let row: Row = vec![Cell::default(); 80];
        assert_eq!(row.len(), 80);
    }

    #[test]
    fn serde_roundtrip() {
        let cell = Cell {
            grapheme: "A".into(),
            fg: Color::Rgb(255, 0, 0),
            bg: Color::Indexed(42),
            flags: CellFlags::BOLD | CellFlags::UNDERLINE,
            hyperlink: None,
        };
        let json = serde_json::to_string(&cell).unwrap();
        let back: Cell = serde_json::from_str(&json).unwrap();
        assert_eq!(cell, back);
    }
}
