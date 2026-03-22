use std::sync::Arc;

use crate::cell::CellFlags;
use crate::color::Color;
use crate::cursor::CursorState;
use crate::event::{Hyperlink, TerminalEvent};
use crate::grid::Grid;
use crate::mode::TerminalMode;
use crate::scrollback::Scrollback;
use crate::vte_handler::VteHandler;

/// Current SGR (Select Graphic Rendition) attributes applied to new cells.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Attrs {
    pub fg: Color,
    pub bg: Color,
    pub flags: CellFlags,
}

impl Default for Attrs {
    fn default() -> Self {
        Self {
            fg: Color::default(),
            bg: Color::Named(0),
            flags: CellFlags::empty(),
        }
    }
}

/// Saved state of the main screen while alternate screen is active.
pub(crate) struct AltScreenState {
    pub grid: Grid,
    pub cursor: CursorState,
    pub attrs: Attrs,
    pub saved_cursor: CursorState,
    pub viewport_offset: usize,
    pub scroll_top: u16,
    pub scroll_bottom: u16,
}

/// Internal terminal state operated on by [`VteHandler`].
///
/// Separated from [`Terminal`] so the vte parser and state can be
/// borrowed as disjoint fields in [`Terminal::process`].
pub(crate) struct TermState {
    pub grid: Grid,
    pub attrs: Attrs,
    pub modes: TerminalMode,
    pub saved_cursor: CursorState,
    pub scroll_top: u16,
    pub scroll_bottom: u16,
    pub pending_wrap: bool,
    pub tabs: Vec<bool>,
    pub scrollback: Scrollback,
    /// Saved main screen state when alternate screen is active.
    pub alt_screen: Option<AltScreenState>,
    /// Channel for emitting terminal events (OSC-sourced).
    pub event_tx: Option<tokio::sync::mpsc::Sender<TerminalEvent>>,
    /// Active hyperlink set by OSC 8 (applied to subsequently printed cells).
    pub current_hyperlink: Option<Arc<Hyperlink>>,
}

/// Terminal emulator state machine.
///
/// Owns the cell grid, vte parser, cursor, modes, and current SGR
/// attributes. The single entry point is [`Terminal::process`] which
/// feeds raw PTY bytes through the VTE state machine.
pub struct Terminal {
    parser: vte::Parser,
    pub(crate) state: TermState,
}

/// Build a tab-stop vector with stops every 8 columns.
fn init_tab_stops(cols: u16) -> Vec<bool> {
    let mut tabs = vec![false; cols as usize];
    for i in (0..cols as usize).step_by(8) {
        tabs[i] = true;
    }
    tabs
}

impl Terminal {
    /// Create a new terminal with the given dimensions.
    ///
    /// Initialises the grid, default modes (WRAPAROUND on), tab stops
    /// every 8 columns, and scroll region spanning the full screen.
    /// Default scrollback capacity (lines).
    const DEFAULT_SCROLLBACK: usize = 4000;

    pub fn new(cols: u16, rows: u16) -> Self {
        Self::with_scrollback(cols, rows, Self::DEFAULT_SCROLLBACK)
    }

    /// Create a new terminal with custom scrollback capacity.
    pub fn with_scrollback(cols: u16, rows: u16, scrollback_lines: usize) -> Self {
        Self {
            parser: vte::Parser::new(),
            state: TermState {
                grid: Grid::new(cols, rows),
                attrs: Attrs::default(),
                modes: TerminalMode::default(),
                saved_cursor: CursorState::default(),
                scroll_top: 0,
                scroll_bottom: rows.saturating_sub(1),
                pending_wrap: false,
                tabs: init_tab_stops(cols),
                scrollback: Scrollback::new(scrollback_lines),
                alt_screen: None,
                event_tx: None,
                current_hyperlink: None,
            },
        }
    }

