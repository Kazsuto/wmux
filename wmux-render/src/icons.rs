/// Centralized icon codepoint registry for UI chrome rendering.
///
/// All UI icons are rendered as text glyphs from Segoe Fluent Icons (Win11 built-in).
/// Each variant maps to a single Unicode codepoint in the Segoe Fluent Icons font.
/// Use [`GlyphonRenderer::has_icon_font()`](crate::text::GlyphonRenderer::has_icon_font)
/// to check availability at runtime before rendering.
///
/// # Usage
///
/// ```ignore
/// let attrs = Attrs::new().family(Family::Name(ICON_FONT_FAMILY));
/// buffer.set_text(font_system, Icon::Close.codepoint(), &attrs, Shaping::Advanced, None);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    // Tab bar
    /// Close/cancel button (×).
    Close,
    /// Add/plus button (+).
    Add,

    // Surface type indicators
    /// Terminal/command prompt indicator.
    Terminal,
    /// Browser/globe indicator.
    Globe,

    // Split
    /// Split pane button (column layout).
    Split,

    // Navigation arrows (split menu directions)
    /// Arrow pointing right.
    ArrowRight,
    /// Arrow pointing left.
    ArrowLeft,
    /// Arrow pointing up.
    ArrowUp,
    /// Arrow pointing down.
    ArrowDown,

    // Search
    /// Search/find (magnifying glass).
    Search,

    // Notifications
    /// Informational (i in circle).
    Info,
    /// Warning (triangle with exclamation).
    Warning,
    /// Error (X in circle).
    Error,

    // Sidebar
    /// Workspace/folder icon.
    Workspace,

    // System
    /// Settings/gear.
    Settings,
    /// Chevron right (disclosure arrow).
    ChevronRight,
    /// Chevron down (expanded disclosure).
    ChevronDown,

    // Window chrome buttons
    /// Window close button (×).
    ChromeClose,
    /// Window minimize button (—).
    ChromeMinimize,
    /// Window maximize button (□).
    ChromeMaximize,
    /// Window restore button (overlapping □).
    ChromeRestore,
}

