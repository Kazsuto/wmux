use serde::{Deserialize, Serialize};

use crate::cell::CellFlags;
use crate::grid::Grid;
use crate::scrollback::Scrollback;

/// Selection granularity mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelectionMode {
    /// Character-level selection (single click + drag).
    Normal,
    /// Word selection (double-click).
    Word,
    /// Line selection (triple-click).
    Line,
}

/// A point in the terminal grid (column, row).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionPoint {
    pub col: usize,
    pub row: usize,
}

/// Text selection state for the terminal.
#[derive(Debug, Clone)]
pub struct Selection {
    /// Anchor point (where the selection started).
    pub start: SelectionPoint,
    /// Moving point (follows the cursor during drag).
    pub end: SelectionPoint,
    /// Selection granularity.
    pub mode: SelectionMode,
    /// Whether the selection is actively being dragged.
    pub active: bool,
}

impl Selection {
    /// Create a new selection starting at the given position.
    pub fn new(col: usize, row: usize, mode: SelectionMode) -> Self {
        let point = SelectionPoint { col, row };
        Self {
            start: point,
            end: point,
            mode,
            active: true,
        }
    }

    /// Update the end point during a drag operation.
    pub fn update(&mut self, col: usize, row: usize) {
        self.end = SelectionPoint { col, row };
    }

    /// Return the selection endpoints in top-left to bottom-right order.
    pub fn normalized(&self) -> (SelectionPoint, SelectionPoint) {
        let (s, e) = (&self.start, &self.end);
        if s.row < e.row || (s.row == e.row && s.col <= e.col) {
            (*s, *e)
        } else {
            (*e, *s)
        }
    }

    /// Check whether the given grid position is within the selection.
    pub fn contains(&self, col: usize, row: usize) -> bool {
        let (start, end) = self.normalized();

        if row < start.row || row > end.row {
            return false;
        }

        if start.row == end.row {
            // Single row selection
            return col >= start.col && col <= end.col;
        }

        if row == start.row {
            col >= start.col
        } else if row == end.row {
            col <= end.col
        } else {
            true // Middle rows are fully selected
        }
    }

    /// Extract the selected text from the grid (and optionally scrollback).
    ///
    /// Returns the selected text with rows joined by newlines. Trailing
    /// spaces on each row are trimmed.
    pub fn extract_text(&self, grid: &Grid, _scrollback: &Scrollback) -> String {
        let (start, end) = self.normalized();
        let cols = grid.cols() as usize;
        let rows = grid.rows() as usize;

        let row_count = end.row.saturating_sub(start.row) + 1;
        let mut lines: Vec<String> = Vec::with_capacity(row_count);

        for row in start.row..=end.row {
            if row >= rows {
                break;
            }

            let col_start = if row == start.row {
                match self.mode {
                    SelectionMode::Line => 0,
                    SelectionMode::Word => self.word_start(grid, start.col, row),
                    SelectionMode::Normal => start.col,
                }
            } else {
                0
            };

            let col_end = if row == end.row {
                match self.mode {
                    SelectionMode::Line => cols.saturating_sub(1),
                    SelectionMode::Word => self.word_end(grid, end.col, row),
                    SelectionMode::Normal => end.col.min(cols.saturating_sub(1)),
                }
            } else {
                cols.saturating_sub(1)
            };

            let mut line = String::new();
            for col in col_start..=col_end {
                if col >= cols {
                    break;
                }
                #[allow(clippy::cast_possible_truncation)]
                let cell = grid.cell(col as u16, row as u16);
                if cell.flags.contains(CellFlags::WIDE_SPACER) {
                    continue;
                }
                line.push_str(&cell.grapheme);
            }
            // Trim trailing spaces in-place (avoid extra allocation).
            let trimmed_len = line.trim_end().len();
            line.truncate(trimmed_len);
            lines.push(line);
        }

        lines.join("\n")
    }

    /// Find the start of a word boundary (expand left).
    fn word_start(&self, grid: &Grid, col: usize, row: usize) -> usize {
        let mut c = col;
        while c > 0 {
            #[allow(clippy::cast_possible_truncation)]
            let cell = grid.cell((c - 1) as u16, row as u16);
            if !is_word_char(&cell.grapheme) {
                break;
            }
            c -= 1;
        }
        c
    }

    /// Find the end of a word boundary (expand right).
    fn word_end(&self, grid: &Grid, col: usize, row: usize) -> usize {
        let cols = grid.cols() as usize;
        let mut c = col;
        while c + 1 < cols {
            #[allow(clippy::cast_possible_truncation)]
            let cell = grid.cell((c + 1) as u16, row as u16);
            if !is_word_char(&cell.grapheme) {
                break;
            }
            c += 1;
        }
        c
    }
}

