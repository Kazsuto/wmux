use thiserror::Error;

use crate::parser::parse_config;

/// Errors that can occur in theme operations.
#[derive(Debug, Error)]
pub enum ThemeError {
    #[error("theme '{name}' not found")]
    NotFound { name: String },

    #[error("invalid color value '{value}': {reason}")]
    InvalidColor { value: String, reason: String },

    #[error("failed to read theme file: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to parse theme: {0}")]
    Parse(String),
}

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

/// Derive UI chrome colors from a terminal color palette.
///
/// Surface elevation is computed by increasing HSL lightness by 5 points per level.
/// Accent is derived from ANSI blue (palette index 4) with saturation boosted to at
/// least 80%. Text hierarchy uses 65%/53%/40% alpha steps for WCAG compliance.
pub fn derive_ui_chrome(palette: &ColorPalette) -> UiChrome {
    let (br, bg, bb) = palette.background;
    let bg_r = br as f32 / 255.0;
    let bg_g = bg as f32 / 255.0;
    let bg_b = bb as f32 / 255.0;

    let (h, s, l) = rgb_to_hsl(bg_r, bg_g, bg_b);

    let surface_at_level = |level: u32| -> [f32; 4] {
        // Dark themes: increase lightness for elevation; light themes: decrease.
        let delta = if l > 0.5 {
            -(level as f32 * 0.05)
        } else {
            level as f32 * 0.05
        };
        let new_l = (l + delta).clamp(0.05, 0.95);
        let (r, g, b) = hsl_to_rgb(h, s, new_l);
        [r, g, b, 1.0]
    };

    let surface_base = [bg_r, bg_g, bg_b, 1.0];
    let surface_0 = surface_at_level(1); // +5L
    let surface_1 = surface_at_level(2); // +10L
    let surface_2 = surface_at_level(3); // +15L
    let surface_3 = surface_at_level(4); // +20L
    let surface_overlay = [surface_1[0], surface_1[1], surface_1[2], 0.95];

    // Accent from ANSI blue (index 4), boost saturation to at least 80%
    let (ar, ag, ab) = palette.ansi[4];
    let (ah, a_s, al) = rgb_to_hsl(ar as f32 / 255.0, ag as f32 / 255.0, ab as f32 / 255.0);
    let boosted_s = a_s.max(0.8);
    let (r, g, b) = hsl_to_rgb(ah, boosted_s, al);
    let accent = [r, g, b, 1.0];

    // Accent hover: shift lightness toward visible contrast (like surface elevation)
    let hover_delta = if al > 0.5 { -0.08 } else { 0.08 };
    let hover_l = (al + hover_delta).clamp(0.05, 0.95);
    let (rh, gh, bh) = hsl_to_rgb(ah, boosted_s, hover_l);
    let accent_hover = [rh, gh, bh, 1.0];

    let accent_muted = [r, g, b, 0.30];
    let accent_glow = [r, g, b, 0.25];
    let accent_glow_core = [r, g, b, 0.60];
    let accent_tint = [r, g, b, 0.08];

    // Text hierarchy from foreground (WCAG-compliant alphas)
    let (fr, fg_c, fb) = palette.foreground;
    let text_primary = u8_to_f32_color(fr, fg_c, fb);
    let tp = text_primary;
    let text_secondary = [tp[0], tp[1], tp[2], 0.65];
    let text_muted = [tp[0], tp[1], tp[2], 0.53];
    let text_faint = [tp[0], tp[1], tp[2], 0.40];
    let text_inverse = [surface_base[0], surface_base[1], surface_base[2], 1.0];

    // Borders from surface_3
    let border_subtle = [surface_3[0], surface_3[1], surface_3[2], 0.40];
    let border_default = [surface_3[0], surface_3[1], surface_3[2], 0.60];
    let border_strong = [surface_3[0], surface_3[1], surface_3[2], 0.80];
    let border_glow = [r, g, b, 0.45];

    // Overlays
    let overlay_dim = [0.0, 0.0, 0.0, 0.50];
    let overlay_tint = [r, g, b, 0.08];

    // Semantic from ANSI colors
    let error = u8_to_f32_color(palette.ansi[1].0, palette.ansi[1].1, palette.ansi[1].2);
    let error_muted = [error[0], error[1], error[2], 0.12];
    let success = u8_to_f32_color(palette.ansi[2].0, palette.ansi[2].1, palette.ansi[2].2);
    let success_muted = [success[0], success[1], success[2], 0.12];
    let warning = u8_to_f32_color(palette.ansi[3].0, palette.ansi[3].1, palette.ansi[3].2);
    let warning_muted = [warning[0], warning[1], warning[2], 0.12];
    let info = accent;
    let info_muted = [r, g, b, 0.12];

    // Selection from palette.selection
    let (sr, sg, sb) = palette.selection;
    let sel = u8_to_f32_color(sr, sg, sb);
    let selection_bg = [sel[0], sel[1], sel[2], 0.30];

    // Search highlights from warning (ANSI yellow)
    let search_match = [warning[0], warning[1], warning[2], 0.30];
    let search_match_active = [warning[0], warning[1], warning[2], 0.50];

    // Drop shadow — black with theme-adaptive alpha
    let shadow_alpha = if l > 0.5 { 0.15 } else { 0.25 };
    let shadow = [0.0, 0.0, 0.0, shadow_alpha];

    // Workspace dots from ANSI palette
    let dot_purple = u8_to_f32_color(palette.ansi[5].0, palette.ansi[5].1, palette.ansi[5].2);
    let dot_cyan = u8_to_f32_color(palette.ansi[6].0, palette.ansi[6].1, palette.ansi[6].2);

    // Cursor alpha
    let cursor_alpha = 0.85;

    UiChrome {
        surface_base,
        surface_0,
        surface_1,
        surface_2,
        surface_3,
        surface_overlay,
        accent,
        accent_hover,
        accent_muted,
        accent_glow,
        accent_glow_core,
        accent_tint,
        text_primary,
        text_secondary,
        text_muted,
        text_faint,
        text_inverse,
        border_subtle,
        border_default,
        border_strong,
        border_glow,
        overlay_dim,
        overlay_tint,
        error,
        error_muted,
        success,
        success_muted,
        warning,
        warning_muted,
        info,
        info_muted,
        selection_bg,
        search_match,
        search_match_active,
        shadow,
        dot_purple,
        dot_cyan,
        cursor_alpha,
    }
}

