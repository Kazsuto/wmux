use std::fmt;

use crate::cell::Cell;
use crate::cursor::CursorState;

/// Terminal cell grid with dirty row tracking.
///
/// Stores cells in a flat contiguous buffer with stride-based indexing
/// (`cells[row * cols + col]`) for cache efficiency. The renderer uses
/// dirty flags to minimize GPU uploads.
#[derive(Clone)]
pub struct Grid {
    /// Flat cell storage indexed by `row * cols + col`.
    cells: Vec<Cell>,
    /// Number of columns (width).
    cols: u16,
    /// Number of rows (height).
    rows: u16,
    /// Per-row dirty flags for GPU upload optimization.
    dirty: Vec<bool>,
    /// Cursor state (position, shape, visibility).
    cursor: CursorState,
}

impl fmt::Debug for Grid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Grid")
            .field("cols", &self.cols)
            .field("rows", &self.rows)
            .field("cursor", &self.cursor)
            .finish_non_exhaustive()
    }
}

impl Grid {
    /// Create a new grid with the given dimensions, filled with default cells.
    ///
    /// # Panics
    /// Panics if `cols` or `rows` is zero.
    pub fn new(cols: u16, rows: u16) -> Self {
        assert!(cols > 0 && rows > 0, "grid dimensions must be nonzero");
        let total = cols as usize * rows as usize;
        Self {
            cells: vec![Cell::default(); total],
            cols,
            rows,
            dirty: vec![false; rows as usize],
            cursor: CursorState::default(),
        }
    }

    /// Column count.
    #[inline]
    pub fn cols(&self) -> u16 {
        self.cols
    }

    /// Row count.
    #[inline]
    pub fn rows(&self) -> u16 {
        self.rows
    }

    /// Index into the flat cell buffer.
    #[inline]
    fn idx(&self, col: u16, row: u16) -> usize {
        row as usize * self.cols as usize + col as usize
    }

    /// Reference to the cell at `(col, row)`.
    ///
    /// # Panics
    /// Panics if `col >= cols` or `row >= rows`.
    #[inline]
    pub fn cell(&self, col: u16, row: u16) -> &Cell {
        assert!(
            col < self.cols && row < self.rows,
            "cell out of bounds: ({col}, {row}) in {}x{} grid",
            self.cols,
            self.rows,
        );
        let i = self.idx(col, row);
        &self.cells[i]
    }

    /// Mutable reference to the cell at `(col, row)`. Does **not** mark
    /// the row dirty — call [`set_cell`] if dirty tracking is needed.
    ///
    /// # Panics
    /// Panics if `col >= cols` or `row >= rows`.
    #[inline]
    pub fn cell_mut(&mut self, col: u16, row: u16) -> &mut Cell {
        assert!(
            col < self.cols && row < self.rows,
            "cell out of bounds: ({col}, {row}) in {}x{} grid",
            self.cols,
            self.rows,
        );
        let i = self.idx(col, row);
        &mut self.cells[i]
    }

    /// Write a cell at `(col, row)` and mark the row dirty.
    pub fn set_cell(&mut self, col: u16, row: u16, cell: Cell) {
        assert!(
            col < self.cols && row < self.rows,
            "cell out of bounds: ({col}, {row}) in {}x{} grid",
            self.cols,
            self.rows,
        );
        let i = self.idx(col, row);
        self.cells[i] = cell;
        self.dirty[row as usize] = true;
    }

    /// Fill columns `[col_start, col_end)` in `row` with clones of `cell`,
    /// marking the row dirty once. More efficient than repeated `set_cell`
    /// calls because it reuses allocations via `clone_from`.
    pub fn fill_cells(&mut self, col_start: u16, col_end: u16, row: u16, cell: &Cell) {
        if row >= self.rows || col_start >= self.cols {
            return;
        }
        let end_col = col_end.min(self.cols) as usize;
        let start = self.idx(col_start, row);
        let end = self.idx(0, row) + end_col;
        for c in &mut self.cells[start..end] {
            c.clone_from(cell);
        }
        self.dirty[row as usize] = true;
    }

