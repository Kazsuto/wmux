use bitflags::bitflags;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

bitflags! {
    /// Per-terminal mode flags controlling input/output behavior.
    ///
    /// Each pane maintains independent mode state.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct TerminalMode: u16 {
        /// DEC origin mode (DECOM). Cursor addressing relative to scroll region.
        const ORIGIN            = 1 << 0;
        /// Auto-wrap mode (DECAWM). Wrap to next line at right margin.
        const WRAPAROUND        = 1 << 1;
        /// Bracketed paste mode. Wrap pasted text in escape sequences.
        const BRACKETED_PASTE   = 1 << 2;
        /// Application cursor keys (DECCKM). Arrow keys send application sequences.
        const APPLICATION_CURSOR = 1 << 3;
        /// Mouse reporting mode. Terminal reports mouse events to the application.
        const MOUSE_REPORTING   = 1 << 4;
    }
}

impl Default for TerminalMode {
    /// Default mode: WRAPAROUND on, all others off.
    fn default() -> Self {
        Self::WRAPAROUND
    }
}

impl Serialize for TerminalMode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.bits().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TerminalMode {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let bits = u16::deserialize(deserializer)?;
        Ok(Self::from_bits_truncate(bits))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_wraparound_only() {
        let mode = TerminalMode::default();
        assert!(mode.contains(TerminalMode::WRAPAROUND));
        assert!(!mode.contains(TerminalMode::ORIGIN));
        assert!(!mode.contains(TerminalMode::BRACKETED_PASTE));
        assert!(!mode.contains(TerminalMode::APPLICATION_CURSOR));
        assert!(!mode.contains(TerminalMode::MOUSE_REPORTING));
    }

    #[test]
    fn set_and_clear_individual_bits() {
        let mut mode = TerminalMode::default();

        mode.insert(TerminalMode::BRACKETED_PASTE);
        assert!(mode.contains(TerminalMode::BRACKETED_PASTE));
        assert!(mode.contains(TerminalMode::WRAPAROUND));

        mode.remove(TerminalMode::WRAPAROUND);
        assert!(!mode.contains(TerminalMode::WRAPAROUND));
        assert!(mode.contains(TerminalMode::BRACKETED_PASTE));
    }

    #[test]
    fn combine_flags() {
        let mode = TerminalMode::ORIGIN | TerminalMode::MOUSE_REPORTING;
        assert!(mode.contains(TerminalMode::ORIGIN));
        assert!(mode.contains(TerminalMode::MOUSE_REPORTING));
        assert!(!mode.contains(TerminalMode::WRAPAROUND));
    }

    #[test]
    fn serde_roundtrip() {
        let mode = TerminalMode::WRAPAROUND | TerminalMode::BRACKETED_PASTE;
        let json = serde_json::to_string(&mode).unwrap();
        let back: TerminalMode = serde_json::from_str(&json).unwrap();
        assert_eq!(mode, back);
    }
}