fn rgb_to_hsl(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;

    if (max - min).abs() < f32::EPSILON {
        return (0.0, 0.0, l);
    }

    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };

    let h = if (max - r).abs() < f32::EPSILON {
        let mut h = (g - b) / d;
        if g < b {
            h += 6.0;
        }
        h
    } else if (max - g).abs() < f32::EPSILON {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };

    (h / 6.0, s, l)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    if s.abs() < f32::EPSILON {
        return (l, l, l);
    }

    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;

    let hue_to_rgb = |t: f32| -> f32 {
        let t = t.rem_euclid(1.0);
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };

    (
        hue_to_rgb(h + 1.0 / 3.0),
        hue_to_rgb(h),
        hue_to_rgb(h - 1.0 / 3.0),
    )
}

fn u8_to_f32_color(r: u8, g: u8, b: u8) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}

// Static theme content embedded at compile time.
const BUNDLED_THEMES: &[(&str, &str)] = &[
    (
        "wmux-default",
        include_str!("../../resources/themes/wmux-default.conf"),
    ),
    (
        "catppuccin-mocha",
        include_str!("../../resources/themes/catppuccin-mocha.conf"),
    ),
    (
        "dracula",
        include_str!("../../resources/themes/dracula.conf"),
    ),
    ("nord", include_str!("../../resources/themes/nord.conf")),
    (
        "gruvbox-dark",
        include_str!("../../resources/themes/gruvbox-dark.conf"),
    ),
    (
        "one-dark",
        include_str!("../../resources/themes/one-dark.conf"),
    ),
];

