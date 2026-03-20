---
task_id: L1_01
title: "Implement Terminal Cell Grid with Dirty Tracking"
status: done
priority: P0
estimated_hours: 2.5
wave: 2
prd_features: [F-01]
archi_sections: [ADR-0001]
depends_on: [L0_02]
blocks: [L1_02, L1_03, L1_07, L1_09]
---

# Task L1_01: Implement Terminal Cell Grid with Dirty Tracking

> **Phase**: Foundation
> **Priority**: P0-Critical
> **Estimated effort**: 2.5 hours
> **Wave**: 2

## Context

The terminal cell grid is the central data structure that holds all visible terminal content. Every character displayed maps to a Cell in the grid. The grid must support efficient updates (individual cell writes, line operations) and expose dirty row flags so the GPU renderer only uploads changed rows. Architecture §5 (wmux-core) specifies "cell grid, scrollback buffer, cursor/mode state". The `.claude/rules/terminal-vte.md` rule mandates contiguous cell storage (Vec<Cell> per row), NOT Vec<Vec<Cell>>.

PRD §1 (Terminal GPU-Acceleré) requires scrollback of 4K lines and 60fps rendering — dirty tracking is essential for performance.

## Prerequisites

- [ ] Task L0_02: Domain Model Types — provides Cell, Row, CursorState, Color, CellFlags

## Scope

### Deliverables
- `Grid` struct in `wmux-core/src/grid.rs` with row storage, dimensions, dirty flags
- Row-level operations: clear_row, insert_chars, delete_chars, set_cell
- Grid-level operations: scroll_up, scroll_down, resize, clear
- Dirty tracking: per-row dirty bitflags, `take_dirty_rows()` method
- Cursor integration: grid tracks cursor position, enforces bounds

### Explicitly Out of Scope
- Scrollback buffer (Task L1_03)
- VTE parsing (Task L1_02)
- Alternate screen buffer (Task L1_03)
- GPU rendering of grid (Task L1_07)

## Implementation Details

### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-core/src/grid.rs` | Grid struct with all operations |
| Modify | `wmux-core/src/lib.rs` | Export grid module |

### Key Decisions
- **Contiguous Vec<Cell> per row** (`.claude/rules/terminal-vte.md`): Better cache locality than Vec<Vec<Cell>>. Each Row is `Vec<Cell>` with fixed capacity equal to column count
- **Dirty bitvec**: Use a `Vec<bool>` (one per row) for dirty tracking. When any cell in a row changes, mark row dirty. Renderer calls `take_dirty_rows()` which returns dirty indices and clears all flags
- **Grid owns cursor position**: Cursor is logically part of grid state. All cell writes go through cursor position. Grid enforces bounds (0..cols, 0..rows)

### Patterns to Follow
- Architecture §5 wmux-core: "contiguous Vec<Cell> per row"
- `.claude/rules/rust-architecture.md`: Pre-allocate Vec with `with_capacity()`, reuse allocations in hot paths
- Row default: all cells initialized to default Cell (space, white on default bg)

### Technical Notes
- Grid resize must handle both grow and shrink. On shrink, truncate excess columns/rows. On grow, pad with default cells
- scroll_up: move rows up by N, clearing bottom N rows. This is the operation triggered by terminal newline at bottom
- scroll_down: move rows down by N, clearing top N rows (reverse index)
- insert_chars/delete_chars operate at cursor position within the current row
- All mutations mark affected rows dirty
- Grid dimensions stored as (cols: u16, rows: u16) — u16 is sufficient (max 65535)

## Success Criteria

- [ ] Grid correctly stores and retrieves cells by (col, row) coordinates
- [ ] Dirty tracking correctly identifies only changed rows
- [ ] `take_dirty_rows()` returns dirty indices and resets all dirty flags
- [ ] scroll_up/scroll_down correctly shift rows and clear new rows
- [ ] Grid resize handles grow and shrink without panic
- [ ] All operations are O(rows) or better
- [ ] `cargo test -p wmux-core` passes with grid tests

## Validation Steps

### Automated Checks
```bash
cargo build --workspace
cargo clippy --workspace -- -W clippy::all
cargo test -p wmux-core
cargo fmt --all -- --check
```

### Manual Verification
1. Unit test: write cells at various positions, verify grid content
2. Unit test: scroll_up 5 rows, verify content shifted and bottom rows cleared
3. Unit test: resize from 80x24 to 120x30, verify padding; resize back, verify truncation

### Edge Cases to Test
- Write to cell at (0,0) and (cols-1, rows-1) — boundary positions
- Scroll up more rows than grid height (should clear entire grid)
- Resize to 1x1 (minimum viable grid)
- Set cell with wide character (WIDE_SPACER in next cell)
- take_dirty_rows() called twice without changes (second call returns empty)

## Dependencies

**Blocks**:
- Task L1_02: VTE Parser Integration
- Task L1_03: Scrollback Ring Buffer
- Task L1_07: Terminal Grid GPU Rendering
- Task L1_09: Mouse Selection, Copy/Paste, Scroll

## References
- **PRD**: §1 Terminal GPU-Acceleré (scrollback, performance)
- **Architecture**: §5 wmux-core, §6 Data Flow — Terminal I/O
- **ADR**: ADR-0002 (dirty-row optimization for GPU)
