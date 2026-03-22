use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::ConfigError;
use crate::parser::{parse_config, ParsedConfig};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub font_family: String,
    pub font_size: f32,
    pub theme: String,
    pub background: Option<String>,
    pub foreground: Option<String>,
    pub palette: [Option<String>; 16],
    pub scrollback_limit: usize,
    pub cursor_style: String,
    pub keybindings: HashMap<String, String>,
    pub sidebar_width: u16,
    pub language: String,
    pub inactive_pane_opacity: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            font_family: "Cascadia Code".to_string(),
            font_size: 12.0,
            theme: "wmux-default".to_string(),
            background: None,
            foreground: None,
            palette: std::array::from_fn(|_| None),
            scrollback_limit: 4000,
            cursor_style: "block".to_string(),
            keybindings: HashMap::new(),
            sidebar_width: 200,
            language: "en".to_string(),
            inactive_pane_opacity: 0.7,
        }
    }
}

impl std::str::FromStr for Config {
    type Err = ConfigError;

    fn from_str(content: &str) -> Result<Config, ConfigError> {
        let values = parse_config(content)?;
        let mut config = Config::default();
        apply_values(&mut config, &values);
        Ok(config)
    }
}

impl Config {
    /// Load config from the system config directory.
    ///
    /// Priority chain: wmux config > Ghostty config > built-in defaults.
    /// The wmux config directory is created if it does not exist.
    ///
    /// # Blocking
    /// This function uses `std::fs` for I/O. When calling from an async
    /// context, wrap in `tokio::task::spawn_blocking`.
    pub fn load() -> Result<Config, ConfigError> {
        let config_dir = dirs::config_dir().ok_or(ConfigError::ConfigDirNotFound)?;
        let mut config = Config::default();

        // Layer 1: Ghostty config (lower priority baseline)
        let ghostty_path = config_dir.join("ghostty").join("config");
        if ghostty_path.exists() {
            match std::fs::read_to_string(&ghostty_path) {
                Ok(content) => match parse_config(&content) {
                    Ok(values) => apply_values(&mut config, &values),
                    Err(e) => tracing::warn!(
                        path = %ghostty_path.display(),
                        error = %e,
                        "failed to parse ghostty config"
                    ),
                },
                Err(e) => tracing::warn!(
                    path = %ghostty_path.display(),
                    error = %e,
                    "failed to read ghostty config"
                ),
            }
        }

        // Layer 2: wmux config (higher priority override)
        let wmux_dir = config_dir.join("wmux");
        ensure_dir(&wmux_dir)?;
        let wmux_path = wmux_dir.join("config");
        if wmux_path.exists() {
            match std::fs::read_to_string(&wmux_path) {
                Ok(content) => match parse_config(&content) {
                    Ok(values) => apply_values(&mut config, &values),
                    Err(e) => tracing::warn!(
                        path = %wmux_path.display(),
                        error = %e,
                        "failed to parse wmux config"
                    ),
                },
                Err(e) => tracing::warn!(
                    path = %wmux_path.display(),
                    error = %e,
                    "failed to read wmux config"
                ),
            }
        }

        Ok(config)
    }

    /// Overlay parsed key-value pairs on top of a base config, returning a new `Config`.
    pub fn merge(base: &Config, overlay: &ParsedConfig) -> Config {
        let mut config = base.clone();
        apply_values(&mut config, overlay);
        config
    }
}

fn ensure_dir(path: &Path) -> Result<(), ConfigError> {
    std::fs::create_dir_all(path)?;
    Ok(())
}

const MAX_SCROLLBACK: usize = 1_000_000;
const MIN_FONT_SIZE: f32 = 4.0;
const MAX_FONT_SIZE: f32 = 200.0;
const MIN_SIDEBAR_WIDTH: u16 = 1;

