use crate::cell::CellFlags;
use crate::color::Color;
use crate::cursor::CursorState;
use crate::grid::Grid;
use crate::mode::TerminalMode;
use crate::vte_handler::VteHandler;

/// Current SGR (Select Graphic Rendition) attributes applied to new cells.
#[derive(Debug, Clone)]
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
    pub fn new(cols: u16, rows: u16) -> Self {
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
            },
        }
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
    /// reinitialises tab stops, and clamps the cursor.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.state.grid.resize(cols, rows);
        self.state.scroll_top = 0;
        self.state.scroll_bottom = rows.saturating_sub(1);
        self.state.pending_wrap = false;
        self.state.tabs = init_tab_stops(cols);
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
}
