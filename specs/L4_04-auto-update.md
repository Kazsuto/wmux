# Task L4_04: Implement Auto-Update System

> **Phase**: Polish
> **Priority**: P3-Low
> **Estimated effort**: 2.5 hours

## Context
wmux checks GitHub Releases for new versions, downloads in the background, and applies on next launch. PRD §14 describes the auto-update behavior. Architecture §9 specifies GitHub Releases API.

## Prerequisites
- [ ] Task L0_01: Error Types and Tracing Infrastructure — basic crate infrastructure

## Scope
### Deliverables
- GitHub Releases API polling (background, hourly)
- Version comparison (semver)
- Background download to temp directory
- Staged install: downloaded but applied on next launch
- Notification pill/badge in title bar when update available
- `wmux update check` CLI command
- `wmux update apply` CLI command

### Explicitly Out of Scope
- In-place update (too risky, restart required)
- Update channels (stable/beta/nightly)
- Code signing verification (post-MVP)

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-app/src/updater.rs` | Update checker, downloader |
| Create | `wmux-cli/src/commands/update.rs` | CLI update commands |
| Modify | `wmux-ui/src/app.rs` | Title bar update notification |

### Key Decisions
- **Staged install**: Download to `%APPDATA%\wmux\updates\`. On next launch, if update found, replace exe and restart
- **GitHub Releases API**: `GET https://api.github.com/repos/owner/wmux/releases/latest` → compare tag version
- **Background poll**: tokio interval, hourly. Only check when idle

### Patterns to Follow
- Architecture §9: "GitHub Releases API poll, download staged, notification in title bar"
- PRD §14: "Vérification automatique via GitHub Releases API"

### Technical Notes
- Version comparison: parse semver tags (v0.1.0 → 0.1.0). Compare with current version
- Download: HTTPS GET release asset (wmux-app.exe). Stream to temp file
- Staged install: write to `%APPDATA%\wmux\updates\wmux-app-vX.Y.Z.exe`. On startup, check for pending update
- Apply: rename current exe → .old, copy new exe → current path, restart. Clean up .old on success
- Title bar: append " (update available: vX.Y.Z)" or show colored pill
- Hourly poll: `tokio::time::interval(Duration::from_secs(3600))`
- Respect `WMUX_DISABLE_UPDATE` env var for CI/testing

## Success Criteria
- [ ] Update check detects new versions
- [ ] Background download works without blocking UI
- [ ] Staged install applied on next launch
- [ ] Title bar shows update notification
- [ ] `wmux update check` works via CLI
- [ ] Update respects disable flag
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
1. Mock a newer release → verify notification appears
2. `wmux update check` → verify version comparison
3. Download update → restart → verify new version
### Edge Cases to Test
- No internet connection (should fail silently, not crash)
- GitHub API rate limit (should back off)
- Corrupt download (should verify checksum/size, discard if wrong)
- Update during active session (should only notify, not interrupt)

## Dependencies
**Blocks**: None — leaf feature

## References
- **PRD**: §14 Auto-Update
- **Architecture**: §9 Infrastructure & Distribution
