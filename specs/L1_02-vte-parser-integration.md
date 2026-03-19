# Task L1_02: Implement VTE Parser Integration (vte::Perform → Grid)

> **Phase**: Foundation
> **Priority**: P0-Critical
> **Estimated effort**: 3 hours

## Context

The VTE parser translates raw bytes from the PTY into structured terminal operations (print character, move cursor, change colors, etc.). This is the bridge between raw terminal output and the cell grid. Architecture §5 specifies vte 0.13 (Alacritty's parser). The `.claude/rules/terminal-vte.md` rule mandates using the vte crate (NEVER custom parser) and lists required escape sequences.

PRD §1 requires correct rendering of all standard ANSI/VT100/VT220/xterm-256color sequences.

## Prerequisites

- [ ] Task L1_01: Terminal Cell Grid — provides Grid struct for VTE handler to mutate

## Scope

### Deliverables
- `Terminal` struct in `wmux-core/src/terminal.rs` — owns Grid, CursorState, TerminalMode, current attributes
- `VteHandler` implementing `vte::Perform` trait in `wmux-core/src/vte_handler.rs`
- Character printing (including wide chars, grapheme clusters)
- Cursor movement: CUU, CUD, CUF, CUB, CUP, HPA, VPA, CR, LF, BS, HT
- Erase operations: ED (erase display), EL (erase line)
- Line operations: IL (insert lines), DL (delete lines), ICH (insert chars), DCH (delete chars)
- SGR (Select Graphic Rendition): colors (16, 256, truecolor), bold, italic, underline, inverse, strikethrough
- Terminal mode set/reset: DECSET/DECRST for key modes (bracketed paste, application cursor, mouse, origin, wraparound)
- `Terminal::process(&mut self, bytes: &[u8])` — feeds bytes into vte parser

### Explicitly Out of Scope
- OSC sequences (Task L1_04)
- Scrollback buffer integration (Task L1_03)
- Mouse reporting output (Task L1_09)
- DCS sequences (Sixel, tmux control mode) — post-MVP

## Implementation Details

### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-core/src/terminal.rs` | Terminal struct, process() entry point |
| Create | `wmux-core/src/vte_handler.rs` | vte::Perform implementation |
| Modify | `wmux-core/src/lib.rs` | Export terminal and vte_handler modules |
| Modify | `wmux-core/Cargo.toml` | Add `vte = "0.13"` dependency |

### Key Decisions
- **Single entry point** (`.claude/rules/terminal-vte.md`): `terminal.process(bytes)` is the only way to feed data. Internally creates a vte::Parser and dispatches to VteHandler
- **VteHandler borrows Terminal mutably**: The Perform impl receives `&mut self` which is the VteHandler holding a `&mut Terminal`. This allows the handler to mutate grid, cursor, and modes
- **SGR parsing**: Support SGR 0 (reset), 1 (bold), 3 (italic), 4 (underline), 7 (inverse), 9 (strikethrough), 22-29 (resets), 30-37/40-47 (standard colors), 38/48;5;N (256-color), 38/48;2;R;G;B (truecolor), 90-97/L3_01-L3_08 (bright colors)

### Patterns to Follow
- `.claude/rules/terminal-vte.md`: Malformed sequences silently discarded — NEVER panic
- Architecture §5: "State machine (terminal modes), Observer (dirty row flags for renderer)"
- vte 0.13 API: `Perform` trait with `print()`, `execute()`, `csi_dispatch()`, `esc_dispatch()`, `osc_dispatch()`, `hook()`, `put()`, `unhook()`

### Technical Notes
- `print()` handles regular character output. Must handle wide characters (mark next cell as WIDE_SPACER)
- `execute()` handles C0 controls: BEL(0x07), BS(0x08), HT(0x09), LF(0x0A), CR(0x0D)
- `csi_dispatch()` handles most operations: cursor movement, erase, scroll, SGR, mode set/reset
- For DECSET/DECRST: mode 1 (app cursor keys), mode 25 (cursor visible), mode 47/1047/1049 (alt screen — placeholder only, Task L1_03), mode 1000/1002/1003 (mouse), mode 2004 (bracketed paste)
- Keep vte::Parser as owned field on Terminal (reused across process() calls to maintain state)

## Success Criteria

- [ ] Terminal::process() correctly updates grid cells for printed characters
- [ ] All standard cursor movement sequences position cursor correctly
- [ ] SGR sequences correctly set cell foreground, background, and attribute flags
- [ ] ED/EL erase operations clear correct regions
- [ ] IL/DL/ICH/DCH correctly insert and delete lines/characters
- [ ] Terminal modes (bracketed paste, app cursor, mouse) toggle correctly
- [ ] Wide characters correctly span two cells
- [ ] Malformed sequences do not panic or corrupt state
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
1. Unit test: feed `"\x1b[31mhello\x1b[0m"` → verify "hello" in red, followed by reset
2. Unit test: feed cursor movement sequences → verify cursor position
3. Unit test: feed `"\x1b[2J"` (clear screen) → verify all cells cleared
4. Unit test: feed SGR 38;2;255;128;0 (truecolor) → verify Rgb color set

### Edge Cases to Test
- Cursor movement beyond grid bounds (should clamp, not panic)
- SGR with missing parameters (should use defaults)
- Print at last column with wraparound mode on/off
- Very long CSI parameter list (should not crash)
- Alternating character sets (should not break grid)

## Dependencies

**Blocks**:
- Task L1_04: OSC Sequence Handlers
- Task L1_10: Single-Pane Terminal Integration

## References
- **PRD**: §1 Terminal GPU-Acceleré (ANSI/VT100/VT220/xterm-256color)
- **Architecture**: §5 wmux-core, §6 Data Flow — Terminal I/O
- **ADR**: ADR-0001 (Rust), referenced `.claude/rules/terminal-vte.md`