    /// Reset all cells in `row` to default and mark it dirty.
    pub fn clear_row(&mut self, row: u16) {
        assert!(
            row < self.rows,
            "row out of bounds: {row} in {} rows",
            self.rows
        );
        let start = self.idx(0, row);
        let end = start + self.cols as usize;
        for cell in &mut self.cells[start..end] {
            *cell = Cell::default();
        }
        self.dirty[row as usize] = true;
    }

    /// Insert `count` blank cells at the cursor column in the cursor row,
    /// shifting existing cells to the right. Cells pushed past the right
    /// margin are discarded.
    pub fn insert_chars(&mut self, count: u16) {
        let row = self.cursor.row as u16;
        let col = self.cursor.col as u16;
        if row >= self.rows || col >= self.cols {
            return;
        }
        let start = self.idx(col, row);
        let row_end = self.idx(0, row) + self.cols as usize;
        let count = count.min(self.cols - col) as usize;

        // Rotate the affected slice right — moves cells without per-element clone.
        self.cells[start..row_end].rotate_right(count);

        // Overwrite the inserted positions with default cells.
        for cell in &mut self.cells[start..start + count] {
            *cell = Cell::default();
        }
        self.dirty[row as usize] = true;
    }

    /// Delete `count` cells at the cursor column in the cursor row,
    /// shifting remaining cells left. New cells at the right margin are
    /// filled with defaults.
    pub fn delete_chars(&mut self, count: u16) {
        let row = self.cursor.row as u16;
        let col = self.cursor.col as u16;
        if row >= self.rows || col >= self.cols {
            return;
        }
        let start = self.idx(col, row);
        let row_end = self.idx(0, row) + self.cols as usize;
        let count = count.min(self.cols - col) as usize;

        // Rotate the affected slice left — moves cells without per-element clone.
        self.cells[start..row_end].rotate_left(count);

        // Overwrite vacated positions at end with defaults.
        for cell in &mut self.cells[row_end - count..row_end] {
            *cell = Cell::default();
        }
        self.dirty[row as usize] = true;
    }

    /// Scroll the grid up by `n` rows. The top `n` rows are discarded, all
    /// other rows move up, and the bottom `n` rows are cleared.
    pub fn scroll_up(&mut self, n: u16) {
        let n = n.min(self.rows) as usize;
        let stride = self.cols as usize;

        // Rotate the flat buffer left by n * stride positions.
        self.cells.rotate_left(n * stride);

        // Clear the bottom n rows.
        let total = self.cells.len();
        for cell in &mut self.cells[total - n * stride..] {
            *cell = Cell::default();
        }

        // Mark all rows dirty.
        self.dirty.iter_mut().for_each(|d| *d = true);
    }

    /// Scroll the grid down by `n` rows. The bottom `n` rows are discarded,
    /// all other rows move down, and the top `n` rows are cleared.
    pub fn scroll_down(&mut self, n: u16) {
        let n = n.min(self.rows) as usize;
        let stride = self.cols as usize;

        // Rotate the flat buffer right by n * stride positions.
        self.cells.rotate_right(n * stride);

        // Clear the top n rows.
        for cell in &mut self.cells[..n * stride] {
            *cell = Cell::default();
        }

        // Mark all rows dirty.
        self.dirty.iter_mut().for_each(|d| *d = true);
    }

