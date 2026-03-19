---
paths:
  - "wmux-core/**/*.rs"
---
# Terminal & VTE Rules — wmux

## VTE Parsing (CRITICAL)
- Use the `vte` crate (Alacritty's parser). NEVER write a custom VTE parser.
- **ALWAYS** handle OSC 7 (current working directory), OSC 9/99/777 (notifications), OSC 8 (hyperlinks), OSC 133 (prompt marks) — these are used by shell integration and AI agents.
- Malformed escape sequences must be silently discarded — NEVER panic or crash on bad input.
- `terminal.process(bytes)` is the single entry point for all PTY output. All state changes flow through the VTE `Perform` trait implementation.

## Grid & Scrollback
- Grid cells stored contiguously (Vec<Cell>), NOT Vec<Vec<Cell>>. One flat allocation per row.
- Scrollback is a ring buffer with configurable max (default 4000 lines). Use VecDeque, not Vec.
- Dirty tracking per row — only upload changed rows to GPU.
- Alternate screen buffer (smcup/rmcup) does NOT share scrollback with the main buffer.

## Cursor & Modes
- Track terminal modes: origin, wraparound, bracketed paste, application cursor keys, mouse reporting.
- Mode state is per-terminal, not global. Each pane has independent mode state.
