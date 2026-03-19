# Task L2_06: Implement Surface (Tab) System Within Panes

> **Phase**: Core
> **Priority**: P1-High
> **Estimated effort**: 2 hours

## Context
Each pane can contain multiple surfaces (tabs). A surface is an individual terminal or browser panel identified by WMUX_SURFACE_ID. Users create surfaces with Ctrl+T and navigate with Ctrl+Tab. PRD §Modèle Conceptuel defines Surface as "onglet dans un pane." Architecture §12 shows surface as a layer between Pane and Panel.

## Prerequisites
- [ ] Task L2_02: PaneTree Layout Engine — provides pane structure to add surfaces to

## Scope
### Deliverables
- `SurfaceManager` per pane: manages list of surfaces, active surface index
- Create new surface: spawn terminal in new surface within focused pane
- Close surface: Ctrl+W closes active surface. If last surface, close pane
- Navigate surfaces: Ctrl+Tab cycles forward, Ctrl+Shift+Tab cycles backward
- Surface ID assignment: unique SurfaceId per surface, exposed via WMUX_SURFACE_ID env var
- Tab bar data model: surface titles, active indicator, close button

### Explicitly Out of Scope
- Tab bar rendering (Task L2_04 handles visual rendering)
- Browser surfaces (Task L3_04)
- Drag surfaces between panes (post-MVP)

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-core/src/surface_manager.rs` | SurfaceManager, surface lifecycle |
| Modify | `wmux-core/src/pane_registry.rs` | PaneState includes SurfaceManager |
| Modify | `wmux-core/src/app_state.rs` | Add surface commands (create, close, cycle) |
| Modify | `wmux-core/src/lib.rs` | Export surface_manager module |

### Key Decisions
- **One SurfaceManager per pane**: Each pane independently manages its surfaces. The active surface is the one currently rendered/visible
- **WMUX_SURFACE_ID injection**: When creating a new surface, generate SurfaceId and inject into PTY environment. AI agents use this to target specific surfaces
- **Tab title**: Default to shell name or working directory basename. Can be renamed via API

### Patterns to Follow
- PRD §Modèle Conceptuel: Surface = tab within pane, identified by WMUX_SURFACE_ID
- Architecture §5: Surface lifecycle managed by wmux-core

### Technical Notes
- SurfaceManager stores `Vec<Surface>` and `active_index: usize`
- Surface struct: `{ id: SurfaceId, title: String, kind: PanelKind, terminal: Option<Terminal>, pty_channels: Option<PtyChannels> }`
- Ctrl+T: create new terminal surface in focused pane, spawn PTY, set as active
- Ctrl+W: close active surface. If surfaces.is_empty(), close pane
- Ctrl+Tab: active_index = (active_index + 1) % surfaces.len()
- Only the active surface receives keyboard input and renders
- Inactive surfaces keep their PTY running (output still processed in background)

## Success Criteria
- [ ] Ctrl+T creates a new terminal surface in the focused pane
- [ ] Multiple surfaces in a pane show a tab bar
- [ ] Ctrl+Tab cycles between surfaces
- [ ] Ctrl+W closes the active surface
- [ ] Closing last surface closes the pane
- [ ] Each surface has a unique WMUX_SURFACE_ID
- [ ] Inactive surfaces keep their PTY running
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
1. Ctrl+T — verify new tab appears in tab bar
2. Type in new tab, Ctrl+Tab — verify switch to previous tab with its content intact
3. Ctrl+W — verify tab closes, previous tab becomes active
4. Ctrl+W on last tab — verify pane closes
### Edge Cases to Test
- Create many surfaces (20+) in one pane — tab bar should handle overflow
- Close surface while PTY is outputting — should clean up gracefully
- Ctrl+Tab with single surface (should be no-op or wrap to same)

## Dependencies
**Blocks**:
- Task L2_07: Workspace Lifecycle (workspaces contain panes with surfaces)
- Task L2_12: Workspace & Surface IPC Handlers

## References
- **PRD**: §Modèle Conceptuel (Surface layer), §2 Multiplexeur (onglets par pane)
- **Architecture**: §5 wmux-core, §12 Project Structure
