# Task L2_07: Implement Workspace Model and Lifecycle

> **Phase**: Core
> **Priority**: P0-Critical
> **Estimated effort**: 2 hours

## Context
Workspaces are the primary organizational unit — each appears as a vertical tab in the sidebar. Each workspace has its own PaneTree and metadata. Users create workspaces with Ctrl+N and switch with Ctrl+1-9. Architecture §5 specifies workspace_manager.rs. PRD §Modèle Conceptuel defines Workspace as "entrée dans la sidebar."

## Prerequisites
- [ ] Task L2_02: PaneTree Layout Engine — each workspace owns a PaneTree

## Scope
### Deliverables
- `Workspace` struct: id, name, pane_tree, metadata, creation order
- `WorkspaceManager`: list of workspaces, active workspace index
- Create workspace: Ctrl+N → new workspace with single terminal pane
- Switch workspace: Ctrl+1-9 → switch to workspace by index
- Close workspace: close all panes in workspace, remove from list
- Rename workspace: via API (CLI/IPC)
- Workspace metadata placeholders: git_branch, cwd, ports (populated by later tasks)

### Explicitly Out of Scope
- Sidebar UI rendering (Task L2_08)
- Sidebar metadata system (Task L2_14)
- Drag-and-drop workspace reorder (Task L2_08)
- Session persistence (Task L3_01)

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-core/src/workspace.rs` | Workspace struct |
| Create | `wmux-core/src/workspace_manager.rs` | WorkspaceManager lifecycle |
| Modify | `wmux-core/src/app_state.rs` | AppState owns WorkspaceManager |
| Modify | `wmux-core/src/lib.rs` | Export workspace modules |

### Key Decisions
- **One PaneTree per workspace**: Switching workspaces switches the entire visible pane layout. Independent state per workspace
- **Index-based switching**: Ctrl+1 = first workspace, Ctrl+9 = ninth (or last if fewer). Matches cmux/tmux convention
- **Default workspace**: App always starts with at least one workspace. Cannot close the last workspace (creates a new empty one)

### Patterns to Follow
- PRD §Modèle Conceptuel: "Workspace = unité d'organisation principale"
- Architecture §5 wmux-core: workspace_manager.rs
- Architecture §6 Data Architecture: session schema has workspaces array

### Technical Notes
- WorkspaceManager: `Vec<Workspace>`, `active_index: usize`
- Workspace metadata struct (initially mostly empty, populated by Tasks L2_14, L3_13, L3_14):
  ```rust
  struct WorkspaceMetadata {
      cwd: Option<PathBuf>,
      git_branch: Option<String>,
      git_dirty: Option<bool>,
      ports: Vec<u16>,
  }
  ```
- Switching workspace: save focus state of current workspace, load focus state of target
- Close workspace: close all PTY actors, remove all panes, remove workspace. If it was active, switch to next available
- New workspace gets auto-generated name "Workspace N" (user can rename)
- Serialize/deserialize support for session persistence

## Success Criteria
- [ ] Ctrl+N creates a new workspace with terminal
- [ ] Ctrl+1-9 switches between workspaces
- [ ] Each workspace has independent pane layout
- [ ] Close workspace cleans up all panes and PTYs
- [ ] Cannot close last workspace (new empty one created)
- [ ] Workspace rename works via AppCommand
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
1. Ctrl+N — verify new workspace created, switches to it
2. Ctrl+1 — verify switches back to first workspace with its panes intact
3. Close workspace — verify panes and PTYs cleaned up
4. Open 3 workspaces, switch between them — verify each has independent state
### Edge Cases to Test
- Create 9+ workspaces (Ctrl+9 should go to 9th, not beyond)
- Close active workspace (should switch to next/previous)
- Close all workspaces (should create one new empty workspace)

## Dependencies
**Blocks**:
- Task L2_08: Sidebar UI Rendering
- Task L2_12: Workspace IPC Handlers
- Task L3_01: Session Persistence
- Task L4_03: SSH Remote (remote workspaces)

## References
- **PRD**: §Modèle Conceptuel (Workspace), §2 Multiplexeur (workspaces)
- **Architecture**: §5 wmux-core (workspace.rs, workspace_manager.rs)
- **ADR**: ADR-0009 (workspace serialization)
