/// 16-color ANSI palette plus background, foreground, cursor, and selection.
/// All colors are stored as `(r, g, b)` tuples.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorPalette {
    /// ANSI colors 0-15.
    pub ansi: [(u8, u8, u8); 16],
    /// Terminal background color.
    pub background: (u8, u8, u8),
    /// Terminal foreground (default text) color.
    pub foreground: (u8, u8, u8),
    /// Cursor color.
    pub cursor: (u8, u8, u8),
    /// Selection background color.
    pub selection: (u8, u8, u8),
}

impl Default for ColorPalette {
    fn default() -> Self {
        // Safe fallback: dark anthracite (matches wmux-default.conf)
        Self {
            ansi: [(0, 0, 0); 16],
            background: (0x0d, 0x11, 0x17),
            foreground: (0xe6, 0xed, 0xf3),
            cursor: (0xe6, 0xed, 0xf3),
            selection: (0x26, 0x4f, 0x78),
        }
    }
}

/// A named terminal theme comprising a color palette.
#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub palette: ColorPalette,
}

/// UI chrome colors derived from the terminal color palette.
///
/// These colors are used for all non-terminal UI elements: sidebar, tab bar,
/// command palette, borders, overlays. They are automatically computed from
/// the current theme's [`ColorPalette`] to ensure visual coherence.
///
/// Design system: "Luminous Void" — refined-dark with Focus Glow signature.
#[derive(Debug, Clone, Copy)]
pub struct UiChrome {
    // ── Surface Elevation (5L steps) ──────────────────────────────────
    /// Base surface — matches theme background (used as clear color).
    pub surface_base: [f32; 4],
    /// Elevation 0 — subtle lift (+5L from base).
    pub surface_0: [f32; 4],
    /// Elevation 1 — sidebar bg, tab bar bg (+10L from base).
    pub surface_1: [f32; 4],
    /// Elevation 2 — hover, active, selections (+15L from base).
    pub surface_2: [f32; 4],
    /// Elevation 3 — borders, dividers (+20L from base).
    pub surface_3: [f32; 4],
    /// Overlay surface — surface_1 at 95% alpha (palette bg, notification panel bg).
    pub surface_overlay: [f32; 4],

    // ── Accent System ─────────────────────────────────────────────────
    /// Primary accent — ANSI blue (palette index 4) with S>=80%.
    pub accent: [f32; 4],
    /// Hover accent — accent + 8L lightness.
    pub accent_hover: [f32; 4],
    /// Muted accent — accent at 30% alpha.
    pub accent_muted: [f32; 4],
    /// Glow outer — accent at 25% alpha (Focus Glow halo).
    pub accent_glow: [f32; 4],
    /// Glow inner — accent at 60% alpha (Focus Glow 1px ring).
    pub accent_glow_core: [f32; 4],
    /// Tint — accent at 8% alpha (overlay ambient coloring).
    pub accent_tint: [f32; 4],

    // ── Text Hierarchy ────────────────────────────────────────────────
    /// Primary text — matches theme foreground (100% alpha).
    pub text_primary: [f32; 4],
    /// Secondary text — foreground at 65% alpha.
    pub text_secondary: [f32; 4],
    /// Muted text — foreground at 53% alpha.
    pub text_muted: [f32; 4],
    /// Faint text — foreground at 40% alpha (decorative metadata only).
    pub text_faint: [f32; 4],
    /// Inverse text — surface_base color for use on accent backgrounds.
    pub text_inverse: [f32; 4],

    // ── Borders ───────────────────────────────────────────────────────
    /// Subtle border — surface_3 at 40% alpha.
    pub border_subtle: [f32; 4],
    /// Default border — surface_3 at 60% alpha.
    pub border_default: [f32; 4],
    /// Strong border — surface_3 at 80% alpha.
    pub border_strong: [f32; 4],
    /// Glow border — accent at 45% alpha (luminous separators).
    pub border_glow: [f32; 4],

    // ── Overlays ──────────────────────────────────────────────────────
    /// Dim overlay — black at 50% alpha (command palette backdrop).
    pub overlay_dim: [f32; 4],
    /// Tint overlay — accent at 8% alpha (layered on overlay_dim).
    pub overlay_tint: [f32; 4],

    // ── Semantic Colors ───────────────────────────────────────────────
    /// Error — from ANSI red (palette index 1).
    pub error: [f32; 4],
    /// Error muted — error at 12% alpha.
    pub error_muted: [f32; 4],
    /// Success — from ANSI green (palette index 2).
    pub success: [f32; 4],
    /// Success muted — success at 12% alpha.
    pub success_muted: [f32; 4],
    /// Warning — from ANSI yellow (palette index 3).
    pub warning: [f32; 4],
    /// Warning muted — warning at 12% alpha.
    pub warning_muted: [f32; 4],
    /// Info — same as accent.
    pub info: [f32; 4],
    /// Info muted — accent at 12% alpha.
    pub info_muted: [f32; 4],

    // ── Visual Pipeline (derived) ──────────────────────────────────────
    /// Selection background — from palette.selection at 30% alpha.
    pub selection_bg: [f32; 4],
    /// Search match highlight — warning color at 30% alpha.
    pub search_match: [f32; 4],
    /// Active search match highlight — warning color at 50% alpha.
    pub search_match_active: [f32; 4],
    /// Drop shadow color — black at 25% alpha (dark themes).
    pub shadow: [f32; 4],
    /// Workspace dot: purple — from ANSI magenta (palette index 5).
    pub dot_purple: [f32; 4],
    /// Workspace dot: cyan — from ANSI cyan (palette index 6).
    pub dot_cyan: [f32; 4],
    /// Cursor transparency (0.0 = invisible, 1.0 = opaque).
    pub cursor_alpha: f32,
}