    /// Set the event channel sender for terminal events (OSC-sourced).
    ///
    /// Events are sent via `try_send` so this never blocks terminal
    /// processing even when the receiver falls behind.
    pub fn set_event_sender(&mut self, tx: tokio::sync::mpsc::Sender<TerminalEvent>) {
        self.state.event_tx = Some(tx);
    }

    /// Create a terminal pre-wired with an event channel (capacity 256).
    ///
    /// Returns the terminal and the receiving end of the channel.
    pub fn with_event_channel(
        cols: u16,
        rows: u16,
    ) -> (Self, tokio::sync::mpsc::Receiver<TerminalEvent>) {
        let (tx, rx) = tokio::sync::mpsc::channel(256);
        let mut term = Self::new(cols, rows);
        term.state.event_tx = Some(tx);
        (term, rx)
    }

    /// Feed raw bytes from the PTY into the VTE parser.
    ///
    /// This is the **single entry point** for all terminal output
    /// processing. The parser dispatches structured operations to
    /// [`VteHandler`] which mutates the grid, cursor, and modes.
    pub fn process(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            let mut handler = VteHandler::new(&mut self.state);
            self.parser.advance(&mut handler, byte);
        }
    }

    /// Immutable reference to the cell grid.
    #[inline]
    pub fn grid(&self) -> &Grid {
        &self.state.grid
    }

    /// Mutable reference to the cell grid.
    #[inline]
    pub fn grid_mut(&mut self) -> &mut Grid {
        &mut self.state.grid
    }

    /// Current terminal mode flags.
    #[inline]
    pub fn modes(&self) -> TerminalMode {
        self.state.modes
    }

    /// Column count.
    #[inline]
    pub fn cols(&self) -> u16 {
        self.state.grid.cols()
    }

    /// Row count.
    #[inline]
    pub fn rows(&self) -> u16 {
        self.state.grid.rows()
    }

    /// Resize the terminal. Resets scroll region to the full screen,
    /// reinitialises tab stops, and clamps the cursor. Also resizes
    /// the alternate screen grid if active.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.state.grid.resize(cols, rows);
        self.state.scroll_top = 0;
        self.state.scroll_bottom = rows.saturating_sub(1);
        self.state.pending_wrap = false;
        self.state.tabs = init_tab_stops(cols);
        if let Some(ref mut alt) = self.state.alt_screen {
            alt.grid.resize(cols, rows);
            // Clamp saved cursors to new dimensions to prevent out-of-bounds
            // panics when exiting alt screen after a resize.
            let max_col = cols.saturating_sub(1) as usize;
            let max_row = rows.saturating_sub(1) as usize;
            alt.cursor.col = alt.cursor.col.min(max_col);
            alt.cursor.row = alt.cursor.row.min(max_row);
            alt.saved_cursor.col = alt.saved_cursor.col.min(max_col);
            alt.saved_cursor.row = alt.saved_cursor.row.min(max_row);
            alt.scroll_top = 0;
            alt.scroll_bottom = rows.saturating_sub(1);
        }
    }

    /// Whether the terminal is currently in alternate screen mode.
    #[inline]
    pub fn is_alt_screen(&self) -> bool {
        self.state.alt_screen.is_some()
    }

    /// Immutable reference to the scrollback buffer.
    #[inline]
    pub fn scrollback(&self) -> &Scrollback {
        &self.state.scrollback
    }

    /// Simultaneous mutable grid and immutable scrollback references.
    ///
    /// Needed for [`TerminalRenderer::update`] which requires both
    /// `&mut Grid` and `&Scrollback` in the same call — impossible with
    /// separate `grid_mut()` / `scrollback()` due to borrow rules.
    #[inline]
    pub fn grid_and_scrollback(&mut self) -> (&mut Grid, &Scrollback) {
        (&mut self.state.grid, &self.state.scrollback)
    }

    /// Current viewport offset (0 = live, positive = scrolled up).
    #[inline]
    pub fn viewport_offset(&self) -> usize {
        self.state.scrollback.viewport_offset()
    }

    /// Scroll the viewport up by `n` lines.
    pub fn scroll_viewport_up(&mut self, n: usize) {
        let current = self.state.scrollback.viewport_offset();
        self.state.scrollback.set_viewport_offset(current + n);
    }

    /// Scroll the viewport down by `n` lines (towards live terminal).
    pub fn scroll_viewport_down(&mut self, n: usize) {
        let current = self.state.scrollback.viewport_offset();
        self.state
            .scrollback
            .set_viewport_offset(current.saturating_sub(n));
    }

    /// Reset viewport to bottom (live terminal).
    pub fn reset_viewport(&mut self) {
        self.state.scrollback.reset_viewport();
    }

    /// Read text from the terminal, combining scrollback and visible grid.
    ///
    /// Positive indices start from the first scrollback row (0).
    /// The visible grid rows follow after all scrollback rows.
    /// Negative indices count from the end (last visible row).
    pub fn read_text(&self, start: isize, end: isize) -> String {
        let sb_len = self.state.scrollback.len() as isize;
        let grid_rows = self.state.grid.rows() as isize;
        let total = sb_len + grid_rows;

        let resolve = |idx: isize| -> usize {
            if idx < 0 {
                (total + idx).max(0) as usize
            } else {
                idx as usize
            }
        };

        let s = resolve(start);
        let e = resolve(end).min(total as usize);

        if s >= e {
            return String::new();
        }

        // Pre-allocate: estimate cols chars per row + 1 newline.
        let cols = self.state.grid.cols() as usize;
        let mut result = String::with_capacity((e - s) * (cols + 1));
        for i in s..e {
            if i > s {
                result.push('\n');
            }
            if (i as isize) < sb_len {
                // Row from scrollback.
                if let Some(row) = self.state.scrollback.get_row(i) {
                    for cell in row {
                        result.push_str(cell.grapheme.as_str());
                    }
                }
            } else {
                // Row from visible grid.
                let grid_row = (i as isize - sb_len) as u16;
                if grid_row < self.state.grid.rows() {
                    for col in 0..self.state.grid.cols() {
                        result.push_str(self.state.grid.cell(col, grid_row).grapheme.as_str());
                    }
                }
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_terminal_defaults() {
        let term = Terminal::new(80, 24);
        assert_eq!(term.cols(), 80);
        assert_eq!(term.rows(), 24);
        assert!(term.modes().contains(TerminalMode::WRAPAROUND));
        assert_eq!(term.state.scroll_top, 0);
        assert_eq!(term.state.scroll_bottom, 23);
    }

    #[test]
    fn tab_stops_every_8_cols() {
        let term = Terminal::new(80, 24);
        assert!(term.state.tabs[0]);
        assert!(term.state.tabs[8]);
        assert!(term.state.tabs[16]);
        assert!(!term.state.tabs[1]);
        assert!(!term.state.tabs[7]);
    }

    #[test]
    fn resize_resets_scroll_region() {
        let mut term = Terminal::new(80, 24);
        term.state.scroll_top = 5;
        term.state.scroll_bottom = 20;

        term.resize(120, 40);

        assert_eq!(term.cols(), 120);
        assert_eq!(term.rows(), 40);
        assert_eq!(term.state.scroll_top, 0);
        assert_eq!(term.state.scroll_bottom, 39);
    }

    #[test]
    fn process_empty_bytes_is_noop() {
        let mut term = Terminal::new(80, 24);
        term.process(b"");
        assert_eq!(term.grid().cursor().col, 0);
        assert_eq!(term.grid().cursor().row, 0);
    }

    #[test]
    fn process_plain_text() {
        let mut term = Terminal::new(80, 24);
        term.process(b"Hello");
        assert_eq!(term.grid().cell(0, 0).grapheme.as_str(), "H");
        assert_eq!(term.grid().cell(1, 0).grapheme.as_str(), "e");
        assert_eq!(term.grid().cell(2, 0).grapheme.as_str(), "l");
        assert_eq!(term.grid().cell(3, 0).grapheme.as_str(), "l");
        assert_eq!(term.grid().cell(4, 0).grapheme.as_str(), "o");
        assert_eq!(term.grid().cursor().col, 5);
    }

    #[test]
    fn process_sgr_red_text() {
        let mut term = Terminal::new(80, 24);
        // ESC[31m = red foreground, then "hi", then ESC[0m = reset
        term.process(b"\x1b[31mhi\x1b[0m");
        assert_eq!(term.grid().cell(0, 0).fg, Color::Named(1));
        assert_eq!(term.grid().cell(1, 0).fg, Color::Named(1));
        // After reset, attrs should be default
        assert_eq!(term.state.attrs.fg, Color::default());
    }

    #[test]
    fn process_cursor_movement() {
        let mut term = Terminal::new(80, 24);
        // CUP row=5, col=10 (1-based: ESC[5;10H)
        term.process(b"\x1b[5;10H");
        assert_eq!(term.grid().cursor().row, 4);
        assert_eq!(term.grid().cursor().col, 9);
    }

    #[test]
    fn process_erase_display() {
        let mut term = Terminal::new(80, 24);
        term.process(b"ABCDE");
        // ESC[2J = clear entire screen
        term.process(b"\x1b[2J");
        for col in 0..5u16 {
            assert_eq!(term.grid().cell(col, 0).grapheme.as_str(), " ");
        }
    }

    #[test]
    fn process_sgr_truecolor() {
        let mut term = Terminal::new(80, 24);
        // ESC[38;2;255;128;0m = truecolor fg
        term.process(b"\x1b[38;2;255;128;0mX");
        assert_eq!(term.grid().cell(0, 0).fg, Color::Rgb(255, 128, 0));
    }

    #[test]
    fn process_cr_lf() {
        let mut term = Terminal::new(80, 24);
        term.process(b"ABC\r\nDEF");
        assert_eq!(term.grid().cell(0, 0).grapheme.as_str(), "A");
        assert_eq!(term.grid().cell(0, 1).grapheme.as_str(), "D");
        assert_eq!(term.grid().cursor().row, 1);
        assert_eq!(term.grid().cursor().col, 3);
    }

    #[test]
    fn process_backspace() {
        let mut term = Terminal::new(80, 24);
        term.process(b"AB\x08C");
        // BS moves cursor back, then 'C' overwrites 'B'
        assert_eq!(term.grid().cell(0, 0).grapheme.as_str(), "A");
        assert_eq!(term.grid().cell(1, 0).grapheme.as_str(), "C");
    }

    #[test]
    fn process_decset_bracketed_paste() {
        let mut term = Terminal::new(80, 24);
        assert!(!term.modes().contains(TerminalMode::BRACKETED_PASTE));
        // DECSET 2004
        term.process(b"\x1b[?2004h");
        assert!(term.modes().contains(TerminalMode::BRACKETED_PASTE));
        // DECRST 2004
        term.process(b"\x1b[?2004l");
        assert!(!term.modes().contains(TerminalMode::BRACKETED_PASTE));
    }

    #[test]
    fn malformed_sequence_does_not_panic() {
        let mut term = Terminal::new(80, 24);
        // Various malformed sequences — none should panic.
        term.process(b"\x1b[999999999m");
        term.process(b"\x1b[;;;;;m");
        term.process(b"\x1b[?99999h");
        // Incomplete CSI: \x1b[ leaves parser waiting for final byte.
        // The next byte 'O' is consumed as the CSI action, so only 'K'
        // is printed as a regular character.
        term.process(b"\x1b[");
        term.process(b"OK");
        assert_eq!(term.grid().cell(0, 0).grapheme.as_str(), "K");
    }

    #[test]
    fn scrollback_captures_on_linefeed_at_bottom() {
        let mut term = Terminal::new(4, 3);
        // Fill all 3 rows.
        term.process(b"AAAA\r\nBBBB\r\nCCCC");
        assert_eq!(term.scrollback().len(), 0);

        // Linefeed at bottom of screen should push row 0 to scrollback.
        term.process(b"\r\n");
        assert_eq!(term.scrollback().len(), 1);
        assert_eq!(
            term.scrollback().get_row(0).unwrap()[0].grapheme.as_str(),
            "A"
        );
    }

    #[test]
    fn scrollback_max_enforced() {
        let mut term = Terminal::with_scrollback(4, 2, 3);
        // Push 5 rows through — only last 3 should be in scrollback.
        for c in b"AAAA\nBBBB\nCCCC\nDDDD\nEEEE" {
            term.process(&[*c]);
        }
        assert!(term.scrollback().len() <= 3);
    }

    #[test]
    fn alt_screen_enter_exit() {
        let mut term = Terminal::new(4, 3);
        term.process(b"MAIN");
        assert!(!term.is_alt_screen());

        // Enter alt screen (DECSET 1049).
        term.process(b"\x1b[?1049h");
        assert!(term.is_alt_screen());
        // Grid should be fresh.
        assert_eq!(term.grid().cell(0, 0).grapheme.as_str(), " ");
        // Write something on alt screen.
        term.process(b"ALT!");

        // Exit alt screen (DECRST 1049).
        term.process(b"\x1b[?1049l");
        assert!(!term.is_alt_screen());
        // Original grid restored.
        assert_eq!(term.grid().cell(0, 0).grapheme.as_str(), "M");
    }

    #[test]
    fn alt_screen_no_scrollback() {
        let mut term = Terminal::new(4, 2);
        term.process(b"\x1b[?1049h");
        let before = term.scrollback().len();
        // Fill and scroll on alt screen.
        term.process(b"AAAA\nBBBB\nCCCC");
        // Scrollback should not grow on alt screen.
        assert_eq!(term.scrollback().len(), before);
        term.process(b"\x1b[?1049l");
    }

    #[test]
    fn exit_alt_screen_without_enter_is_noop() {
        let mut term = Terminal::new(80, 24);
        term.process(b"Hello");
        term.process(b"\x1b[?1049l");
        assert!(!term.is_alt_screen());
        assert_eq!(term.grid().cell(0, 0).grapheme.as_str(), "H");
    }

    #[test]
    fn viewport_offset_operations() {
        let mut term = Terminal::new(4, 2);
        // Push some rows into scrollback.
        term.process(b"AA\nBB\nCC\nDD\nEE");
        assert!(term.scrollback().len() > 0);

        term.scroll_viewport_up(2);
        assert_eq!(term.viewport_offset(), 2);

        term.scroll_viewport_down(1);
        assert_eq!(term.viewport_offset(), 1);

        term.reset_viewport();
        assert_eq!(term.viewport_offset(), 0);
    }

    #[test]
    fn read_text_includes_scrollback_and_grid() {
        let mut term = Terminal::with_scrollback(3, 2, 100);
        // Push some content that scrolls.
        term.process(b"AAA\nBBB\nCCC\nDDD");
        // Scrollback should have some rows, grid has 2 visible rows.
        let sb_len = term.scrollback().len();
        assert!(sb_len > 0);

        // read_text(-1, total) should return the last visible row.
        let last = term.read_text(-1, (sb_len + 2) as isize);
        assert!(!last.is_empty());
    }

    #[test]
    fn resize_also_resizes_alt_grid() {
        let mut term = Terminal::new(80, 24);
        term.process(b"\x1b[?1049h");
        term.resize(40, 12);
        assert_eq!(term.cols(), 40);
        assert_eq!(term.rows(), 12);
        // Saved main grid should also be resized.
        assert!(term.state.alt_screen.is_some());
        assert_eq!(term.state.alt_screen.as_ref().unwrap().grid.cols(), 40);
    }
}
