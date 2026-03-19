---
paths:
  - "**/*.rs"
  - "resources/locales/**"
---
# Localization Rules — wmux

## i18n (IMPORTANT)
- **NEVER** hardcode user-visible strings in Rust source. All UI text goes through the i18n system.
- Locale files in `resources/locales/{lang}.toml` (fr.toml, en.toml).
- English is the fallback: if a key is missing in the active locale, use English.
- Detect system language via Windows API. Manual override in config: `language = "fr"`.
- String keys use dot notation by section: `sidebar.new_workspace`, `palette.search_placeholder`, `notification.clear_all`.
