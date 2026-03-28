use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use wmux_core::mode::TerminalMode;

/// Translates winit 0.30 keyboard events to VT byte sequences for PTY input.
#[derive(Debug, Clone, Copy)]
pub struct InputHandler;

impl Default for InputHandler {
    fn default() -> Self {
        Self
    }
}

impl InputHandler {
    pub fn new() -> Self {
        Self
    }

    /// Translate a winit key event to bytes to send to the PTY.
    ///
    /// Returns `None` for events that should be ignored (releases, reserved shortcuts).
    /// Returns `Some(bytes)` for events to forward to the PTY.
    pub fn handle_key_event(
        &self,
        event: &KeyEvent,
        modifiers: &ModifiersState,
        modes: TerminalMode,
    ) -> Option<Vec<u8>> {
        if event.state == ElementState::Released {
            return None;
        }
        translate_key(
            &event.logical_key,
            modifiers.control_key(),
            modifiers.alt_key(),
            modifiers.shift_key(),
            modes,
        )
    }

    /// Wrap pasted text with bracketed paste escape sequences if the mode is active.
    ///
    /// Strips ESC bytes (0x1B) from the pasted text to prevent paste injection
    /// attacks where a malicious clipboard payload contains `\x1b[201~` to
    /// break out of the bracketed paste envelope.
    pub fn wrap_bracketed_paste(&self, text: &str, modes: TerminalMode) -> Vec<u8> {
        if modes.contains(TerminalMode::BRACKETED_PASTE) {
            // Sanitize: strip ESC bytes to prevent bracketed paste escape
            let sanitized: Vec<u8> = text.bytes().filter(|&b| b != 0x1B).collect();
            let mut result = Vec::with_capacity(6 + sanitized.len() + 6);
            result.extend_from_slice(b"\x1b[200~");
            result.extend_from_slice(&sanitized);
            result.extend_from_slice(b"\x1b[201~");
            result
        } else {
            text.as_bytes().to_vec()
        }
    }
}

fn translate_key(
    logical_key: &Key,
    ctrl: bool,
    alt: bool,
    shift: bool,
    modes: TerminalMode,
) -> Option<Vec<u8>> {
    match logical_key {
        Key::Character(s) => {
            // Global shortcuts (Ctrl+Shift+C, Ctrl+Shift+V, Ctrl+D, etc.) are
            // intercepted by the ShortcutMap before reaching this function.
            // translate_key handles only terminal byte sequences.

            let bytes = if ctrl {
                let ch = s.chars().next()?;
                vec![ctrl_byte(ch)?]
            } else {
                s.as_bytes().to_vec()
            };

            if alt {
                let mut result = Vec::with_capacity(1 + bytes.len());
                result.push(0x1b);
                result.extend_from_slice(&bytes);
                Some(result)
            } else {
                Some(bytes)
            }
        }
        Key::Named(named) => named_key_bytes(*named, ctrl, shift, modes),
        Key::Dead(_) | Key::Unidentified(_) => None,
    }
}

/// Map a character pressed with Ctrl to its control code byte.
fn ctrl_byte(ch: char) -> Option<u8> {
    match ch {
        'a'..='z' => Some(ch as u8 - b'a' + 1),
        'A'..='Z' => Some(ch as u8 - b'A' + 1),
        '@' => Some(0x00),
        '[' => Some(0x1b),
        '\\' => Some(0x1c),
        ']' => Some(0x1d),
        '^' => Some(0x1e),
        '_' => Some(0x1f),
        _ => None,
    }
}

