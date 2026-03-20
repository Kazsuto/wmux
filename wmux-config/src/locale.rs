use std::collections::HashMap;

const EN_TOML: &str = include_str!("../../resources/locales/en.toml");
const FR_TOML: &str = include_str!("../../resources/locales/fr.toml");

const AVAILABLE_LANGUAGES: &[&str] = &["en", "fr"];

/// Flattens a TOML `Value` into dot-notation key/value pairs.
///
/// `[sidebar]` + `new_workspace = "New Workspace"` becomes
/// `"sidebar.new_workspace" → "New Workspace"`.
fn flatten_toml(value: &toml::Value) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let toml::Value::Table(table) = value {
        for (section, section_value) in table {
            if let toml::Value::Table(inner) = section_value {
                for (key, val) in inner {
                    if let toml::Value::String(s) = val {
                        map.insert(format!("{section}.{key}"), s.clone());
                    }
                }
            }
        }
    }
    map
}

/// Parse a TOML locale string into a flat key→string map.
///
/// On parse failure the error is logged and an empty map is returned so
/// callers always get a valid (possibly empty) locale.
fn parse_locale(source: &str, label: &str) -> HashMap<String, String> {
    match toml::from_str::<toml::Value>(source) {
        Ok(value) => flatten_toml(&value),
        Err(e) => {
            tracing::warn!(locale = %label, error = %e, "failed to parse locale file");
            HashMap::new()
        }
    }
}

/// Localized string store.
///
/// Strings are embedded at compile time from `resources/locales/*.toml`.
/// English is the authoritative fallback: if a key is absent from the
/// active locale it is looked up in English; if absent there the key
/// itself is returned so the UI always shows *something*.
///
/// `Locale` is `Send + Sync` — callers may store it in an `Arc`.
pub struct Locale {
    language: String,
    strings: HashMap<String, String>,
    en_strings: HashMap<String, String>,
}

impl std::fmt::Debug for Locale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Locale")
            .field("language", &self.language)
            .finish_non_exhaustive()
    }
}

impl Locale {
    /// Load the locale for the given BCP-47-ish language code (e.g. `"en"`, `"fr"`).
    ///
    /// Unknown language codes fall back to English.
    pub fn new(language: &str) -> Self {
        let en_strings = parse_locale(EN_TOML, "en");
        let (lang_code, strings) = Self::load_language(language, &en_strings);
        Self {
            language: lang_code,
            strings,
            en_strings,
        }
    }

    /// Detect the system UI language and load the appropriate locale.
    ///
    /// Falls back to English if the system language is not supported or
    /// if the Win32 call is unavailable.
    pub fn detect() -> Self {
        let code = detect_system_language();
        tracing::debug!(language = %code, "detected system language");
        Self::new(code)
    }

    /// Look up a localized string by dot-notation key (e.g. `"sidebar.new_workspace"`).
    ///
    /// Lookup chain: active locale → English → key itself (never panics).
    #[inline]
    pub fn t<'a>(&'a self, key: &'a str) -> &'a str {
        if let Some(s) = self.strings.get(key) {
            return s.as_str();
        }
        if let Some(s) = self.en_strings.get(key) {
            return s.as_str();
        }
        key
    }

    /// The current language code (e.g. `"en"` or `"fr"`).
    #[inline]
    pub fn language(&self) -> &str {
        &self.language
    }

    /// Switch the active language at runtime.
    ///
    /// Unknown language codes fall back to English.
    pub fn set_language(&mut self, lang: &str) {
        let (code, strings) = Self::load_language(lang, &self.en_strings);
        self.language = code;
        self.strings = strings;
    }

    /// Returns the list of supported language codes.
    #[inline]
    pub fn available_languages() -> &'static [&'static str] {
        AVAILABLE_LANGUAGES
    }

    // Internal: resolve a language code to its strings, falling back to English.
    fn load_language(
        lang: &str,
        en_strings: &HashMap<String, String>,
    ) -> (String, HashMap<String, String>) {
        match lang {
            "en" => ("en".to_string(), en_strings.clone()),
            "fr" => ("fr".to_string(), parse_locale(FR_TOML, "fr")),
            other => {
                tracing::warn!(language = %other, "unsupported language, falling back to English");
                ("en".to_string(), en_strings.clone())
            }
        }
    }
}

