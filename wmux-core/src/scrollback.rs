use std::collections::VecDeque;

use crate::cell::Row;

/// Ring-buffer scrollback storing rows that scroll off the visible grid.
///
/// Uses `VecDeque<Row>` for O(1) push/pop. When the buffer exceeds
/// `max_lines`, the oldest rows are evicted automatically.
#[derive(Debug, Clone)]
pub struct Scrollback {
    /// Ring buffer of rows (index 0 = oldest).
    rows: VecDeque<Row>,
    /// Maximum number of rows to retain.
    max_lines: usize,
    /// Viewport offset from bottom (0 = live terminal, positive = scrolled up).
    viewport_offset: usize,
}

/// Hard upper limit for scrollback lines to prevent unbounded memory growth.
const MAX_SCROLLBACK_LIMIT: usize = 100_000;

impl Scrollback {
    /// Create a new scrollback buffer with the given maximum line capacity.
    ///
    /// `max_lines` is clamped to an internal hard limit (100,000) to
    /// prevent misconfiguration from causing unbounded memory growth.
    #[must_use]
    pub fn new(max_lines: usize) -> Self {
        let max_lines = max_lines.min(MAX_SCROLLBACK_LIMIT);
        Self {
            rows: VecDeque::with_capacity(max_lines.min(4096)),
            max_lines,
            viewport_offset: 0,
        }
    }

    /// Push a row into the scrollback. If the buffer exceeds `max_lines`,
    /// the oldest row is evicted.
    #[inline]
    pub fn push_row(&mut self, row: Row) {
        if self.max_lines == 0 {
            return;
        }
        if self.rows.len() >= self.max_lines {
            self.rows.pop_front();
            // Decrement viewport offset so it tracks the same content row
            // after eviction shifts all indices down by one.
            self.viewport_offset = self.viewport_offset.saturating_sub(1);
        }
        self.rows.push_back(row);
    }

    /// Number of rows currently stored.
    #[inline]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether the scrollback is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Configured maximum line capacity.
    #[inline]
    pub fn max_lines(&self) -> usize {
        self.max_lines
    }

    /// Current viewport offset (0 = bottom/live, positive = scrolled up).
    #[inline]
    pub fn viewport_offset(&self) -> usize {
        self.viewport_offset
    }

    /// Set the viewport offset, clamped to `[0, len()]`.
    pub fn set_viewport_offset(&mut self, offset: usize) {
        self.viewport_offset = offset.min(self.rows.len());
    }

    /// Reset viewport to bottom (live terminal).
    pub fn reset_viewport(&mut self) {
        self.viewport_offset = 0;
    }

    /// Get a row by index (0 = oldest row in buffer).
    pub fn get_row(&self, index: usize) -> Option<&Row> {
        self.rows.get(index)
    }

    /// Clear all scrollback content and reset viewport.
    pub fn clear(&mut self) {
        self.rows.clear();
        self.viewport_offset = 0;
    }

