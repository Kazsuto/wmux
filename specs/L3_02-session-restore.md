# Task L3_02: Implement Session Restore on Launch

> **Phase**: Integration
> **Priority**: P1-High
> **Estimated effort**: 2 hours

## Context
When wmux starts, it should restore the previous session: workspaces, pane layouts, working directories, scrollback, and browser URLs. ADR-0009 specifies graceful handling of corrupt files. PRD §8 lists what is restored vs not.

## Prerequisites
- [ ] Task L3_01: Session Auto-Save — provides session.json to read

## Scope
### Deliverables
- Read and deserialize `%APPDATA%\wmux\session.json` on startup
- Recreate workspaces with names and order
- Recreate pane tree layouts (splits, ratios)
- Spawn shells in correct working directories for each terminal surface
- Restore scrollback text (best-effort)
- Placeholder for browser URL restore (actual WebView2 creation in Task L3_04)
- Restore window geometry (position, size, maximized state)
- Restore sidebar width
- Corrupt/missing file handling: log warning, start fresh session

### Explicitly Out of Scope
- Restoring running processes (vim, ssh, etc.) — PRD explicitly excludes
- Restoring terminal state (colors, modes) beyond scrollback text
- Full browser state restore (cookies, history) — only URL

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Modify | `wmux-core/src/session.rs` | Deserialization, restore logic |
| Modify | `wmux-app/src/main.rs` | Call restore before entering event loop |
| Modify | `wmux-core/src/app_state.rs` | Accept restored state during initialization |

### Key Decisions
- **Schema version check**: If version doesn't match expected, start fresh (NEVER crash). Log warning
- **Best-effort scrollback**: Inject scrollback text as if it were PTY output (feed through VTE parser). May not perfectly reproduce colors/attributes but content is accurate
- **Shell respawn**: Each terminal surface spawns a fresh shell in the saved CWD. The shell starts fresh — no command history, no running processes

### Patterns to Follow
- ADR-0009: "Corrupt/unreadable → log warning, start fresh"
- `.claude/rules/persistence.md`: "Corrupt/unreadable: log warning + start fresh (NEVER crash)"

### Technical Notes
- Restore order: (1) parse JSON, (2) recreate workspaces, (3) recreate pane trees, (4) spawn PTYs per surface, (5) inject scrollback, (6) set active workspace
- Window geometry: use `Window::set_outer_position()` and `Window::request_inner_size()` from winit. If saved position is off-screen, default to center
- Scrollback injection: feed saved text as raw bytes to Terminal::process(). This is imperfect but gives user their command history back
- Browser surfaces: record URL in session. Actual WebView2 creation happens when Task L3_04 is implemented. For now, create a placeholder terminal surface with "[Browser: url]" message

## Success Criteria
- [ ] Session restores workspace names and order
- [ ] Pane tree layout matches saved structure (splits, ratios)
- [ ] Terminals spawn in correct working directories
- [ ] Scrollback text is approximately restored
- [ ] Window position and size restored
- [ ] Active workspace restored
- [ ] Corrupt file handled gracefully (fresh start)
- [ ] Missing file handled gracefully (fresh start)
- [ ] Restore completes in < 3s for 10 workspaces
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
1. Create 3 workspaces with splits → close wmux → reopen → verify layout restored
2. Verify working directories match (run `pwd` in each terminal)
3. Scroll up in restored terminal → verify previous output visible
4. Delete session.json → start wmux → verify fresh session (no crash)
### Edge Cases to Test
- Session with many workspaces (20+) — verify restore time
- Saved CWD that no longer exists (should fall back to home directory)
- Saved window position off-screen (should reposition to visible area)
- Schema version mismatch (should start fresh)
- Empty session file (should start fresh)

## Dependencies
**Blocks**: None — session restore is a leaf integration task

## References
- **PRD**: §8 Persistance de Session (restore requirements)
- **Architecture**: §6 Session Persistence Schema
- **ADR**: ADR-0009 (restore behavior, corruption handling)
