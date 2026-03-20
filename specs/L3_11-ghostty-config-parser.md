---
task_id: L3_11
title: "Implement Ghostty-Compatible Config Parser"
status: pending
priority: P1
estimated_hours: 2.5
wave: 1
prd_features: [F-10]
archi_sections: [ADR-0001, ADR-0010]
depends_on: [L0_01]
blocks: [L3_12, L4_06]
---

# Task L3_11: Implement Ghostty-Compatible Config Parser

> **Phase**: Integration
> **Priority**: P1-High
> **Estimated effort**: 2.5 hours
> **Wave**: 1

## Context
wmux uses a Ghostty-compatible config format (`key = value`, NOT standard TOML sections) to reuse 50+ Ghostty community themes. Architecture §5 (wmux-config) specifies the parser. ADR-0010 mandates Ghostty-compatible format.

## Prerequisites
- [ ] Task L0_01: Error Types and Tracing Infrastructure — provides ConfigError enum

## Scope
### Deliverables
- Ghostty-compatible key-value parser (NOT standard TOML)
- `Config` struct with all wmux settings (font, colors, keybindings, terminal, sidebar)
- Config file loading from `%APPDATA%\wmux\config`
- Ghostty config import from `%APPDATA%\ghostty\config`
- Priority chain: wmux config > Ghostty import > built-in defaults
- Default config generation when no config exists
- Unknown key handling: warn in log, don't error (forward compatibility)

### Explicitly Out of Scope
- Theme engine and color palette (Task L3_12)
- Live config reload (Task L3_12)
- Dark/light mode detection (Task L3_12)

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-config/src/parser.rs` | Ghostty-compat key-value parser |
| Create | `wmux-config/src/config.rs` | Config struct with all settings |
| Modify | `wmux-config/src/lib.rs` | Export parser, config modules |
| Modify | `wmux-config/Cargo.toml` | Add toml, serde, dirs, tracing deps |

### Key Decisions
- **Custom parser** (ADR-0010): Ghostty uses `key = value` without TOML sections. Lines starting with `#` are comments. Values can be strings, numbers, booleans
- **Unknown keys logged, not rejected**: Forward compatibility. If Ghostty adds new keys, wmux ignores them with a warning
- **Config priority**: wmux config overrides Ghostty config overrides defaults. Merge at key level

### Patterns to Follow
- ADR-0010: "Ghostty-compatible key-value format (NOT standard TOML)"
- `.claude/rules/persistence.md`: Config at `%APPDATA%\wmux\config`
- Architecture §5 wmux-config: "custom parser layer on top of TOML"

### Technical Notes
- Parser: line-by-line. Split on first `=`. Trim whitespace. Handle quoted strings, booleans (true/false), integers, floats
- Key examples: `font-family`, `font-size`, `theme`, `background`, `foreground`, `scrollback-limit`, `cursor-style`, `keybind`
- Config struct fields: font_family, font_size, theme, colors (16 palette), scrollback_limit, cursor_style, keybindings (HashMap), sidebar_width, language
- Keybind format: `keybind = ctrl+n=new_workspace` (key=action pairs)
- Default font: "Cascadia Code" → "Consolas" fallback
- Default scrollback: 4000
- Default theme: "wmux-default" (light/dark auto)
- Config directory: create if absent with `dirs::config_dir()` + "wmux"
- Ghostty import: check `%APPDATA%\ghostty\config`, parse same format, use as base

## Success Criteria
- [ ] Parser correctly reads Ghostty-format key-value files
- [ ] Config struct populated with all settings
- [ ] wmux config overrides Ghostty config overrides defaults
- [ ] Unknown keys logged as warnings, not errors
- [ ] Missing config file creates default
- [ ] Comments (#) correctly ignored
- [ ] All standard Ghostty keys recognized
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
1. Create `%APPDATA%\wmux\config` with `font-family = Cascadia Code` → verify parsed
2. Test with Ghostty config file → verify import works
3. Test with unknown key → verify warning logged, no error
### Edge Cases to Test
- Empty config file (should use all defaults)
- Config with only comments (should use all defaults)
- Invalid value type (e.g., non-numeric font-size) — should warn and use default
- Config file with BOM (should handle gracefully)
- Very long lines (should not crash)

## Dependencies
**Blocks**:
- Task L3_12: Theme Engine + Dark/Light Detection
- Task L4_06: Localization FR/EN (reads language setting)

## References
- **PRD**: §10 Thèmes & Configuration
- **Architecture**: §5 wmux-config (parser.rs)
- **ADR**: ADR-0010 (Ghostty-compatible format)
