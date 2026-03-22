mod chrome;
mod registry;
mod types;

pub use chrome::derive_ui_chrome;
pub use types::{ColorPalette, Theme, UiChrome};

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

// Static theme content embedded at compile time.
const BUNDLED_THEMES: &[(&str, &str)] = &[
    (
        "wmux-default",
        include_str!("../../../resources/themes/wmux-default.conf"),
    ),
    (
        "catppuccin-mocha",
        include_str!("../../../resources/themes/catppuccin-mocha.conf"),
    ),
    (
        "dracula",
        include_str!("../../../resources/themes/dracula.conf"),
    ),
    ("nord", include_str!("../../../resources/themes/nord.conf")),
    (
        "gruvbox-dark",
        include_str!("../../../resources/themes/gruvbox-dark.conf"),
    ),
    (
        "one-dark",
        include_str!("../../../resources/themes/one-dark.conf"),
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
        registry::read_apps_use_light_theme().is_none_or(|light| !light)
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
    fn ui_chrome_is_send_and_sync() {
        assert_send::<UiChrome>();
        assert_sync::<UiChrome>();
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
}