    /// Resize the grid. On shrink, excess cells/rows are truncated.
    /// On grow, new cells/rows are filled with defaults. The cursor is
    /// clamped to the new bounds. All rows are marked dirty.
    ///
    /// # Panics
    /// Panics if `new_cols` or `new_rows` is zero.
    pub fn resize(&mut self, new_cols: u16, new_rows: u16) {
        assert!(
            new_cols > 0 && new_rows > 0,
            "grid dimensions must be nonzero",
        );
        let old_cols = self.cols as usize;
        let old_rows = self.rows as usize;
        let nc = new_cols as usize;
        let nr = new_rows as usize;

        let mut new_cells = vec![Cell::default(); nc * nr];

        let copy_rows = old_rows.min(nr);
        let copy_cols = old_cols.min(nc);
        for row in 0..copy_rows {
            let dst_start = row * nc;
            let src_start = row * old_cols;
            new_cells[dst_start..dst_start + copy_cols]
                .clone_from_slice(&self.cells[src_start..src_start + copy_cols]);
        }

        self.cells = new_cells;
        self.cols = new_cols;
        self.rows = new_rows;

        self.dirty = vec![true; nr];

        // Clamp cursor.
        self.clamp_cursor();
    }

    /// Reset all cells to default and mark every row dirty.
    pub fn clear(&mut self) {
        for cell in &mut self.cells {
            *cell = Cell::default();
        }
        self.dirty.iter_mut().for_each(|d| *d = true);
    }

    /// Return an immutable slice of all cells in `row`.
    ///
    /// This is the zero-copy counterpart of [`extract_row`] — it borrows
    /// directly from the flat cell buffer, avoiding any allocation.
    ///
    /// # Panics
    /// Panics if `row >= rows`.
    #[inline]
    pub fn row_slice(&self, row: u16) -> &[Cell] {
        assert!(
            row < self.rows,
            "row out of bounds: {row} in {} rows",
            self.rows
        );
        let start = self.idx(0, row);
        let end = start + self.cols as usize;
        &self.cells[start..end]
    }

    /// Copy cells from the given row into a `Row` (`Vec<Cell>`).
    ///
    /// Used to capture rows before they scroll off the grid into the
    /// scrollback buffer.
    ///
    /// # Panics
    /// Panics if `row >= rows`.
    pub fn extract_row(&self, row: u16) -> crate::cell::Row {
        assert!(
            row < self.rows,
            "row out of bounds: {row} in {} rows",
            self.rows
        );
        let start = self.idx(0, row);
        let end = start + self.cols as usize;
        self.cells[start..end].to_vec()
    }

    /// Return the indices of all dirty rows and reset all dirty flags.
    pub fn take_dirty_rows(&mut self) -> Vec<u16> {
        let mut result = Vec::with_capacity(self.rows as usize);
        for (i, flag) in self.dirty.iter_mut().enumerate() {
            if *flag {
                result.push(i as u16);
                *flag = false;
            }
        }
        result
    }

    /// Immutable reference to the cursor state.
    #[inline]
    pub fn cursor(&self) -> &CursorState {
        &self.cursor
    }

    /// Mutable reference to the cursor state.
    #[inline]
    pub fn cursor_mut(&mut self) -> &mut CursorState {
        &mut self.cursor
    }

    /// Move the cursor, clamping to grid bounds.
    pub fn set_cursor_pos(&mut self, col: u16, row: u16) {
        self.cursor.col = (col as usize).min(self.cols.saturating_sub(1) as usize);
        self.cursor.row = (row as usize).min(self.rows.saturating_sub(1) as usize);
    }

