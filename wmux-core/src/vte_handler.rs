use std::sync::Arc;

use compact_str::CompactString;
use unicode_width::UnicodeWidthChar;

use crate::cell::{Cell, CellFlags};
use crate::color::Color;
use crate::event::{self, Hyperlink, PromptMark, TerminalEvent};
use crate::grid::Grid;
use crate::mode::TerminalMode;
use crate::terminal::{AltScreenState, TermState};

/// VTE handler that bridges the vte parser to terminal state.
///
/// Implements [`vte::Perform`] by mutating the terminal's grid, cursor,
/// modes, and current SGR attributes. Created transiently inside
/// [`Terminal::process`] with a mutable borrow of [`TermState`].
pub(crate) struct VteHandler<'a> {
    state: &'a mut TermState,
}

impl<'a> VteHandler<'a> {
    pub fn new(state: &'a mut TermState) -> Self {
        Self { state }
    }

    // ── Helpers ──────────────────────────────────────────────────────

    /// Perform a linefeed: move cursor down or scroll if at the bottom
    /// of the scroll region.
    fn linefeed(&mut self) {
        let row = self.state.grid.cursor().row as u16;
        if row == self.state.scroll_bottom {
            self.capture_rows_to_scrollback(1);
            self.state
                .grid
                .scroll_up_in_region(self.state.scroll_top, self.state.scroll_bottom, 1);
        } else if row < self.state.grid.rows() - 1 {
            self.state.grid.cursor_mut().row += 1;
        }
    }

    /// Capture the top `n` rows of the scroll region into scrollback,
    /// but only when scrolling the full screen (scroll_top == 0) and
    /// not on the alternate screen.
    fn capture_rows_to_scrollback(&mut self, n: u16) {
        if self.state.scroll_top != 0 || self.state.alt_screen.is_some() {
            return;
        }
        let capture = n.min(self.state.grid.rows());
        for row_idx in 0..capture {
            let evicted = self.state.grid.extract_row(row_idx);
            self.state.scrollback.push_row(evicted);
        }
        self.state.scrollback.reset_viewport();
    }

    /// Build a blank cell using the current background color (for erase
    /// operations).
    #[inline]
    fn erase_cell(&self) -> Cell {
        Cell {
            grapheme: CompactString::const_new(" "),
            fg: self.state.attrs.fg,
            bg: self.state.attrs.bg,
            flags: CellFlags::empty(),
            hyperlink: None,
        }
    }

    /// Extract a single CSI parameter with a default value.
    #[inline]
    fn param(params: &vte::Params, idx: usize, default: u16) -> u16 {
        params
            .iter()
            .nth(idx)
            .and_then(|p| p.first().copied())
            .map(|v| if v == 0 { default } else { v })
            .unwrap_or(default)
    }

    // ── CSI dispatchers ─────────────────────────────────────────────

    fn csi_cursor_up(&mut self, n: u16) {
        let top = self.state.scroll_top as usize;
        let bottom = self.state.scroll_bottom as usize;
        let cursor = self.state.grid.cursor_mut();
        // If inside scroll region, clamp to scroll_top; otherwise clamp to 0.
        let floor = if cursor.row >= top && cursor.row <= bottom {
            top
        } else {
            0
        };
        cursor.row = cursor.row.saturating_sub(n as usize).max(floor);
        self.state.pending_wrap = false;
    }

    fn csi_cursor_down(&mut self, n: u16) {
        let top = self.state.scroll_top as usize;
        let bottom = self.state.scroll_bottom as usize;
        let max = self.state.grid.rows().saturating_sub(1) as usize;
        let cursor = self.state.grid.cursor_mut();
        // If inside scroll region, clamp to scroll_bottom; otherwise clamp to last row.
        let ceiling = if cursor.row >= top && cursor.row <= bottom {
            bottom
        } else {
            max
        };
        cursor.row = (cursor.row + n as usize).min(ceiling);
        self.state.pending_wrap = false;
    }

    fn csi_cursor_forward(&mut self, n: u16) {
        let max = self.state.grid.cols().saturating_sub(1) as usize;
        let cursor = self.state.grid.cursor_mut();
        cursor.col = (cursor.col + n as usize).min(max);
        self.state.pending_wrap = false;
    }

    fn csi_cursor_back(&mut self, n: u16) {
        let cursor = self.state.grid.cursor_mut();
        cursor.col = cursor.col.saturating_sub(n as usize);
        self.state.pending_wrap = false;
    }

    fn csi_cursor_position(&mut self, row: u16, col: u16) {
        let max_col = self.state.grid.cols().saturating_sub(1);
        let origin = self.state.modes.contains(TerminalMode::ORIGIN);
        let base_row = row.saturating_sub(1);
        let (abs_row, max_row) = if origin {
            (base_row + self.state.scroll_top, self.state.scroll_bottom)
        } else {
            (base_row, self.state.grid.rows().saturating_sub(1))
        };
        let cursor = self.state.grid.cursor_mut();
        cursor.row = abs_row.min(max_row) as usize;
        cursor.col = col.saturating_sub(1).min(max_col) as usize;
        self.state.pending_wrap = false;
    }

    fn csi_cursor_horizontal_absolute(&mut self, col: u16) {
        let max_col = self.state.grid.cols().saturating_sub(1);
        self.state.grid.cursor_mut().col = col.saturating_sub(1).min(max_col) as usize;
        self.state.pending_wrap = false;
    }

    fn csi_cursor_vertical_absolute(&mut self, row: u16) {
        let origin = self.state.modes.contains(TerminalMode::ORIGIN);
        let base_row = row.saturating_sub(1);
        let (abs_row, max_row) = if origin {
            (base_row + self.state.scroll_top, self.state.scroll_bottom)
        } else {
            (base_row, self.state.grid.rows().saturating_sub(1))
        };
        self.state.grid.cursor_mut().row = abs_row.min(max_row) as usize;
        self.state.pending_wrap = false;
    }

    fn csi_erase_display(&mut self, mode: u16) {
        let cols = self.state.grid.cols();
        let rows = self.state.grid.rows();
        let cursor_row = self.state.grid.cursor().row as u16;
        let cursor_col = self.state.grid.cursor().col as u16;
        let blank = self.erase_cell();

        match mode {
            // Cursor to end of screen.
            0 => {
                self.state
                    .grid
                    .fill_cells(cursor_col, cols, cursor_row, &blank);
                for row in cursor_row + 1..rows {
                    self.state.grid.fill_cells(0, cols, row, &blank);
                }
            }
            // Start of screen to cursor.
            1 => {
                for row in 0..cursor_row {
                    self.state.grid.fill_cells(0, cols, row, &blank);
                }
                self.state
                    .grid
                    .fill_cells(0, cursor_col + 1, cursor_row, &blank);
            }
            // Entire screen.
            2 | 3 => {
                for row in 0..rows {
                    self.state.grid.fill_cells(0, cols, row, &blank);
                }
            }
            _ => {}
        }
    }

