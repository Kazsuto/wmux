# Task L1_03: Implement Scrollback Ring Buffer and Alternate Screen

> **Phase**: Foundation
> **Priority**: P0-Critical
> **Estimated effort**: 2 hours

## Context

The scrollback buffer stores terminal history that has scrolled off the visible grid. Users and agents need to scroll back through terminal output (up to 4K lines). The alternate screen buffer (used by vim, less, htop) provides a separate grid without scrollback. Architecture §5 specifies "ring buffer scrollback" using VecDeque. PRD §1 specifies 4K lines default scrollback.

## Prerequisites

- [ ] Task L1_01: Terminal Cell Grid — provides Grid struct that scrollback extends

## Scope

### Deliverables
- `Scrollback` struct in `wmux-core/src/scrollback.rs` — VecDeque<Row> ring buffer
- Configurable max lines (default 4000)
- Push rows from grid top into scrollback on scroll_up
- Viewport offset tracking for scroll position
- Alternate screen buffer: separate Grid, no scrollback
- Integration with Terminal: smcup (enter alt screen) / rmcup (exit alt screen)
- `read_text(start, end)` method for API read access

### Explicitly Out of Scope
- GPU rendering of scrollback (Task L1_07)
- Search within scrollback (Task L4_02)
- Scrollback serialization for persistence (Task L3_01)

## Implementation Details

### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-core/src/scrollback.rs` | Scrollback ring buffer |
| Modify | `wmux-core/src/terminal.rs` | Integrate scrollback + alternate screen |
| Modify | `wmux-core/src/grid.rs` | Add scroll_up callback to push rows to scrollback |

### Key Decisions
- **VecDeque<Row>** (`.claude/rules/terminal-vte.md`): Efficient push_back with automatic front eviction when exceeding max_lines. O(1) push and pop
- **Viewport offset**: 0 = bottom (live terminal), positive = scrolled up N lines. Scroll wheel changes offset, new output resets to 0
- **Alternate screen** (`.claude/rules/terminal-vte.md`): Completely separate Grid. Entering alt screen saves main grid + cursor. Exiting restores them. Alt screen does NOT have scrollback

### Patterns to Follow
- `.claude/rules/terminal-vte.md`: "Alternate screen buffer (smcup/rmcup) does NOT share scrollback"
- Architecture §10 Failure Modes: "Scrollback hard-capped at 4K lines / 400K chars per terminal"
- ADR-0009: Scrollback serialization limit of 4000 lines / 400K chars

### Technical Notes
- When grid scroll_up is triggered (newline at bottom), the top row is pushed to scrollback before being cleared
- If scrollback exceeds max_lines, VecDeque::pop_front() evicts the oldest row
- read_text(start, end) returns text as String, concatenating rows with newlines. Negative start = relative from end
- DECSET 1049 (enter alt screen): save main grid + scrollback viewport, create fresh grid, reset cursor
- DECRST 1049 (exit alt screen): restore main grid + scrollback viewport, discard alt grid
- Also handle DECSET 47/1047 (simpler alt screen without cursor save)

## Success Criteria

- [ ] Scrollback stores rows pushed from grid scroll_up operations
- [ ] Maximum line limit is enforced (oldest rows evicted)
- [ ] Viewport offset allows browsing scrollback history
- [ ] Alternate screen creates a clean grid and restores the original on exit
- [ ] read_text() returns accurate text content including scrollback
- [ ] Memory usage stays bounded (no unbounded growth)
- [ ] `cargo test -p wmux-core` passes

## Validation Steps

### Automated Checks
```bash
cargo build --workspace
cargo clippy --workspace -- -W clippy::all
cargo test -p wmux-core
cargo fmt --all -- --check
```

### Manual Verification
1. Unit test: push 5000 rows into 4000-line scrollback → verify only last 4000 remain
2. Unit test: scroll viewport up 10 lines → verify correct rows visible
3. Unit test: enter alt screen → modify grid → exit alt screen → verify original grid restored

### Edge Cases to Test
- Scrollback with 0 max_lines (disabled — rows are discarded)
- Exit alt screen without entering (should be no-op)
- read_text with start beyond scrollback (return empty or available portion)
- Very long rows (ensure no char limit per row breaks, only line limit)

## Dependencies

**Blocks**:
- Task L1_10: Single-Pane Terminal Integration
- Task L4_02: Terminal Search

## References
- **PRD**: §1 Terminal GPU-Acceleré (scrollback 4K), §6 Lecture du Terminal (read_text)
- **Architecture**: §5 wmux-core (ring buffer), §6 Data Architecture (400K chars limit)
- **ADR**: ADR-0009 (scrollback limits in persistence)
