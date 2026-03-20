---
task_id: L4_02
title: "Implement Terminal Search with Match Highlighting"
status: pending
priority: P2
estimated_hours: 2
wave: 9
prd_features: [F-12]
archi_sections: [ADR-0001, ADR-0003]
depends_on: [L1_03, L2_04]
blocks: [L4_07]
---

# Task L4_02: Implement Terminal Search with Match Highlighting

> **Phase**: Polish
> **Priority**: P2-Medium
> **Estimated effort**: 2 hours
> **Wave**: 9

## Context
Terminal search (Ctrl+F) lets users find text in the scrollback and visible grid. Matches are highlighted and navigable with n/N (vi-style). PRD §12 requires regex support and < 100ms search on 4K lines.

## Prerequisites
- [ ] Task L1_03: Scrollback Ring Buffer — provides scrollback content to search
- [ ] Task L2_04: Multi-Pane GPU Rendering — provides per-pane rendering context for highlights

## Scope
### Deliverables
- Search overlay (Ctrl+F): input field at bottom of active pane
- Search through scrollback + visible grid for matches
- Match highlighting via QuadPipeline overlays (colored background on matches)
- n/N navigation: jump to next/previous match
- Match count display ("3/15 matches")
- Optional regex mode
- Search clears on Escape or Ctrl+F toggle

### Explicitly Out of Scope
- Cross-surface search (that's command palette Ctrl+P in Task L4_01)
- Replace functionality (post-MVP)
- Case sensitivity toggle (always case-insensitive, post-MVP for toggle)

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-ui/src/search.rs` | Search overlay, match finding, navigation |
| Modify | `wmux-render/src/terminal.rs` | Add match highlight rendering |
| Modify | `wmux-ui/src/shortcuts.rs` | Add Ctrl+F shortcut |
| Modify | `wmux-ui/src/app.rs` | Integrate search overlay |

### Key Decisions
- **Search model in wmux-core**: Search returns list of (row, col_start, col_end) match positions. Renderer uses these to draw highlight quads
- **Incremental search**: Update results on every keystroke (not just on Enter)
- **Alternate screen**: Search only the active screen (main or alternate), not both

### Patterns to Follow
- PRD §12: "vi-style (n/N)", "< 100ms on 4K lines"

### Technical Notes
- Search algorithm: iterate rows (scrollback + grid), string search per row. Collect all (row, col_start, col_end) tuples
- Current match index: tracks which match is "focused" (highlighted differently)
- n key: advance to next match, scroll viewport if needed
- N key: go to previous match
- Match highlight: QuadPipeline rect with yellow/orange semi-transparent background
- Current match: brighter highlight (distinct from other matches)
- Performance: search 4000 lines × 200 cols = 800K chars. Simple string search is O(n) and fast
- Regex: use `regex` crate. Compile once per search query, apply per row
- Search overlay renders at bottom of the focused pane (not full screen)
- When search is active, n/N keys are intercepted (not sent to terminal)

## Success Criteria
- [ ] Ctrl+F opens search input
- [ ] Typing highlights matches in terminal
- [ ] n/N navigate between matches
- [ ] Match count displayed
- [ ] Escape closes search and clears highlights
- [ ] Search completes in < 100ms on 4K lines
- [ ] Regex mode works
- [ ] Viewport scrolls to show match when navigating
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
1. Generate lots of output → Ctrl+F → search for a term → verify highlights
2. n/N → verify navigation between matches
3. Regex search → verify pattern matching works
4. Escape → verify highlights cleared
### Edge Cases to Test
- No matches (should show "0/0 matches")
- Very many matches (1000+) — should not freeze
- Search empty string (should show no matches, not highlight everything)
- Invalid regex (should show error in search bar, not crash)

## Dependencies
**Blocks**: None — leaf polish task

## References
- **PRD**: §12 Recherche Terminal
- **Architecture**: §5 wmux-ui (overlay.rs)