impl Icon {
    /// Segoe Fluent Icons codepoint for this icon.
    ///
    /// Returns a `&'static str` containing the single Unicode character
    /// that maps to the icon glyph in the Segoe Fluent Icons font.
    /// These codepoints are inherited from Segoe MDL2 Assets and are
    /// stable across Windows 10 1809+ and Windows 11.
    pub const fn codepoint(&self) -> &'static str {
        match self {
            // Tab bar
            Icon::Close => "\u{e711}", // Cancel
            Icon::Add => "\u{e710}",   // Add

            // Surface type indicators
            Icon::Terminal => "\u{e756}", // CommandPrompt
            Icon::Globe => "\u{e774}",    // Globe

            // Split
            Icon::Split => "\u{e738}", // ColumnDouble (two vertical panes)

            // Navigation arrows
            Icon::ArrowRight => "\u{e72a}", // Forward
            Icon::ArrowLeft => "\u{e72b}",  // Back
            Icon::ArrowUp => "\u{e74a}",    // Up
            Icon::ArrowDown => "\u{e74b}",  // Down

            // Search
            Icon::Search => "\u{e721}", // Search

            // Notifications
            Icon::Info => "\u{e946}",    // Info
            Icon::Warning => "\u{e7ba}", // Warning
            Icon::Error => "\u{ea39}",   // StatusErrorFull

            // Sidebar
            Icon::Workspace => "\u{e8b7}", // Library

            // System
            Icon::Settings => "\u{e713}", // Settings
            Icon::ChevronRight => "\u{e76c}",
            Icon::ChevronDown => "\u{e70d}",

            // Window chrome
            Icon::ChromeClose => "\u{e8bb}",    // ChromeClose
            Icon::ChromeMinimize => "\u{e921}", // ChromeMinimize
            Icon::ChromeMaximize => "\u{e922}", // ChromeMaximize
            Icon::ChromeRestore => "\u{e923}",  // ChromeRestore
        }
    }

    /// Resolve a human-readable icon name to an `Icon` variant.
    ///
    /// Used by `StatusEntry::icon` to map IPC-provided icon names
    /// (e.g., `"check"`, `"warning"`) to renderable icon glyphs.
    /// Returns `None` for unrecognized names (caller should skip rendering).
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "check" | "success" | "ok" => Some(Icon::Info), // reuse Info until Success variant added
            "warning" | "alert" | "warn" => Some(Icon::Warning),
            "error" | "fail" | "x" => Some(Icon::Error),
            "info" | "i" | "information" => Some(Icon::Info),
            "search" | "find" => Some(Icon::Search),
            "settings" | "gear" | "config" => Some(Icon::Settings),
            "terminal" | "console" | "shell" => Some(Icon::Terminal),
            "globe" | "browser" | "web" => Some(Icon::Globe),
            "workspace" | "folder" | "project" => Some(Icon::Workspace),
            _ => None,
        }
    }

    /// CustomGlyph ID for the SVG version of this icon.
    ///
    /// Maps each `Icon` variant to its corresponding constant in
    /// [`crate::svg_icons`]. Used to construct `CustomGlyph { id, .. }`
    /// for SVG-based rendering via `prepare_with_custom()`.
    pub const fn svg_id(&self) -> u16 {
        use crate::svg_icons::*;
        match self {
            Icon::Close => ICON_CLOSE,
            Icon::Add => ICON_ADD,
            Icon::Terminal => ICON_TERMINAL,
            Icon::Globe => ICON_GLOBE,
            Icon::Split => ICON_SPLIT_H,
            Icon::ArrowRight => ICON_ARROW_RIGHT,
            Icon::ArrowLeft => ICON_ARROW_LEFT,
            Icon::ArrowUp => ICON_ARROW_UP,
            Icon::ArrowDown => ICON_ARROW_DOWN,
            Icon::Search => ICON_SEARCH,
            Icon::Info => ICON_INFO,
            Icon::Warning => ICON_WARNING,
            Icon::Error => ICON_ERROR,
            Icon::Workspace => ICON_FOLDER,
            Icon::Settings => ICON_SETTINGS,
            Icon::ChevronRight => ICON_CHEVRON_RIGHT,
            Icon::ChevronDown => ICON_CHEVRON_DOWN,
            Icon::ChromeClose => ICON_CHROME_CLOSE,
            Icon::ChromeMinimize => ICON_CHROME_MINIMIZE,
            Icon::ChromeMaximize => ICON_CHROME_MAXIMIZE,
            Icon::ChromeRestore => ICON_CHROME_RESTORE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_codepoints_are_single_char() {
        let icons = [
            Icon::Close,
            Icon::Add,
            Icon::Terminal,
            Icon::Globe,
            Icon::Split,
            Icon::ArrowRight,
            Icon::ArrowLeft,
            Icon::ArrowUp,
            Icon::ArrowDown,
            Icon::Search,
            Icon::Info,
            Icon::Warning,
            Icon::Error,
            Icon::Workspace,
            Icon::Settings,
            Icon::ChevronRight,
            Icon::ChevronDown,
            Icon::ChromeClose,
            Icon::ChromeMinimize,
            Icon::ChromeMaximize,
            Icon::ChromeRestore,
        ];
        for icon in &icons {
            let cp = icon.codepoint();
            assert_eq!(
                cp.chars().count(),
                1,
                "{icon:?} codepoint should be exactly one character, got {cp:?}"
            );
        }
    }

    #[test]
    fn codepoints_are_in_private_use_area() {
        let icons = [
            Icon::Close,
            Icon::Add,
            Icon::Terminal,
            Icon::Globe,
            Icon::Split,
            Icon::ArrowRight,
            Icon::ArrowLeft,
            Icon::ArrowUp,
            Icon::ArrowDown,
            Icon::Search,
            Icon::Info,
            Icon::Warning,
            Icon::Error,
            Icon::Workspace,
            Icon::Settings,
            Icon::ChevronRight,
            Icon::ChevronDown,
            Icon::ChromeClose,
            Icon::ChromeMinimize,
            Icon::ChromeMaximize,
            Icon::ChromeRestore,
        ];
        for icon in &icons {
            let ch = icon.codepoint().chars().next().unwrap();
            let code = ch as u32;
            // Segoe MDL2/Fluent icons live in U+E000..U+F8FF (PUA) or U+EA00+ range
            assert!(
                (0xE000..=0xF8FF).contains(&code),
                "{icon:?} codepoint U+{code:04X} is outside Private Use Area"
            );
        }
    }

    #[test]
    fn icon_is_copy_and_eq() {
        let a = Icon::Close;
        let b = a; // Copy
        assert_eq!(a, b);
    }

    #[test]
    fn all_variants_have_unique_codepoints() {
        let icons = [
            Icon::Close,
            Icon::Add,
            Icon::Terminal,
            Icon::Globe,
            Icon::Split,
            Icon::ArrowRight,
            Icon::ArrowLeft,
            Icon::ArrowUp,
            Icon::ArrowDown,
            Icon::Search,
            Icon::Info,
            Icon::Warning,
            Icon::Error,
            Icon::Workspace,
            Icon::Settings,
            Icon::ChevronRight,
            Icon::ChevronDown,
            Icon::ChromeClose,
            Icon::ChromeMinimize,
            Icon::ChromeMaximize,
            Icon::ChromeRestore,
        ];
        let mut seen = std::collections::HashSet::new();
        for icon in &icons {
            let cp = icon.codepoint();
            assert!(seen.insert(cp), "{icon:?} has duplicate codepoint {cp:?}");
        }
    }

    #[test]
    fn from_name_known_names() {
        assert_eq!(Icon::from_name("warning"), Some(Icon::Warning));
        assert_eq!(Icon::from_name("error"), Some(Icon::Error));
        assert_eq!(Icon::from_name("terminal"), Some(Icon::Terminal));
        assert_eq!(Icon::from_name("globe"), Some(Icon::Globe));
        assert_eq!(Icon::from_name("settings"), Some(Icon::Settings));
    }

    #[test]
    fn from_name_unknown_returns_none() {
        assert_eq!(Icon::from_name("nonexistent"), None);
        assert_eq!(Icon::from_name(""), None);
    }
}
