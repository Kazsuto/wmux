# Task L4_05: Implement Mica/Acrylic Visual Effects

> **Phase**: Polish
> **Priority**: P3-Low
> **Estimated effort**: 2 hours

## Context
Windows 11 supports Mica and Acrylic backdrop effects for modern UI appearance. wmux uses these for the sidebar. Windows 10 falls back to opaque background. PRD §15 describes the effects. Architecture §3 specifies DWM API.

## Prerequisites
- [ ] Task L2_08: Sidebar UI Rendering — provides sidebar to apply backdrop to

## Scope
### Deliverables
- OS version detection: Win11 build 22000+ → Mica, older → opaque fallback
- DWM API `DwmSetWindowAttribute` for Mica backdrop
- Acrylic option as alternative
- wgpu alpha compositing for transparent sidebar
- Native rounded corners on Win11
- Graceful fallback on Win10 (opaque background with theme color)

### Explicitly Out of Scope
- Custom blur amount control
- Per-pane transparency
- Transparency for terminal area (only sidebar)

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-ui/src/effects.rs` | DWM effects, OS detection |
| Modify | `wmux-ui/src/app.rs` | Apply effects on window creation |
| Modify | `wmux-ui/Cargo.toml` | Add windows crate DWM features |

### Key Decisions
- **Three-tier detection**: Win11 22H2+ (build 22621) → Mica Alt, Win11 22000+ → Mica, Win10 → opaque fallback
- **DWM attribute**: `DWMWA_SYSTEMBACKDROP_TYPE = 38`. Value 2 = Mica, 3 = Acrylic, 4 = Mica Alt
- **wgpu alpha**: Clear color alpha must be 0.0 for transparent regions. Sidebar quads use semi-transparent colors

### Patterns to Follow
- PRD §15: "Mica/Acrylic via DWM API, fallback opaque on Win10"
- `.claude/rules/windows-platform.md`: "Mica/Acrylic Win11 only — ALWAYS feature-detect + fallback"

### Technical Notes
- OS version: `RtlGetVersion` or `GetVersionExW` (with compatibility manifest). Check build number
- DWM call: `DwmSetWindowAttribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, &value, size_of_val)`
- Also set `DWMWA_USE_IMMERSIVE_DARK_MODE` for dark mode title bar
- Rounded corners: automatic on Win11 when using DWM (no extra API needed)
- wgpu: ensure surface format supports alpha. Clear with (0, 0, 0, 0) for transparent areas
- Fallback: opaque sidebar with theme background color. No visual glitch

## Success Criteria
- [ ] Mica/Acrylic effect visible on Windows 11
- [ ] Sidebar has translucent backdrop on Win11
- [ ] Opaque fallback works on Windows 10
- [ ] Dark mode title bar matches theme
- [ ] Rounded corners on Win11
- [ ] No visual glitches during dark/light mode transition
- [ ] `cargo clippy --workspace` zero warnings

## Validation Steps
### Automated Checks
```bash
cargo build --workspace
cargo clippy --workspace -- -W clippy::all
cargo test --workspace
cargo fmt --all -- --check
```
### Manual Verification
1. Run on Win11 → verify Mica backdrop on sidebar
2. Run on Win10 (or compatibility mode) → verify opaque fallback
3. Toggle dark/light mode → verify smooth transition
### Edge Cases to Test
- DWM compositor disabled (should fallback gracefully)
- High contrast mode (should respect system settings)
- Multiple monitors with different DPI (should render correctly)

## Dependencies
**Blocks**: None — leaf polish task

## References
- **PRD**: §15 Effets Visuels Windows 11
- **Architecture**: §3 Adaptation Table (DWM API)
