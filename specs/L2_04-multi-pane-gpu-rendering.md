# Task L2_04: Implement Multi-Pane GPU Rendering

> **Phase**: Core
> **Priority**: P0-Critical
> **Estimated effort**: 2.5 hours

## Context
With the PaneTree computing layout rects, each pane must be rendered in its allocated viewport region. The GPU renderer must clip each pane's terminal to its rect, render pane borders, show the focused pane highlight, and render tab bars for panes with multiple surfaces. Architecture §5 (wmux-render) manages all render state centrally. ADR-0002 mandates custom wgpu rendering.

## Prerequisites
- [ ] Task L2_02: PaneTree Layout Engine — provides layout rects per pane
- [ ] Task L0_03: QuadPipeline — provides colored rectangles for borders/dividers

## Scope
### Deliverables
- Multi-pane render loop: iterate PaneTree layout, render each pane in its rect
- Viewport clipping: set wgpu scissor rect per pane to prevent overflow
- Pane borders: 1px border quads around each pane
- Focused pane highlight: distinct border color for focused pane
- Divider rendering: visible gap between split panes
- Tab bar rendering: when a pane has multiple surfaces, render tab strip at top
- Active tab highlighting

### Explicitly Out of Scope
- Draggable divider interaction (Task L2_05 — this task only renders them)
- Sidebar rendering (Task L2_08)
- Overlay rendering (Tasks L4_01, L4_02, L3_09)

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-render/src/pane.rs` | PaneRenderer (multi-pane orchestration) |
| Modify | `wmux-render/src/terminal.rs` | Accept viewport rect parameter for offset rendering |
| Modify | `wmux-ui/src/app.rs` | Call PaneRenderer instead of single TerminalRenderer |
| Modify | `wmux-render/src/lib.rs` | Export pane module |

### Key Decisions
- **Scissor rect per pane**: wgpu `RenderPass::set_scissor_rect()` clips rendering to pane bounds. This prevents terminal text from bleeding into adjacent panes
- **Shared render pass**: All panes render in the same wgpu render pass. First all quads (backgrounds, borders), then all text. Minimizes GPU state changes
- **Tab bar height**: 24px at top of pane when multiple surfaces exist. Single surface = no tab bar (maximize terminal space)

### Patterns to Follow
- Architecture §5 wmux-render: "All render state owned centrally by App"
- ADR-0002: Custom wgpu rendering, no iced/egui
- `.claude/rules/rendering.md`: wgpu 28 patterns, render pass setup

### Technical Notes
- Render order per frame: (1) Clear background, (2) For each pane: set scissor rect → render cell backgrounds → render tab bar → render terminal text → render cursor, (3) Render pane borders/dividers (unclipped), (4) Render focus highlight
- Border colors: normal = theme border color, focused = accent color (e.g., blue)
- Tab bar: render using QuadPipeline (background) + GlyphonRenderer (tab titles). Active tab has distinct background
- When zoomed, only render the zoomed pane (skip PaneTree iteration, use full viewport rect)
- Divider gap: 4px between adjacent panes. Rendered as background-colored quads

## Success Criteria
- [ ] Multiple panes render correctly in their allocated regions
- [ ] Terminal content is clipped to pane boundaries (no bleeding)
- [ ] Pane borders are visible and correctly positioned
- [ ] Focused pane has visually distinct border
- [ ] Tab bar renders when pane has multiple surfaces
- [ ] Zoomed pane fills entire workspace area
- [ ] Performance: < 16ms for 4 panes with typical content
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
1. Split right (Ctrl+D) — verify two panes render side by side
2. Split bottom (Ctrl+Shift+D) — verify pane splits vertically
3. Focus different panes — verify border color changes
4. Verify terminal content in each pane is independent and correctly clipped
### Edge Cases to Test
- Many panes (8+) — verify all render correctly with small sizes
- Resize window with multiple panes — verify layout recalculates
- Pane at minimum size — verify content still renders (truncated)

## Dependencies
**Blocks**:
- Task L2_05: Draggable Dividers + Pane Resize
- Task L4_02: Terminal Search (match highlighting in pane context)

## References
- **PRD**: §2 Multiplexeur (split panes, visual)
- **Architecture**: §5 wmux-render, §4 Component Diagram
- **ADR**: ADR-0002 (custom wgpu), ADR-0003 (glyphon)
