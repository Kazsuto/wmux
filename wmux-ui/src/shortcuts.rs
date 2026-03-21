use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey};

/// Actions that can be triggered by global keyboard shortcuts.
///
/// These are intercepted before terminal input and dispatched by `window.rs`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShortcutAction {
    // Pane management
    /// Ctrl+D — split focused pane right (horizontal split).
    SplitRight,
    /// Alt+D — split focused pane down (vertical split).
    /// Note: Ctrl+Shift+D conflicts with Windows keyboard layout switcher.
    SplitDown,
    /// Ctrl+W — close focused surface/pane.
    ClosePane,
    /// Ctrl+Shift+Enter — toggle zoom on focused pane.
    ZoomToggle,

    // Focus navigation
    /// Alt+Up — move focus to the pane above.
    FocusUp,
    /// Alt+Down — move focus to the pane below.
    FocusDown,
    /// Alt+Left — move focus to the pane to the left.
    FocusLeft,
    /// Alt+Right — move focus to the pane to the right.
    FocusRight,

    // Workspace
    /// Ctrl+N — create a new workspace.
    NewWorkspace,
    /// Ctrl+1–9 — switch to workspace by 1-based index.
    SwitchWorkspace(u8),

    // Surface/Tab
    /// Ctrl+T — create a new surface in the focused pane.
    NewSurface,
    /// Ctrl+Tab — cycle surfaces forward in the focused pane.
    CycleSurfaceForward,
    /// Ctrl+Shift+Tab — cycle surfaces backward in the focused pane.
    CycleSurfaceBackward,

    // Clipboard
    /// Ctrl+Shift+C — copy selection to clipboard.
    Copy,
    /// Ctrl+Shift+V — paste from clipboard.
    Paste,

    // Sidebar
    /// Ctrl+B — toggle sidebar visibility.
    ToggleSidebar,

    // Future placeholders (detected but not yet implemented)
    /// Ctrl+Shift+P — open command palette (Task L4_01).
    CommandPalette,
    /// Ctrl+F or Ctrl+Shift+F — find/search.
    Find,
    /// F12 — toggle developer tools.
    ToggleDevTools,
}

/// Maps keyboard combinations to `ShortcutAction` values.
///
/// Priority: shortcuts are checked before terminal input. A matching shortcut
/// prevents the key from reaching the PTY.
pub struct ShortcutMap;

impl Default for ShortcutMap {
    fn default() -> Self {
        Self
    }
}

