---
task_id: L1_07
title: "Implement Terminal Grid GPU Rendering Pipeline"
status: pending
priority: P0
estimated_hours: 3
wave: 3
prd_features: [F-01]
archi_sections: [ADR-0001, ADR-0002, ADR-0003]
depends_on: [L1_01, L0_03]
blocks: [L1_10]
---

# Task L1_07: Implement Terminal Grid GPU Rendering Pipeline

> **Phase**: Foundation
> **Priority**: P0-Critical
> **Estimated effort**: 3 hours
> **Wave**: 3

## Context

This task connects the terminal cell grid to the GPU rendering pipeline. Each cell's character must be rendered via glyphon, and each cell's background color via QuadPipeline. The renderer must only upload changed rows (dirty tracking) to maintain 60fps. Architecture §5 (wmux-render) specifies the rendering pipeline. ADR-0002 mandates custom wgpu rendering (NEVER iced/egui). The `.claude/rules/rendering.md` details wgpu 28 and glyphon 0.10 API specifics.

PRD §1 requires latency < 16ms input-to-display.

## Prerequisites

- [ ] Task L1_01: Terminal Cell Grid — provides Grid with dirty tracking and Cell data
- [ ] Task L0_03: QuadPipeline — provides colored rectangle rendering for backgrounds/cursor

## Scope

### Deliverables
- `TerminalRenderer` struct in `wmux-render/src/terminal.rs` — orchestrates grid → GPU
- Cell-to-glyph mapping: read cells from grid, prepare glyphon text buffers per row
- Background color rendering: push cell background quads to QuadPipeline
- Cursor rendering: block/underline/bar via QuadPipeline (with blink timer)
- `TerminalMetrics` struct: cell_width, cell_height, computed from font metrics
- Dirty row optimization: only re-prepare glyphon buffers for changed rows
- Scrollback viewport rendering: render visible portion based on viewport offset

### Explicitly Out of Scope
- Multi-pane rendering (Task L2_04)
- Selection highlighting (Task L1_09 — visual selection rendered later)
- Text search highlighting (Task L4_02)
- Sidebar or UI chrome rendering (Task L2_08)

## Implementation Details

### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-render/src/terminal.rs` | TerminalRenderer, TerminalMetrics |
| Modify | `wmux-render/src/lib.rs` | Export terminal module |
| Modify | `wmux-render/src/text.rs` | Add methods for multi-buffer management |

### Key Decisions
- **One glyphon Buffer per row**: Each row gets its own text buffer. When a row is dirty, only that buffer is re-prepared. This enables O(dirty_rows) per-frame cost instead of O(total_rows)
- **TerminalMetrics from font**: Measure a reference character ('M') via glyphon/cosmic-text to get cell_width and cell_height (line_height). All grid positioning is derived from these metrics
- **Color mapping**: Cell Color enum → wgpu Color. Named colors map to the current theme palette (default xterm-256 until config system exists)

### Patterns to Follow
- `.claude/rules/rendering.md`: "glyphon 0.10 patterns — Buffer::set_text() takes 5 args, use Shaping::Advanced"
- Architecture §5 wmux-render: "Retained-mode rendering (cache glyph atlas across frames), dirty-flag updates"
- `.claude/rules/rendering.md`: "Monospace fonts only for terminal grid"

### Technical Notes
- glyphon 0.10 API: `Buffer::set_text(&mut font_system, text, &attrs, shaping, Option<Align>)` — attrs is `&Attrs`, not owned
- Each row text: concatenate cell graphemes into a single string per row. Apply per-cell color via `AttrsList` with spans
- Cursor rendering: overlay a quad at cursor position. Block = filled rectangle, Underline = thin bottom rect, Bar = thin left rect
- Cursor blink: 500ms on/off cycle. Track with `Instant` and toggle visibility
- Default font: "Cascadia Code" → "Consolas" → system monospace fallback
- Cell position: `(col * cell_width, row * cell_height)` in pixels from top-left of terminal viewport
- Viewport with scrollback: when viewport_offset > 0, shift row indices to read from scrollback

## Success Criteria

- [ ] Terminal content renders correctly (characters at right positions with right colors)
- [ ] Cell background colors render behind text
- [ ] Cursor renders at correct position with correct shape
- [ ] Dirty row optimization: only changed rows trigger glyphon buffer updates
- [ ] TerminalMetrics correctly computes cell dimensions from font
- [ ] Scrollback viewport renders correct rows when scrolled up
- [ ] Frame time < 16ms for 80x24 terminal with full dirty repaint
- [ ] `cargo clippy --workspace` zero warnings

## Validation Steps

### Automated Checks
```bash
cargo build --workspace
cargo clippy --workspace -- -W clippy::all
cargo test -p wmux-render
cargo fmt --all -- --check
```

### Manual Verification
1. Create a test grid filled with colored text, render it, verify visual correctness
2. Modify a single row, verify only that row's buffer is re-prepared (tracing log)
3. Test cursor at various positions — verify correct rendering
4. Test with scrollback offset — verify historical rows appear

### Edge Cases to Test
- Empty grid (all spaces) — should render clean background
- Grid with only wide characters (CJK) — cells should double-width render
- Row with mixed colors (many SGR changes) — verify all spans render correctly
- Very long scrollback viewport offset (close to 4000 lines)

## Dependencies

**Blocks**:
- Task L1_10: Single-Pane Terminal Integration

## References
- **PRD**: §1 Terminal GPU-Acceleré (< 16ms latency, GPU rendering)
- **Architecture**: §5 wmux-render, §6 Data Flow — Terminal I/O (GPU rendering step)
- **ADR**: ADR-0002 (custom wgpu), ADR-0003 (glyphon 0.10)