    fn csi_erase_line(&mut self, mode: u16) {
        let cols = self.state.grid.cols();
        let cursor_row = self.state.grid.cursor().row as u16;
        let cursor_col = self.state.grid.cursor().col as u16;
        let blank = self.erase_cell();

        match mode {
            // Cursor to end of line.
            0 => {
                self.state
                    .grid
                    .fill_cells(cursor_col, cols, cursor_row, &blank);
            }
            // Start of line to cursor.
            1 => {
                self.state
                    .grid
                    .fill_cells(0, cursor_col + 1, cursor_row, &blank);
            }
            // Entire line.
            2 => {
                self.state.grid.fill_cells(0, cols, cursor_row, &blank);
            }
            _ => {}
        }
    }

    fn csi_insert_lines(&mut self, n: u16) {
        let cursor_row = self.state.grid.cursor().row as u16;
        if cursor_row >= self.state.scroll_top && cursor_row <= self.state.scroll_bottom {
            self.state
                .grid
                .scroll_down_in_region(cursor_row, self.state.scroll_bottom, n);
        }
    }

    fn csi_delete_lines(&mut self, n: u16) {
        let cursor_row = self.state.grid.cursor().row as u16;
        if cursor_row >= self.state.scroll_top && cursor_row <= self.state.scroll_bottom {
            self.state
                .grid
                .scroll_up_in_region(cursor_row, self.state.scroll_bottom, n);
        }
    }

    fn csi_set_scroll_region(&mut self, params: &vte::Params) {
        let rows = self.state.grid.rows();
        let top = Self::param(params, 0, 1)
            .saturating_sub(1)
            .min(rows.saturating_sub(1));
        let bottom = Self::param(params, 1, rows)
            .saturating_sub(1)
            .min(rows.saturating_sub(1));

        if top < bottom {
            self.state.scroll_top = top;
            self.state.scroll_bottom = bottom;
        }

        // Home cursor — relative to scroll region if ORIGIN mode active.
        let home_row = if self.state.modes.contains(TerminalMode::ORIGIN) {
            self.state.scroll_top as usize
        } else {
            0
        };
        let cursor = self.state.grid.cursor_mut();
        cursor.row = home_row;
        cursor.col = 0;
        self.state.pending_wrap = false;
    }

    fn csi_mode_set(&mut self, params: &vte::Params) {
        for param in params.iter() {
            match param {
                [1] => self.state.modes.insert(TerminalMode::APPLICATION_CURSOR),
                [6] => self.state.modes.insert(TerminalMode::ORIGIN),
                [7] => self.state.modes.insert(TerminalMode::WRAPAROUND),
                [25] => self.state.grid.cursor_mut().visible = true,
                [47 | 1047] => self.enter_alt_screen(false),
                [1049] => self.enter_alt_screen(true),
                [1000 | 1002 | 1003] => {
                    self.state.modes.insert(TerminalMode::MOUSE_REPORTING);
                }
                [2004] => self.state.modes.insert(TerminalMode::BRACKETED_PASTE),
                _ => {}
            }
        }
    }

    fn csi_mode_reset(&mut self, params: &vte::Params) {
        for param in params.iter() {
            match param {
                [1] => self.state.modes.remove(TerminalMode::APPLICATION_CURSOR),
                [6] => self.state.modes.remove(TerminalMode::ORIGIN),
                [7] => self.state.modes.remove(TerminalMode::WRAPAROUND),
                [25] => self.state.grid.cursor_mut().visible = false,
                [47 | 1047] => self.exit_alt_screen(false),
                [1049] => self.exit_alt_screen(true),
                [1000 | 1002 | 1003] => {
                    self.state.modes.remove(TerminalMode::MOUSE_REPORTING);
                }
                [2004] => self.state.modes.remove(TerminalMode::BRACKETED_PASTE),
                _ => {}
            }
        }
    }

    /// Enter alternate screen buffer.
    ///
    /// Saves the current main grid, cursor, attrs, and scrollback viewport.
    /// Replaces the grid with a fresh one. If `save_cursor` is true
    /// (DECSET 1049), also saves/resets cursor position.
    fn enter_alt_screen(&mut self, save_cursor: bool) {
        // Already in alt screen — no-op.
        if self.state.alt_screen.is_some() {
            return;
        }

        let cols = self.state.grid.cols();
        let rows = self.state.grid.rows();
        let cursor = *self.state.grid.cursor();

        let saved = AltScreenState {
            grid: std::mem::replace(&mut self.state.grid, Grid::new(cols, rows)),
            cursor,
            attrs: self.state.attrs,
            saved_cursor: self.state.saved_cursor,
            viewport_offset: self.state.scrollback.viewport_offset(),
            scroll_top: self.state.scroll_top,
            scroll_bottom: self.state.scroll_bottom,
        };
        self.state.alt_screen = Some(saved);

        // Reset state for alt screen.
        if save_cursor {
            self.state.saved_cursor = cursor;
        }
        self.state.grid.set_cursor_pos(0, 0);
        self.state.scroll_top = 0;
        self.state.scroll_bottom = rows.saturating_sub(1);
        self.state.pending_wrap = false;
        self.state.scrollback.reset_viewport();
    }

    /// Exit alternate screen buffer.
    ///
    /// Restores the saved main grid, cursor, attrs, and viewport.
    /// Discards the alternate screen grid. If `restore_cursor` is true
    /// (DECRST 1049), also restores cursor position.
    fn exit_alt_screen(&mut self, restore_cursor: bool) {
        let Some(saved) = self.state.alt_screen.take() else {
            return;
        };

        self.state.grid = saved.grid;
        self.state.attrs = saved.attrs;
        self.state.saved_cursor = saved.saved_cursor;
        self.state.scroll_top = saved.scroll_top;
        self.state.scroll_bottom = saved.scroll_bottom;
        self.state
            .scrollback
            .set_viewport_offset(saved.viewport_offset);

        if restore_cursor {
            let c = saved.cursor;
            *self.state.grid.cursor_mut() = c;
        }
        self.state.pending_wrap = false;
    }

    // ── SGR parsing ─────────────────────────────────────────────────

