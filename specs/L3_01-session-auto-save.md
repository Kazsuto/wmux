# Task L3_01: Implement Session Auto-Save

> **Phase**: Integration
> **Priority**: P1-High
> **Estimated effort**: 2.5 hours

## Context
Session persistence saves workspace layout, pane trees, and scrollback so users can close and reopen wmux without losing their environment. Architecture §6 defines the session JSON schema. ADR-0009 mandates JSON format with auto-save every 8 seconds and atomic writes. PRD §8 describes what is and isn't restored.

## Prerequisites
- [ ] Task L2_07: Workspace Lifecycle — provides workspace/pane tree structure to serialize

## Scope
### Deliverables
- `SessionState` struct matching Architecture §6 schema (version, workspaces, window geometry)
- Serialization of workspace tree, pane layout, surface info, CWD, scrollback
- Auto-save timer: 8-second interval, non-blocking
- Atomic write: serialize → temp file → rename (`MoveFileExW`)
- Scrollback truncation: 4000 lines / 400K chars per terminal before serialization
- Session file location: `%APPDATA%\wmux\session.json`
- Schema version field (`"version": 1`)

### Explicitly Out of Scope
- Session restore (Task L3_02)
- Process restoration (PRD explicitly excludes this from v1)
- Browser state beyond URL (Task L3_02 handles URL restore)

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-core/src/session.rs` | SessionState, serialization logic |
| Modify | `wmux-core/src/app_state.rs` | Add persistence actor (timer + serialize) |
| Modify | `wmux-core/src/lib.rs` | Export session module |

### Key Decisions
- **Serialize on main thread, write on tokio task** (ADR-0009): Serialization is fast (~5-20ms). File write happens in `tokio::spawn` to avoid blocking
- **Atomic write** (ADR-0009): Write to `session.json.tmp`, then `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING`. Prevents corruption if crash during write
- **Scrollback truncation**: Before serializing, truncate each terminal's scrollback to 4000 lines / 400K chars total

### Patterns to Follow
- ADR-0009: JSON file, version field, atomic writes, 8s interval
- `.claude/rules/persistence.md`: Session file location, scrollback limits, corrupt handling
- Architecture §6: Session Persistence Schema

### Technical Notes
- SessionState mirrors Architecture §6 schema: `{ version: u32, workspaces: Vec<WorkspaceSnapshot>, active_workspace: WorkspaceId, sidebar_width: u16, window: WindowGeometry }`
- WorkspaceSnapshot: `{ id, name, pane_tree: PaneTreeSnapshot, metadata }`
- PaneTreeSnapshot: recursive enum matching PaneTree but with serialized terminal data
- Terminal snapshot: `{ surface_id, cwd, scrollback_text: Option<String> }`
- Browser snapshot: `{ surface_id, url }`
- `%APPDATA%\wmux\` directory created on first save if absent
- Timer: `tokio::time::interval(Duration::from_secs(8))`

## Success Criteria
- [ ] Session auto-saves every 8 seconds
- [ ] Session file contains correct workspace/pane/surface structure
- [ ] Scrollback is truncated to limits before serialization
- [ ] Atomic write prevents corruption
- [ ] Schema version field is present
- [ ] File written to correct location (%APPDATA%\wmux\session.json)
- [ ] Auto-save does not block the UI thread
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
1. Run app, create workspaces/panes, wait 10s → verify session.json exists with correct content
2. Verify JSON is valid and matches expected schema
3. Check file size is reasonable (< 5MB for typical session)
### Edge Cases to Test
- Session with large scrollback (4000+ lines per terminal) — verify truncation
- `%APPDATA%\wmux\` directory doesn't exist (should create it)
- Crash during write (temp file + rename prevents corruption)
- Empty session (no workspaces) — should still save valid JSON

## Dependencies
**Blocks**:
- Task L3_02: Session Restore on Launch

## References
- **PRD**: §8 Persistance de Session
- **Architecture**: §6 Session Persistence Schema, §10 Failure Modes
- **ADR**: ADR-0009 (Session Persistence)
