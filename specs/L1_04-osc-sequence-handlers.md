---
task_id: L1_04
title: "Implement OSC Sequence Handlers and Terminal Event Bus"
status: done
priority: P1
estimated_hours: 1.5
wave: 4
prd_features: [F-01, F-07, F-13]
archi_sections: [ADR-0001]
depends_on: [L1_02]
blocks: [L1_10, L3_08, L3_14]
---

# Task L1_04: Implement OSC Sequence Handlers and Terminal Event Bus

> **Phase**: Foundation
> **Priority**: P1-High
> **Estimated effort**: 1.5 hours
> **Wave**: 4

## Context

OSC (Operating System Command) sequences carry metadata from shell processes to the terminal: current working directory (OSC 7), notifications (OSC 9/99/777), shell prompt marks (OSC 133), and hyperlinks (OSC 8). These are critical for sidebar metadata (git branch detection relies on CWD changes) and the notification system. The `.claude/rules/terminal-vte.md` mandates handling all listed OSC sequences. The `.claude/rules/notifications.md` requires OSC 9/99/777 forwarding.

## Prerequisites

- [ ] Task L1_02: VTE Parser Integration — provides vte::Perform dispatch where osc_dispatch() is called

## Scope

### Deliverables
- OSC handler implementations in VteHandler's `osc_dispatch()` method
- `TerminalEvent` enum for events propagated from terminal to application
- Event channel (`mpsc::Sender<TerminalEvent>`) on Terminal for outbound events
- OSC 7: Parse file URI → extract CWD path → emit `TerminalEvent::CwdChanged(PathBuf)`
- OSC 9: Parse body → emit `TerminalEvent::Notification { title, body }`
- OSC 99: Parse kitty notification (id, title, body) → emit `TerminalEvent::Notification`
- OSC 777: Parse rxvt notification → emit `TerminalEvent::Notification`
- OSC 133: Parse prompt marks (A=prompt start, B=command start, C=output start, D=command end) → emit `TerminalEvent::PromptMark`
- OSC 8: Parse hyperlink → store on cell attributes (id, uri)

### Explicitly Out of Scope
- Rendering hyperlinks (visual underline + click handler — post-MVP)
- Acting on notifications (Task L3_08)
- Git detection from CWD (Task L3_14)
- Shell integration scripts that emit these sequences (Task L3_13)

## Implementation Details

### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-core/src/event.rs` | TerminalEvent enum |
| Modify | `wmux-core/src/terminal.rs` | Add event sender, forward OSC events |
| Modify | `wmux-core/src/vte_handler.rs` | Implement osc_dispatch() cases |
| Modify | `wmux-core/src/cell.rs` | Add hyperlink field (Option<Hyperlink>) to Cell |
| Modify | `wmux-core/src/lib.rs` | Export event module |

### Key Decisions
- **Tokio mpsc channel for events**: Terminal produces events, application consumes them. Bounded channel (256 capacity) for backpressure. Terminal never blocks on send (use `try_send`, drop if full)
- **OSC 7 URI parsing**: Format `file://hostname/path`. Extract path, convert to PathBuf. On Windows, handle `/C:/` → `C:\` conversion
- **Hyperlink stored per-cell**: OSC 8 sets/clears a hyperlink on subsequently printed cells. Store as `Option<Arc<Hyperlink>>` (shared across cells in same link)

### Patterns to Follow
- `.claude/rules/terminal-vte.md`: "ALWAYS handle OSC 7, OSC 9/99/777, OSC 133, OSC 8"
- `.claude/rules/notifications.md`: "Forward to NotificationStore — NEVER drop silently"
- ADR-0008: Bounded channels for inter-component communication

### Technical Notes
- vte 0.13 `osc_dispatch()` receives `&[&[u8]]` — params split by `;`. First param is the OSC number
- OSC 7 format: `\x1b]7;file://host/path\x07` — only first two params
- OSC 9 format: `\x1b]9;body\x07` — simple body text
- OSC 99 format: `\x1b]99;i=id:d=0;title\x07` or with body via `\x1b]99;i=id:d=1;body\x07`
- OSC 133 format: `\x1b]133;A\x07` (prompt start), `\x1b]133;B\x07` (command start), etc.
- OSC 8 format: `\x1b]8;params;uri\x07` — empty URI closes the hyperlink

## Success Criteria

- [ ] OSC 7 correctly parses file URI and emits CwdChanged event with correct Windows path
- [ ] OSC 9 emits Notification event with body text
- [ ] OSC 99 parses kitty notification format including id and title
- [ ] OSC 777 parses rxvt notification format
- [ ] OSC 133 emits PromptMark events with correct mark type
- [ ] OSC 8 sets and clears hyperlink on cells
- [ ] Unknown OSC sequences are silently ignored (no panic, no error log)
- [ ] Event channel handles backpressure without blocking terminal processing

## Validation Steps

### Automated Checks
```bash
cargo build --workspace
cargo clippy --workspace -- -W clippy::all
cargo test -p wmux-core
cargo fmt --all -- --check
```

### Manual Verification
1. Unit test: feed OSC 7 sequence → verify CwdChanged event with correct path
2. Unit test: feed OSC 9 body → verify Notification event
3. Unit test: feed OSC 133;A and 133;B → verify PromptMark events

### Edge Cases to Test
- OSC 7 with Windows path containing spaces
- OSC 7 with empty path (should ignore or emit with empty path)
- OSC 99 with malformed parameters (missing id) — should not panic
- OSC 8 with empty URI (close hyperlink)
- Unknown OSC number (e.g., OSC 52 for clipboard) — silently ignore
- Event channel full (try_send fails) — log warning, don't block

## Dependencies

**Blocks**:
- Task L1_10: Single-Pane Terminal Integration (terminal event consumption)
- Task L3_08: Notification Store + OSC Detection
- Task L3_14: Git Branch Detection (relies on CWD events)

## References
- **PRD**: §7 Notifications (OSC sources), §13 Shell Integration (OSC 7/133)
- **Architecture**: §5 wmux-core (terminal event bus), §6 Data Flow
- **ADR**: ADR-0008 (bounded channels for events)