    fn sgr_dispatch(&mut self, params: &vte::Params) {
        let mut iter = params.iter();

        // Handle empty params (ESC[m) as reset.
        if params.is_empty() {
            self.sgr_reset();
            return;
        }

        while let Some(param) = iter.next() {
            match param {
                // Colon-separated truecolor: 38:2:_:R:G:B or 38:2:R:G:B
                [38, 2, r, g, b] => {
                    self.state.attrs.fg = Color::Rgb(*r as u8, *g as u8, *b as u8);
                }
                [38, 2, _, r, g, b] => {
                    self.state.attrs.fg = Color::Rgb(*r as u8, *g as u8, *b as u8);
                }
                // Colon-separated indexed: 38:5:N
                [38, 5, idx] => {
                    self.state.attrs.fg = Color::Indexed(*idx as u8);
                }
                // Colon-separated truecolor bg: 48:2:_:R:G:B or 48:2:R:G:B
                [48, 2, r, g, b] => {
                    self.state.attrs.bg = Color::Rgb(*r as u8, *g as u8, *b as u8);
                }
                [48, 2, _, r, g, b] => {
                    self.state.attrs.bg = Color::Rgb(*r as u8, *g as u8, *b as u8);
                }
                // Colon-separated indexed bg: 48:5:N
                [48, 5, idx] => {
                    self.state.attrs.bg = Color::Indexed(*idx as u8);
                }
                // Semicolon-separated: each param is a single-element slice.
                [n] => self.sgr_single(&mut iter, *n),
                _ => {}
            }
        }
    }

    /// Handle a single SGR parameter value. For extended colors (38/48),
    /// consumes additional params from the iterator.
    fn sgr_single(&mut self, iter: &mut vte::ParamsIter<'_>, n: u16) {
        match n {
            0 => self.sgr_reset(),
            1 => self.state.attrs.flags.insert(CellFlags::BOLD),
            2 => self.state.attrs.flags.insert(CellFlags::DIM),
            3 => self.state.attrs.flags.insert(CellFlags::ITALIC),
            4 => self.state.attrs.flags.insert(CellFlags::UNDERLINE),
            7 => self.state.attrs.flags.insert(CellFlags::INVERSE),
            8 => self.state.attrs.flags.insert(CellFlags::HIDDEN),
            9 => self.state.attrs.flags.insert(CellFlags::STRIKETHROUGH),
            21 => self.state.attrs.flags.remove(CellFlags::BOLD),
            22 => {
                self.state.attrs.flags.remove(CellFlags::BOLD);
                self.state.attrs.flags.remove(CellFlags::DIM);
            }
            23 => self.state.attrs.flags.remove(CellFlags::ITALIC),
            24 => self.state.attrs.flags.remove(CellFlags::UNDERLINE),
            27 => self.state.attrs.flags.remove(CellFlags::INVERSE),
            28 => self.state.attrs.flags.remove(CellFlags::HIDDEN),
            29 => self.state.attrs.flags.remove(CellFlags::STRIKETHROUGH),
            // Standard foreground colors (30-37).
            30..=37 => self.state.attrs.fg = Color::Named((n - 30) as u8),
            // Extended foreground: 38;5;N or 38;2;R;G;B
            38 => self.parse_extended_color(iter, true),
            // Default foreground.
            39 => self.state.attrs.fg = Color::default(),
            // Standard background colors (40-47).
            40..=47 => self.state.attrs.bg = Color::Named((n - 40) as u8),
            // Extended background: 48;5;N or 48;2;R;G;B
            48 => self.parse_extended_color(iter, false),
            // Default background.
            49 => self.state.attrs.bg = Color::Named(0),
            // Bright foreground colors (90-97).
            90..=97 => self.state.attrs.fg = Color::Named((n - 90 + 8) as u8),
            // Bright background colors (100-107).
            100..=107 => self.state.attrs.bg = Color::Named((n - 100 + 8) as u8),
            _ => {}
        }
    }

    fn sgr_reset(&mut self) {
        self.state.attrs.fg = Color::default();
        self.state.attrs.bg = Color::Named(0);
        self.state.attrs.flags = CellFlags::empty();
    }

    /// Parse extended color from semicolon-separated params (38;5;N or
    /// 38;2;R;G;B). `is_fg` selects foreground vs background.
    fn parse_extended_color(&mut self, iter: &mut vte::ParamsIter<'_>, is_fg: bool) {
        match iter.next().and_then(|p| p.first().copied()) {
            // 256-color: 38;5;N
            Some(5) => {
                if let Some(idx) = iter.next().and_then(|p| p.first().copied()) {
                    let color = Color::Indexed(idx as u8);
                    if is_fg {
                        self.state.attrs.fg = color;
                    } else {
                        self.state.attrs.bg = color;
                    }
                }
            }
            // Truecolor: 38;2;R;G;B
            Some(2) => {
                let r = iter.next().and_then(|p| p.first().copied()).unwrap_or(0);
                let g = iter.next().and_then(|p| p.first().copied()).unwrap_or(0);
                let b = iter.next().and_then(|p| p.first().copied()).unwrap_or(0);
                let color = Color::Rgb(r as u8, g as u8, b as u8);
                if is_fg {
                    self.state.attrs.fg = color;
                } else {
                    self.state.attrs.bg = color;
                }
            }
            _ => {}
        }
    }

    // ── OSC helpers ─────────────────────────────────────────────────

