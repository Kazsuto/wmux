# Task L1_05: Implement ConPTY Shell Spawning via portable-pty

> **Phase**: Foundation
> **Priority**: P0-Critical
> **Estimated effort**: 2 hours

## Context

Every terminal pane needs a shell process (PowerShell, cmd, bash) running in a pseudo-terminal. ConPTY is the Windows PTY API (available since Windows 10 1809). Architecture §5 (wmux-pty) specifies portable-pty 0.9 from the WezTerm project for ConPTY abstraction. ADR-0004 mandates NEVER using raw ConPTY API directly.

PRD §1 requires support for PowerShell 5/7, cmd.exe, bash (Git Bash/MSYS2), and WSL shells.

## Prerequisites

- [ ] Task L0_01: Error Types and Tracing Infrastructure — provides PtyError enum

## Scope

### Deliverables
- `PtyManager` struct in `wmux-pty/src/manager.rs` — manages PTY lifecycle
- Shell detection: `detect_shell()` → prioritized search (pwsh → powershell → cmd)
- `spawn(config)` method: create ConPTY, spawn shell, return handles
- Environment variable injection (WMUX_SOCKET_PATH, WMUX_WORKSPACE_ID, WMUX_SURFACE_ID, WMUX_WINDOW_ID, TERM, TERM_PROGRAM)
- PTY resize (cols, rows)
- Working directory setting
- `PtyHandle` struct with reader/writer/child handles

### Explicitly Out of Scope
- Async I/O integration (Task L1_06)
- WSL shell detection (deferred per architecture audit — too complex for v1)
- Shell integration hook injection (Task L3_13)

## Implementation Details

### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-pty/src/manager.rs` | PtyManager struct, spawn, detect_shell |
| Create | `wmux-pty/src/shell.rs` | Shell detection logic |
| Modify | `wmux-pty/src/lib.rs` | Export manager and shell modules |
| Modify | `wmux-pty/Cargo.toml` | Add portable-pty, tokio, dirs dependencies |

### Key Decisions
- **portable-pty 0.9** (ADR-0004): Battle-tested ConPTY wrapper. Handles Win10 1809+ quirks
- **Shell detection chain** (`.claude/rules/windows-platform.md`): `pwsh` (PowerShell 7) → `powershell` (PowerShell 5) → `cmd.exe`. Check `PATH` for each
- **Environment injection**: Set TERM=xterm-256color, TERM_PROGRAM=wmux, and WMUX_* variables in the spawned process environment

### Patterns to Follow
- ADR-0004: "NEVER raw ConPTY API directly"
- `.claude/rules/windows-platform.md`: "spawn shells via portable-pty, NEVER raw ConPTY"
- `.claude/rules/rust-architecture.md`: thiserror for PtyError variants

### Technical Notes
- portable-pty 0.9 API: `PtySystem::default().openpty(size)` returns `(MasterPty, SlavePty)`. Then `slave.spawn_command(cmd)` spawns the process
- `CommandBuilder` from portable-pty sets program, args, env, cwd
- MasterPty provides `.try_clone_reader()` and `.take_writer()` for I/O
- PtySize: cols/rows as u16, pixel_width/pixel_height as u16
- Environment variables must be set before spawn — cannot modify after
- Shell detection should use `which` equivalent (check PATH) or `where.exe` on Windows
- Default working directory: user's home directory (`dirs::home_dir()`)

## Success Criteria

- [ ] PtyManager spawns PowerShell (or detected shell) in a ConPTY
- [ ] Shell detection correctly finds the highest-priority available shell
- [ ] Environment variables (TERM, TERM_PROGRAM, WMUX_*) are set in spawned process
- [ ] PTY resize correctly updates terminal dimensions
- [ ] Working directory is correctly set for spawned shell
- [ ] PtyHandle provides reader and writer handles
- [ ] PtyError covers spawn failures, resize failures, I/O errors
- [ ] `cargo test -p wmux-pty` passes (integration test with `#[ignore]` for CI)

## Validation Steps

### Automated Checks
```bash
cargo build --workspace
cargo clippy --workspace -- -W clippy::all
cargo test -p wmux-pty
cargo fmt --all -- --check
```

### Manual Verification
1. Write an integration test (mark `#[ignore]`): spawn shell, write `echo %TERM_PROGRAM%`, read output, verify "wmux"
2. Test shell detection returns "pwsh" if PowerShell 7 is installed, "powershell" otherwise
3. Test PTY resize does not panic

### Edge Cases to Test
- No shell found in PATH (should fall back to cmd.exe, which always exists)
- Very small terminal size (1x1) — should not panic
- Working directory that doesn't exist (should fall back to home dir or fail gracefully)
- Spawn with empty environment (should inherit parent + add WMUX vars)

## Dependencies

**Blocks**:
- Task L1_06: PTY Async I/O Integration

## References
- **PRD**: §1 Terminal GPU-Acceleré (ConPTY, shell support)
- **Architecture**: §5 wmux-pty (PtyManager, shell detection)
- **ADR**: ADR-0004 (portable-pty 0.9, NEVER raw ConPTY)