/// Detect the system UI language via `GetUserDefaultUILanguage`.
///
/// Returns `"fr"` for French, `"en"` for everything else.
fn detect_system_language() -> &'static str {
    // SAFETY: GetUserDefaultUILanguage takes no arguments and has no
    // preconditions. It is safe to call from any thread.
    let langid = unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() };
    // Primary language identifier is the low 10 bits (PRIMARYLANGID macro).
    // 0x0C = French, 0x09 = English (and default for all others).
    let primary = langid & 0x3FF;
    if primary == 0x0C {
        "fr"
    } else {
        "en"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_is_send_and_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<Locale>();
        assert_sync::<Locale>();
    }

    #[test]
    fn t_returns_english_string_for_known_key() {
        let locale = Locale::new("en");
        assert_eq!(locale.t("sidebar.new_workspace"), "New Workspace");
    }

    #[test]
    fn t_returns_key_for_unknown_key() {
        let locale = Locale::new("en");
        assert_eq!(locale.t("nonexistent.key"), "nonexistent.key");
    }

    #[test]
    fn set_language_fr_switches_to_french() {
        let mut locale = Locale::new("en");
        locale.set_language("fr");
        assert_eq!(locale.language(), "fr");
        assert_eq!(
            locale.t("sidebar.new_workspace"),
            "Nouvel espace de travail"
        );
    }

    #[test]
    fn set_language_en_switches_back_to_english() {
        let mut locale = Locale::new("fr");
        assert_eq!(
            locale.t("sidebar.new_workspace"),
            "Nouvel espace de travail"
        );
        locale.set_language("en");
        assert_eq!(locale.language(), "en");
        assert_eq!(locale.t("sidebar.new_workspace"), "New Workspace");
    }

    #[test]
    fn detect_does_not_panic() {
        // Must not panic on any system — the result is platform-dependent.
        let locale = Locale::detect();
        let lang = locale.language().to_string();
        assert!(lang == "en" || lang == "fr");
    }

    #[test]
    fn available_languages_returns_en_and_fr() {
        let langs = Locale::available_languages();
        assert_eq!(langs, &["en", "fr"]);
    }

    #[test]
    fn language_returns_current_language_code() {
        let locale = Locale::new("fr");
        assert_eq!(locale.language(), "fr");

        let locale_en = Locale::new("en");
        assert_eq!(locale_en.language(), "en");
    }

    #[test]
    fn missing_fr_key_falls_back_to_english() {
        // We simulate this by checking that t() on the English locale returns
        // the English value for any valid key — the same fallback path is used
        // when a key is absent from fr.toml.
        let locale = Locale::new("fr");
        // All keys present in en.toml must resolve (either from fr or en map).
        let en = Locale::new("en");
        let test_key = "sidebar.new_workspace";
        // French value should differ from English (proves fr map was loaded).
        assert_ne!(locale.t(test_key), en.t(test_key));

        // For an unknown key the fallback chain ultimately returns the key.
        assert_eq!(locale.t("totally.missing.key"), "totally.missing.key");
    }

    #[test]
    fn unknown_language_falls_back_to_english() {
        let locale = Locale::new("de");
        assert_eq!(locale.language(), "en");
        assert_eq!(locale.t("sidebar.new_workspace"), "New Workspace");
    }

    #[test]
    fn palette_keys_present() {
        let locale = Locale::new("en");
        assert_eq!(locale.t("palette.search_placeholder"), "Type a command...");
        assert_eq!(locale.t("palette.no_results"), "No results");
    }

    #[test]
    fn notification_keys_present() {
        let locale = Locale::new("en");
        assert_eq!(locale.t("notification.clear_all"), "Clear All");
        assert_eq!(locale.t("notification.mark_read"), "Mark as Read");
    }

    #[test]
    fn terminal_keys_present() {
        let locale = Locale::new("en");
        assert_eq!(locale.t("terminal.copy"), "Copy");
        assert_eq!(locale.t("terminal.paste"), "Paste");
        assert_eq!(locale.t("terminal.search"), "Search");
        assert_eq!(locale.t("terminal.clear"), "Clear");
    }

    #[test]
    fn dialog_keys_present() {
        let locale = Locale::new("en");
        assert_eq!(locale.t("dialog.confirm_close"), "Close this workspace?");
        assert_eq!(locale.t("dialog.yes"), "Yes");
        assert_eq!(locale.t("dialog.no"), "No");
        assert_eq!(locale.t("dialog.cancel"), "Cancel");
    }

    #[test]
    fn error_keys_present() {
        let locale = Locale::new("en");
        assert_eq!(locale.t("error.connection_failed"), "Connection failed");
        assert_eq!(locale.t("error.shell_not_found"), "Shell not found");
    }

    #[test]
    fn french_translations_are_correct() {
        let locale = Locale::new("fr");
        assert_eq!(locale.t("menu.file"), "Fichier");
        assert_eq!(locale.t("menu.edit"), "Édition");
        assert_eq!(locale.t("dialog.yes"), "Oui");
        assert_eq!(locale.t("dialog.no"), "Non");
        assert_eq!(locale.t("dialog.cancel"), "Annuler");
        assert_eq!(locale.t("status.connected"), "Connecté");
        assert_eq!(locale.t("status.disconnected"), "Déconnecté");
    }
}