    /// Extract an OSC param as a UTF-8 string.
    #[inline]
    fn utf8_param<'p>(params: &[&'p [u8]], idx: usize) -> Option<&'p str> {
        params.get(idx).and_then(|b| std::str::from_utf8(b).ok())
    }

    /// Try-send a terminal event. Drops the event if the channel is full.
    fn emit_event(&self, event: TerminalEvent) {
        if let Some(tx) = &self.state.event_tx {
            if tx.try_send(event).is_err() {
                tracing::warn!("terminal event channel full, dropping event");
            }
        }
    }

    /// OSC 7 — current working directory.
    ///
    /// Format: `\x1b]7;file://host/path\x07`
    /// OSC 10/11 — foreground/background color query.
    ///
    /// When param[1] is "?" the terminal responds with the current color
    /// in xterm format: `\x1b]N;rgb:RRRR/GGGG/BBBB\x1b\\`
    fn osc_color_query(&mut self, params: &[&[u8]], is_foreground: bool) {
        let Some(query) = Self::utf8_param(params, 1) else {
            return;
        };
        if query != "?" {
            return; // Setting colors is not supported — only queries.
        }
        let (r, g, b) = if is_foreground {
            self.state.theme_fg
        } else {
            self.state.theme_bg
        };
        // xterm reports 16-bit color channels: duplicate the 8-bit value (e.g. 0xe2 → 0xe2e2).
        let osc_num = if is_foreground { 10 } else { 11 };
        let response =
            format!("\x1b]{osc_num};rgb:{r:02x}{r:02x}/{g:02x}{g:02x}/{b:02x}{b:02x}\x1b\\");
        self.emit_event(TerminalEvent::PtyWrite(response.into_bytes()));
    }

    fn osc_cwd(&mut self, params: &[&[u8]]) {
        let Some(uri_str) = Self::utf8_param(params, 1) else {
            return;
        };
        if let Some(path) = event::parse_file_uri(uri_str) {
            self.emit_event(TerminalEvent::CwdChanged(path));
        }
    }

    /// OSC 8 — hyperlink.
    ///
    /// Format: `\x1b]8;params;uri\x07` (empty URI closes the link).
    fn osc_hyperlink(&mut self, params: &[&[u8]]) {
        let Some(uri) = Self::utf8_param(params, 2) else {
            // Malformed or missing URI — close any active link.
            self.state.current_hyperlink = None;
            return;
        };

        if uri.is_empty() {
            // Empty URI closes the hyperlink.
            self.state.current_hyperlink = None;
            return;
        }

        // Parse optional `id=value` from the params field.
        let id = Self::utf8_param(params, 1).and_then(|p| {
            p.split(':')
                .find_map(|kv| kv.strip_prefix("id=").map(String::from))
        });

        self.state.current_hyperlink = Some(Arc::new(Hyperlink {
            id,
            uri: uri.to_string(),
        }));
    }

    /// OSC 9 — iTerm2 notification.
    ///
    /// Format: `\x1b]9;body\x07`
    fn osc_notification_iterm(&mut self, params: &[&[u8]]) {
        let Some(body) = Self::utf8_param(params, 1) else {
            return;
        };
        self.emit_event(TerminalEvent::Notification {
            title: None,
            body: body.to_string(),
            id: None,
        });
    }

    /// OSC 99 — kitty notification.
    ///
    /// Format: `\x1b]99;i=id:d=0;title\x07` or `\x1b]99;i=id:d=1;body\x07`
    /// vte splits on `;` so: params = [b"99", b"i=id:d=0", b"title"]
    fn osc_notification_kitty(&mut self, params: &[&[u8]]) {
        let Some(kvs) = Self::utf8_param(params, 1) else {
            return;
        };
        let Some(payload) = Self::utf8_param(params, 2) else {
            return;
        };

        let mut id: Option<String> = None;
        let mut is_body = false;
        for kv in kvs.split(':') {
            if let Some(val) = kv.strip_prefix("i=") {
                id = Some(val.to_string());
            } else if kv == "d=1" {
                is_body = true;
            }
        }

        // Kitty uses d=0 (default) for title, d=1 for body.
        let (title, body) = if is_body {
            (None, payload.to_string())
        } else {
            (Some(payload.to_string()), String::new())
        };

        self.emit_event(TerminalEvent::Notification { title, body, id });
    }

    /// OSC 133 — prompt mark.
    ///
    /// Format: `\x1b]133;X\x07` where X is A, B, C, or D.
    fn osc_prompt_mark(&mut self, params: &[&[u8]]) {
        if params.len() < 2 || params[1].is_empty() {
            return;
        }

        let mark = match params[1][0] {
            b'A' => PromptMark::PromptStart,
            b'B' => PromptMark::CommandStart,
            b'C' => PromptMark::OutputStart,
            b'D' => PromptMark::CommandEnd,
            _ => return,
        };

        self.emit_event(TerminalEvent::PromptMark(mark));
    }

    /// OSC 777 — rxvt notification.
    ///
    /// Format: `\x1b]777;notify;title;body\x07`
    /// vte splits on `;`: params = [b"777", b"notify", b"title", b"body"]
    fn osc_notification_rxvt(&mut self, params: &[&[u8]]) {
        // params[1] should be "notify".
        if params.get(1).copied() != Some(b"notify".as_slice()) {
            return;
        }

        let Some(title) = Self::utf8_param(params, 2) else {
            return;
        };

        let body = Self::utf8_param(params, 3).unwrap_or("");

        self.emit_event(TerminalEvent::Notification {
            title: Some(title.to_string()),
            body: body.to_string(),
            id: None,
        });
    }
}

// ── vte::Perform ────────────────────────────────────────────────────

