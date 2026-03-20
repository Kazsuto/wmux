---
task_id: L2_12
title: "Implement Workspace and Surface IPC Handlers"
status: pending
priority: P0
estimated_hours: 2.5
wave: 9
prd_features: [F-03]
archi_sections: [ADR-0001, ADR-0005]
depends_on: [L2_11, L2_07]
blocks: [L2_16]
---

# Task L2_12: Implement Workspace and Surface IPC Handlers

> **Phase**: Core
> **Priority**: P0-Critical
> **Estimated effort**: 2.5 hours
> **Wave**: 9

## Context
AI agents and CLI need to manage workspaces and surfaces programmatically. These handlers implement the workspace.* and surface.* method families. PRD §3 lists workspace and surface methods. Architecture §6 shows the IPC command data flow.

## Prerequisites
- [ ] Task L2_11: IPC Handler Trait + Router — provides Handler trait and dispatch
- [ ] Task L2_07: Workspace Lifecycle — provides workspace operations to delegate to

## Scope
### Deliverables
- `WorkspaceHandler`: workspace.list, workspace.create, workspace.select, workspace.current, workspace.close, workspace.rename
- `SurfaceHandler`: surface.split, surface.list, surface.focus, surface.close
- Parameter validation for each method
- Response formatting matching cmux protocol

### Explicitly Out of Scope
- surface.send_text, surface.send_key, surface.read_text (Task L2_13)
- browser.* handlers (Task L3_07)
- Target resolution for --window/--workspace/--surface CLI options (Task L2_15)

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-ipc/src/handlers/workspace.rs` | WorkspaceHandler implementation |
| Create | `wmux-ipc/src/handlers/surface.rs` | SurfaceHandler implementation |
| Modify | `wmux-ipc/src/handlers/mod.rs` | Register workspace and surface handlers |
| Modify | `wmux-ipc/src/router.rs` | Register handlers in router |

### Key Decisions
- **Handlers → AppCommand → AppState**: Each handler method translates JSON params to an AppCommand, sends to AppState channel, awaits response. This keeps IPC layer thin
- **Target resolution**: Methods accept optional workspace_id/surface_id params. If omitted, use the "current" workspace/surface (tracked by AppState). This matches cmux behavior
- **workspace.list response**: Array of workspace objects with id, name, active flag, pane count

### Patterns to Follow
- `.claude/rules/ipc-protocol.md`: Method names match cmux exactly
- ADR-0008: Handler → AppState channel communication

### Technical Notes
- workspace.create params: `{ "name": "optional-name" }` → returns `{ "workspace_id": "..." }`
- workspace.select params: `{ "workspace_id": "..." }` or `{ "index": 1 }` (1-based)
- workspace.current: no params → returns current workspace info
- surface.split params: `{ "direction": "right"|"bottom", "workspace_id": "optional" }` → returns `{ "surface_id": "..." }`
- surface.list params: `{ "workspace_id": "optional" }` → returns array of surface info objects
- surface.focus params: `{ "surface_id": "..." }` → focuses surface (switches workspace if needed)
- surface.close params: `{ "surface_id": "..." }` → closes surface

## Success Criteria
- [ ] workspace.list returns correct workspace information
- [ ] workspace.create creates a new workspace and returns its ID
- [ ] workspace.select switches to the specified workspace
- [ ] workspace.close closes the workspace and cleans up
- [ ] workspace.rename updates workspace name
- [ ] surface.split creates a new pane with correct direction
- [ ] surface.list returns all surfaces in workspace
- [ ] surface.focus switches to the specified surface
- [ ] surface.close closes the surface
- [ ] All handlers validate parameters and return appropriate errors
- [ ] `cargo clippy --workspace` zero warnings

## Validation Steps
### Automated Checks
```bash
cargo build --workspace
cargo clippy --workspace -- -W clippy::all
cargo test -p wmux-ipc
cargo fmt --all -- --check
```
### Manual Verification
1. Send workspace.create → verify new workspace appears in sidebar
2. Send workspace.list → verify response includes all workspaces
3. Send surface.split → verify pane splits in UI
4. Send surface.close → verify surface removed
### Edge Cases to Test
- workspace.select with invalid ID (should return error)
- surface.split when pane is at minimum size (should return error)
- workspace.close on last workspace (should create new empty workspace)
- Methods with missing required params (should return invalid_params error)

## Dependencies
**Blocks**:
- Task L2_16: CLI Domain Commands

## References
- **PRD**: §3 CLI & API IPC (workspace.*, surface.* methods)
- **Architecture**: §5 wmux-ipc (handlers/workspace.rs, handlers/surface.rs)
- **ADR**: ADR-0005 (JSON-RPC v2)