/// Loads, manages, and applies themes for the terminal.
///
/// Theme search order:
/// 1. Bundled (embedded at compile time)
/// 2. User themes directory: `%APPDATA%\wmux\themes\`
#[derive(Debug)]
pub struct ThemeEngine {
    current: Theme,
}

impl ThemeEngine {
    /// Create a new `ThemeEngine` loaded with the default dark theme.
    pub fn new() -> Self {
        Self {
            current: Self::default_theme(),
        }
    }

    /// Return the wmux-default dark theme.
    pub fn default_theme() -> Theme {
        parse_theme_content("wmux-default", BUNDLED_THEMES[0].1)
    }

    /// Load a theme by name from bundled themes or the user themes directory.
    ///
    /// Returns `ThemeError::NotFound` if the theme cannot be located.
    ///
    /// # Blocking
    /// This function uses `std::fs` for user theme loading. When calling from
    /// an async context, wrap in `tokio::task::spawn_blocking`.
    pub fn load_theme(&self, name: &str) -> Result<Theme, ThemeError> {
        // Reject theme names containing path traversal characters.
        if name.contains(['/', '\\', '\0']) || name.contains("..") || name.is_empty() {
            return Err(ThemeError::NotFound {
                name: name.to_string(),
            });
        }

        // 1. Search bundled themes.
        for (bundle_name, content) in BUNDLED_THEMES {
            if *bundle_name == name {
                return Ok(parse_theme_content(name, content));
            }
        }

        // 2. Search user themes directory: %APPDATA%\wmux\themes\<name>.conf
        if let Some(appdata) = dirs::config_dir() {
            let path = appdata
                .join("wmux")
                .join("themes")
                .join(format!("{name}.conf"));
            if path.exists() {
                let content = std::fs::read_to_string(&path)?;
                return Ok(parse_theme_content(name, &content));
            }
        }

        Err(ThemeError::NotFound {
            name: name.to_string(),
        })
    }

    /// Return a reference to the currently active theme.
    #[inline]
    pub fn current_theme(&self) -> &Theme {
        &self.current
    }

    /// Switch to the named theme.
    ///
    /// Returns `ThemeError::NotFound` if the theme cannot be located.
    pub fn set_theme(&mut self, name: &str) -> Result<(), ThemeError> {
        let theme = self.load_theme(name)?;
        tracing::info!(theme = %name, "theme changed");
        self.current = theme;
        Ok(())
    }

    /// List names of all available themes (bundled + user directory).
    ///
    /// # Blocking
    /// This function uses `std::fs` to scan the user themes directory. When
    /// calling from an async context, wrap in `tokio::task::spawn_blocking`.
    pub fn list_themes(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::with_capacity(BUNDLED_THEMES.len());
        names.extend(BUNDLED_THEMES.iter().map(|(name, _)| (*name).to_string()));

        if let Some(appdata) = dirs::config_dir() {
            let themes_dir = appdata.join("wmux").join("themes");
            if let Ok(entries) = std::fs::read_dir(themes_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) != Some("conf") {
                        continue;
                    }
                    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                        continue;
                    };
                    let stem = stem.to_string();
                    if !names.contains(&stem) {
                        names.push(stem);
                    }
                }
            }
        }

        names
    }

    /// Detect whether Windows is configured for dark mode by reading the registry.
    ///
    /// Returns `true` (dark mode) when detection fails or the app is in dark mode.
    /// Returns `false` when Windows is set to light mode.
    pub fn is_dark_mode() -> bool {
        read_apps_use_light_theme().is_none_or(|light| !light)
    }
}

impl Default for ThemeEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a `#RRGGBB` hex color string into an `(r, g, b)` tuple (public API).
///
/// Used by the UI layer to apply config color overrides.
pub fn parse_hex_color_public(s: &str) -> Result<(u8, u8, u8), ThemeError> {
    parse_hex_color(s)
}

