---
task_id: L2_05
title: "Implement Draggable Dividers and Pane Resize"
status: pending
priority: P1
estimated_hours: 2
wave: 9
prd_features: [F-02]
archi_sections: [ADR-0001, ADR-0007]
depends_on: [L2_04]
blocks: [L4_07]
---

# Task L2_05: Implement Draggable Dividers and Pane Resize

> **Phase**: Core
> **Priority**: P1-High
> **Estimated effort**: 2 hours
> **Wave**: 9

## Context
Users need to resize panes by dragging the dividers between them. This requires mouse hit-testing on divider regions, cursor change on hover, and updating the PaneTree split ratio during drag. PRD §2 mentions "dividers draggables."

## Prerequisites
- [ ] Task L2_04: Multi-Pane GPU Rendering — dividers are visible and pane rects are known

## Scope
### Deliverables
- Divider hit-testing: detect mouse hover over divider regions (4px gap between panes)
- Cursor change: show resize cursor (ew-resize or ns-resize) when hovering divider
- Drag handling: mouse press on divider → track drag → update split ratio
- Minimum pane size enforcement during drag
- Divider visual feedback: highlight divider on hover

### Explicitly Out of Scope
- Keyboard-based pane resize (post-MVP)
- Drag pane to different workspace (post-MVP)

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-ui/src/divider.rs` | Divider hit-test, drag handling |
| Modify | `wmux-ui/src/mouse.rs` | Route mouse events to divider handler when applicable |
| Modify | `wmux-ui/src/app.rs` | Integrate divider handler into event loop |
| Modify | `wmux-ui/src/lib.rs` | Export divider module |

### Key Decisions
- **Hit-test region**: 4px divider gap + 2px padding on each side (total 8px target area) for easy mouse targeting
- **Ratio clamping**: During drag, clamp ratio so neither child pane falls below minimum size
- **Real-time resize**: Update PaneTree ratio and re-layout on every mouse move during drag (not just on mouse release)

### Patterns to Follow
- winit cursor API: `Window::set_cursor(CursorIcon::EwResize)` for horizontal divider, `NsResize` for vertical
- PRD §2: "dividers draggables"

### Technical Notes
- Mouse position → divider identification: iterate PaneTree splits, check if mouse is within divider rect (between children rects)
- During drag: calculate new ratio = (mouse_position - split_start) / split_total_size
- Clamp: min_ratio = min_pane_size / split_total_size, max_ratio = 1.0 - min_ratio
- Reset cursor to default when leaving divider area
- Double-click divider: reset ratio to 0.5 (equal split) — nice UX touch

## Success Criteria
- [ ] Cursor changes to resize icon when hovering over divider
- [ ] Dragging divider resizes adjacent panes in real-time
- [ ] Minimum pane size is enforced during drag
- [ ] Divider highlights on hover
- [ ] Double-click resets split to equal
- [ ] Cursor resets when leaving divider area
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
1. Split right, hover over divider — verify cursor changes
2. Drag divider left and right — verify panes resize
3. Drag to extreme position — verify minimum size prevents collapse
4. Double-click divider — verify reset to 50/50
### Edge Cases to Test
- Drag past window edge (should clamp)
- Very fast drag movement (should track smoothly)
- Nested dividers (inner divider between sub-panes) — should resize correct split

## Dependencies
**Blocks**: None — leaf task in the pane interaction chain

## References
- **PRD**: §2 Multiplexeur (dividers draggables)
- **Architecture**: §5 wmux-ui (split_container.rs)
