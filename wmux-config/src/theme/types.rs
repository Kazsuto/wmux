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
        // Safe fallback: VS Code Dark+ inspired (matches wmux-default.conf)
        Self {
            ansi: [(0, 0, 0); 16],
            background: (0x1e, 0x1e, 0x1e),
            foreground: (0xd4, 0xd4, 0xd4),
            cursor: (0xae, 0xaf, 0xad),
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

/// Shadow depth token — Gaussian sigma and vertical offset for analytical shadows.
#[derive(Debug, Clone, Copy)]
pub struct ShadowDepth {
    /// Gaussian blur standard deviation in pixels.
    pub sigma: f32,
    /// Vertical offset in pixels (positive = downward).
    pub offset_y: f32,
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
    // ── Surface Elevation (adaptive steps) ─────────────────────────────
    /// Base surface — matches theme background (used as clear color).
    pub surface_base: [f32; 4],
    /// Elevation 0 — subtle lift (+1 step from base).
    pub surface_0: [f32; 4],
    /// Elevation 1 — sidebar bg, tab bar bg (+2 steps from base).
    pub surface_1: [f32; 4],
    /// Elevation 2 — hover, active, selections (+3 steps from base).
    pub surface_2: [f32; 4],
    /// Elevation 3 — borders, dividers (+4 steps from base).
    pub surface_3: [f32; 4],
    /// Overlay surface — surface_1 at 95% alpha (palette bg, notification panel bg).
    pub surface_overlay: [f32; 4],

    // ── Accent System ─────────────────────────────────────────────────
    /// Primary accent — ANSI blue (palette index 4) with S>=80%.
    pub accent: [f32; 4],
    /// Lighter accent — accent blended 22% toward white (hover, keywords).
    pub accent_hi: [f32; 4],
    /// Muted accent — accent at 30% alpha.
    pub accent_muted: [f32; 4],
    /// Glow outer — accent at 20% alpha (Focus Glow halo).
    pub accent_glow: [f32; 4],
    /// Glow inner — accent at 60% alpha (Focus Glow 1px ring).
    pub accent_glow_core: [f32; 4],

    // ── Attention (Amber) — used in exactly three places: unsaved tab dot,
    //    workspace-reconnecting indicator, status-bar warning state.
    //    NEVER used as a fill or next to the accent blue (confusion risk).
    /// Amber — attention color, fixed #c58a3a regardless of theme.
    pub amber: [f32; 4],
    /// Amber soft — amber at 22% alpha (dot glow, badge bg, reconnecting pill).
    pub amber_soft: [f32; 4],

    // ── Text Hierarchy ────────────────────────────────────────────────
    /// Primary text — matches theme foreground (100% alpha).
    pub text_primary: [f32; 4],
    /// Secondary text — boosted foreground at 88% alpha.
    pub text_secondary: [f32; 4],
    /// Muted text — boosted foreground at 75% alpha.
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
    /// Glow border — accent at 45% alpha (luminous separators).
    pub border_glow: [f32; 4],

    // ── Overlays ──────────────────────────────────────────────────────
    /// Dim overlay — background-tinted at 50% alpha (command palette backdrop).
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
    /// Info muted — accent at 12% alpha.
    pub info_muted: [f32; 4],

    // ── Visual Pipeline (derived) ──────────────────────────────────────
    /// Selection background — from palette.selection at 30% alpha.
    pub selection_bg: [f32; 4],
    /// Search match highlight — warning color at 30% alpha.
    pub search_match: [f32; 4],
    /// Active search match highlight — warning color at 50% alpha.
    pub search_match_active: [f32; 4],
    /// Drop shadow color — background-tinted at 20-30% alpha.
    pub shadow: [f32; 4],
    /// Shadow small — sigma=3, offset_y=1 (tab bar, status bar).
    pub shadow_sm: ShadowDepth,
    /// Shadow medium — sigma=5, offset_y=2 (sidebar, panels).
    pub shadow_md: ShadowDepth,
    /// Shadow large — sigma=8, offset_y=4 (overlays, command palette).
    pub shadow_lg: ShadowDepth,
    /// Workspace dot: purple — from ANSI magenta (palette index 5).
    pub dot_purple: [f32; 4],
    /// Workspace dot: cyan — from ANSI cyan (palette index 6).
    pub dot_cyan: [f32; 4],
    /// Cursor transparency (0.0 = invisible, 1.0 = opaque).
    pub cursor_alpha: f32,
}