/// Check if a grapheme is a "word" character (alphanumeric or underscore).
fn is_word_char(grapheme: &str) -> bool {
    grapheme
        .chars()
        .next()
        .is_some_and(|c| c.is_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::Cell;
    use compact_str::CompactString;

    fn make_grid(text_rows: &[&str]) -> Grid {
        let cols = text_rows.iter().map(|r| r.len()).max().unwrap_or(10) as u16;
        let rows = text_rows.len() as u16;
        let mut grid = Grid::new(cols, rows);
        #[allow(clippy::cast_possible_truncation)]
        for (r, text) in text_rows.iter().enumerate() {
            for (c, ch) in text.chars().enumerate() {
                let mut cell = Cell::default();
                cell.grapheme = CompactString::from(ch.to_string());
                grid.set_cell(c as u16, r as u16, cell);
            }
        }
        grid
    }

    #[test]
    fn new_selection_has_same_start_end() {
        let sel = Selection::new(5, 3, SelectionMode::Normal);
        assert_eq!(sel.start, sel.end);
        assert!(sel.active);
        assert_eq!(sel.mode, SelectionMode::Normal);
    }

    #[test]
    fn update_changes_end_point() {
        let mut sel = Selection::new(0, 0, SelectionMode::Normal);
        sel.update(10, 5);
        assert_eq!(sel.end.col, 10);
        assert_eq!(sel.end.row, 5);
        assert_eq!(sel.start.col, 0);
        assert_eq!(sel.start.row, 0);
    }

    #[test]
    fn normalized_orders_correctly() {
        // Forward selection
        let sel = Selection {
            start: SelectionPoint { col: 2, row: 1 },
            end: SelectionPoint { col: 8, row: 3 },
            mode: SelectionMode::Normal,
            active: false,
        };
        let (s, e) = sel.normalized();
        assert_eq!(s.row, 1);
        assert_eq!(e.row, 3);

        // Backward selection
        let sel = Selection {
            start: SelectionPoint { col: 8, row: 3 },
            end: SelectionPoint { col: 2, row: 1 },
            mode: SelectionMode::Normal,
            active: false,
        };
        let (s, e) = sel.normalized();
        assert_eq!(s.row, 1);
        assert_eq!(e.row, 3);
    }

    #[test]
    fn contains_single_row() {
        let sel = Selection {
            start: SelectionPoint { col: 3, row: 0 },
            end: SelectionPoint { col: 7, row: 0 },
            mode: SelectionMode::Normal,
            active: false,
        };
        assert!(!sel.contains(2, 0));
        assert!(sel.contains(3, 0));
        assert!(sel.contains(5, 0));
        assert!(sel.contains(7, 0));
        assert!(!sel.contains(8, 0));
        assert!(!sel.contains(5, 1));
    }

    #[test]
    fn contains_multi_row() {
        let sel = Selection {
            start: SelectionPoint { col: 5, row: 1 },
            end: SelectionPoint { col: 3, row: 3 },
            mode: SelectionMode::Normal,
            active: false,
        };
        // Row 0 — not selected
        assert!(!sel.contains(5, 0));
        // Row 1 — from col 5 onward
        assert!(!sel.contains(4, 1));
        assert!(sel.contains(5, 1));
        assert!(sel.contains(10, 1));
        // Row 2 — fully selected
        assert!(sel.contains(0, 2));
        assert!(sel.contains(50, 2));
        // Row 3 — up to col 3
        assert!(sel.contains(0, 3));
        assert!(sel.contains(3, 3));
        assert!(!sel.contains(4, 3));
        // Row 4 — not selected
        assert!(!sel.contains(0, 4));
    }

    #[test]
    fn extract_text_normal() {
        let grid = make_grid(&["hello world", "foo bar    "]);
        let scrollback = Scrollback::new(100);

        let sel = Selection {
            start: SelectionPoint { col: 0, row: 0 },
            end: SelectionPoint { col: 4, row: 0 },
            mode: SelectionMode::Normal,
            active: false,
        };
        assert_eq!(sel.extract_text(&grid, &scrollback), "hello");
    }

    #[test]
    fn extract_text_multi_row() {
        let grid = make_grid(&["hello world", "foo bar    "]);
        let scrollback = Scrollback::new(100);

        let sel = Selection {
            start: SelectionPoint { col: 6, row: 0 },
            end: SelectionPoint { col: 2, row: 1 },
            mode: SelectionMode::Normal,
            active: false,
        };
        assert_eq!(sel.extract_text(&grid, &scrollback), "world\nfoo");
    }

    #[test]
    fn extract_text_word_mode() {
        let grid = make_grid(&["hello world test"]);
        let scrollback = Scrollback::new(100);

        let sel = Selection {
            start: SelectionPoint { col: 7, row: 0 },
            end: SelectionPoint { col: 8, row: 0 },
            mode: SelectionMode::Word,
            active: false,
        };
        assert_eq!(sel.extract_text(&grid, &scrollback), "world");
    }

    #[test]
    fn extract_text_line_mode() {
        let grid = make_grid(&["hello world  ", "foo bar      "]);
        let scrollback = Scrollback::new(100);

        let sel = Selection {
            start: SelectionPoint { col: 3, row: 0 },
            end: SelectionPoint { col: 3, row: 0 },
            mode: SelectionMode::Line,
            active: false,
        };
        assert_eq!(sel.extract_text(&grid, &scrollback), "hello world");
    }

    #[test]
    fn is_word_char_works() {
        assert!(is_word_char("a"));
        assert!(is_word_char("Z"));
        assert!(is_word_char("5"));
        assert!(is_word_char("_"));
        assert!(!is_word_char(" "));
        assert!(!is_word_char("."));
        assert!(!is_word_char("-"));
        assert!(!is_word_char(""));
    }

    #[test]
    fn selection_mode_serde_roundtrip() {
        let mode = SelectionMode::Word;
        let json = serde_json::to_string(&mode).unwrap();
        let back: SelectionMode = serde_json::from_str(&json).unwrap();
        assert_eq!(mode, back);
    }
}
