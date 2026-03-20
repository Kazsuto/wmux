---
task_id: L4_07
title: "Implement Packaging and Distribution"
status: pending
priority: P2
estimated_hours: 2.5
wave: 12
prd_features: [F-14]
archi_sections: [ADR-0001]
depends_on: [L2_03, L2_05, L2_06, L2_10, L2_16, L3_02, L3_04, L3_07, L3_09, L3_10, L3_12, L3_13, L3_14, L4_01, L4_02, L4_03, L4_04, L4_05, L4_06]
blocks: []
---

# Task L4_07: Implement Packaging and Distribution

> **Phase**: Polish
> **Priority**: P2-Medium
> **Estimated effort**: 2.5 hours
> **Wave**: 12

## Context
wmux needs to be distributed via multiple channels for easy installation. Architecture §9 specifies MSI, winget, Scoop, and portable zip. PRD mentions distribution via GitHub Releases.

## Prerequisites
- All prior tasks should be substantially complete before final packaging

## Scope
### Deliverables
- MSI installer via WiX 4 (includes wmux-app.exe, wmux-cli.exe, wmuxd-remote binaries)
- winget manifest for Microsoft package manager
- Scoop bucket manifest
- Portable .zip build (no installer needed)
- CI/CD GitHub Actions workflow: clippy → fmt → test → build → package → release
- Release profile optimization (LTO, strip, panic=abort — already in Cargo.toml)

### Explicitly Out of Scope
- Microsoft Store listing (post-MVP)
- Code signing certificate (post-MVP)
- Homebrew (macOS, not applicable)

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `installer/wmux.wxs` | WiX 4 installer definition |
| Create | `.github/workflows/release.yml` | CI/CD release workflow |
| Create | `scripts/package.ps1` | Build + package script |
| Create | `winget/wmux.yaml` | winget manifest |
| Create | `scoop/wmux.json` | Scoop bucket manifest |

### Key Decisions
- **WiX 4** (not 3): Modern MSI toolchain, supports cargo-wix workflow
- **GitHub Actions**: windows-latest runner, Rust toolchain, build both app and CLI
- **Portable zip**: Just the binaries + themes + locale files in a .zip. No registry changes

### Patterns to Follow
- Architecture §9: "MSI installer, winget, Scoop, portable zip"
- Architecture §9: Build profile with LTO, strip symbols

### Technical Notes
- WiX MSI: install to Program Files, add CLI to PATH, register AUMID for Toast
- winget manifest: submit to winget-pkgs repository or host own manifest
- Scoop: JSON manifest with URL, hash, bin references
- CI workflow: trigger on tag push (v*). Build release → upload artifacts → create GitHub Release
- Include wmuxd-remote (Go binary) in package: compile Go → include in MSI/zip
- Binary size target: < 15MB wmux-app, < 5MB wmux-cli

## Success Criteria
- [ ] MSI installer installs wmux correctly
- [ ] MSI adds wmux-cli to PATH
- [ ] winget manifest validates and installs
- [ ] Scoop manifest validates and installs
- [ ] Portable zip works without installation
- [ ] CI/CD workflow builds and packages automatically
- [ ] Uninstaller removes all files cleanly
- [ ] `cargo clippy --workspace` zero warnings

## Validation Steps
### Automated Checks
```bash
cargo build --release --workspace
cargo clippy --workspace -- -W clippy::all
cargo test --workspace
```
### Manual Verification
1. Build MSI → install → verify wmux runs → uninstall → verify clean removal
2. Test `scoop install wmux` from custom bucket
3. Extract portable zip → run wmux → verify works
### Edge Cases to Test
- Install over existing installation (upgrade scenario)
- Install without admin privileges (should work for user install)
- Multiple Windows user accounts (should not interfere)
- Antivirus false positive (sign binary in post-MVP)

## Dependencies
**Blocks**: None — final distribution task

## References
- **PRD**: §Hors Scope mentions distribution channels
- **Architecture**: §9 Infrastructure & Distribution
