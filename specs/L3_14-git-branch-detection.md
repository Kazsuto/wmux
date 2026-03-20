---
task_id: L3_14
title: "Implement Git Branch and Port Detection for Sidebar"
status: pending
priority: P2
estimated_hours: 2
wave: 10
prd_features: [F-13]
archi_sections: [ADR-0001]
depends_on: [L2_08, L1_04]
blocks: [L4_07]
---

# Task L3_14: Implement Git Branch and Port Detection for Sidebar

> **Phase**: Integration
> **Priority**: P2-Medium
> **Estimated effort**: 2 hours
> **Wave**: 10

## Context
The sidebar shows git branch, dirty state, and listening ports for each workspace. Git detection triggers on CWD changes (OSC 7 events). Port detection uses Windows API. PRD §13 describes these sidebar metadata items.

## Prerequisites
- [ ] Task L2_08: Sidebar UI Rendering — provides sidebar display for git/port data
- [ ] Task L1_04: OSC Sequence Handlers — provides CWD change events

## Scope
### Deliverables
- Git branch detection: run `git rev-parse --abbrev-ref HEAD` in workspace CWD
- Git dirty state: run `git status --porcelain` and check for output
- Listening port detection via `GetExtendedTcpTable` Win32 API
- Update sidebar metadata on CWD change event
- Periodic refresh timer (5 seconds for ports, on-demand for git)

### Explicitly Out of Scope
- PR status detection (requires `gh` CLI — post-MVP)
- Git log or commit history
- Port process identification (which process owns the port)

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-core/src/git_detector.rs` | Git branch/dirty detection |
| Create | `wmux-core/src/port_scanner.rs` | Listening port detection |
| Modify | `wmux-core/src/app_state.rs` | Wire CWD events → git detection → metadata update |
| Modify | `wmux-core/src/lib.rs` | Export modules |

### Key Decisions
- **Command execution for git** (architecture audit): Use `std::process::Command` (via `tokio::process::Command`) to run `git` CLI. Simpler and more reliable than `git2` crate for our needs
- **GetExtendedTcpTable for ports**: Win32 API provides listening TCP ports without spawning processes. Filter for LISTEN state
- **On-demand git**: Only re-detect git when CWD changes (OSC 7 event). Not polling

### Patterns to Follow
- `.claude/rules/rust-architecture.md`: Use spawn_blocking for shell commands
- PRD §13: "Branche git détectée dans les 2s", "Ports en écoute détectés dans les 5s"

### Technical Notes
- Git detection: `tokio::process::Command::new("git").args(["rev-parse", "--abbrev-ref", "HEAD"]).current_dir(cwd).output().await`
- If git command fails (not a repo), set git_branch to None
- Dirty detection: `git status --porcelain` → empty output = clean, any output = dirty
- Port scanning: `GetExtendedTcpTable` with `AF_INET` → filter `MIB_TCP_STATE_LISTEN` → extract ports
- Filter ports: only show common dev ports (3000-9999 range, or configurable)
- Update WorkspaceMetadata.git_branch, git_dirty, ports
- Debounce: if CWD changes rapidly (many `cd` in quick succession), debounce git detection (500ms)

## Success Criteria
- [ ] Git branch detected and shown in sidebar within 2s of cd into repo
- [ ] Git dirty state correctly shown (clean/dirty indicator)
- [ ] Non-git directories show no git info (not an error)
- [ ] Listening ports detected and shown in sidebar
- [ ] Port detection refreshes every 5 seconds
- [ ] CWD change triggers git re-detection
- [ ] `cargo clippy --workspace` zero warnings

## Validation Steps
### Automated Checks
```bash
cargo build --workspace
cargo clippy --workspace -- -W clippy::all
cargo test -p wmux-core
cargo fmt --all -- --check
```
### Manual Verification
1. cd into a git repo → verify branch appears in sidebar
2. Modify a file → verify dirty indicator
3. Start a server on port 3000 → verify port appears in sidebar
4. cd out of git repo → verify git info clears
### Edge Cases to Test
- git not installed (should handle gracefully, no git info)
- Very large git repo (should not block UI — async detection)
- Many listening ports (should show all, not truncate)
- Detached HEAD (should show commit hash instead of branch name)

## Dependencies
**Blocks**: None — leaf sidebar feature

## References
- **PRD**: §13 Shell Integration & Détection Git
- **Architecture**: §5 wmux-core, §12 Project Structure
