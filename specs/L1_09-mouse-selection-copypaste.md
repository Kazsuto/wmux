# Task L1_09: Implement Mouse Selection, Copy/Paste, and Scroll

> **Phase**: Foundation
> **Priority**: P1-High
> **Estimated effort**: 2.5 hours

## Context

Users need to select text with the mouse, copy to clipboard, paste from clipboard, and scroll through terminal history. Mouse reporting to the PTY is also needed for TUI applications (vim, htop). Architecture §3 specifies arboard for clipboard. The `.claude/rules/terminal-vte.md` mentions mouse reporting. PRD §1 lists selection, copy/paste, and scroll as terminal requirements.

## Prerequisites

- [ ] Task L1_01: Terminal Cell Grid — provides grid content for selection
- [ ] Task L1_06: PTY Async I/O — provides write channel for mouse reporting to PTY

## Scope

### Deliverables
- Text selection model: `Selection` struct (start, end, mode: Normal/Word/Line)
- Click-drag selection (single click + drag = character selection)
- Double-click = word selection, triple-click = line selection
- Copy: `Ctrl+Shift+C` → extract selected text → arboard clipboard
- Paste: `Ctrl+Shift+V` → read clipboard → send to PTY (with bracketed paste wrapping if mode enabled)
- Scroll: mouse wheel → adjust scrollback viewport offset
- Mouse reporting: when terminal has mouse mode enabled, forward mouse events as VT sequences to PTY (SGR format)
- Selection visual highlighting (mark cells for GPU renderer to highlight)

### Explicitly Out of Scope
- Right-click context menu (post-MVP)
- Drag-and-drop (file drop into terminal)
- Hyperlink click handling (post-MVP)
- URL auto-detection and highlighting (post-MVP)

## Implementation Details

### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-core/src/selection.rs` | Selection model (start, end, mode) |
| Create | `wmux-ui/src/mouse.rs` | Mouse event handler (selection, scroll, reporting) |
| Modify | `wmux-ui/src/lib.rs` | Export mouse module |
| Modify | `wmux-core/src/lib.rs` | Export selection module |
| Modify | `Cargo.toml` (workspace) | Ensure arboard dependency |

### Key Decisions
- **Ctrl+Shift+C/V for copy/paste** (Architecture §3): Avoids conflict with Ctrl+C (SIGINT) in terminal. This is a hard requirement
- **SGR mouse format** (`.claude/rules/terminal-vte.md`): `\x1b[<button;col;row;M/m` — modern format supporting coordinates > 255. Only when mouse mode is enabled (DECSET 1000/1002/1003)
- **Selection coordinates in grid space**: Selection start/end are (col, row) in the grid (accounting for scrollback offset). Selection model is in wmux-core, mouse handler is in wmux-ui

### Patterns to Follow
- Architecture §3 Cross-Cutting Concerns: "arboard 3 — Ctrl+Shift+C / Ctrl+Shift+V"
- `.claude/rules/terminal-vte.md`: Mouse reporting SGR format
- Selection text extraction: iterate cells in selection range, concatenate graphemes, trim trailing spaces per row

### Technical Notes
- Mouse position in pixels → grid coordinates: `(pixel_x / cell_width, pixel_y / cell_height)` using TerminalMetrics
- Word selection: expand from click position to word boundaries (alphanumeric + underscore)
- Line selection: select entire row
- Scroll wheel: delta → adjust viewport_offset. Clamp to [0, scrollback.len()]. Scrolling down past 0 resets to live terminal
- Mouse mode 1000 = button events only, 1002 = button + drag, 1003 = all motion
- SGR encoding: `\x1b[<0;col;rowM` for press, `\x1b[<0;col;rowm` for release. Button 0=left, 1=middle, 2=right
- Selection rendering: mark selected cells with a flag or provide selection range to renderer for overlay quad rendering
- When mouse reporting is active AND user holds Shift, bypass reporting and do selection instead (standard convention)

## Success Criteria

- [ ] Click-drag selects text correctly in the terminal
- [ ] Double-click selects a word, triple-click selects a line
- [ ] Ctrl+Shift+C copies selected text to system clipboard
- [ ] Ctrl+Shift+V pastes clipboard content to PTY
- [ ] Bracketed paste wraps pasted content when mode is enabled
- [ ] Mouse wheel scrolls through scrollback history
- [ ] Mouse reporting sends correct SGR sequences when mouse mode is enabled
- [ ] Shift+click bypasses mouse reporting for selection
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
1. Unit test: selection from (0,0) to (10,0) → extract text → verify correct substring
2. Unit test: word selection at position within "hello world" → verify "hello" or "world" selected
3. Test paste with bracketed paste mode → verify wrapping sequences

### Edge Cases to Test
- Selection spanning multiple rows (should include newlines)
- Selection with wide characters (CJK) — word boundary detection
- Paste empty clipboard (should be no-op)
- Scroll wheel at top of scrollback (clamp, don't crash)
- Scroll wheel at bottom (reset to live view)
- Mouse reporting with coordinates > 255 (SGR handles this natively)

## Dependencies

**Blocks**:
- Task L1_10: Single-Pane Terminal Integration

## References
- **PRD**: §1 Terminal GPU-Acceleré (selection, copy/paste, scroll)
- **Architecture**: §3 Cross-Cutting Concerns (arboard, Ctrl+Shift+C/V)
- **ADR**: ADR-0007 (winit mouse events)
