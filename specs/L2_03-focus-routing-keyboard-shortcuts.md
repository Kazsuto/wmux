---
task_id: L2_03
title: "Implement Focus Routing and Keyboard Shortcut Dispatcher"
status: pending
priority: P0
estimated_hours: 2
wave: 8
prd_features: [F-02]
archi_sections: [ADR-0001, ADR-0007]
depends_on: [L2_02]
blocks: [L4_07]
---

# Task L2_03: Implement Focus Routing and Keyboard Shortcut Dispatcher

> **Phase**: Core
> **Priority**: P0-Critical
> **Estimated effort**: 2 hours
> **Wave**: 8

## Context
With multiple panes, keyboard input must be routed to the focused pane. Global shortcuts (Ctrl+N, Ctrl+D, etc.) must be intercepted before reaching the terminal. The shortcut dispatcher has a priority chain: global shortcuts > overlay shortcuts > pane-local input. PRD §Raccourcis Clavier defines all keyboard shortcuts. Architecture §13 Phase 2 mentions "Global keyboard shortcut priority dispatcher."

## Prerequisites
- [ ] Task L2_02: PaneTree Layout Engine — provides pane structure for focus navigation

## Scope
### Deliverables
- Focus state tracking: `focused_pane: PaneId` in AppState
- Directional focus navigation: `Alt+Ctrl+Arrow` to move focus between adjacent panes
- Global shortcut map with priority dispatcher
- Shortcut priority chain: Global → Overlay → Pane
- All PRD shortcuts: Ctrl+N, Ctrl+1-9, Ctrl+T, Ctrl+D, Ctrl+Shift+D, Ctrl+W, Ctrl+Shift+Enter (zoom), Ctrl+Shift+C/V, Ctrl+Shift+P, Ctrl+F, Ctrl+Shift+I, Ctrl+Shift+U
- Zoom/unzoom pane (Ctrl+Shift+Enter): focused pane fills workspace, toggle back

### Explicitly Out of Scope
- Custom keybinding configuration (Task L3_11)
- Command palette (Task L4_01 — shortcut dispatches to it)
- Overlay input handling (overlays handle their own input when active)

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-ui/src/shortcuts.rs` | Shortcut map, priority dispatcher |
| Modify | `wmux-ui/src/input.rs` | Route input through shortcut dispatcher first |
| Modify | `wmux-core/src/app_state.rs` | Add focus state, zoom state |

### Key Decisions
- **Priority chain**: Check global shortcuts first. If no match and an overlay is active, route to overlay. Otherwise, route to focused pane's InputHandler
- **Directional navigation**: From focused pane, find the adjacent pane in the requested direction using PaneTree geometry (compare rects). Focus the closest pane whose center is in that direction
- **Zoom state**: When zoomed, PaneTree layout returns only the zoomed pane rect (full viewport). Other panes still exist but aren't rendered. Unzoom restores normal layout

### Patterns to Follow
- PRD §Raccourcis Clavier: Full mapping table
- Architecture §13 Phase 2: "Global keyboard shortcut priority dispatcher"

### Technical Notes
- Ctrl+1-9: Navigate to workspace by index (1-based). If workspace doesn't exist, ignore
- Ctrl+D: Split focused pane right (horizontal split)
- Ctrl+Shift+D: Split focused pane down (vertical split)
- Ctrl+W: Close focused surface (if last surface in pane, close pane)
- Ctrl+Tab: Cycle surfaces within focused pane (Task L2_06)
- Focus visual: focused pane has a distinct border color (rendered by Task L2_04)
- Directional nav algorithm: from focus pane rect center, cast a ray in direction, find first pane rect intersected

## Success Criteria
- [ ] Keyboard input routes to focused pane only
- [ ] Alt+Ctrl+Arrows navigate focus between panes
- [ ] Global shortcuts are intercepted before reaching terminal
- [ ] Ctrl+Shift+Enter toggles zoom on focused pane
- [ ] Ctrl+D splits right, Ctrl+Shift+D splits down
- [ ] Ctrl+W closes current surface/pane
- [ ] Ctrl+N creates new workspace (placeholder — delegates to Task L2_07)
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
1. Create two panes (split right), type in left, verify only left receives input
2. Alt+Ctrl+Right → verify focus moves to right pane
3. Ctrl+Shift+Enter → verify pane zooms to fill workspace
4. Ctrl+Shift+Enter again → verify returns to split view
### Edge Cases to Test
- Navigate direction with no adjacent pane (should be no-op)
- Zoom while only one pane (should still work — full viewport = same as normal)
- Ctrl+W on last pane in workspace (should close workspace or leave empty)
- Rapid shortcut input (should handle all events without dropping)

## Dependencies
**Blocks**:
- Task L2_04: Multi-Pane GPU Rendering (needs focus state for border colors)
- Task L2_05: Draggable Dividers (needs focus for cursor change)

## References
- **PRD**: §2 Multiplexeur (focus routing), §Raccourcis Clavier (full shortcut table)
- **Architecture**: §13 Phase 2 (keyboard shortcut dispatcher)
- **ADR**: ADR-0007 (winit keyboard events)