    /// Read text from scrollback as a newline-joined string.
    ///
    /// - Positive `start`/`end`: absolute indices from oldest (0).
    /// - Negative `start`/`end`: relative from end (-1 = last row).
    /// - `end` is exclusive. If `end > len()`, it is clamped.
    /// - Returns empty string if range is empty or out of bounds.
    pub fn read_text(&self, start: isize, end: isize) -> String {
        let len = self.rows.len() as isize;
        if len == 0 {
            return String::new();
        }

        let resolve = |idx: isize| -> usize {
            if idx < 0 {
                (len + idx).max(0) as usize
            } else {
                idx as usize
            }
        };

        let s = resolve(start);
        let e = resolve(end).min(self.rows.len());

        if s >= e {
            return String::new();
        }

        // Pre-allocate: estimate ~cols chars per row + 1 newline.
        let estimated_cols = self.rows.front().map_or(80, |r| r.len());
        let mut result = String::with_capacity((e - s) * (estimated_cols + 1));
        for (i, row) in self.rows.range(s..e).enumerate() {
            if i > 0 {
                result.push('\n');
            }
            for cell in row {
                result.push_str(cell.grapheme.as_str());
            }
        }
        // Trim trailing whitespace per line is not needed — return raw content.
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::Cell;
    use compact_str::CompactString;

    /// Helper: create a row of cells from a string (one char per cell).
    fn make_row(s: &str) -> Row {
        s.chars()
            .map(|c| Cell {
                grapheme: CompactString::from(c.to_string()),
                ..Cell::default()
            })
            .collect()
    }

    #[test]
    fn new_scrollback_is_empty() {
        let sb = Scrollback::new(4000);
        assert!(sb.is_empty());
        assert_eq!(sb.len(), 0);
        assert_eq!(sb.max_lines(), 4000);
        assert_eq!(sb.viewport_offset(), 0);
    }

    #[test]
    fn push_and_retrieve_rows() {
        let mut sb = Scrollback::new(100);
        sb.push_row(make_row("hello"));
        sb.push_row(make_row("world"));
        assert_eq!(sb.len(), 2);
        assert_eq!(sb.get_row(0).unwrap().len(), 5);
        assert_eq!(sb.get_row(1).unwrap().len(), 5);
        assert!(sb.get_row(2).is_none());
    }

    #[test]
    fn eviction_at_max_lines() {
        let mut sb = Scrollback::new(3);
        sb.push_row(make_row("AAA"));
        sb.push_row(make_row("BBB"));
        sb.push_row(make_row("CCC"));
        assert_eq!(sb.len(), 3);

        sb.push_row(make_row("DDD"));
        assert_eq!(sb.len(), 3);
        // Oldest row (AAA) should be evicted; first row is now BBB.
        assert_eq!(sb.get_row(0).unwrap()[0].grapheme.as_str(), "B");
        assert_eq!(sb.get_row(2).unwrap()[0].grapheme.as_str(), "D");
    }

    #[test]
    fn push_5000_into_4000_keeps_last_4000() {
        let mut sb = Scrollback::new(4000);
        for i in 0..5000 {
            sb.push_row(make_row(&format!("{i:04}")));
        }
        assert_eq!(sb.len(), 4000);
        // First row should be row 1000 (0-indexed from original).
        assert_eq!(sb.get_row(0).unwrap()[0].grapheme.as_str(), "1");
    }

    #[test]
    fn zero_max_lines_discards_all() {
        let mut sb = Scrollback::new(0);
        sb.push_row(make_row("hello"));
        assert!(sb.is_empty());
        assert_eq!(sb.len(), 0);
    }

    #[test]
    fn viewport_offset_clamped() {
        let mut sb = Scrollback::new(100);
        sb.push_row(make_row("A"));
        sb.push_row(make_row("B"));

        sb.set_viewport_offset(10);
        assert_eq!(sb.viewport_offset(), 2); // clamped to len()

        sb.set_viewport_offset(1);
        assert_eq!(sb.viewport_offset(), 1);

        sb.reset_viewport();
        assert_eq!(sb.viewport_offset(), 0);
    }

    #[test]
    fn viewport_offset_adjusted_on_eviction() {
        let mut sb = Scrollback::new(2);
        sb.push_row(make_row("A"));
        sb.push_row(make_row("B"));
        sb.set_viewport_offset(2);
        assert_eq!(sb.viewport_offset(), 2);

        // Push evicts oldest, viewport should be clamped.
        sb.push_row(make_row("C"));
        assert!(sb.viewport_offset() <= sb.len());
    }

    #[test]
    fn clear_resets_everything() {
        let mut sb = Scrollback::new(100);
        sb.push_row(make_row("A"));
        sb.push_row(make_row("B"));
        sb.set_viewport_offset(1);

        sb.clear();
        assert!(sb.is_empty());
        assert_eq!(sb.viewport_offset(), 0);
    }

    #[test]
    fn read_text_basic() {
        let mut sb = Scrollback::new(100);
        sb.push_row(make_row("AB"));
        sb.push_row(make_row("CD"));
        sb.push_row(make_row("EF"));

        let text = sb.read_text(0, 3);
        assert_eq!(text, "AB\nCD\nEF");
    }

    #[test]
    fn read_text_negative_indices() {
        let mut sb = Scrollback::new(100);
        sb.push_row(make_row("AA"));
        sb.push_row(make_row("BB"));
        sb.push_row(make_row("CC"));

        // Last 2 rows.
        let text = sb.read_text(-2, 3);
        assert_eq!(text, "BB\nCC");

        // Last row only.
        let text = sb.read_text(-1, 3);
        assert_eq!(text, "CC");
    }

    #[test]
    fn read_text_out_of_bounds() {
        let mut sb = Scrollback::new(100);
        sb.push_row(make_row("XX"));

        // Start beyond length.
        assert_eq!(sb.read_text(5, 10), "");
        // End before start.
        assert_eq!(sb.read_text(1, 0), "");
        // Empty scrollback.
        let empty = Scrollback::new(100);
        assert_eq!(empty.read_text(0, 1), "");
    }

    #[test]
    fn read_text_clamped_end() {
        let mut sb = Scrollback::new(100);
        sb.push_row(make_row("AB"));
        sb.push_row(make_row("CD"));

        // end=100 is clamped to len()=2.
        let text = sb.read_text(0, 100);
        assert_eq!(text, "AB\nCD");
    }
}