/// Parse a `#RRGGBB` hex color string into an `(r, g, b)` tuple.
///
/// Returns an error if the string is not in valid `#RRGGBB` format.
fn parse_hex_color(s: &str) -> Result<(u8, u8, u8), ThemeError> {
    let s = s.trim();
    let hex = s
        .strip_prefix('#')
        .ok_or_else(|| ThemeError::InvalidColor {
            value: s.to_string(),
            reason: "expected '#RRGGBB' format".to_string(),
        })?;

    if hex.len() != 6 {
        return Err(ThemeError::InvalidColor {
            value: s.to_string(),
            reason: format!("expected 6 hex digits, got {}", hex.len()),
        });
    }

    let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| ThemeError::InvalidColor {
        value: s.to_string(),
        reason: "invalid hex digit in red channel".to_string(),
    })?;
    let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| ThemeError::InvalidColor {
        value: s.to_string(),
        reason: "invalid hex digit in green channel".to_string(),
    })?;
    let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| ThemeError::InvalidColor {
        value: s.to_string(),
        reason: "invalid hex digit in blue channel".to_string(),
    })?;

    Ok((r, g, b))
}

/// Parse theme file content (Ghostty format) into a `Theme`.
///
/// Invalid or missing color values fall back to `ColorPalette::default()`.
fn parse_theme_content(name: &str, content: &str) -> Theme {
    let mut palette = ColorPalette::default();

    let pairs = match parse_config(content) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(theme = %name, error = %e, "failed to parse theme content, using defaults");
            return Theme {
                name: name.to_string(),
                palette,
            };
        }
    };

    for (key, value) in &pairs {
        match key.as_str() {
            "background" => match parse_hex_color(value) {
                Ok(c) => palette.background = c,
                Err(e) => {
                    tracing::warn!(theme = %name, key = %key, error = %e, "invalid color, using default")
                }
            },
            "foreground" => match parse_hex_color(value) {
                Ok(c) => palette.foreground = c,
                Err(e) => {
                    tracing::warn!(theme = %name, key = %key, error = %e, "invalid color, using default")
                }
            },
            "cursor-color" => match parse_hex_color(value) {
                Ok(c) => palette.cursor = c,
                Err(e) => {
                    tracing::warn!(theme = %name, key = %key, error = %e, "invalid color, using default")
                }
            },
            "selection-background" => match parse_hex_color(value) {
                Ok(c) => palette.selection = c,
                Err(e) => {
                    tracing::warn!(theme = %name, key = %key, error = %e, "invalid color, using default")
                }
            },
            "palette" => {
                // Format: palette = <index>=<#RRGGBB>
                if let Some((idx_str, color_str)) = value.split_once('=') {
                    match idx_str.trim().parse::<usize>() {
                        Ok(idx) if idx < 16 => match parse_hex_color(color_str.trim()) {
                            Ok(c) => palette.ansi[idx] = c,
                            Err(e) => tracing::warn!(
                                theme = %name,
                                palette_idx = idx,
                                error = %e,
                                "invalid palette color, using default"
                            ),
                        },
                        Ok(idx) => tracing::warn!(
                            theme = %name,
                            palette_idx = idx,
                            "palette index out of range (0-15), skipping"
                        ),
                        Err(_) => tracing::warn!(
                            theme = %name,
                            value = %value,
                            "invalid palette index, skipping"
                        ),
                    }
                } else {
                    tracing::warn!(theme = %name, value = %value, "palette entry missing '=', skipping");
                }
            }
            _ => {} // Unknown keys silently ignored for forward-compat
        }
    }

    Theme {
        name: name.to_string(),
        palette,
    }
}

