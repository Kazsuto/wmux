use std::time::Instant;

use wmux_core::grid::Grid;
use wmux_core::scrollback::Scrollback;
use wmux_core::selection::{Selection, SelectionMode};

/// Result of processing a mouse event.
#[derive(Debug)]
pub enum MouseAction {
    /// No action taken.
    None,
    /// A new selection was started.
    SelectionStarted,
    /// The current selection was updated during drag.
    SelectionUpdated,
    /// The selection was finalized (mouse released).
    SelectionFinished,
    /// SGR mouse report bytes to send to PTY.
    Report(Vec<u8>),
    /// New viewport offset after scroll.
    Scroll(usize),
}

/// Handles mouse events for selection, clipboard, scroll, and mouse reporting.
#[derive(Debug)]
pub struct MouseHandler {
    selection: Option<Selection>,
    last_click_time: Option<Instant>,
    last_click_pos: Option<(usize, usize)>,
    click_count: u8,
}

/// Maximum interval between clicks to count as multi-click (ms).
const MULTI_CLICK_THRESHOLD_MS: u128 = 500;

/// Mouse button identifiers (matching winit conventions).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

impl MouseHandler {
    /// Create a new mouse handler with no active selection.
    pub fn new() -> Self {
        Self {
            selection: None,
            last_click_time: None,
            last_click_pos: None,
            click_count: 0,
        }
    }

    /// Handle a mouse button press event.
    ///
    /// When mouse reporting mode is active and Shift is not held, generates
    /// SGR mouse report bytes. Otherwise starts or updates text selection.
    pub fn handle_mouse_press(
        &mut self,
        col: usize,
        row: usize,
        button: MouseButton,
        shift_held: bool,
        mouse_mode: bool,
    ) -> MouseAction {
        // Mouse reporting: forward to PTY unless Shift bypasses it
        if mouse_mode && !shift_held {
            let sgr_button = match button {
                MouseButton::Left => 0,
                MouseButton::Middle => 1,
                MouseButton::Right => 2,
            };
            return MouseAction::Report(sgr_press(sgr_button, col, row));
        }

        // Only left button starts selection
        if button != MouseButton::Left {
            return MouseAction::None;
        }

        // Multi-click detection
        let now = Instant::now();
        let same_pos = self
            .last_click_pos
            .is_some_and(|(c, r)| c == col && r == row);
        let within_threshold = self
            .last_click_time
            .is_some_and(|t| now.duration_since(t).as_millis() < MULTI_CLICK_THRESHOLD_MS);

        if same_pos && within_threshold {
            self.click_count = match self.click_count {
                1 => 2,
                2 => 3,
                _ => 1,
            };
        } else {
            self.click_count = 1;
        }

        self.last_click_time = Some(now);
        self.last_click_pos = Some((col, row));

        let mode = match self.click_count {
            2 => SelectionMode::Word,
            3 => SelectionMode::Line,
            _ => SelectionMode::Normal,
        };

        self.selection = Some(Selection::new(col, row, mode));
        MouseAction::SelectionStarted
    }

    /// Handle mouse motion (drag) events.
    ///
    /// Updates the active selection or generates mouse motion reports.
    pub fn handle_mouse_motion(&mut self, col: usize, row: usize, mouse_mode: bool) -> MouseAction {
        // Update active selection
        if let Some(ref mut sel) = self.selection {
            if sel.active {
                sel.update(col, row);
                return MouseAction::SelectionUpdated;
            }
        }

        // Mouse motion reporting (mode 1003 = all motion)
        if mouse_mode {
            // Motion report uses button 32 + movement flag
            return MouseAction::Report(sgr_press(35, col, row));
        }

        MouseAction::None
    }

