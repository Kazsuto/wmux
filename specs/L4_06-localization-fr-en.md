---
task_id: L4_06
title: "Implement Localization FR/EN"
status: pending
priority: P2
estimated_hours: 2
wave: 2
prd_features: [F-16]
archi_sections: [ADR-0001, ADR-0010]
depends_on: [L3_11]
blocks: [L4_07]
---

# Task L4_06: Implement Localization FR/EN

> **Phase**: Polish
> **Priority**: P2-Medium
> **Estimated effort**: 2 hours
> **Wave**: 2

## Context
wmux supports French and English with auto-detection of system language. All user-visible strings come from locale TOML files. Architecture §3 Cross-Cutting Concerns specifies the i18n approach. PRD §16 requires 100% string coverage.

## Prerequisites
- [ ] Task L3_11: Ghostty Config Parser — provides `language` config setting

## Scope
### Deliverables
- Locale TOML files: `resources/locales/en.toml`, `resources/locales/fr.toml`
- All user-visible strings extracted to locale files
- System language detection via `GetUserDefaultUILanguage` Win32 API
- Manual override via `language = "fr"` in config
- Runtime language switch (AppEvent::LanguageChanged)
- String lookup function: `t("sidebar.new_workspace")` → localized string

### Explicitly Out of Scope
- Additional languages beyond FR/EN (post-MVP)
- RTL (right-to-left) language support
- Pluralization rules (simple string replacement only)

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `resources/locales/en.toml` | English strings |
| Create | `resources/locales/fr.toml` | French strings |
| Create | `wmux-config/src/locale.rs` | Locale loading, string lookup, detection |
| Modify | `wmux-config/src/lib.rs` | Export locale module |
| Modify | various UI files | Replace hardcoded strings with t() calls |

### Key Decisions
- **TOML locale files** (Architecture §3): Standard key-value with dot-notation keys. English fallback for missing keys
- **System detection**: `GetUserDefaultUILanguage()` returns LANGID. 0x040C = French, 0x0409 = English. Fallback to English
- **Compile-time embedding**: Use `include_str!` to embed locale files in binary. No runtime file loading needed

### Patterns to Follow
- `.claude/rules/localization.md`: "NEVER hardcode user-visible strings", "dot notation keys"
- Architecture §3: "resources/locales/{en,fr}.toml"

### Technical Notes
- Locale file format:
```toml
[sidebar]
new_workspace = "New Workspace"
close_workspace = "Close Workspace"
[palette]
search_placeholder = "Type a command..."
[notification]
clear_all = "Clear All"
```
- Lookup: `HashMap<String, String>` loaded from TOML. `t(key)` → look up in current locale → fallback to English → fallback to key itself
- Language change: update locale HashMap, mark all UI dirty for re-render
- Coverage: sidebar labels, palette placeholder, notification texts, error messages, search labels, keyboard shortcut descriptions
- Approximately 50-100 string keys for MVP

## Success Criteria
- [ ] English strings display correctly
- [ ] French strings display correctly when language=fr
- [ ] System language auto-detected on startup
- [ ] Manual override via config works
- [ ] Language change without restart (if supported by config reload)
- [ ] English fallback for missing French keys
- [ ] 100% user-visible strings localized
- [ ] `cargo clippy --workspace` zero warnings

## Validation Steps
### Automated Checks
```bash
cargo build --workspace
cargo clippy --workspace -- -W clippy::all
cargo test -p wmux-config
cargo fmt --all -- --check
```
### Manual Verification
1. Set `language = "fr"` → verify French UI strings
2. Set `language = "en"` → verify English UI strings
3. Remove language setting → verify system language detection
### Edge Cases to Test
- System language neither FR nor EN (should fallback to EN)
- Missing key in fr.toml (should fallback to en.toml)
- Empty locale file (should use all fallbacks)
- Key exists in en.toml but not in fr.toml (should use English value)

## Dependencies
**Blocks**: None — leaf polish task

## References
- **PRD**: §16 Localisation FR/EN
- **Architecture**: §3 Cross-Cutting Concerns (i18n)