/// Return the VT byte sequence for a named key.
fn named_key_bytes(key: NamedKey, ctrl: bool, shift: bool, modes: TerminalMode) -> Option<Vec<u8>> {
    let app_cursor = modes.contains(TerminalMode::APPLICATION_CURSOR);

    let seq: &[u8] = match key {
        NamedKey::ArrowUp => {
            if app_cursor {
                b"\x1bOA"
            } else {
                b"\x1b[A"
            }
        }
        NamedKey::ArrowDown => {
            if app_cursor {
                b"\x1bOB"
            } else {
                b"\x1b[B"
            }
        }
        NamedKey::ArrowRight => {
            if app_cursor {
                b"\x1bOC"
            } else {
                b"\x1b[C"
            }
        }
        NamedKey::ArrowLeft => {
            if app_cursor {
                b"\x1bOD"
            } else {
                b"\x1b[D"
            }
        }
        NamedKey::F1 => b"\x1bOP",
        NamedKey::F2 => b"\x1bOQ",
        NamedKey::F3 => b"\x1bOR",
        NamedKey::F4 => b"\x1bOS",
        NamedKey::F5 => b"\x1b[15~",
        NamedKey::F6 => b"\x1b[17~",
        NamedKey::F7 => b"\x1b[18~",
        NamedKey::F8 => b"\x1b[19~",
        NamedKey::F9 => b"\x1b[20~",
        NamedKey::F10 => b"\x1b[21~",
        NamedKey::F11 => b"\x1b[23~",
        NamedKey::F12 => b"\x1b[24~",
        NamedKey::Home => b"\x1b[H",
        NamedKey::End => b"\x1b[F",
        NamedKey::PageUp => b"\x1b[5~",
        NamedKey::PageDown => b"\x1b[6~",
        NamedKey::Insert => b"\x1b[2~",
        NamedKey::Delete => b"\x1b[3~",
        NamedKey::Enter => b"\r",
        NamedKey::Backspace => b"\x7f",
        NamedKey::Tab if shift => b"\x1b[Z",
        NamedKey::Tab => b"\x09",
        NamedKey::Escape => b"\x1b",
        NamedKey::Space => {
            if ctrl {
                return Some(vec![0x00]);
            }
            b" "
        }
        _ => return None,
    };

    Some(seq.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::keyboard::SmolStr;

    fn char_key(s: &str) -> Key {
        Key::Character(SmolStr::new(s))
    }

    fn normal_modes() -> TerminalMode {
        TerminalMode::default()
    }

    fn app_cursor_modes() -> TerminalMode {
        TerminalMode::default() | TerminalMode::APPLICATION_CURSOR
    }

    fn bracketed_paste_modes() -> TerminalMode {
        TerminalMode::default() | TerminalMode::BRACKETED_PASTE
    }

    #[test]
    fn char_a_no_modifiers() {
        assert_eq!(
            translate_key(&char_key("a"), false, false, false, normal_modes()),
            Some(b"a".to_vec())
        );
    }

    #[test]
    fn char_utf8_multi_byte() {
        assert_eq!(
            translate_key(&char_key("é"), false, false, false, normal_modes()),
            Some("é".as_bytes().to_vec())
        );
    }

    #[test]
    fn ctrl_c_sends_etx() {
        assert_eq!(
            translate_key(&char_key("c"), true, false, false, normal_modes()),
            Some(vec![0x03])
        );
    }

    #[test]
    fn ctrl_a_sends_soh() {
        assert_eq!(
            translate_key(&char_key("a"), true, false, false, normal_modes()),
            Some(vec![0x01])
        );
    }

    #[test]
    fn ctrl_z_sends_sub() {
        assert_eq!(
            translate_key(&char_key("z"), true, false, false, normal_modes()),
            Some(vec![0x1a])
        );
    }

    #[test]
    fn ctrl_at_sends_nul() {
        assert_eq!(
            translate_key(&char_key("@"), true, false, false, normal_modes()),
            Some(vec![0x00])
        );
    }

    #[test]
    fn ctrl_bracket_open_sends_escape() {
        assert_eq!(
            translate_key(&char_key("["), true, false, false, normal_modes()),
            Some(vec![0x1b])
        );
    }

    #[test]
    fn ctrl_backslash_sends_fs() {
        assert_eq!(
            translate_key(&char_key("\\"), true, false, false, normal_modes()),
            Some(vec![0x1c])
        );
    }

    #[test]
    fn ctrl_bracket_close_sends_gs() {
        assert_eq!(
            translate_key(&char_key("]"), true, false, false, normal_modes()),
            Some(vec![0x1d])
        );
    }

    #[test]
    fn ctrl_caret_sends_rs() {
        assert_eq!(
            translate_key(&char_key("^"), true, false, false, normal_modes()),
            Some(vec![0x1e])
        );
    }

    #[test]
    fn ctrl_underscore_sends_us() {
        assert_eq!(
            translate_key(&char_key("_"), true, false, false, normal_modes()),
            Some(vec![0x1f])
        );
    }

    #[test]
    fn alt_x_sends_esc_prefix() {
        assert_eq!(
            translate_key(&char_key("x"), false, true, false, normal_modes()),
            Some(vec![0x1b, b'x'])
        );
    }

    // Note: Ctrl+Shift+C, Ctrl+Shift+V, and other global shortcuts are now
    // intercepted by ShortcutMap in window.rs before reaching translate_key.
    // These key combinations therefore never reach translate_key in normal
    // operation. The following tests verify that if they did reach it, the
    // ctrl byte is produced (ctrl+c = 0x03, ctrl+v = 0x16).

    #[test]
    fn ctrl_shift_c_produces_ctrl_code() {
        // ShortcutMap intercepts this before it reaches translate_key.
        // If it somehow arrives here, ctrl+c maps to ETX (0x03).
        assert_eq!(
            translate_key(&char_key("c"), true, false, true, normal_modes()),
            Some(vec![0x03])
        );
    }

    #[test]
    fn ctrl_shift_v_produces_ctrl_code() {
        // ShortcutMap intercepts this before it reaches translate_key.
        // If it somehow arrives here, ctrl+v maps to 0x16.
        assert_eq!(
            translate_key(&char_key("v"), true, false, true, normal_modes()),
            Some(vec![0x16])
        );
    }

    #[test]
    fn ctrl_shift_uppercase_c_produces_ctrl_code() {
        // ShortcutMap intercepts this before it reaches translate_key.
        // If it somehow arrives here, ctrl+C maps to ETX (0x03).
        assert_eq!(
            translate_key(&char_key("C"), true, false, true, normal_modes()),
            Some(vec![0x03])
        );
    }

    #[test]
    fn arrow_up_normal_mode() {
        assert_eq!(
            translate_key(
                &Key::Named(NamedKey::ArrowUp),
                false,
                false,
                false,
                normal_modes()
            ),
            Some(b"\x1b[A".to_vec())
        );
    }

    #[test]
    fn arrow_up_app_cursor_mode() {
        assert_eq!(
            translate_key(
                &Key::Named(NamedKey::ArrowUp),
                false,
                false,
                false,
                app_cursor_modes()
            ),
            Some(b"\x1bOA".to_vec())
        );
    }

    #[test]
    fn arrow_down_normal_mode() {
        assert_eq!(
            translate_key(
                &Key::Named(NamedKey::ArrowDown),
                false,
                false,
                false,
                normal_modes()
            ),
            Some(b"\x1b[B".to_vec())
        );
    }

    #[test]
    fn arrow_right_app_cursor_mode() {
        assert_eq!(
            translate_key(
                &Key::Named(NamedKey::ArrowRight),
                false,
                false,
                false,
                app_cursor_modes()
            ),
            Some(b"\x1bOC".to_vec())
        );
    }

    #[test]
    fn arrow_left_app_cursor_mode() {
        assert_eq!(
            translate_key(
                &Key::Named(NamedKey::ArrowLeft),
                false,
                false,
                false,
                app_cursor_modes()
            ),
            Some(b"\x1bOD".to_vec())
        );
    }

    #[test]
    fn f1_key() {
        assert_eq!(
            translate_key(
                &Key::Named(NamedKey::F1),
                false,
                false,
                false,
                normal_modes()
            ),
            Some(b"\x1bOP".to_vec())
        );
    }

    #[test]
    fn f4_key() {
        assert_eq!(
            translate_key(
                &Key::Named(NamedKey::F4),
                false,
                false,
                false,
                normal_modes()
            ),
            Some(b"\x1bOS".to_vec())
        );
    }

    #[test]
    fn f5_key() {
        assert_eq!(
            translate_key(
                &Key::Named(NamedKey::F5),
                false,
                false,
                false,
                normal_modes()
            ),
            Some(b"\x1b[15~".to_vec())
        );
    }

    #[test]
    fn f12_key() {
        assert_eq!(
            translate_key(
                &Key::Named(NamedKey::F12),
                false,
                false,
                false,
                normal_modes()
            ),
            Some(b"\x1b[24~".to_vec())
        );
    }

    #[test]
    fn enter_key() {
        assert_eq!(
            translate_key(
                &Key::Named(NamedKey::Enter),
                false,
                false,
                false,
                normal_modes()
            ),
            Some(b"\r".to_vec())
        );
    }

    #[test]
    fn backspace_key() {
        assert_eq!(
            translate_key(
                &Key::Named(NamedKey::Backspace),
                false,
                false,
                false,
                normal_modes()
            ),
            Some(vec![0x7f])
        );
    }

    #[test]
    fn tab_key() {
        assert_eq!(
            translate_key(
                &Key::Named(NamedKey::Tab),
                false,
                false,
                false,
                normal_modes()
            ),
            Some(b"\x09".to_vec())
        );
    }

    #[test]
    fn shift_tab_sends_backtab() {
        assert_eq!(
            translate_key(
                &Key::Named(NamedKey::Tab),
                false,
                false,
                true, // shift
                normal_modes()
            ),
            Some(b"\x1b[Z".to_vec())
        );
    }

    #[test]
    fn escape_key() {
        assert_eq!(
            translate_key(
                &Key::Named(NamedKey::Escape),
                false,
                false,
                false,
                normal_modes()
            ),
            Some(b"\x1b".to_vec())
        );
    }

    #[test]
    fn ctrl_space_sends_nul() {
        assert_eq!(
            translate_key(
                &Key::Named(NamedKey::Space),
                true,
                false,
                false,
                normal_modes()
            ),
            Some(vec![0x00])
        );
    }

    #[test]
    fn space_no_ctrl() {
        assert_eq!(
            translate_key(
                &Key::Named(NamedKey::Space),
                false,
                false,
                false,
                normal_modes()
            ),
            Some(b" ".to_vec())
        );
    }

    #[test]
    fn home_key() {
        assert_eq!(
            translate_key(
                &Key::Named(NamedKey::Home),
                false,
                false,
                false,
                normal_modes()
            ),
            Some(b"\x1b[H".to_vec())
        );
    }

    #[test]
    fn end_key() {
        assert_eq!(
            translate_key(
                &Key::Named(NamedKey::End),
                false,
                false,
                false,
                normal_modes()
            ),
            Some(b"\x1b[F".to_vec())
        );
    }

    #[test]
    fn page_up_key() {
        assert_eq!(
            translate_key(
                &Key::Named(NamedKey::PageUp),
                false,
                false,
                false,
                normal_modes()
            ),
            Some(b"\x1b[5~".to_vec())
        );
    }

    #[test]
    fn page_down_key() {
        assert_eq!(
            translate_key(
                &Key::Named(NamedKey::PageDown),
                false,
                false,
                false,
                normal_modes()
            ),
            Some(b"\x1b[6~".to_vec())
        );
    }

    #[test]
    fn insert_key() {
        assert_eq!(
            translate_key(
                &Key::Named(NamedKey::Insert),
                false,
                false,
                false,
                normal_modes()
            ),
            Some(b"\x1b[2~".to_vec())
        );
    }

    #[test]
    fn delete_key() {
        assert_eq!(
            translate_key(
                &Key::Named(NamedKey::Delete),
                false,
                false,
                false,
                normal_modes()
            ),
            Some(b"\x1b[3~".to_vec())
        );
    }

    #[test]
    fn dead_key_returns_none() {
        assert_eq!(
            translate_key(&Key::Dead(Some('a')), false, false, false, normal_modes()),
            None
        );
    }

    #[test]
    fn bracketed_paste_enabled() {
        let handler = InputHandler::new();
        assert_eq!(
            handler.wrap_bracketed_paste("hello", bracketed_paste_modes()),
            b"\x1b[200~hello\x1b[201~".to_vec()
        );
    }

    #[test]
    fn bracketed_paste_disabled() {
        let handler = InputHandler::new();
        assert_eq!(
            handler.wrap_bracketed_paste("hello", normal_modes()),
            b"hello".to_vec()
        );
    }

    #[test]
    fn bracketed_paste_empty_string() {
        let handler = InputHandler::new();
        assert_eq!(
            handler.wrap_bracketed_paste("", bracketed_paste_modes()),
            b"\x1b[200~\x1b[201~".to_vec()
        );
    }
}