    /// Scroll rows `[top..=bottom]` up by `n`. The top `n` rows in the
    /// region are discarded, remaining rows shift up, and the bottom `n`
    /// rows are cleared. Marks affected rows dirty.
    pub fn scroll_up_in_region(&mut self, top: u16, bottom: u16, n: u16) {
        if top > bottom || bottom >= self.rows {
            return;
        }
        let n = n.min(bottom - top + 1) as usize;
        let stride = self.cols as usize;
        let top = top as usize;
        let bottom = bottom as usize;

        // Shift rows up: copy row[top+n..=bottom] to row[top..=bottom-n].
        // dst < src so forward iteration is safe with split_at_mut.
        for dst_row in top..=bottom - n {
            let src_row = dst_row + n;
            let dst_start = dst_row * stride;
            let src_start = src_row * stride;
            let (left, right) = self.cells.split_at_mut(src_start);
            left[dst_start..dst_start + stride].clone_from_slice(&right[..stride]);
        }

        // Clear the bottom n rows of the region.
        for row in (bottom + 1 - n)..=bottom {
            let start = row * stride;
            for cell in &mut self.cells[start..start + stride] {
                *cell = Cell::default();
            }
        }

        // Mark affected rows dirty.
        for row in top..=bottom {
            self.dirty[row] = true;
        }
    }

    /// Scroll rows `[top..=bottom]` down by `n`. The bottom `n` rows in
    /// the region are discarded, remaining rows shift down, and the top
    /// `n` rows are cleared. Marks affected rows dirty.
    pub fn scroll_down_in_region(&mut self, top: u16, bottom: u16, n: u16) {
        if top > bottom || bottom >= self.rows {
            return;
        }
        let n = n.min(bottom - top + 1) as usize;
        let stride = self.cols as usize;
        let top = top as usize;
        let bottom = bottom as usize;

        // Shift rows down: copy row[top..=bottom-n] to row[top+n..=bottom].
        // Reverse iteration so src is always below dst (dst > src).
        for dst_row in (top + n..=bottom).rev() {
            let src_row = dst_row - n;
            let dst_start = dst_row * stride;
            let src_start = src_row * stride;
            let (left, right) = self.cells.split_at_mut(dst_start);
            right[..stride].clone_from_slice(&left[src_start..src_start + stride]);
        }

        // Clear the top n rows of the region.
        for row in top..top + n {
            let start = row * stride;
            for cell in &mut self.cells[start..start + stride] {
                *cell = Cell::default();
            }
        }

        // Mark affected rows dirty.
        for row in top..=bottom {
            self.dirty[row] = true;
        }
    }

