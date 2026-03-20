use crate::ConfigError;

/// A parsed key-value pair from a Ghostty-format config file.
///
/// Uses a `Vec` instead of a `HashMap` to preserve multi-value keys
/// like `keybind` which can appear multiple times in the config.
pub type ParsedConfig = Vec<(String, String)>;

/// Parse Ghostty-format config content into a list of key-value pairs.
///
/// - Lines starting with `#` are comments and are skipped.
/// - Empty lines are skipped.
/// - Format: `key = value` (whitespace trimmed).
/// - Quoted string values have surrounding `"` stripped.
/// - A leading UTF-8 BOM (`\u{FEFF}`) is stripped if present.
/// - Lines without `=` are logged via `tracing::warn!` and skipped.
/// - Unknown keys are logged but do not cause an error.
/// - Repeated keys (e.g., `keybind`) are preserved — all values are returned.
pub fn parse_config(content: &str) -> Result<ParsedConfig, ConfigError> {
    let content = content.strip_prefix('\u{FEFF}').unwrap_or(content);
    let mut pairs = Vec::with_capacity(content.lines().count());

    for (idx, raw_line) in content.lines().enumerate() {
        let line_num = idx + 1;
        let line = raw_line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        match line.split_once('=') {
            Some((key, value)) => {
                let key = key.trim().to_string();
                let value = strip_quotes(value.trim()).to_string();

                if key.is_empty() {
                    tracing::warn!(line = line_num, "config line has empty key, skipping");
                    continue;
                }

                pairs.push((key, value));
            }
            None => {
                tracing::warn!(line = line_num, "config line has no '=', skipping");
            }
        }
    }

    Ok(pairs)
}

fn strip_quotes(s: &str) -> &str {
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find_value<'a>(pairs: &'a ParsedConfig, key: &str) -> Option<&'a str> {
        pairs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    fn find_all_values<'a>(pairs: &'a ParsedConfig, key: &str) -> Vec<&'a str> {
        pairs
            .iter()
            .filter(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .collect()
    }

    #[test]
    fn parses_key_value() {
        let input = "font-size = 14\ntheme = dark\n";
        let pairs = parse_config(input).unwrap();
        assert_eq!(find_value(&pairs, "font-size"), Some("14"));
        assert_eq!(find_value(&pairs, "theme"), Some("dark"));
    }

    #[test]
    fn skips_comments() {
        let input = "# this is a comment\nfont-size = 12\n";
        let pairs = parse_config(input).unwrap();
        assert!(find_value(&pairs, "# this is a comment").is_none());
        assert_eq!(find_value(&pairs, "font-size"), Some("12"));
    }

    #[test]
    fn skips_empty_lines() {
        let input = "\n\nfont-size = 12\n\n";
        let pairs = parse_config(input).unwrap();
        assert_eq!(pairs.len(), 1);
        assert_eq!(find_value(&pairs, "font-size"), Some("12"));
    }

    #[test]
    fn strips_quoted_values() {
        let input = r#"font-family = "Cascadia Code""#;
        let pairs = parse_config(input).unwrap();
        assert_eq!(find_value(&pairs, "font-family"), Some("Cascadia Code"));
    }

    #[test]
    fn strips_bom() {
        let input = "\u{FEFF}font-size = 12\n";
        let pairs = parse_config(input).unwrap();
        assert_eq!(find_value(&pairs, "font-size"), Some("12"));
    }

    #[test]
    fn trims_whitespace() {
        let input = "  font-size  =  16  \n";
        let pairs = parse_config(input).unwrap();
        assert_eq!(find_value(&pairs, "font-size"), Some("16"));
    }

    #[test]
    fn value_with_equals_splits_on_first() {
        let input = "keybind = ctrl+n=new_workspace\n";
        let pairs = parse_config(input).unwrap();
        assert_eq!(find_value(&pairs, "keybind"), Some("ctrl+n=new_workspace"));
    }

    #[test]
    fn handles_line_without_equals() {
        let input = "no-equals-here\nfont-size = 12\n";
        let pairs = parse_config(input).unwrap();
        assert!(find_value(&pairs, "no-equals-here").is_none());
        assert_eq!(find_value(&pairs, "font-size"), Some("12"));
    }

    #[test]
    fn empty_input_returns_empty_list() {
        let pairs = parse_config("").unwrap();
        assert!(pairs.is_empty());
    }

    #[test]
    fn comments_only_returns_empty_list() {
        let input = "# comment 1\n# comment 2\n";
        let pairs = parse_config(input).unwrap();
        assert!(pairs.is_empty());
    }

    #[test]
    fn preserves_multiple_keybinds() {
        let input =
            "keybind = ctrl+n=new_workspace\nkeybind = ctrl+t=new_tab\nkeybind = ctrl+w=close\n";
        let pairs = parse_config(input).unwrap();
        let keybinds = find_all_values(&pairs, "keybind");
        assert_eq!(keybinds.len(), 3);
        assert_eq!(keybinds[0], "ctrl+n=new_workspace");
        assert_eq!(keybinds[1], "ctrl+t=new_tab");
        assert_eq!(keybinds[2], "ctrl+w=close");
    }
}
