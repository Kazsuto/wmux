---
task_id: L2_02
title: "Implement PaneTree Binary Split Layout Engine"
status: done
priority: P0
estimated_hours: 2.5
wave: 7
prd_features: [F-02]
archi_sections: [ADR-0001]
depends_on: [L2_01, L0_02]
blocks: [L2_03, L2_04, L2_06, L2_07, L3_04]
---

# Task L2_02: Implement PaneTree Binary Split Layout Engine

> **Phase**: Core
> **Priority**: P0-Critical
> **Estimated effort**: 2.5 hours
> **Wave**: 7

## Context
The multiplexer needs a binary tree layout engine where each split divides a region into two children (horizontal or vertical). This is the spatial layout core of wmux. Architecture §5 (wmux-core) specifies "Binary split tree" in pane_tree.rs. PRD §2 describes split panes with dividers.

## Prerequisites
- [ ] Task L2_01: AppState Actor — provides PaneRegistry and actor infrastructure
- [ ] Task L0_02: Domain Model Types — provides PaneId, SurfaceId, SplitDirection

## Scope
### Deliverables
- `PaneTree` enum in `wmux-core/src/pane_tree.rs`: `Leaf(PaneId)` | `Split { direction, ratio, first, second }`
- Layout calculation: `layout(viewport: Rect) -> Vec<(PaneId, Rect)>`
- `split_pane(pane_id, direction)` → creates new pane, returns new PaneId
- `close_pane(pane_id)` → removes pane, promotes sibling
- `swap_pane(a, b)` → swap two panes
- `resize_split(split_node, new_ratio)` → adjust split ratio
- `find_pane(pane_id)` → returns path to pane in tree
- `Rect` struct (x, y, width, height as f32)
- Minimum pane size enforcement (80px width, 40px height)

### Explicitly Out of Scope
- GPU rendering of panes (Task L2_04)
- Draggable dividers (Task L2_05)
- Focus routing (Task L2_03)
- Surface (tab) management within panes (Task L2_06)

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-core/src/pane_tree.rs` | PaneTree enum, layout algorithm, mutations |
| Create | `wmux-core/src/rect.rs` | Rect struct (x, y, w, h) |
| Modify | `wmux-core/src/app_state.rs` | Add PaneTree to AppState |
| Modify | `wmux-core/src/lib.rs` | Export pane_tree, rect modules |

### Key Decisions
- **Binary tree (not n-ary)**: Each split has exactly two children. Simpler algorithm, matches cmux/tmux model. N-way splits are achieved by nesting binary splits
- **Ratio-based splitting**: Each split stores a ratio (0.0-1.0) for the first child's share. Default 0.5 (equal split). Divider drag adjusts ratio (Task L2_05)
- **Recursive layout**: `layout()` recursively subdivides the viewport rect. Horizontal split divides width, vertical split divides height

### Patterns to Follow
- Architecture §5 wmux-core: "Binary split tree"
- Architecture §12 Project Structure: `wmux-core/src/pane_tree.rs`
- Serde derive for persistence (Task L3_01)

### Technical Notes
- Split horizontal: first child gets left portion (width * ratio), second gets right (width * (1-ratio)). Gap of ~4px for divider
- Split vertical: first child gets top portion, second gets bottom
- close_pane: find pane in tree, replace parent split with sibling subtree. If only one pane remains, tree becomes Leaf
- swap_pane: find both panes, swap their PaneId values in the leaves
- Minimum size: during layout(), if a pane rect would be smaller than minimum, clamp it and adjust sibling. If still can't fit, skip rendering the too-small pane
- PaneTree is serializable for session persistence

## Success Criteria
- [ ] PaneTree correctly computes layout rects for any tree configuration
- [ ] split_pane creates correct binary tree structure
- [ ] close_pane correctly promotes sibling
- [ ] swap_pane exchanges pane positions
- [ ] Minimum pane size is enforced
- [ ] Layout rects have no gaps or overlaps (accounting for divider width)
- [ ] Nested splits (3+ levels deep) work correctly
- [ ] `cargo test -p wmux-core` passes with layout tests

## Validation Steps
### Automated Checks
```bash
cargo build --workspace
cargo clippy --workspace -- -W clippy::all
cargo test -p wmux-core
cargo fmt --all -- --check
```
### Manual Verification
1. Unit test: single pane → layout = full viewport
2. Unit test: horizontal split → two rects side by side
3. Unit test: split left child again → three panes with correct geometry
4. Unit test: close middle pane → verify sibling promotion
### Edge Cases to Test
- Split when at minimum size (should refuse or force minimum)
- Close last remaining pane (tree becomes empty — should be handled by workspace)
- Deeply nested tree (10 levels) — verify no stack overflow
- Ratio of 0.0 or 1.0 (degenerate split — one child invisible)

## Dependencies
**Blocks**:
- Task L2_03: Focus Routing + Keyboard Shortcut Dispatcher
- Task L2_04: Multi-Pane GPU Rendering
- Task L2_05: Draggable Dividers
- Task L2_06: Surface Tab System
- Task L2_07: Workspace Lifecycle
- Task L3_04: WebView2 Browser Panel (needs pane layout)

## References
- **PRD**: §2 Multiplexeur (split panes, binary tree, dividers)
- **Architecture**: §5 wmux-core (pane_tree.rs), §4 Component Diagram (Multiplexer)
- **ADR**: ADR-0009 (serializable for persistence)