    /// Clamp cursor to current grid bounds.
    fn clamp_cursor(&mut self) {
        self.cursor.col = self.cursor.col.min(self.cols.saturating_sub(1) as usize);
        self.cursor.row = self.cursor.row.min(self.rows.saturating_sub(1) as usize);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::CellFlags;
    use crate::color::Color;
    use compact_str::CompactString;

    #[test]
    fn new_grid_defaults() {
        let grid = Grid::new(80, 24);
        assert_eq!(grid.cols(), 80);
        assert_eq!(grid.rows(), 24);
        assert_eq!(grid.cell(0, 0), &Cell::default());
        assert_eq!(grid.cell(79, 23), &Cell::default());
    }

    #[test]
    fn set_cell_and_read_back() {
        let mut grid = Grid::new(80, 24);
        let cell = Cell {
            grapheme: CompactString::from("A"),
            fg: Color::Rgb(255, 0, 0),
            bg: Color::Named(0),
            flags: CellFlags::BOLD,
            hyperlink: None,
        };
        grid.set_cell(5, 10, cell.clone());
        assert_eq!(grid.cell(5, 10), &cell);
    }

    #[test]
    fn dirty_tracking() {
        let mut grid = Grid::new(80, 24);
        // Fresh grid has no dirty rows.
        assert!(grid.take_dirty_rows().is_empty());

        grid.set_cell(0, 5, Cell::default());
        grid.set_cell(0, 10, Cell::default());

        let dirty = grid.take_dirty_rows();
        assert_eq!(dirty, vec![5, 10]);
    }

    #[test]
    fn take_dirty_rows_twice_returns_empty() {
        let mut grid = Grid::new(80, 24);
        grid.set_cell(0, 0, Cell::default());
        grid.take_dirty_rows();

        assert!(grid.take_dirty_rows().is_empty());
    }

    #[test]
    fn clear_row_resets_cells() {
        let mut grid = Grid::new(80, 24);
        let cell = Cell {
            grapheme: CompactString::from("X"),
            ..Cell::default()
        };
        grid.set_cell(0, 3, cell);
        grid.take_dirty_rows(); // clear dirty flags

        grid.clear_row(3);
        assert_eq!(grid.cell(0, 3), &Cell::default());

        let dirty = grid.take_dirty_rows();
        assert!(dirty.contains(&3));
    }

    #[test]
    fn scroll_up_shifts_content() {
        let mut grid = Grid::new(4, 4);
        // Put identifiable content in each row.
        for row in 0..4u16 {
            let cell = Cell {
                grapheme: CompactString::from(format!("{row}")),
                ..Cell::default()
            };
            grid.set_cell(0, row, cell);
        }
        grid.take_dirty_rows(); // clear

        grid.scroll_up(2);

        // Row 0 should now have what was row 2.
        assert_eq!(grid.cell(0, 0).grapheme.as_str(), "2");
        assert_eq!(grid.cell(0, 1).grapheme.as_str(), "3");
        // Bottom rows should be default.
        assert_eq!(grid.cell(0, 2), &Cell::default());
        assert_eq!(grid.cell(0, 3), &Cell::default());
    }

    #[test]
    fn scroll_up_more_than_height_clears_grid() {
        let mut grid = Grid::new(4, 4);
        let cell = Cell {
            grapheme: CompactString::from("X"),
            ..Cell::default()
        };
        grid.set_cell(0, 0, cell);

        grid.scroll_up(100);

        for row in 0..4u16 {
            for col in 0..4u16 {
                assert_eq!(grid.cell(col, row), &Cell::default());
            }
        }
    }

    #[test]
    fn scroll_down_shifts_content() {
        let mut grid = Grid::new(4, 4);
        for row in 0..4u16 {
            let cell = Cell {
                grapheme: CompactString::from(format!("{row}")),
                ..Cell::default()
            };
            grid.set_cell(0, row, cell);
        }

        grid.scroll_down(1);

        assert_eq!(grid.cell(0, 0), &Cell::default());
        assert_eq!(grid.cell(0, 1).grapheme.as_str(), "0");
        assert_eq!(grid.cell(0, 2).grapheme.as_str(), "1");
        assert_eq!(grid.cell(0, 3).grapheme.as_str(), "2");
    }

    #[test]
    fn resize_grow() {
        let mut grid = Grid::new(4, 3);
        let cell = Cell {
            grapheme: CompactString::from("A"),
            ..Cell::default()
        };
        grid.set_cell(0, 0, cell.clone());

        grid.resize(6, 5);

        assert_eq!(grid.cols(), 6);
        assert_eq!(grid.rows(), 5);
        assert_eq!(grid.cell(0, 0), &cell);
        // New cells are default.
        assert_eq!(grid.cell(5, 4), &Cell::default());
    }

    #[test]
    fn resize_shrink() {
        let mut grid = Grid::new(80, 24);
        let cell = Cell {
            grapheme: CompactString::from("Z"),
            ..Cell::default()
        };
        grid.set_cell(0, 0, cell.clone());

        grid.resize(40, 12);

        assert_eq!(grid.cols(), 40);
        assert_eq!(grid.rows(), 12);
        assert_eq!(grid.cell(0, 0), &cell);
    }

    #[test]
    fn resize_1x1() {
        let mut grid = Grid::new(80, 24);
        grid.resize(1, 1);
        assert_eq!(grid.cols(), 1);
        assert_eq!(grid.rows(), 1);
        assert_eq!(grid.cell(0, 0), &Cell::default());
    }

    #[test]
    fn insert_chars_shifts_right() {
        let mut grid = Grid::new(6, 1);
        // Fill row: A B C D E F
        for col in 0..6u16 {
            let c = (b'A' + col as u8) as char;
            grid.set_cell(
                col,
                0,
                Cell {
                    grapheme: CompactString::from(c.to_string()),
                    ..Cell::default()
                },
            );
        }
        grid.set_cursor_pos(2, 0);
        grid.take_dirty_rows(); // clear

        grid.insert_chars(2);

        // Expected: A B _ _ C D  (E F pushed off)
        assert_eq!(grid.cell(0, 0).grapheme.as_str(), "A");
        assert_eq!(grid.cell(1, 0).grapheme.as_str(), "B");
        assert_eq!(grid.cell(2, 0).grapheme.as_str(), " ");
        assert_eq!(grid.cell(3, 0).grapheme.as_str(), " ");
        assert_eq!(grid.cell(4, 0).grapheme.as_str(), "C");
        assert_eq!(grid.cell(5, 0).grapheme.as_str(), "D");
    }

    #[test]
    fn delete_chars_shifts_left() {
        let mut grid = Grid::new(6, 1);
        for col in 0..6u16 {
            let c = (b'A' + col as u8) as char;
            grid.set_cell(
                col,
                0,
                Cell {
                    grapheme: CompactString::from(c.to_string()),
                    ..Cell::default()
                },
            );
        }
        grid.set_cursor_pos(1, 0);
        grid.take_dirty_rows();

        grid.delete_chars(2);

        // Expected: A D E F _ _
        assert_eq!(grid.cell(0, 0).grapheme.as_str(), "A");
        assert_eq!(grid.cell(1, 0).grapheme.as_str(), "D");
        assert_eq!(grid.cell(2, 0).grapheme.as_str(), "E");
        assert_eq!(grid.cell(3, 0).grapheme.as_str(), "F");
        assert_eq!(grid.cell(4, 0).grapheme.as_str(), " ");
        assert_eq!(grid.cell(5, 0).grapheme.as_str(), " ");
    }

    #[test]
    fn boundary_cells() {
        let mut grid = Grid::new(80, 24);
        let cell = Cell {
            grapheme: CompactString::from("X"),
            ..Cell::default()
        };
        grid.set_cell(0, 0, cell.clone());
        grid.set_cell(79, 23, cell.clone());
        assert_eq!(grid.cell(0, 0), &cell);
        assert_eq!(grid.cell(79, 23), &cell);
    }

    #[test]
    fn cursor_bounds_clamped() {
        let mut grid = Grid::new(80, 24);
        grid.set_cursor_pos(200, 100);
        assert_eq!(grid.cursor().col, 79);
        assert_eq!(grid.cursor().row, 23);
    }

    #[test]
    fn clear_marks_all_dirty() {
        let mut grid = Grid::new(4, 4);
        grid.take_dirty_rows();

        grid.clear();

        let dirty = grid.take_dirty_rows();
        assert_eq!(dirty.len(), 4);
    }

    #[test]
    fn wide_char_spacer() {
        let mut grid = Grid::new(80, 24);
        let wide = Cell {
            grapheme: CompactString::from("\u{4E16}"), // CJK
            ..Cell::default()
        };
        let spacer = Cell {
            grapheme: CompactString::default(),
            flags: CellFlags::WIDE_SPACER,
            ..Cell::default()
        };
        grid.set_cell(0, 0, wide.clone());
        grid.set_cell(1, 0, spacer.clone());
        assert_eq!(grid.cell(0, 0), &wide);
        assert!(grid.cell(1, 0).flags.contains(CellFlags::WIDE_SPACER));
    }

    #[test]
    #[should_panic(expected = "grid dimensions must be nonzero")]
    fn new_zero_cols_panics() {
        Grid::new(0, 24);
    }

    #[test]
    #[should_panic(expected = "grid dimensions must be nonzero")]
    fn new_zero_rows_panics() {
        Grid::new(80, 0);
    }

    #[test]
    #[should_panic(expected = "grid dimensions must be nonzero")]
    fn resize_zero_panics() {
        let mut grid = Grid::new(80, 24);
        grid.resize(0, 10);
    }

    #[test]
    fn resize_clamps_cursor() {
        let mut grid = Grid::new(80, 24);
        grid.set_cursor_pos(79, 23);
        grid.resize(40, 12);
        assert_eq!(grid.cursor().col, 39);
        assert_eq!(grid.cursor().row, 11);
    }

    #[test]
    fn scroll_up_in_region_shifts_content() {
        let mut grid = Grid::new(4, 6);
        for row in 0..6u16 {
            let cell = Cell {
                grapheme: CompactString::from(format!("{row}")),
                ..Cell::default()
            };
            grid.set_cell(0, row, cell);
        }
        grid.take_dirty_rows();

        // Scroll rows 1..=4 up by 1.
        grid.scroll_up_in_region(1, 4, 1);

        assert_eq!(grid.cell(0, 0).grapheme.as_str(), "0"); // untouched
        assert_eq!(grid.cell(0, 1).grapheme.as_str(), "2"); // was row 2
        assert_eq!(grid.cell(0, 2).grapheme.as_str(), "3");
        assert_eq!(grid.cell(0, 3).grapheme.as_str(), "4");
        assert_eq!(grid.cell(0, 4), &Cell::default()); // cleared
        assert_eq!(grid.cell(0, 5).grapheme.as_str(), "5"); // untouched
    }

    #[test]
    fn scroll_down_in_region_shifts_content() {
        let mut grid = Grid::new(4, 6);
        for row in 0..6u16 {
            let cell = Cell {
                grapheme: CompactString::from(format!("{row}")),
                ..Cell::default()
            };
            grid.set_cell(0, row, cell);
        }
        grid.take_dirty_rows();

        // Scroll rows 1..=4 down by 2.
        grid.scroll_down_in_region(1, 4, 2);

        assert_eq!(grid.cell(0, 0).grapheme.as_str(), "0"); // untouched
        assert_eq!(grid.cell(0, 1), &Cell::default()); // cleared
        assert_eq!(grid.cell(0, 2), &Cell::default()); // cleared
        assert_eq!(grid.cell(0, 3).grapheme.as_str(), "1"); // was row 1
        assert_eq!(grid.cell(0, 4).grapheme.as_str(), "2"); // was row 2
        assert_eq!(grid.cell(0, 5).grapheme.as_str(), "5"); // untouched
    }

    #[test]
    fn scroll_region_n_exceeds_size_clears_region() {
        let mut grid = Grid::new(4, 4);
        for row in 0..4u16 {
            let cell = Cell {
                grapheme: CompactString::from("X"),
                ..Cell::default()
            };
            grid.set_cell(0, row, cell);
        }

        grid.scroll_up_in_region(1, 2, 100);

        assert_eq!(grid.cell(0, 0).grapheme.as_str(), "X"); // untouched
        assert_eq!(grid.cell(0, 1), &Cell::default()); // cleared
        assert_eq!(grid.cell(0, 2), &Cell::default()); // cleared
        assert_eq!(grid.cell(0, 3).grapheme.as_str(), "X"); // untouched
    }

    #[test]
    fn scroll_region_invalid_bounds_is_noop() {
        let mut grid = Grid::new(4, 4);
        let cell = Cell {
            grapheme: CompactString::from("A"),
            ..Cell::default()
        };
        grid.set_cell(0, 0, cell);
        grid.take_dirty_rows();

        // top > bottom: no-op
        grid.scroll_up_in_region(3, 1, 1);
        assert_eq!(grid.cell(0, 0).grapheme.as_str(), "A");
        assert!(grid.take_dirty_rows().is_empty());

        // bottom >= rows: no-op
        grid.scroll_down_in_region(0, 10, 1);
        assert_eq!(grid.cell(0, 0).grapheme.as_str(), "A");
    }
}
