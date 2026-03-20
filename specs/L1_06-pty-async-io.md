---
task_id: L1_06
title: "Implement PTY Async I/O with Tokio"
status: done
priority: P0
estimated_hours: 1.5
wave: 2
prd_features: [F-01]
archi_sections: [ADR-0001, ADR-0004, ADR-0008]
depends_on: [L1_05]
blocks: [L1_08, L1_09, L1_10, L3_13]
---

# Task L1_06: Implement PTY Async I/O with Tokio

> **Phase**: Foundation
> **Priority**: P0-Critical
> **Estimated effort**: 1.5 hours
> **Wave**: 2

## Context

PTY reads are blocking operations (portable-pty uses std::io::Read). These must be bridged to the async tokio runtime without blocking the event loop. Architecture §5 (wmux-pty) specifies `tokio::task::spawn_blocking` for PTY reads. ADR-0008 mandates the actor pattern with bounded channels. The `.claude/rules/rust-architecture.md` rule says NEVER block the tokio runtime.

## Prerequisites

- [ ] Task L1_05: ConPTY Shell Spawning — provides PtyHandle with reader/writer

## Scope

### Deliverables
- `PtyActor` struct — tokio task that manages I/O for one PTY instance
- Async PTY reader: `spawn_blocking` loop → bounded channel for output bytes
- Async PTY writer: receive bytes from channel → write to PTY
- Resize channel: send new dimensions to PTY actor
- Process exit detection: watch child process, emit exit event
- Graceful shutdown: drop channels → actor exits

### Explicitly Out of Scope
- Terminal parsing of output bytes (Task L1_02)
- Multiple PTY management (Task L2_01 handles pane registry)
- Shell integration hooks (Task L3_13)

## Implementation Details

### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-pty/src/actor.rs` | PtyActor with async read/write/resize |
| Modify | `wmux-pty/src/lib.rs` | Export actor module |
| Modify | `wmux-pty/Cargo.toml` | Ensure tokio features include "rt", "sync", "process" |

### Key Decisions
- **spawn_blocking for reads** (ADR-0004, `.claude/rules/rust-architecture.md`): PTY reader is blocking I/O. Wrap in `tokio::task::spawn_blocking`. Read into a fixed buffer (4096 bytes), send buffer via bounded channel
- **Bounded channels** (ADR-0008): Output channel bounded at 256 messages. Input (write) channel bounded at 256. Resize channel bounded at 4 (only latest resize matters)
- **Actor pattern**: PtyActor owns the PTY handle. External code communicates only via channels. No `Arc<Mutex>` on PTY handles

### Patterns to Follow
- ADR-0008: "Actor pattern via bounded tokio channels — NO Arc<Mutex<T>>"
- `.claude/rules/rust-architecture.md`: "NEVER block tokio runtime with std::thread::sleep, std::fs::*, or CPU-heavy work"
- Read buffer: 4096 bytes per read (matches common pipe buffer sizes)

### Technical Notes
- Read loop: `spawn_blocking(move || { loop { let n = reader.read(&mut buf)?; if n == 0 { break; } tx.blocking_send(buf[..n].to_vec())?; } })`
- Write: `while let Some(data) = rx.recv().await { writer.write_all(&data)?; }`
- Resize: `master.resize(PtySize { rows, cols, .. })` — call from the actor task
- Process exit: portable-pty `Child::wait()` returns ExitStatus. Emit `PtyEvent::Exited(ExitStatus)`
- On channel close (sender dropped), the actor should clean up and exit gracefully
- `PtyEvent` enum: `Output(Vec<u8>)`, `Exited(ExitStatus)`

## Success Criteria

- [ ] PtyActor reads PTY output asynchronously without blocking tokio runtime
- [ ] Output bytes are delivered via bounded channel
- [ ] Write channel correctly sends input to PTY
- [ ] Resize correctly updates PTY dimensions
- [ ] Process exit is detected and reported
- [ ] Actor shuts down cleanly when channels are dropped
- [ ] No data loss under normal operation (channel backpressure instead)

## Validation Steps

### Automated Checks
```bash
cargo build --workspace
cargo clippy --workspace -- -W clippy::all
cargo test -p wmux-pty
cargo fmt --all -- --check
```

### Manual Verification
1. Integration test (`#[ignore]`): spawn PTY actor, send "echo hello\r\n", verify "hello" in output channel
2. Test resize: spawn actor, resize to 40x10, verify no panic
3. Test shutdown: drop write channel, verify actor exits cleanly

### Edge Cases to Test
- PTY process exits unexpectedly (actor should emit Exited event, not panic)
- Output channel full (spawn_blocking should block on channel send — bounded backpressure)
- Rapid consecutive resizes (only latest should matter)
- Empty read (0 bytes) — should be treated as EOF

## Dependencies

**Blocks**:
- Task L1_08: Keyboard Input → PTY Dispatch
- Task L1_09: Mouse Selection, Copy/Paste, Scroll
- Task L1_10: Single-Pane Terminal Integration
- Task L3_13: Shell Integration Hooks

## References
- **PRD**: §1 Terminal GPU-Acceleré (ConPTY I/O)
- **Architecture**: §5 wmux-pty (spawn_blocking pattern), §6 Data Flow — Terminal I/O
- **ADR**: ADR-0004 (portable-pty), ADR-0008 (actor pattern, bounded channels)