impl ShortcutMap {
    /// Create a shortcut map with the default PRD bindings.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Check whether a key event matches a global shortcut.
    ///
    /// Uses `physical_key` (KeyCode) for Ctrl+letter shortcuts to avoid issues
    /// with winit reporting control characters instead of letters on Windows.
    /// Falls back to `logical_key` for non-Ctrl shortcuts and named keys.
    #[must_use]
    pub fn match_shortcut(
        &self,
        key: &Key,
        physical_key: PhysicalKey,
        modifiers: &ModifiersState,
    ) -> Option<ShortcutAction> {
        let ctrl = modifiers.control_key();
        let shift = modifiers.shift_key();
        let alt = modifiers.alt_key();

        // Match letter shortcuts using logical_key (respects keyboard layout: AZERTY, QWERTZ, etc.)
        // PhysicalKey (KeyCode) is QWERTY-positional and wrong on non-QWERTY layouts.
        if let Key::Character(ch) = key {
            let s = ch.as_str();
            // Ctrl+letter shortcuts
            if ctrl {
                match (shift, alt, s) {
                    // Pane management
                    (false, false, "d" | "D") => return Some(ShortcutAction::SplitRight),
                    (false, false, "w" | "W") => return Some(ShortcutAction::ClosePane),

                    // Workspace
                    (false, false, "n" | "N") => return Some(ShortcutAction::NewWorkspace),

                    // Surface/Tab
                    (false, false, "t" | "T") => return Some(ShortcutAction::NewSurface),

                    // Sidebar
                    (false, false, "b" | "B") => return Some(ShortcutAction::ToggleSidebar),

                    // Clipboard
                    (true, false, "c" | "C") => return Some(ShortcutAction::Copy),
                    (true, false, "v" | "V") => return Some(ShortcutAction::Paste),

                    // Command palette
                    (true, false, "p" | "P") => return Some(ShortcutAction::CommandPalette),

                    // Find
                    (_, false, "f" | "F") => return Some(ShortcutAction::Find),

                    _ => {}
                }
            }
        }

        // Workspace switch: Ctrl+1..9 — use physical_key (Digit row is positional).
        // On AZERTY, unshifted digit row produces &é"'( etc., not digits.
        // PhysicalKey::Code(KeyCode::DigitN) is layout-independent.
        if ctrl && !shift && !alt {
            if let PhysicalKey::Code(code) = physical_key {
                match code {
                    KeyCode::Digit1 => return Some(ShortcutAction::SwitchWorkspace(1)),
                    KeyCode::Digit2 => return Some(ShortcutAction::SwitchWorkspace(2)),
                    KeyCode::Digit3 => return Some(ShortcutAction::SwitchWorkspace(3)),
                    KeyCode::Digit4 => return Some(ShortcutAction::SwitchWorkspace(4)),
                    KeyCode::Digit5 => return Some(ShortcutAction::SwitchWorkspace(5)),
                    KeyCode::Digit6 => return Some(ShortcutAction::SwitchWorkspace(6)),
                    KeyCode::Digit7 => return Some(ShortcutAction::SwitchWorkspace(7)),
                    KeyCode::Digit8 => return Some(ShortcutAction::SwitchWorkspace(8)),
                    KeyCode::Digit9 => return Some(ShortcutAction::SwitchWorkspace(9)),
                    _ => {}
                }
            }
        }

        // Alt+letter shortcuts (no Ctrl) — respects keyboard layout via logical_key
        if alt && !ctrl && !shift {
            if let Key::Character(ch) = key {
                match ch.as_str() {
                    "d" | "D" => return Some(ShortcutAction::SplitDown),
                    _ => {}
                }
            }
        }

        // Named keys (arrows, enter, tab, F-keys)
        if let Key::Named(named) = key {
            match named {
                // Alt+Arrows for focus navigation (NOT Ctrl+Alt — conflicts with
                // Intel/AMD graphics drivers screen rotation on Windows)
                NamedKey::ArrowUp if alt && !ctrl => return Some(ShortcutAction::FocusUp),
                NamedKey::ArrowDown if alt && !ctrl => return Some(ShortcutAction::FocusDown),
                NamedKey::ArrowLeft if alt && !ctrl => return Some(ShortcutAction::FocusLeft),
                NamedKey::ArrowRight if alt && !ctrl => return Some(ShortcutAction::FocusRight),
                NamedKey::Enter if ctrl && shift => return Some(ShortcutAction::ZoomToggle),
                NamedKey::Tab if ctrl && shift => {
                    return Some(ShortcutAction::CycleSurfaceBackward)
                }
                NamedKey::Tab if ctrl && !shift => {
                    return Some(ShortcutAction::CycleSurfaceForward)
                }
                NamedKey::F12 => return Some(ShortcutAction::ToggleDevTools),
                _ => {}
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::keyboard::{KeyCode, SmolStr};

    fn char_key(s: &str) -> Key {
        Key::Character(SmolStr::new(s))
    }

    fn named_key(k: NamedKey) -> Key {
        Key::Named(k)
    }

    fn phys(code: KeyCode) -> PhysicalKey {
        PhysicalKey::Code(code)
    }

    /// No physical key — for tests that only exercise named-key matching.
    fn no_phys() -> PhysicalKey {
        PhysicalKey::Unidentified(winit::keyboard::NativeKeyCode::Unidentified)
    }

    fn mods(ctrl: bool, shift: bool, alt: bool) -> ModifiersState {
        let mut m = ModifiersState::empty();
        if ctrl {
            m |= ModifiersState::CONTROL;
        }
        if shift {
            m |= ModifiersState::SHIFT;
        }
        if alt {
            m |= ModifiersState::ALT;
        }
        m
    }

    fn map() -> ShortcutMap {
        ShortcutMap::new()
    }

    #[test]
    fn ctrl_d_splits_right() {
        assert_eq!(
            map().match_shortcut(
                &char_key("d"),
                phys(KeyCode::KeyD),
                &mods(true, false, false)
            ),
            Some(ShortcutAction::SplitRight)
        );
    }

    #[test]
    fn ctrl_d_uppercase_splits_right() {
        assert_eq!(
            map().match_shortcut(
                &char_key("D"),
                phys(KeyCode::KeyD),
                &mods(true, false, false)
            ),
            Some(ShortcutAction::SplitRight)
        );
    }

    #[test]
    fn alt_d_splits_down() {
        assert_eq!(
            map().match_shortcut(
                &char_key("d"),
                phys(KeyCode::KeyD),
                &mods(false, false, true)
            ),
            Some(ShortcutAction::SplitDown)
        );
    }

    #[test]
    fn ctrl_w_closes_pane() {
        assert_eq!(
            map().match_shortcut(
                &char_key("w"),
                phys(KeyCode::KeyW),
                &mods(true, false, false)
            ),
            Some(ShortcutAction::ClosePane)
        );
    }

    #[test]
    fn ctrl_shift_enter_zoom_toggle() {
        assert_eq!(
            map().match_shortcut(
                &named_key(NamedKey::Enter),
                no_phys(),
                &mods(true, true, false)
            ),
            Some(ShortcutAction::ZoomToggle)
        );
    }

    #[test]
    fn alt_arrow_focus_up() {
        assert_eq!(
            map().match_shortcut(
                &named_key(NamedKey::ArrowUp),
                no_phys(),
                &mods(false, false, true)
            ),
            Some(ShortcutAction::FocusUp)
        );
    }

    #[test]
    fn alt_arrow_focus_down() {
        assert_eq!(
            map().match_shortcut(
                &named_key(NamedKey::ArrowDown),
                no_phys(),
                &mods(false, false, true)
            ),
            Some(ShortcutAction::FocusDown)
        );
    }

    #[test]
    fn alt_arrow_focus_left() {
        assert_eq!(
            map().match_shortcut(
                &named_key(NamedKey::ArrowLeft),
                no_phys(),
                &mods(false, false, true)
            ),
            Some(ShortcutAction::FocusLeft)
        );
    }

    #[test]
    fn alt_arrow_focus_right() {
        assert_eq!(
            map().match_shortcut(
                &named_key(NamedKey::ArrowRight),
                no_phys(),
                &mods(false, false, true)
            ),
            Some(ShortcutAction::FocusRight)
        );
    }

    #[test]
    fn ctrl_n_new_workspace() {
        assert_eq!(
            map().match_shortcut(
                &char_key("n"),
                phys(KeyCode::KeyN),
                &mods(true, false, false)
            ),
            Some(ShortcutAction::NewWorkspace)
        );
    }

    #[test]
    fn ctrl_1_through_9_switch_workspace() {
        let codes = [
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
            KeyCode::Digit5,
            KeyCode::Digit6,
            KeyCode::Digit7,
            KeyCode::Digit8,
            KeyCode::Digit9,
        ];
        for (idx, &code) in codes.iter().enumerate() {
            let i = (idx + 1) as u8;
            assert_eq!(
                map().match_shortcut(
                    &char_key(&i.to_string()),
                    phys(code),
                    &mods(true, false, false)
                ),
                Some(ShortcutAction::SwitchWorkspace(i)),
                "Ctrl+{i} should switch to workspace {i}"
            );
        }
    }

    #[test]
    fn ctrl_t_new_surface() {
        assert_eq!(
            map().match_shortcut(
                &char_key("t"),
                phys(KeyCode::KeyT),
                &mods(true, false, false)
            ),
            Some(ShortcutAction::NewSurface)
        );
    }

    #[test]
    fn ctrl_tab_cycle_forward() {
        assert_eq!(
            map().match_shortcut(
                &named_key(NamedKey::Tab),
                no_phys(),
                &mods(true, false, false)
            ),
            Some(ShortcutAction::CycleSurfaceForward)
        );
    }

    #[test]
    fn ctrl_shift_tab_cycle_backward() {
        assert_eq!(
            map().match_shortcut(
                &named_key(NamedKey::Tab),
                no_phys(),
                &mods(true, true, false)
            ),
            Some(ShortcutAction::CycleSurfaceBackward)
        );
    }

    #[test]
    fn ctrl_shift_c_copy() {
        assert_eq!(
            map().match_shortcut(
                &char_key("c"),
                phys(KeyCode::KeyC),
                &mods(true, true, false)
            ),
            Some(ShortcutAction::Copy)
        );
    }

    #[test]
    fn ctrl_shift_v_paste() {
        assert_eq!(
            map().match_shortcut(
                &char_key("v"),
                phys(KeyCode::KeyV),
                &mods(true, true, false)
            ),
            Some(ShortcutAction::Paste)
        );
    }

    #[test]
    fn ctrl_shift_p_command_palette() {
        assert_eq!(
            map().match_shortcut(
                &char_key("p"),
                phys(KeyCode::KeyP),
                &mods(true, true, false)
            ),
            Some(ShortcutAction::CommandPalette)
        );
    }

    #[test]
    fn f12_toggle_dev_tools() {
        assert_eq!(
            map().match_shortcut(
                &named_key(NamedKey::F12),
                no_phys(),
                &mods(false, false, false)
            ),
            Some(ShortcutAction::ToggleDevTools)
        );
    }

    #[test]
    fn plain_key_no_match() {
        assert_eq!(
            map().match_shortcut(
                &char_key("a"),
                phys(KeyCode::KeyA),
                &mods(false, false, false)
            ),
            None
        );
    }

    #[test]
    fn ctrl_a_no_match() {
        assert_eq!(
            map().match_shortcut(
                &char_key("a"),
                phys(KeyCode::KeyA),
                &mods(true, false, false)
            ),
            None
        );
    }

    #[test]
    fn dead_key_no_match() {
        assert_eq!(
            map().match_shortcut(&Key::Dead(Some('a')), no_phys(), &mods(true, false, false)),
            None
        );
    }

    #[test]
    fn arrow_without_modifiers_no_match() {
        assert_eq!(
            map().match_shortcut(
                &named_key(NamedKey::ArrowUp),
                no_phys(),
                &mods(false, false, false)
            ),
            None
        );
    }
}
