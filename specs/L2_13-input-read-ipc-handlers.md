# Task L2_13: Implement Input and Read IPC Handlers

> **Phase**: Core
> **Priority**: P0-Critical
> **Estimated effort**: 2 hours

## Context
AI agents need to send input to terminals and read their output programmatically. surface.send_text writes to the PTY, surface.send_key simulates key presses, and surface.read_text captures terminal content (like tmux's capture-pane). PRD §3 and §6 describe these capabilities.

## Prerequisites
- [ ] Task L2_11: IPC Handler Trait + Router — provides Handler dispatch

## Scope
### Deliverables
- `InputHandler` (IPC): surface.send_text, surface.send_key
- surface.send_text: write raw bytes to target surface's PTY
- surface.send_key: translate key name (e.g., "Enter", "Ctrl+C") to VT sequence → PTY
- surface.read_text: capture visible grid and/or scrollback as text string
- Target surface resolution: by surface_id, or current if omitted

### Explicitly Out of Scope
- Terminal grid implementation (already done in Task L1_01-L1_03)
- Streaming/subscription mode (post-MVP)
- pipe-pane (post-MVP per PRD)

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-ipc/src/handlers/input.rs` | Input and read handlers |
| Modify | `wmux-ipc/src/handlers/mod.rs` | Register input handler |
| Modify | `wmux-ipc/src/router.rs` | Register "surface" handler for send_text, send_key, read_text |

### Key Decisions
- **send_text is raw bytes** (`.claude/rules/ipc-protocol.md`): "surface.send_text transmits raw bytes — responsibility is on the caller." No escaping, no interpretation
- **read_text returns plain text**: Concatenate cell graphemes from grid + scrollback. Rows joined by newlines. Trailing whitespace trimmed per row
- **send_key maps names**: "Enter" → "\r", "Tab" → "\t", "Ctrl+C" → "\x03", "Up" → "\x1b[A"

### Patterns to Follow
- `.claude/rules/ipc-protocol.md`: Raw byte transmission for send_text
- Architecture §6 Data Flow: IPC → Multiplexer → PTY

### Technical Notes
- surface.send_text params: `{ "surface_id": "optional", "text": "ls -la\n" }` — text is sent as UTF-8 bytes to PTY
- surface.send_key params: `{ "surface_id": "optional", "key": "Enter" }` — key name translated to VT bytes
- surface.read_text params: `{ "surface_id": "optional", "start": -L3_01, "end": null }` — start is relative line offset (negative = from end), end defaults to current line
- Key name mapping: maintain a lookup table of common key names to VT sequences
- read_text must handle the viewport offset (scrollback) and alternate screen scenarios
- For read_text, request grid content from AppState actor (which owns terminal state)

## Success Criteria
- [ ] surface.send_text correctly writes text to target PTY
- [ ] surface.send_key correctly translates key names and sends to PTY
- [ ] surface.read_text returns accurate terminal content
- [ ] read_text supports start/end range parameters
- [ ] Target resolution works (by surface_id or current)
- [ ] Invalid surface_id returns appropriate error
- [ ] `cargo clippy --workspace` zero warnings

## Validation Steps
### Automated Checks
```bash
cargo build --workspace
cargo clippy --workspace -- -W clippy::all
cargo test -p wmux-ipc
cargo fmt --all -- --check
```
### Manual Verification
1. Send `surface.send_text` with "echo hello\n" → verify command executes in terminal
2. Send `surface.send_key` with "Enter" → verify newline in terminal
3. Send `surface.read_text` → verify returned text matches visible terminal content
### Edge Cases to Test
- send_text with empty string (should be no-op)
- send_key with unknown key name (should return error)
- read_text with start beyond scrollback (return available portion)
- read_text on alternate screen (return alt screen content, not main)
- send_text to browser surface (should return error — not a terminal)

## Dependencies
**Blocks**:
- Task L2_16: CLI Domain Commands (CLI wraps these methods)

## References
- **PRD**: §3 CLI & API IPC (surface.send_text, send_key), §6 Lecture du Terminal (read_text)
- **Architecture**: §5 wmux-ipc, §6 Data Flow — IPC Command
