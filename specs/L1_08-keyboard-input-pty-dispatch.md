# Task L1_08: Implement Keyboard Input → PTY Dispatch

> **Phase**: Foundation
> **Priority**: P0-Critical
> **Estimated effort**: 2 hours

## Context

Keyboard input must be translated from winit key events to VT byte sequences and written to the PTY. Different terminal modes (application cursor keys, bracketed paste) change the byte sequences sent. This is the "input" half of the terminal I/O loop. Architecture §6 Data Flow shows User → Event Loop → PTY Manager → ConPTY path.

PRD §1 requires support for all standard keyboard input including modifiers.

## Prerequisites

- [ ] Task L1_06: PTY Async I/O — provides write channel to send bytes to PTY

## Scope

### Deliverables
- `InputHandler` in `wmux-ui/src/input.rs` — translates winit KeyEvent to VT bytes
- Regular character input: UTF-8 encode and send
- Modifier handling: Ctrl+letter (0x01-0x1A), Alt+key (ESC prefix), Shift
- Special keys: Arrow, Home, End, PageUp/Down, Insert, Delete, F1-F12, Tab, Enter, Backspace, Escape
- Application cursor mode: arrows send `\x1bOA` instead of `\x1b[A`
- Bracketed paste mode: paste wrapped in `\x1b[200~` ... `\x1b[201~`
- NumPad key handling

### Explicitly Out of Scope
- Global keyboard shortcut interception (Task L2_03)
- Mouse input handling (Task L1_09)
- IME/CJK input composition (post-MVP per Architecture §14)
- Custom keybinding configuration (Task L3_11)

## Implementation Details

### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-ui/src/input.rs` | InputHandler, key→VT byte mapping |
| Modify | `wmux-ui/src/lib.rs` | Export input module |
| Modify | `wmux-ui/Cargo.toml` | Ensure winit dep is available |

### Key Decisions
- **Mode-aware dispatch**: InputHandler receives current TerminalMode to decide byte sequences. Application cursor mode and bracketed paste mode change output significantly
- **winit 0.30 keyboard API**: Use `KeyEvent` with `logical_key` (for characters) and `physical_key` (for special keys). Handle `ElementState::Pressed` only (not Released for terminal input)
- **Ctrl+C = 0x03**: Standard terminal SIGINT. Ctrl+letter maps to 0x01 + (letter - 'a')

### Patterns to Follow
- `.claude/rules/terminal-vte.md`: Terminal modes affect key sequences
- Architecture §6 Data Flow: "User → Event Loop → PTY Manager"
- winit 0.30 text/IME handling per `.claude/rules/rendering.md`

### Technical Notes
- Arrow keys: Normal mode `\x1b[A/B/C/D`, Application mode `\x1bOA/B/C/D`
- F1-F4: `\x1bOP`, `\x1bOQ`, `\x1bOR`, `\x1bOS`
- F5-F12: `\x1b[15~`, `\x1b[17~`, `\x1b[18~`, `\x1b[19~`, `\x1b[20~`, `\x1b[21~`, `\x1b[23~`, `\x1b[24~`
- Home/End: `\x1b[H`, `\x1b[F` (or `\x1b[1~`/`\x1b[4~` in some modes)
- Alt+key: send `\x1b` followed by the key byte
- Ctrl+Shift+C/V are intercepted for copy/paste (Task L1_09), NOT sent to PTY
- Enter: `\r` (CR). In some modes `\r\n`
- Backspace: `\x7f` (DEL) or `\x08` (BS) depending on terminal config
- Tab: `\x09`

## Success Criteria

- [ ] Regular character input (a-z, numbers, symbols) sends correct UTF-8 bytes to PTY
- [ ] Ctrl+letter sends correct control codes (0x01-0x1A)
- [ ] Alt+key sends ESC prefix followed by key
- [ ] Arrow keys respect application cursor mode toggle
- [ ] Special keys (F1-F12, Home, End, etc.) send correct VT sequences
- [ ] Bracketed paste wraps pasted text correctly
- [ ] No input is sent for key release events
- [ ] `cargo clippy --workspace` zero warnings

## Validation Steps

### Automated Checks
```bash
cargo build --workspace
cargo clippy --workspace -- -W clippy::all
cargo test -p wmux-ui
cargo fmt --all -- --check
```

### Manual Verification
1. Unit test: map 'a' key → verify `b"a"` output
2. Unit test: Ctrl+C → verify `[0x03]` output
3. Unit test: Arrow Up in app mode → verify `\x1bOA`, normal mode → `\x1b[A`
4. Unit test: F5 → verify `\x1b[15~`

### Edge Cases to Test
- Key with no logical mapping (e.g., volume keys) — should be ignored
- Ctrl+@ → NUL byte (0x00)
- Ctrl+[ → ESC (0x1B)
- Unicode character input (emoji, accented chars) — send UTF-8 encoded
- Rapid key repeat (should handle all events)

## Dependencies

**Blocks**:
- Task L1_10: Single-Pane Terminal Integration

## References
- **PRD**: §1 Terminal GPU-Acceleré (keyboard input)
- **Architecture**: §6 Data Flow — Terminal I/O
- **ADR**: ADR-0007 (winit 0.30 keyboard events)
