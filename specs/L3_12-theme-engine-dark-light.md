# Task L3_12: Implement Theme Engine and Dark/Light Mode Detection

> **Phase**: Integration
> **Priority**: P1-High
> **Estimated effort**: 2 hours

## Context
Themes control terminal colors, sidebar appearance, and UI chrome. wmux imports Ghostty themes (50+ community themes) and detects the Windows dark/light mode preference. PRD §10 describes theme management and live switching.

## Prerequisites
- [ ] Task L3_11: Ghostty Config Parser — provides config struct with theme setting

## Scope
### Deliverables
- `ThemeEngine`: load, manage, and apply color themes
- Theme file loading from `%APPDATA%\wmux\themes/`
- Color palette: 16 ANSI colors + background + foreground + cursor + selection
- Live theme switching without restart
- Dark/light mode detection via Windows registry/WinRT UISettings
- Auto-switch theme on system dark/light change
- CLI: `wmux themes list`, `wmux themes set <name>`, `wmux themes clear`
- Bundle 10-20 popular themes (catppuccin-mocha, dracula, nord, gruvbox, etc.)

### Explicitly Out of Scope
- Theme marketplace (post-MVP)
- Custom theme creation UI
- Per-workspace themes

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-config/src/theme.rs` | ThemeEngine, color palette management |
| Create | `resources/themes/` | Bundled theme files |
| Modify | `wmux-config/src/lib.rs` | Export theme module |
| Modify | `wmux-render/src/terminal.rs` | Use theme colors for rendering |

### Key Decisions
- **Ghostty theme format**: Each theme is a key-value file with `palette = N=#RRGGBB` entries for 16 colors, plus `background`, `foreground`, `cursor-color`, `selection-background`
- **Live reload**: Use `ArcSwap<Theme>` or `RwLock<Theme>` for thread-safe theme access. Theme change → mark all rows dirty → full re-render
- **Dark/light detection**: Registry key `HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Themes\Personalize\AppsUseLightTheme`. Also WinRT `UISettings.GetColorValue(UIColorType.Background)` for real-time changes

### Patterns to Follow
- PRD §10: Theme sources, CLI commands
- Architecture §5 wmux-config: "ThemeEngine for color palette management"
- ADR-0010: "50+ Ghostty community themes"

### Technical Notes
- Theme file format: same as Ghostty config but with only color keys
- Theme directory search: `%APPDATA%\wmux\themes/` → `resources/themes/` (bundled)
- Theme struct: `{ name, background, foreground, cursor, selection, palette: [Color; 16] }`
- Color parsing: `#RRGGBB` hex → (u8, u8, u8)
- Registry watcher for dark/light mode: `RegNotifyChangeKeyValue` for real-time system theme changes
- On theme change: update Color::Named(n) → RGB mapping for terminal renderer
- `wmux themes list`: read themes directory, print names
- `wmux themes set`: write `theme = name` to wmux config
- `wmux themes clear`: remove `theme` line from config, revert to default

## Success Criteria
- [ ] Themes load from theme files correctly
- [ ] Terminal colors match the selected theme
- [ ] Theme switch applies immediately without restart
- [ ] Dark/light mode detected on startup
- [ ] System dark/light change triggers auto-switch
- [ ] CLI theme commands work (list, set, clear)
- [ ] Bundled themes included and loadable
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
1. `wmux themes list` → verify theme names displayed
2. `wmux themes set catppuccin-mocha` → verify colors change
3. Toggle Windows dark/light mode → verify wmux auto-switches
4. `wmux themes clear` → verify default theme restored
### Edge Cases to Test
- Theme file with missing color keys (should use defaults for missing)
- Theme file with invalid hex colors (should warn and use default)
- Non-existent theme name (should error)
- Theme directory doesn't exist (should use bundled themes only)

## Dependencies
**Blocks**: None — leaf configuration task

## References
- **PRD**: §10 Thèmes & Configuration
- **Architecture**: §5 wmux-config (theme.rs)
- **ADR**: ADR-0010 (Ghostty-compatible themes)