    /// Handle mouse button release events.
    ///
    /// Finalizes any active selection or generates release reports.
    pub fn handle_mouse_release(
        &mut self,
        col: usize,
        row: usize,
        button: MouseButton,
        mouse_mode: bool,
    ) -> MouseAction {
        // Finalize selection
        if let Some(ref mut sel) = self.selection {
            if sel.active {
                sel.update(col, row);
                sel.active = false;
                return MouseAction::SelectionFinished;
            }
        }

        // Mouse reporting
        if mouse_mode {
            let sgr_button = match button {
                MouseButton::Left => 0,
                MouseButton::Middle => 1,
                MouseButton::Right => 2,
            };
            return MouseAction::Report(sgr_release(sgr_button, col, row));
        }

        MouseAction::None
    }

    /// Handle mouse wheel scroll events.
    ///
    /// Returns the new viewport offset after scrolling. Positive delta
    /// scrolls up (into history), negative scrolls down (toward live).
    pub fn handle_scroll(
        &self,
        delta: f64,
        viewport_offset: usize,
        scrollback_len: usize,
    ) -> MouseAction {
        let lines: usize = 3;
        let new_offset = if delta > 0.0 {
            // Scroll up (into history)
            viewport_offset.saturating_add(lines).min(scrollback_len)
        } else {
            // Scroll down (toward live)
            viewport_offset.saturating_sub(lines)
        };
        MouseAction::Scroll(new_offset)
    }

    /// Get the current selection, if any.
    pub fn selection(&self) -> Option<&Selection> {
        self.selection.as_ref()
    }

    /// Clear the current selection.
    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    /// Copy the selected text to the system clipboard.
    ///
    /// Returns the copied text, or `None` if there is no selection or
    /// clipboard access fails.
    pub fn copy_selection(&self, grid: &Grid, scrollback: &Scrollback) -> Option<String> {
        let sel = self.selection.as_ref()?;
        let text = sel.extract_text(grid, scrollback);
        if text.is_empty() {
            return None;
        }

        match arboard::Clipboard::new() {
            Ok(mut clipboard) => {
                if let Err(e) = clipboard.set_text(&text) {
                    tracing::warn!(error = %e, "failed to copy to clipboard");
                    return None;
                }
                Some(text)
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to open clipboard");
                None
            }
        }
    }