/// Read the `AppsUseLightTheme` DWORD from the Windows registry.
///
/// Returns `Some(true)` for light mode, `Some(false)` for dark mode,
/// `None` if the registry value cannot be read.
fn read_apps_use_light_theme() -> Option<bool> {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_READ, REG_DWORD,
        REG_VALUE_TYPE,
    };

    // Registry path: HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Themes\Personalize
    let subkey: Vec<u16> = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\0"
        .encode_utf16()
        .collect();
    let value_name: Vec<u16> = "AppsUseLightTheme\0".encode_utf16().collect();

    let mut hkey = windows::Win32::System::Registry::HKEY::default();

    // SAFETY: `subkey` is a valid null-terminated UTF-16 string. `hkey` is a local
    // variable used only within this function and is closed before return.
    let open_result = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            Some(0),
            KEY_READ,
            &mut hkey,
        )
    };

    if open_result.is_err() {
        tracing::debug!("AppsUseLightTheme registry key not found, defaulting to dark mode");
        return None;
    }

    let mut data: u32 = 0;
    let mut data_size: u32 = std::mem::size_of::<u32>() as u32;
    let mut reg_type = REG_VALUE_TYPE::default();

    // SAFETY: `data` is a stack-allocated u32 cast to `*mut u8` as required by the
    // Windows API. `data_size` correctly reflects its byte length (4 bytes). `hkey`
    // was successfully opened above and remains valid for this call.
    let query_result = unsafe {
        RegQueryValueExW(
            hkey,
            PCWSTR(value_name.as_ptr()),
            None,
            Some(&mut reg_type),
            Some(&mut data as *mut u32 as *mut u8),
            Some(&mut data_size),
        )
    };

    // SAFETY: `hkey` was successfully opened above and must be closed exactly once.
    // The return value is intentionally ignored — if close fails there is nothing
    // actionable to do and we still proceed with the queried data.
    unsafe {
        let _ = RegCloseKey(hkey);
    };

    if query_result.is_err() {
        tracing::debug!("failed to query AppsUseLightTheme, defaulting to dark mode");
        return None;
    }

    if reg_type != REG_DWORD {
        tracing::debug!(
            reg_type = reg_type.0,
            "AppsUseLightTheme unexpected type, defaulting to dark mode"
        );
        return None;
    }

    // 0 = dark mode, 1 = light mode
    Some(data != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    #[test]
    fn types_are_send_and_sync() {
        assert_send::<ThemeEngine>();
        assert_sync::<ThemeEngine>();
        assert_send::<Theme>();
        assert_sync::<Theme>();
        assert_send::<ColorPalette>();
        assert_sync::<ColorPalette>();
    }

    #[test]
    fn parse_hex_color_valid() {
        assert_eq!(parse_hex_color("#1e1e2e").unwrap(), (0x1e, 0x1e, 0x2e));
        assert_eq!(parse_hex_color("#ffffff").unwrap(), (255, 255, 255));
        assert_eq!(parse_hex_color("#000000").unwrap(), (0, 0, 0));
        assert_eq!(parse_hex_color("#AABBCC").unwrap(), (0xaa, 0xbb, 0xcc));
    }

    #[test]
    fn parse_hex_color_invalid_missing_hash() {
        assert!(parse_hex_color("1e1e2e").is_err());
    }

    #[test]
    fn parse_hex_color_invalid_too_short() {
        assert!(parse_hex_color("#1e1e").is_err());
    }

    #[test]
    fn parse_hex_color_invalid_too_long() {
        assert!(parse_hex_color("#1e1e2e2e").is_err());
    }

    #[test]
    fn parse_hex_color_invalid_non_hex() {
        assert!(parse_hex_color("#zzzzzz").is_err());
    }

    #[test]
    fn default_theme_has_all_16_colors() {
        let theme = ThemeEngine::default_theme();
        assert_eq!(theme.name, "wmux-default");
        // All 16 ANSI slots must be non-default-zero after parse
        // (wmux-default.conf explicitly sets all 16)
        for (i, &color) in theme.palette.ansi.iter().enumerate() {
            assert_ne!(
                color,
                (0u8, 0u8, 0u8),
                "ANSI color {i} is zero — wmux-default.conf must set all 16 palette entries"
            );
        }
    }

    #[test]
    fn list_themes_contains_bundled() {
        let engine = ThemeEngine::new();
        let themes = engine.list_themes();
        assert!(themes.contains(&"wmux-default".to_string()));
        assert!(themes.contains(&"catppuccin-mocha".to_string()));
        assert!(themes.contains(&"dracula".to_string()));
        assert!(themes.contains(&"nord".to_string()));
        assert!(themes.contains(&"gruvbox-dark".to_string()));
        assert!(themes.contains(&"one-dark".to_string()));
    }

    #[test]
    fn load_theme_wmux_default_succeeds() {
        let engine = ThemeEngine::new();
        let theme = engine.load_theme("wmux-default").unwrap();
        assert_eq!(theme.name, "wmux-default");
        assert_eq!(theme.palette.background, (0x0d, 0x11, 0x17));
        assert_eq!(theme.palette.foreground, (0xe6, 0xed, 0xf3));
    }

    #[test]
    fn load_theme_nonexistent_returns_error() {
        let engine = ThemeEngine::new();
        let result = engine.load_theme("nonexistent-theme-xyz");
        assert!(matches!(result, Err(ThemeError::NotFound { .. })));
    }

    #[test]
    fn set_theme_changes_current() {
        let mut engine = ThemeEngine::new();
        engine.set_theme("dracula").unwrap();
        assert_eq!(engine.current_theme().name, "dracula");
    }

    #[test]
    fn set_theme_nonexistent_returns_error() {
        let mut engine = ThemeEngine::new();
        let result = engine.set_theme("no-such-theme");
        assert!(result.is_err());
        // Current theme must remain unchanged
        assert_eq!(engine.current_theme().name, "wmux-default");
    }

    #[test]
    fn is_dark_mode_does_not_panic() {
        // Just verify it doesn't crash — actual value depends on system config.
        let _ = ThemeEngine::is_dark_mode();
    }

    #[test]
    fn parse_theme_invalid_color_falls_back_gracefully() {
        let content = "background = #ZZZZZZ\nforeground = #d4d4d4\n";
        let theme = parse_theme_content("test", content);
        // Foreground must still parse correctly
        assert_eq!(theme.palette.foreground, (0xd4, 0xd4, 0xd4));
        // Background should use default (graceful degradation)
        assert_eq!(theme.palette.background, ColorPalette::default().background);
    }

    #[test]
    fn new_engine_uses_default_theme() {
        let engine = ThemeEngine::new();
        assert_eq!(engine.current_theme().name, "wmux-default");
    }

    #[test]
    fn ui_chrome_is_send_and_sync() {
        assert_send::<UiChrome>();
        assert_sync::<UiChrome>();
    }

    #[test]
    fn derive_ui_chrome_produces_valid_colors() {
        let palette = ThemeEngine::default_theme().palette;
        let chrome = derive_ui_chrome(&palette);

        // All color channels must be in 0.0..=1.0
        let all_colors = [
            chrome.surface_base,
            chrome.surface_0,
            chrome.surface_1,
            chrome.surface_2,
            chrome.surface_3,
            chrome.surface_overlay,
            chrome.accent,
            chrome.accent_hover,
            chrome.accent_muted,
            chrome.accent_glow,
            chrome.accent_glow_core,
            chrome.accent_tint,
            chrome.text_primary,
            chrome.text_secondary,
            chrome.text_muted,
            chrome.text_faint,
            chrome.text_inverse,
            chrome.border_subtle,
            chrome.border_default,
            chrome.border_strong,
            chrome.border_glow,
            chrome.overlay_dim,
            chrome.overlay_tint,
            chrome.error,
            chrome.error_muted,
            chrome.success,
            chrome.success_muted,
            chrome.warning,
            chrome.warning_muted,
            chrome.info,
            chrome.info_muted,
            chrome.selection_bg,
            chrome.search_match,
            chrome.search_match_active,
            chrome.shadow,
            chrome.dot_purple,
            chrome.dot_cyan,
        ];
        for color in all_colors {
            for (i, &c) in color.iter().enumerate() {
                assert!(
                    (0.0..=1.0).contains(&c),
                    "channel {i} out of range: {c} in color {color:?}"
                );
            }
        }
    }

    #[test]
    fn derive_ui_chrome_surface_elevation_increases() {
        let palette = ThemeEngine::default_theme().palette;
        let chrome = derive_ui_chrome(&palette);

        // Each surface level must have increasing luminance (R+G+B sum as proxy)
        let lum = |c: [f32; 4]| c[0] + c[1] + c[2];
        assert!(
            lum(chrome.surface_0) > lum(chrome.surface_base),
            "surface_0 ({}) must be lighter than surface_base ({})",
            lum(chrome.surface_0),
            lum(chrome.surface_base)
        );
        assert!(
            lum(chrome.surface_1) > lum(chrome.surface_0),
            "surface_1 ({}) must be lighter than surface_0 ({})",
            lum(chrome.surface_1),
            lum(chrome.surface_0)
        );
        assert!(
            lum(chrome.surface_2) > lum(chrome.surface_1),
            "surface_2 ({}) must be lighter than surface_1 ({})",
            lum(chrome.surface_2),
            lum(chrome.surface_1)
        );
        assert!(
            lum(chrome.surface_3) > lum(chrome.surface_2),
            "surface_3 ({}) must be lighter than surface_2 ({})",
            lum(chrome.surface_3),
            lum(chrome.surface_2)
        );
    }

    #[test]
    fn derive_ui_chrome_accent_variants_have_correct_alpha() {
        let palette = ThemeEngine::default_theme().palette;
        let chrome = derive_ui_chrome(&palette);

        assert!((chrome.accent[3] - 1.0).abs() < f32::EPSILON);
        assert!((chrome.accent_hover[3] - 1.0).abs() < f32::EPSILON);
        assert!((chrome.accent_muted[3] - 0.30).abs() < f32::EPSILON);
        assert!((chrome.accent_glow[3] - 0.25).abs() < f32::EPSILON);
        assert!((chrome.accent_glow_core[3] - 0.60).abs() < f32::EPSILON);
        assert!((chrome.accent_tint[3] - 0.08).abs() < f32::EPSILON);
        // RGB channels must match for alpha variants
        for variant in [
            chrome.accent_muted,
            chrome.accent_glow,
            chrome.accent_glow_core,
            chrome.accent_tint,
        ] {
            assert_eq!(chrome.accent[0], variant[0]);
            assert_eq!(chrome.accent[1], variant[1]);
            assert_eq!(chrome.accent[2], variant[2]);
        }
    }

    #[test]
    fn derive_ui_chrome_text_hierarchy_alphas() {
        let palette = ThemeEngine::default_theme().palette;
        let chrome = derive_ui_chrome(&palette);

        assert!((chrome.text_primary[3] - 1.0).abs() < f32::EPSILON);
        assert!((chrome.text_secondary[3] - 0.65).abs() < f32::EPSILON);
        assert!((chrome.text_muted[3] - 0.53).abs() < f32::EPSILON);
        assert!((chrome.text_faint[3] - 0.40).abs() < f32::EPSILON);
        assert!((chrome.text_inverse[3] - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn derive_ui_chrome_semantic_muted_variants() {
        let palette = ThemeEngine::default_theme().palette;
        let chrome = derive_ui_chrome(&palette);

        for (full, muted) in [
            (chrome.error, chrome.error_muted),
            (chrome.success, chrome.success_muted),
            (chrome.warning, chrome.warning_muted),
        ] {
            assert_eq!(full[0], muted[0]);
            assert_eq!(full[1], muted[1]);
            assert_eq!(full[2], muted[2]);
            assert!((muted[3] - 0.12).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn derive_ui_chrome_border_variants() {
        let palette = ThemeEngine::default_theme().palette;
        let chrome = derive_ui_chrome(&palette);

        assert!((chrome.border_subtle[3] - 0.40).abs() < f32::EPSILON);
        assert!((chrome.border_default[3] - 0.60).abs() < f32::EPSILON);
        assert!((chrome.border_strong[3] - 0.80).abs() < f32::EPSILON);
        assert!((chrome.border_glow[3] - 0.45).abs() < f32::EPSILON);
    }

    #[test]
    fn derive_ui_chrome_all_bundled_themes() {
        let engine = ThemeEngine::new();
        for name in engine.list_themes() {
            let theme = engine.load_theme(&name).unwrap();
            let chrome = derive_ui_chrome(&theme.palette);

            // Surface base must match background
            let (r, g, b) = theme.palette.background;
            let expected = [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0];
            assert!(
                (chrome.surface_base[0] - expected[0]).abs() < 0.001
                    && (chrome.surface_base[1] - expected[1]).abs() < 0.001
                    && (chrome.surface_base[2] - expected[2]).abs() < 0.001,
                "theme '{name}': surface_base doesn't match background"
            );

            // Text primary must match foreground
            let (r, g, b) = theme.palette.foreground;
            let expected = [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0];
            assert!(
                (chrome.text_primary[0] - expected[0]).abs() < 0.001
                    && (chrome.text_primary[1] - expected[1]).abs() < 0.001
                    && (chrome.text_primary[2] - expected[2]).abs() < 0.001,
                "theme '{name}': text_primary doesn't match foreground"
            );
        }
    }

    #[test]
    fn derive_ui_chrome_visual_pipeline_fields() {
        let palette = ThemeEngine::default_theme().palette;
        let chrome = derive_ui_chrome(&palette);

        // selection_bg derives from palette.selection at 30% alpha
        let (sr, sg, sb) = palette.selection;
        let expected_sel = [sr as f32 / 255.0, sg as f32 / 255.0, sb as f32 / 255.0];
        assert!((chrome.selection_bg[0] - expected_sel[0]).abs() < 0.001);
        assert!((chrome.selection_bg[1] - expected_sel[1]).abs() < 0.001);
        assert!((chrome.selection_bg[2] - expected_sel[2]).abs() < 0.001);
        assert!((chrome.selection_bg[3] - 0.30).abs() < f32::EPSILON);

        // search_match and search_match_active derive from warning
        assert_eq!(chrome.search_match[0], chrome.warning[0]);
        assert_eq!(chrome.search_match[1], chrome.warning[1]);
        assert_eq!(chrome.search_match[2], chrome.warning[2]);
        assert!((chrome.search_match[3] - 0.30).abs() < f32::EPSILON);
        assert!((chrome.search_match_active[3] - 0.50).abs() < f32::EPSILON);

        // shadow is black with alpha
        assert!((chrome.shadow[0]).abs() < f32::EPSILON);
        assert!((chrome.shadow[1]).abs() < f32::EPSILON);
        assert!((chrome.shadow[2]).abs() < f32::EPSILON);
        assert!(chrome.shadow[3] > 0.0 && chrome.shadow[3] <= 0.30);

        // cursor_alpha default
        assert!((chrome.cursor_alpha - 0.85).abs() < f32::EPSILON);
    }

    #[test]
    fn hsl_roundtrip_preserves_color() {
        let test_cases = [
            (0.5, 0.5, 0.5),    // mid gray
            (1.0, 0.0, 0.0),    // red
            (0.0, 1.0, 0.0),    // green
            (0.0, 0.0, 1.0),    // blue
            (0.12, 0.12, 0.14), // wmux-default bg approx
        ];

        for (r, g, b) in test_cases {
            let (h, s, l) = rgb_to_hsl(r, g, b);
            let (r2, g2, b2) = hsl_to_rgb(h, s, l);
            assert!(
                (r - r2).abs() < 0.01 && (g - g2).abs() < 0.01 && (b - b2).abs() < 0.01,
                "roundtrip failed for ({r}, {g}, {b}) → HSL({h}, {s}, {l}) → ({r2}, {g2}, {b2})"
            );
        }
    }
}