fn apply_values(config: &mut Config, values: &[(String, String)]) {
    for (key, value) in values {
        match key.as_str() {
            "font-family" => config.font_family = value.clone(),
            "font-size" => match value.parse::<f32>() {
                Ok(v) if v.is_finite() && (MIN_FONT_SIZE..=MAX_FONT_SIZE).contains(&v) => {
                    config.font_size = v;
                }
                Ok(v) => tracing::warn!(
                    value = v,
                    "font-size out of range ({MIN_FONT_SIZE}-{MAX_FONT_SIZE}), using default"
                ),
                Err(_) => tracing::warn!(
                    key = %key,
                    value = %value,
                    "invalid font-size value, using default"
                ),
            },
            "theme" => config.theme = value.clone(),
            "background" => config.background = Some(value.clone()),
            "foreground" => config.foreground = Some(value.clone()),
            "scrollback-limit" => match value.parse::<usize>() {
                Ok(v) if (1..=MAX_SCROLLBACK).contains(&v) => config.scrollback_limit = v,
                Ok(v) => {
                    config.scrollback_limit = v.clamp(1, MAX_SCROLLBACK);
                    tracing::warn!(
                        value = v,
                        clamped = config.scrollback_limit,
                        "scrollback-limit clamped to valid range (1-{MAX_SCROLLBACK})"
                    );
                }
                Err(_) => tracing::warn!(
                    key = %key,
                    value = %value,
                    "invalid scrollback-limit value, using default"
                ),
            },
            "cursor-style" => config.cursor_style = value.clone(),
            "sidebar-width" => match value.parse::<u16>() {
                Ok(v) if v >= MIN_SIDEBAR_WIDTH => config.sidebar_width = v,
                Ok(_) => tracing::warn!(
                    value = %value,
                    "sidebar-width must be >= {MIN_SIDEBAR_WIDTH}, using default"
                ),
                Err(_) => tracing::warn!(
                    key = %key,
                    value = %value,
                    "invalid sidebar-width value, using default"
                ),
            },
            "language" => config.language = value.clone(),
            "inactive-pane-opacity" => match value.parse::<f32>() {
                Ok(v) if v.is_finite() && (0.0..=1.0).contains(&v) => {
                    config.inactive_pane_opacity = v;
                }
                Ok(v) if v.is_finite() => {
                    config.inactive_pane_opacity = v.clamp(0.0, 1.0);
                    tracing::warn!(
                        value = v,
                        clamped = config.inactive_pane_opacity,
                        "inactive-pane-opacity clamped to 0.0-1.0"
                    );
                }
                _ => tracing::warn!(
                    key = %key,
                    value = %value,
                    "invalid inactive-pane-opacity value, using default"
                ),
            },
            "keybind" => {
                // Format: keybind = ctrl+n=new_workspace
                if let Some((kb, action)) = value.split_once('=') {
                    config
                        .keybindings
                        .insert(kb.trim().to_string(), action.trim().to_string());
                } else {
                    tracing::warn!(value = %value, "keybind missing '=', skipping");
                }
            }
            k if k.starts_with("color-") => {
                let suffix = &k["color-".len()..];
                if let Ok(idx) = suffix.parse::<usize>() {
                    if idx < 16 {
                        config.palette[idx] = Some(value.clone());
                    } else {
                        tracing::warn!(key = %k, "color index out of range (0-15), skipping");
                    }
                } else {
                    tracing::warn!(key = %k, "invalid color index, skipping");
                }
            }
            k => tracing::warn!(key = %k, "unknown config key"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn _assert_send<T: Send>() {}
    fn _assert_sync<T: Sync>() {}

    #[test]
    fn config_is_send_and_sync() {
        _assert_send::<Config>();
        _assert_sync::<Config>();
    }

    #[test]
    fn defaults_are_correct() {
        let c = Config::default();
        assert_eq!(c.font_family, "Cascadia Code");
        assert_eq!(c.font_size, 12.0);
        assert_eq!(c.theme, "wmux-default");
        assert!(c.background.is_none());
        assert!(c.foreground.is_none());
        assert_eq!(c.scrollback_limit, 4000);
        assert_eq!(c.cursor_style, "block");
        assert_eq!(c.sidebar_width, 200);
        assert_eq!(c.language, "en");
        assert!(c.keybindings.is_empty());
        assert!(c.palette.iter().all(|p| p.is_none()));
    }

    #[test]
    fn from_str_parses_known_keys() {
        let content = "font-family = Fira Code\nfont-size = 14\ntheme = dark\n";
        let c = content.parse::<Config>().unwrap();
        assert_eq!(c.font_family, "Fira Code");
        assert_eq!(c.font_size, 14.0);
        assert_eq!(c.theme, "dark");
    }

    #[test]
    fn from_str_empty_gives_defaults() {
        let c = "".parse::<Config>().unwrap();
        assert_eq!(c.font_family, "Cascadia Code");
        assert_eq!(c.font_size, 12.0);
    }

    #[test]
    fn from_str_comments_only_gives_defaults() {
        let c = "# just a comment\n# another\n".parse::<Config>().unwrap();
        assert_eq!(c.font_size, 12.0);
        assert_eq!(c.theme, "wmux-default");
    }

    #[test]
    fn unknown_keys_do_not_error() {
        let content = "totally-unknown-key = some-value\nfont-size = 13\n";
        let c = content.parse::<Config>().unwrap();
        assert_eq!(c.font_size, 13.0);
    }

    #[test]
    fn invalid_font_size_keeps_default() {
        let content = "font-size = not-a-number\n";
        let c = content.parse::<Config>().unwrap();
        assert_eq!(c.font_size, 12.0);
    }

    #[test]
    fn invalid_scrollback_limit_keeps_default() {
        let content = "scrollback-limit = abc\n";
        let c = content.parse::<Config>().unwrap();
        assert_eq!(c.scrollback_limit, 4000);
    }

    #[test]
    fn invalid_sidebar_width_keeps_default() {
        let content = "sidebar-width = xyz\n";
        let c = content.parse::<Config>().unwrap();
        assert_eq!(c.sidebar_width, 200);
    }

    #[test]
    fn keybind_parsing() {
        let content = "keybind = ctrl+n=new_workspace\n";
        let c = content.parse::<Config>().unwrap();
        assert_eq!(
            c.keybindings.get("ctrl+n"),
            Some(&"new_workspace".to_string())
        );
    }

    #[test]
    fn palette_parsing() {
        let content = "color-0 = #000000\ncolor-15 = #ffffff\n";
        let c = content.parse::<Config>().unwrap();
        assert_eq!(c.palette[0], Some("#000000".to_string()));
        assert_eq!(c.palette[15], Some("#ffffff".to_string()));
    }

    #[test]
    fn palette_out_of_range_skipped() {
        let content = "color-16 = #aabbcc\n";
        let c = content.parse::<Config>().unwrap();
        assert!(c.palette.iter().all(|p| p.is_none()));
    }

    #[test]
    fn merge_overlay_takes_precedence() {
        let base = Config::default();
        let overlay = vec![
            ("font-family".to_string(), "Consolas".to_string()),
            ("font-size".to_string(), "16".to_string()),
        ];
        let merged = Config::merge(&base, &overlay);
        assert_eq!(merged.font_family, "Consolas");
        assert_eq!(merged.font_size, 16.0);
        // Untouched fields keep base values
        assert_eq!(merged.theme, "wmux-default");
        assert_eq!(merged.scrollback_limit, 4000);
    }

    #[test]
    fn font_size_nan_rejected() {
        let content = "font-size = NaN\n";
        let c = content.parse::<Config>().unwrap();
        assert_eq!(c.font_size, 12.0); // default
    }

    #[test]
    fn font_size_negative_rejected() {
        let content = "font-size = -5\n";
        let c = content.parse::<Config>().unwrap();
        assert_eq!(c.font_size, 12.0); // default
    }

    #[test]
    fn font_size_zero_rejected() {
        let content = "font-size = 0\n";
        let c = content.parse::<Config>().unwrap();
        assert_eq!(c.font_size, 12.0); // default
    }

    #[test]
    fn scrollback_limit_capped() {
        let content = "scrollback-limit = 999999999\n";
        let c = content.parse::<Config>().unwrap();
        assert_eq!(c.scrollback_limit, 1_000_000);
    }

    #[test]
    fn multiple_keybinds_preserved() {
        let content =
            "keybind = ctrl+n=new_workspace\nkeybind = ctrl+t=new_tab\nkeybind = ctrl+w=close\n";
        let c = content.parse::<Config>().unwrap();
        assert_eq!(c.keybindings.len(), 3);
        assert_eq!(c.keybindings.get("ctrl+n"), Some(&"new_workspace".into()));
        assert_eq!(c.keybindings.get("ctrl+t"), Some(&"new_tab".into()));
        assert_eq!(c.keybindings.get("ctrl+w"), Some(&"close".into()));
    }

    #[test]
    fn background_foreground_parsing() {
        let content = "background = #1e1e1e\nforeground = #d4d4d4\n";
        let c = content.parse::<Config>().unwrap();
        assert_eq!(c.background, Some("#1e1e1e".to_string()));
        assert_eq!(c.foreground, Some("#d4d4d4".to_string()));
    }

    #[test]
    fn scrollback_and_sidebar_parsing() {
        let content = "scrollback-limit = 10000\nsidebar-width = 300\n";
        let c = content.parse::<Config>().unwrap();
        assert_eq!(c.scrollback_limit, 10000);
        assert_eq!(c.sidebar_width, 300);
    }

    #[test]
    fn language_parsing() {
        let content = "language = fr\n";
        let c = content.parse::<Config>().unwrap();
        assert_eq!(c.language, "fr");
    }
}