impl vte::Perform for VteHandler<'_> {
    fn print(&mut self, c: char) {
        let cols = self.state.grid.cols();

        // Handle pending wrap.
        if self.state.pending_wrap {
            if self.state.modes.contains(TerminalMode::WRAPAROUND) {
                self.state.grid.cursor_mut().col = 0;
                self.linefeed();
            }
            self.state.pending_wrap = false;
        }

        let width = c.width().unwrap_or(1);
        let col = self.state.grid.cursor().col as u16;
        let row = self.state.grid.cursor().row as u16;

        // If wide char won't fit on this line, wrap first.
        if width == 2 && col + 1 >= cols {
            if self.state.modes.contains(TerminalMode::WRAPAROUND) {
                // Fill remainder with space.
                let blank = Cell {
                    grapheme: CompactString::from(" "),
                    fg: self.state.attrs.fg,
                    bg: self.state.attrs.bg,
                    flags: CellFlags::empty(),
                    hyperlink: None,
                };
                self.state.grid.set_cell(col, row, blank);
                self.state.grid.cursor_mut().col = 0;
                self.linefeed();
            } else {
                return;
            }
        }

        let col = self.state.grid.cursor().col as u16;
        let row = self.state.grid.cursor().row as u16;

        // Write the cell.
        let cell = Cell {
            grapheme: {
                let mut buf = [0u8; 4];
                CompactString::from(c.encode_utf8(&mut buf) as &str)
            },
            fg: self.state.attrs.fg,
            bg: self.state.attrs.bg,
            flags: self.state.attrs.flags,
            hyperlink: self.state.current_hyperlink.clone(),
        };
        self.state.grid.set_cell(col, row, cell);

        // Handle wide characters: mark the next cell as a spacer.
        if width == 2 && col + 1 < cols {
            let spacer = Cell {
                grapheme: CompactString::default(),
                fg: self.state.attrs.fg,
                bg: self.state.attrs.bg,
                flags: CellFlags::WIDE_SPACER,
                hyperlink: self.state.current_hyperlink.clone(),
            };
            self.state.grid.set_cell(col + 1, row, spacer);
        }

        // Advance cursor.
        let advance = if width == 2 { 2 } else { 1 };
        let new_col = col as usize + advance;
        if new_col >= cols as usize {
            // At right margin — set pending wrap.
            self.state.grid.cursor_mut().col = (cols - 1) as usize;
            self.state.pending_wrap = true;
        } else {
            self.state.grid.cursor_mut().col = new_col;
        }
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            // BEL — placeholder.
            0x07 => {}
            // BS — backspace.
            0x08 => {
                let cursor = self.state.grid.cursor_mut();
                cursor.col = cursor.col.saturating_sub(1);
                self.state.pending_wrap = false;
            }
            // HT — horizontal tab.
            0x09 => {
                let cols = self.state.grid.cols() as usize;
                let col = self.state.grid.cursor().col;
                // Find next tab stop.
                let next = (col + 1..cols)
                    .find(|&c| self.state.tabs.get(c).copied().unwrap_or(false))
                    .unwrap_or(cols - 1);
                self.state.grid.cursor_mut().col = next;
                self.state.pending_wrap = false;
            }
            // LF, VT, FF — linefeed.
            0x0A..=0x0C => {
                self.linefeed();
            }
            // CR — carriage return.
            0x0D => {
                self.state.grid.cursor_mut().col = 0;
                self.state.pending_wrap = false;
            }
            _ => {}
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        intermediates: &[u8],
        ignore: bool,
        action: char,
    ) {
        if ignore {
            return;
        }

        // Private mode sequences (DECSET/DECRST).
        if intermediates == [b'?'] {
            match action {
                'h' => self.csi_mode_set(params),
                'l' => self.csi_mode_reset(params),
                _ => {}
            }
            return;
        }

        // Don't process CSI sequences with unknown intermediates.
        if !intermediates.is_empty() {
            return;
        }

        match action {
            // Cursor movement.
            'A' => self.csi_cursor_up(Self::param(params, 0, 1)),
            'B' => self.csi_cursor_down(Self::param(params, 0, 1)),
            'C' => self.csi_cursor_forward(Self::param(params, 0, 1)),
            'D' => self.csi_cursor_back(Self::param(params, 0, 1)),
            'H' | 'f' => {
                let row = Self::param(params, 0, 1);
                let col = Self::param(params, 1, 1);
                self.csi_cursor_position(row, col);
            }
            'G' | '`' => {
                self.csi_cursor_horizontal_absolute(Self::param(params, 0, 1));
            }
            'd' => {
                self.csi_cursor_vertical_absolute(Self::param(params, 0, 1));
            }

            // Erase operations.
            'J' => self.csi_erase_display(Self::param(params, 0, 0)),
            'K' => self.csi_erase_line(Self::param(params, 0, 0)),

            // Line operations.
            'L' => self.csi_insert_lines(Self::param(params, 0, 1)),
            'M' => self.csi_delete_lines(Self::param(params, 0, 1)),

            // Character operations.
            '@' => {
                let n = Self::param(params, 0, 1);
                self.state.grid.insert_chars(n);
            }
            'P' => {
                let n = Self::param(params, 0, 1);
                self.state.grid.delete_chars(n);
            }

            // Scroll up (CSI S).
            'S' => {
                let n = Self::param(params, 0, 1);
                self.capture_rows_to_scrollback(n);
                self.state.grid.scroll_up_in_region(
                    self.state.scroll_top,
                    self.state.scroll_bottom,
                    n,
                );
            }
            'T' => {
                let n = Self::param(params, 0, 1);
                self.state.grid.scroll_down_in_region(
                    self.state.scroll_top,
                    self.state.scroll_bottom,
                    n,
                );
            }

            // SGR — Select Graphic Rendition.
            'm' => self.sgr_dispatch(params),

            // DSR — Device Status Report.
            'n' => {
                let mode = Self::param(params, 0, 0);
                match mode {
                    5 => {
                        // Device status: respond "OK"
                        self.emit_event(TerminalEvent::PtyWrite(b"\x1b[0n".to_vec()));
                    }
                    6 => {
                        // CPR (Cursor Position Report): respond ESC[row;colR (1-based)
                        let row = self.state.grid.cursor().row + 1;
                        let col = self.state.grid.cursor().col + 1;
                        self.emit_event(TerminalEvent::PtyWrite(
                            format!("\x1b[{row};{col}R").into_bytes(),
                        ));
                    }
                    _ => {}
                }
            }

            // DA1 — Primary Device Attributes.
            'c' => {
                if Self::param(params, 0, 0) == 0 {
                    // Identify as VT220 with ANSI color support
                    self.emit_event(TerminalEvent::PtyWrite(b"\x1b[?62;22c".to_vec()));
                }
            }

            // Scroll region (DECSTBM).
            'r' => self.csi_set_scroll_region(params),

            // ECH — Erase Character: erase N chars at cursor without moving it.
            'X' => {
                let n = Self::param(params, 0, 1);
                let cols = self.state.grid.cols();
                let cursor_row = self.state.grid.cursor().row as u16;
                let cursor_col = self.state.grid.cursor().col as u16;
                let end = cursor_col.saturating_add(n).min(cols);
                let blank = self.erase_cell();
                self.state
                    .grid
                    .fill_cells(cursor_col, end, cursor_row, &blank);
            }

            _ => {}
        }
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], ignore: bool, byte: u8) {
        if ignore {
            return;
        }

        // Ignore sequences with intermediates we don't handle.
        if !intermediates.is_empty() {
            return;
        }

        match byte {
            // DECSC — save cursor.
            b'7' => {
                self.state.saved_cursor = *self.state.grid.cursor();
            }
            // DECRC — restore cursor.
            b'8' => {
                let saved = self.state.saved_cursor;
                *self.state.grid.cursor_mut() = saved;
                self.state.pending_wrap = false;
            }
            // IND — index (linefeed).
            b'D' => {
                self.linefeed();
            }
            // RI — reverse index.
            b'M' => {
                let row = self.state.grid.cursor().row as u16;
                if row == self.state.scroll_top {
                    self.state.grid.scroll_down_in_region(
                        self.state.scroll_top,
                        self.state.scroll_bottom,
                        1,
                    );
                } else if row > 0 {
                    self.state.grid.cursor_mut().row -= 1;
                }
            }
            // NEL — next line (CR + LF).
            b'E' => {
                self.state.grid.cursor_mut().col = 0;
                self.linefeed();
            }
            _ => {}
        }
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if params.is_empty() {
            return;
        }

        // First param is the OSC number as ASCII digits.
        let Some(osc_num) = std::str::from_utf8(params[0])
            .ok()
            .and_then(|s| s.parse::<u16>().ok())
        else {
            return;
        };

        match osc_num {
            7 => self.osc_cwd(params),
            8 => self.osc_hyperlink(params),
            9 => self.osc_notification_iterm(params),
            // OSC 10: query/set foreground color.
            10 => self.osc_color_query(params, true),
            // OSC 11: query/set background color.
            11 => self.osc_color_query(params, false),
            99 => self.osc_notification_kitty(params),
            133 => self.osc_prompt_mark(params),
            777 => self.osc_notification_rxvt(params),
            _ => {} // Unknown OSC — silently ignore.
        }
    }

    fn hook(&mut self, _params: &vte::Params, _intermediates: &[u8], _ignore: bool, _action: char) {
        // DCS sequences out of scope.
    }

    fn put(&mut self, _byte: u8) {
        // DCS data out of scope.
    }

    fn unhook(&mut self) {
        // DCS end out of scope.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::Terminal;

    #[test]
    fn sgr_bold_italic_underline() {
        let mut term = Terminal::new(80, 24);
        // ESC[1;3;4m = bold + italic + underline
        term.process(b"\x1b[1;3;4mA");
        let cell = term.grid().cell(0, 0);
        assert!(cell.flags.contains(CellFlags::BOLD));
        assert!(cell.flags.contains(CellFlags::ITALIC));
        assert!(cell.flags.contains(CellFlags::UNDERLINE));
    }

    #[test]
    fn sgr_reset_clears_all() {
        let mut term = Terminal::new(80, 24);
        term.process(b"\x1b[1;31m");
        term.process(b"\x1b[0mA");
        let cell = term.grid().cell(0, 0);
        assert!(!cell.flags.contains(CellFlags::BOLD));
        assert_eq!(cell.fg, Color::default());
    }

    #[test]
    fn sgr_256_color() {
        let mut term = Terminal::new(80, 24);
        // ESC[38;5;196m = indexed fg 196
        term.process(b"\x1b[38;5;196mX");
        assert_eq!(term.grid().cell(0, 0).fg, Color::Indexed(196));
    }

    #[test]
    fn sgr_bright_colors() {
        let mut term = Terminal::new(80, 24);
        // ESC[90m = bright black (named 8)
        term.process(b"\x1b[90mX");
        assert_eq!(term.grid().cell(0, 0).fg, Color::Named(8));
        // ESC[107m = bright white bg (named 15)
        term.process(b"\x1b[107mY");
        assert_eq!(term.grid().cell(1, 0).bg, Color::Named(15));
    }

    #[test]
    fn sgr_inverse_and_strikethrough() {
        let mut term = Terminal::new(80, 24);
        term.process(b"\x1b[7;9mA");
        let cell = term.grid().cell(0, 0);
        assert!(cell.flags.contains(CellFlags::INVERSE));
        assert!(cell.flags.contains(CellFlags::STRIKETHROUGH));
        // Reset individual flags.
        term.process(b"\x1b[27;29mB");
        let cell = term.grid().cell(1, 0);
        assert!(!cell.flags.contains(CellFlags::INVERSE));
        assert!(!cell.flags.contains(CellFlags::STRIKETHROUGH));
    }

    #[test]
    fn erase_line_modes() {
        let mut term = Terminal::new(10, 1);
        term.process(b"ABCDEFGHIJ");
        // Move cursor to col 5, erase to end of line.
        term.process(b"\x1b[6G\x1b[K");
        assert_eq!(term.grid().cell(4, 0).grapheme.as_str(), "E");
        assert_eq!(term.grid().cell(5, 0).grapheme.as_str(), " ");
        assert_eq!(term.grid().cell(9, 0).grapheme.as_str(), " ");
    }

    #[test]
    fn insert_and_delete_lines() {
        let mut term = Terminal::new(4, 4);
        term.process(b"AAAA\r\nBBBB\r\nCCCC\r\nDDDD");
        // Move to row 1 and insert 1 line.
        term.process(b"\x1b[2;1H\x1b[L");
        assert_eq!(term.grid().cell(0, 0).grapheme.as_str(), "A");
        assert_eq!(term.grid().cell(0, 1).grapheme.as_str(), " "); // inserted
        assert_eq!(term.grid().cell(0, 2).grapheme.as_str(), "B");
        assert_eq!(term.grid().cell(0, 3).grapheme.as_str(), "C");
        // D pushed off
    }

    #[test]
    fn tab_advances_to_next_stop() {
        let mut term = Terminal::new(80, 24);
        term.process(b"A\tB");
        // Tab at col 1 should advance to col 8.
        assert_eq!(term.grid().cell(0, 0).grapheme.as_str(), "A");
        assert_eq!(term.grid().cell(8, 0).grapheme.as_str(), "B");
    }

    #[test]
    fn scroll_region_confines_scrolling() {
        let mut term = Terminal::new(4, 6);
        term.process(b"0000\r\n1111\r\n2222\r\n3333\r\n4444\r\n5555");
        // Set scroll region rows 2-5 (1-based: ESC[2;5r).
        term.process(b"\x1b[2;5r");
        // Move to bottom of region and linefeed to scroll.
        term.process(b"\x1b[5;1H");
        term.process(b"\n");
        // Row 0 (outside region) should be untouched.
        assert_eq!(term.grid().cell(0, 0).grapheme.as_str(), "0");
        // Row 5 (outside region) should be untouched.
        assert_eq!(term.grid().cell(0, 5).grapheme.as_str(), "5");
        // Inside region: rows shifted up by 1.
        assert_eq!(term.grid().cell(0, 1).grapheme.as_str(), "2");
        assert_eq!(term.grid().cell(0, 2).grapheme.as_str(), "3");
        assert_eq!(term.grid().cell(0, 3).grapheme.as_str(), "4");
        // Bottom of region cleared.
        assert_eq!(term.grid().cell(0, 4).grapheme.as_str(), " ");
    }

    #[test]
    fn decsc_decrc_save_restore_cursor() {
        let mut term = Terminal::new(80, 24);
        term.process(b"\x1b[5;10H"); // move cursor
        term.process(b"\x1b7"); // save
        term.process(b"\x1b[1;1H"); // move to home
        assert_eq!(term.grid().cursor().row, 0);
        term.process(b"\x1b8"); // restore
        assert_eq!(term.grid().cursor().row, 4);
        assert_eq!(term.grid().cursor().col, 9);
    }

    #[test]
    fn reverse_index_scrolls_down() {
        let mut term = Terminal::new(4, 4);
        term.process(b"AAAA\r\nBBBB\r\nCCCC\r\nDDDD");
        // Move to top and reverse index.
        term.process(b"\x1b[1;1H\x1bM");
        // Row 0 should be cleared (scrolled down).
        assert_eq!(term.grid().cell(0, 0).grapheme.as_str(), " ");
        assert_eq!(term.grid().cell(0, 1).grapheme.as_str(), "A");
    }

    #[test]
    fn wide_character_handling() {
        let mut term = Terminal::new(10, 1);
        // CJK character (世) is 2 cells wide.
        term.process("世".as_bytes());
        assert_eq!(term.grid().cell(0, 0).grapheme.as_str(), "世");
        assert!(term
            .grid()
            .cell(1, 0)
            .flags
            .contains(CellFlags::WIDE_SPACER));
        assert_eq!(term.grid().cursor().col, 2);
    }

    #[test]
    fn wraparound_at_right_margin() {
        let mut term = Terminal::new(5, 2);
        term.process(b"ABCDEFG");
        // First 5 chars on row 0.
        assert_eq!(term.grid().cell(4, 0).grapheme.as_str(), "E");
        // F and G wrap to row 1.
        assert_eq!(term.grid().cell(0, 1).grapheme.as_str(), "F");
        assert_eq!(term.grid().cell(1, 1).grapheme.as_str(), "G");
    }

    #[test]
    fn no_wraparound_mode() {
        let mut term = Terminal::new(5, 2);
        // Disable wraparound.
        term.process(b"\x1b[?7l");
        term.process(b"ABCDEFG");
        // Without wraparound, chars overwrite last column.
        assert_eq!(term.grid().cell(4, 0).grapheme.as_str(), "G");
        // Row 1 should be untouched.
        assert_eq!(term.grid().cell(0, 1).grapheme.as_str(), " ");
    }

    #[test]
    fn cursor_visibility_via_decset() {
        let mut term = Terminal::new(80, 24);
        assert!(term.grid().cursor().visible);
        term.process(b"\x1b[?25l"); // hide
        assert!(!term.grid().cursor().visible);
        term.process(b"\x1b[?25h"); // show
        assert!(term.grid().cursor().visible);
    }

    #[test]
    fn application_cursor_mode() {
        let mut term = Terminal::new(80, 24);
        assert!(!term.modes().contains(TerminalMode::APPLICATION_CURSOR));
        term.process(b"\x1b[?1h");
        assert!(term.modes().contains(TerminalMode::APPLICATION_CURSOR));
        term.process(b"\x1b[?1l");
        assert!(!term.modes().contains(TerminalMode::APPLICATION_CURSOR));
    }

    #[test]
    fn erase_display_from_cursor() {
        let mut term = Terminal::new(5, 3);
        term.process(b"AAAAA\r\nBBBBB\r\nCCCCC");
        // Move to row 1, col 2 and erase from cursor.
        term.process(b"\x1b[2;3H\x1b[J");
        assert_eq!(term.grid().cell(0, 0).grapheme.as_str(), "A"); // untouched
        assert_eq!(term.grid().cell(1, 1).grapheme.as_str(), "B"); // before cursor
        assert_eq!(term.grid().cell(2, 1).grapheme.as_str(), " "); // erased
        assert_eq!(term.grid().cell(0, 2).grapheme.as_str(), " "); // erased
    }

    #[test]
    fn next_line_esc_e() {
        let mut term = Terminal::new(80, 24);
        term.process(b"ABC\x1bE");
        assert_eq!(term.grid().cursor().row, 1);
        assert_eq!(term.grid().cursor().col, 0);
    }

    #[test]
    fn very_long_params_do_not_crash() {
        let mut term = Terminal::new(80, 24);
        // Build a sequence with many parameters.
        let mut seq = b"\x1b[".to_vec();
        for _ in 0..200 {
            seq.extend_from_slice(b"1;");
        }
        seq.push(b'm');
        term.process(&seq);
        // Should not crash; terminal still functional.
        term.process(b"OK");
    }

    #[test]
    fn origin_mode_cup_relative_to_scroll_region() {
        let mut term = Terminal::new(80, 24);
        // Set scroll region rows 5-15 (1-based).
        term.process(b"\x1b[5;15r");
        // Enable ORIGIN mode.
        term.process(b"\x1b[?6h");
        // CUP row=1, col=1 should land at (scroll_top, 0) = (4, 0).
        term.process(b"\x1b[1;1H");
        assert_eq!(term.grid().cursor().row, 4);
        assert_eq!(term.grid().cursor().col, 0);
        // CUP row=3, col=5 should land at (scroll_top + 2, 4) = (6, 4).
        term.process(b"\x1b[3;5H");
        assert_eq!(term.grid().cursor().row, 6);
        assert_eq!(term.grid().cursor().col, 4);
        // CUP beyond region bottom should clamp.
        term.process(b"\x1b[99;1H");
        assert_eq!(term.grid().cursor().row, 14); // scroll_bottom
    }

    #[test]
    fn cuu_cud_respect_scroll_margins() {
        let mut term = Terminal::new(80, 10);
        // Set scroll region rows 3-8 (1-based), 0-based: 2-7.
        term.process(b"\x1b[3;8r");
        // Place cursor inside region at row 5 (0-based: 4).
        term.process(b"\x1b[5;1H");
        // CUU 10 should stop at scroll_top (row 2), not row 0.
        term.process(b"\x1b[10A");
        assert_eq!(term.grid().cursor().row, 2);
        // CUD 10 should stop at scroll_bottom (row 7), not row 9.
        term.process(b"\x1b[10B");
        assert_eq!(term.grid().cursor().row, 7);
    }

    #[test]
    fn cuu_outside_region_clamps_to_zero() {
        let mut term = Terminal::new(80, 10);
        // Set scroll region rows 3-8 (1-based).
        term.process(b"\x1b[3;8r");
        // Place cursor outside region at row 0.
        term.process(b"\x1b[1;1H");
        // CUU should clamp to row 0 (outside region).
        term.process(b"\x1b[10A");
        assert_eq!(term.grid().cursor().row, 0);
    }

    // ── OSC tests ───────────────────────────────────────────────────

    #[test]
    fn osc7_emits_cwd_changed() {
        let (mut term, mut rx) = Terminal::with_event_channel(80, 24);
        term.process(b"\x1b]7;file://DESKTOP/C:/Users/me/project\x07");
        let event = rx.try_recv().unwrap();
        assert_eq!(
            event,
            TerminalEvent::CwdChanged(std::path::PathBuf::from("C:\\Users\\me\\project"))
        );
    }

    #[test]
    fn osc7_with_spaces() {
        let (mut term, mut rx) = Terminal::with_event_channel(80, 24);
        term.process(b"\x1b]7;file:///C:/my%20dir/proj\x07");
        let event = rx.try_recv().unwrap();
        assert_eq!(
            event,
            TerminalEvent::CwdChanged(std::path::PathBuf::from("C:\\my dir\\proj"))
        );
    }

    #[test]
    fn osc7_empty_path_ignored() {
        let (mut term, mut rx) = Terminal::with_event_channel(80, 24);
        // Only "7" param, no URI.
        term.process(b"\x1b]7\x07");
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn osc9_emits_notification() {
        let (mut term, mut rx) = Terminal::with_event_channel(80, 24);
        term.process(b"\x1b]9;Build complete\x07");
        let event = rx.try_recv().unwrap();
        assert_eq!(
            event,
            TerminalEvent::Notification {
                title: None,
                body: "Build complete".to_string(),
                id: None,
            }
        );
    }

    #[test]
    fn osc99_kitty_notification_title() {
        let (mut term, mut rx) = Terminal::with_event_channel(80, 24);
        // d=0 means this payload is the title.
        term.process(b"\x1b]99;i=notify1:d=0;Build done\x07");
        let event = rx.try_recv().unwrap();
        assert_eq!(
            event,
            TerminalEvent::Notification {
                title: Some("Build done".to_string()),
                body: String::new(),
                id: Some("notify1".to_string()),
            }
        );
    }

    #[test]
    fn osc99_kitty_notification_body() {
        let (mut term, mut rx) = Terminal::with_event_channel(80, 24);
        // d=1 means this payload is the body.
        term.process(b"\x1b]99;i=notify1:d=1;Details here\x07");
        let event = rx.try_recv().unwrap();
        assert_eq!(
            event,
            TerminalEvent::Notification {
                title: None,
                body: "Details here".to_string(),
                id: Some("notify1".to_string()),
            }
        );
    }

    #[test]
    fn osc99_malformed_no_panic() {
        let (mut term, mut rx) = Terminal::with_event_channel(80, 24);
        // Missing payload — only "99" param.
        term.process(b"\x1b]99\x07");
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn osc133_prompt_marks() {
        let (mut term, mut rx) = Terminal::with_event_channel(80, 24);

        term.process(b"\x1b]133;A\x07");
        assert_eq!(
            rx.try_recv().unwrap(),
            TerminalEvent::PromptMark(PromptMark::PromptStart)
        );

        term.process(b"\x1b]133;B\x07");
        assert_eq!(
            rx.try_recv().unwrap(),
            TerminalEvent::PromptMark(PromptMark::CommandStart)
        );

        term.process(b"\x1b]133;C\x07");
        assert_eq!(
            rx.try_recv().unwrap(),
            TerminalEvent::PromptMark(PromptMark::OutputStart)
        );

        term.process(b"\x1b]133;D\x07");
        assert_eq!(
            rx.try_recv().unwrap(),
            TerminalEvent::PromptMark(PromptMark::CommandEnd)
        );
    }

    #[test]
    fn osc133_unknown_mark_ignored() {
        let (mut term, mut rx) = Terminal::with_event_channel(80, 24);
        term.process(b"\x1b]133;Z\x07");
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn osc777_rxvt_notification() {
        let (mut term, mut rx) = Terminal::with_event_channel(80, 24);
        term.process(b"\x1b]777;notify;Task;All tests pass\x07");
        let event = rx.try_recv().unwrap();
        assert_eq!(
            event,
            TerminalEvent::Notification {
                title: Some("Task".to_string()),
                body: "All tests pass".to_string(),
                id: None,
            }
        );
    }

    #[test]
    fn osc777_without_body() {
        let (mut term, mut rx) = Terminal::with_event_channel(80, 24);
        term.process(b"\x1b]777;notify;Done\x07");
        let event = rx.try_recv().unwrap();
        assert_eq!(
            event,
            TerminalEvent::Notification {
                title: Some("Done".to_string()),
                body: String::new(),
                id: None,
            }
        );
    }

    #[test]
    fn osc777_non_notify_ignored() {
        let (mut term, mut rx) = Terminal::with_event_channel(80, 24);
        term.process(b"\x1b]777;something;else\x07");
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn osc8_hyperlink_on_cells() {
        let (mut term, _rx) = Terminal::with_event_channel(80, 24);
        // Open hyperlink.
        term.process(b"\x1b]8;id=link1;https://example.com\x07");
        term.process(b"ABC");
        // Close hyperlink.
        term.process(b"\x1b]8;;\x07");
        term.process(b"D");

        // Cells A, B, C should have the hyperlink.
        let link = term.grid().cell(0, 0).hyperlink.as_ref().unwrap();
        assert_eq!(link.uri, "https://example.com");
        assert_eq!(link.id.as_deref(), Some("link1"));

        let link_b = term.grid().cell(1, 0).hyperlink.as_ref().unwrap();
        // Same Arc — shared reference.
        assert!(Arc::ptr_eq(link, link_b));

        // Cell D should have no hyperlink.
        assert!(term.grid().cell(3, 0).hyperlink.is_none());
    }

    #[test]
    fn osc8_empty_uri_closes_link() {
        let (mut term, _rx) = Terminal::with_event_channel(80, 24);
        term.process(b"\x1b]8;;https://example.com\x07");
        term.process(b"A");
        assert!(term.grid().cell(0, 0).hyperlink.is_some());

        // Close.
        term.process(b"\x1b]8;;\x07");
        term.process(b"B");
        assert!(term.grid().cell(1, 0).hyperlink.is_none());
    }

    #[test]
    fn unknown_osc_silently_ignored() {
        let (mut term, mut rx) = Terminal::with_event_channel(80, 24);
        // OSC 52 (clipboard) — not handled, should not panic.
        term.process(b"\x1b]52;c;SGVsbG8=\x07");
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn event_channel_not_required() {
        // Terminal without event channel — OSC sequences should not panic.
        let mut term = Terminal::new(80, 24);
        term.process(b"\x1b]7;file:///C:/test\x07");
        term.process(b"\x1b]9;hello\x07");
        term.process(b"\x1b]133;A\x07");
        term.process(b"\x1b]8;;https://x.com\x07");
        term.process(b"A");
        term.process(b"\x1b]8;;\x07");
        // No panic = success.
    }
}