    /// Copy pre-extracted text to the system clipboard.
    ///
    /// Used in the actor pattern where text is read from the actor
    /// rather than directly from the grid.
    pub fn copy_text_to_clipboard(&self, text: &str) {
        if text.is_empty() {
            return;
        }
        match arboard::Clipboard::new() {
            Ok(mut clipboard) => {
                if let Err(e) = clipboard.set_text(text) {
                    tracing::warn!(error = %e, "failed to copy to clipboard");
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to open clipboard");
            }
        }
    }

    /// Paste text from the system clipboard.
    ///
    /// Returns the clipboard content, or `None` if the clipboard is empty
    /// or access fails.
    pub fn paste_from_clipboard(&self) -> Option<String> {
        match arboard::Clipboard::new() {
            Ok(mut clipboard) => match clipboard.get_text() {
                Ok(text) if !text.is_empty() => Some(text),
                Ok(_) => None,
                Err(e) => {
                    tracing::warn!(error = %e, "failed to read clipboard");
                    None
                }
            },
            Err(e) => {
                tracing::warn!(error = %e, "failed to open clipboard");
                None
            }
        }
    }
}

impl Default for MouseHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate SGR mouse press bytes: `\x1b[<button;col;rowM`
///
/// Col and row are 1-based in the SGR protocol.
#[inline]
fn sgr_press(button: u8, col: usize, row: usize) -> Vec<u8> {
    use std::io::Write;
    let mut buf = Vec::with_capacity(16);
    // write! to Vec<u8> is infallible
    let _ = write!(buf, "\x1b[<{};{};{}M", button, col + 1, row + 1);
    buf
}

/// Generate SGR mouse release bytes: `\x1b[<button;col;rowm`
///
/// Col and row are 1-based in the SGR protocol.
#[inline]
fn sgr_release(button: u8, col: usize, row: usize) -> Vec<u8> {
    use std::io::Write;
    let mut buf = Vec::with_capacity(16);
    let _ = write!(buf, "\x1b[<{};{};{}m", button, col + 1, row + 1);
    buf
}

/// Generate SGR mouse wheel bytes.
///
/// Wheel up = button 64, wheel down = button 65.
#[inline]
pub fn sgr_wheel(up: bool, col: usize, row: usize) -> Vec<u8> {
    let button: u8 = if up { 64 } else { 65 };
    sgr_press(button, col, row)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_handler_has_no_selection() {
        let handler = MouseHandler::new();
        assert!(handler.selection().is_none());
    }

    #[test]
    fn single_click_starts_normal_selection() {
        let mut handler = MouseHandler::new();
        let action = handler.handle_mouse_press(5, 3, MouseButton::Left, false, false);
        assert!(matches!(action, MouseAction::SelectionStarted));
        let sel = handler.selection().unwrap();
        assert_eq!(sel.mode, SelectionMode::Normal);
        assert!(sel.active);
    }

    #[test]
    fn double_click_starts_word_selection() {
        let mut handler = MouseHandler::new();
        handler.handle_mouse_press(5, 3, MouseButton::Left, false, false);
        // Release
        handler.handle_mouse_release(5, 3, MouseButton::Left, false);
        // Second click at same position quickly
        let action = handler.handle_mouse_press(5, 3, MouseButton::Left, false, false);
        assert!(matches!(action, MouseAction::SelectionStarted));
        let sel = handler.selection().unwrap();
        assert_eq!(sel.mode, SelectionMode::Word);
    }

    #[test]
    fn triple_click_starts_line_selection() {
        let mut handler = MouseHandler::new();
        // Click 1
        handler.handle_mouse_press(5, 3, MouseButton::Left, false, false);
        handler.handle_mouse_release(5, 3, MouseButton::Left, false);
        // Click 2
        handler.handle_mouse_press(5, 3, MouseButton::Left, false, false);
        handler.handle_mouse_release(5, 3, MouseButton::Left, false);
        // Click 3
        let action = handler.handle_mouse_press(5, 3, MouseButton::Left, false, false);
        assert!(matches!(action, MouseAction::SelectionStarted));
        let sel = handler.selection().unwrap();
        assert_eq!(sel.mode, SelectionMode::Line);
    }

    #[test]
    fn mouse_motion_updates_selection() {
        let mut handler = MouseHandler::new();
        handler.handle_mouse_press(0, 0, MouseButton::Left, false, false);
        let action = handler.handle_mouse_motion(10, 2, false);
        assert!(matches!(action, MouseAction::SelectionUpdated));
        let sel = handler.selection().unwrap();
        assert_eq!(sel.end.col, 10);
        assert_eq!(sel.end.row, 2);
    }

    #[test]
    fn mouse_release_finalizes_selection() {
        let mut handler = MouseHandler::new();
        handler.handle_mouse_press(0, 0, MouseButton::Left, false, false);
        let action = handler.handle_mouse_release(10, 2, MouseButton::Left, false);
        assert!(matches!(action, MouseAction::SelectionFinished));
        let sel = handler.selection().unwrap();
        assert!(!sel.active);
    }

    #[test]
    fn mouse_mode_generates_sgr_report() {
        let mut handler = MouseHandler::new();
        let action = handler.handle_mouse_press(5, 10, MouseButton::Left, false, true);
        match action {
            MouseAction::Report(bytes) => {
                let s = String::from_utf8(bytes).unwrap();
                assert_eq!(s, "\x1b[<0;6;11M");
            }
            _ => panic!("expected Report"),
        }
    }

    #[test]
    fn shift_bypasses_mouse_mode() {
        let mut handler = MouseHandler::new();
        let action = handler.handle_mouse_press(5, 10, MouseButton::Left, true, true);
        assert!(matches!(action, MouseAction::SelectionStarted));
    }

    #[test]
    fn sgr_press_format() {
        assert_eq!(
            String::from_utf8(sgr_press(0, 0, 0)).unwrap(),
            "\x1b[<0;1;1M"
        );
        assert_eq!(
            String::from_utf8(sgr_press(2, 9, 19)).unwrap(),
            "\x1b[<2;10;20M"
        );
    }

    #[test]
    fn sgr_release_format() {
        assert_eq!(
            String::from_utf8(sgr_release(0, 0, 0)).unwrap(),
            "\x1b[<0;1;1m"
        );
    }

    #[test]
    fn sgr_wheel_format() {
        assert_eq!(
            String::from_utf8(sgr_wheel(true, 5, 5)).unwrap(),
            "\x1b[<64;6;6M"
        );
        assert_eq!(
            String::from_utf8(sgr_wheel(false, 5, 5)).unwrap(),
            "\x1b[<65;6;6M"
        );
    }

    #[test]
    fn scroll_up_increases_offset() {
        let handler = MouseHandler::new();
        let action = handler.handle_scroll(1.0, 0, 100);
        match action {
            MouseAction::Scroll(offset) => assert_eq!(offset, 3),
            _ => panic!("expected Scroll"),
        }
    }

    #[test]
    fn scroll_down_decreases_offset() {
        let handler = MouseHandler::new();
        let action = handler.handle_scroll(-1.0, 10, 100);
        match action {
            MouseAction::Scroll(offset) => assert_eq!(offset, 7),
            _ => panic!("expected Scroll"),
        }
    }

    #[test]
    fn scroll_clamps_to_bounds() {
        let handler = MouseHandler::new();
        // Scroll up past max
        let action = handler.handle_scroll(1.0, 99, 100);
        match action {
            MouseAction::Scroll(offset) => assert_eq!(offset, 100),
            _ => panic!("expected Scroll"),
        }
        // Scroll down past 0
        let action = handler.handle_scroll(-1.0, 1, 100);
        match action {
            MouseAction::Scroll(offset) => assert_eq!(offset, 0),
            _ => panic!("expected Scroll"),
        }
    }

    #[test]
    fn clear_selection_removes_it() {
        let mut handler = MouseHandler::new();
        handler.handle_mouse_press(0, 0, MouseButton::Left, false, false);
        assert!(handler.selection().is_some());
        handler.clear_selection();
        assert!(handler.selection().is_none());
    }

    #[test]
    fn right_click_in_normal_mode_does_nothing() {
        let mut handler = MouseHandler::new();
        let action = handler.handle_mouse_press(5, 5, MouseButton::Right, false, false);
        assert!(matches!(action, MouseAction::None));
    }

    #[test]
    fn right_click_in_mouse_mode_reports() {
        let mut handler = MouseHandler::new();
        let action = handler.handle_mouse_press(5, 5, MouseButton::Right, false, true);
        match action {
            MouseAction::Report(bytes) => {
                let s = String::from_utf8(bytes).unwrap();
                assert_eq!(s, "\x1b[<2;6;6M");
            }
            _ => panic!("expected Report"),
        }
    }

    #[test]
    #[ignore] // Requires system clipboard
    fn copy_paste_clipboard() {
        use wmux_core::cell::Cell;

        let mut grid = Grid::new(10, 1);
        for (i, ch) in "hello".chars().enumerate() {
            let mut cell = Cell::default();
            cell.grapheme = ch.to_string().into();
            #[allow(clippy::cast_possible_truncation)]
            grid.set_cell(i as u16, 0, cell);
        }
        let scrollback = Scrollback::new(100);

        let mut handler = MouseHandler::new();
        handler.handle_mouse_press(0, 0, MouseButton::Left, false, false);
        handler.handle_mouse_release(4, 0, MouseButton::Left, false);

        let copied = handler.copy_selection(&grid, &scrollback);
        assert_eq!(copied.as_deref(), Some("hello"));

        let pasted = handler.paste_from_clipboard();
        assert_eq!(pasted.as_deref(), Some("hello"));
    }
}
