# ADR-0004: PTY Backend — portable-pty 0.9 (ConPTY)

> **Status**: Accepted
> **Date**: 2026-03-19
> **Confidence**: High
> **Deciders**: wmux team

## Context

wmux needs to spawn shell processes (PowerShell, cmd, bash, WSL) with full pseudo-terminal support on Windows. Windows uses ConPTY (Console Pseudo Terminal, available since Windows 10 1809) instead of Unix `openpty`/`posix_spawn`. The PTY abstraction must handle process lifecycle, I/O pipes, resize, and environment variable injection.

## Decision Drivers

- Must support ConPTY on Windows 10 1809+ (our minimum platform target)
- Must handle PowerShell, cmd.exe, Git Bash, and WSL shell spawning
- Must integrate with tokio (async reads from PTY output — ConPTY reads are blocking)
- Battle-tested in a production Windows terminal
- Active maintenance for ConPTY edge cases (resize races, output buffering)

## Decision

**portable-pty 0.9** (from the WezTerm project). Wraps ConPTY in a safe Rust API. Blocking reads are moved to `tokio::task::spawn_blocking` for async integration.

## Alternatives Considered

### Raw ConPTY via `windows` crate
- **Pros**: No external dependency. Full control over ConPTY API. Can optimize for wmux-specific needs
- **Cons**: ConPTY has many edge cases: resize race conditions, output pipe buffering quirks, process exit detection timing. WezTerm has 5+ years of fixes for these. Reimplementing would repeat those bugs
- **Why rejected**: ConPTY is deceptively complex. The `windows` rules say "spawn shells via portable-pty, NEVER raw ConPTY API directly." WezTerm's battle-tested abstractions save months of debugging

### xpty 0.3.6
- **Pros**: Fork of portable-pty 0.9 with native tokio async support (AsyncRead/AsyncWrite traits). Modern error types. Actively developed (March 2026)
- **Cons**: Very new (0.3.x). Low download count. Not yet proven in production. API may change
- **Why rejected**: Too immature for a production dependency. Watching for v1.0. If portable-pty's `spawn_blocking` pattern becomes a bottleneck, xpty is the upgrade path

### winpty-rs 1.0.5
- **Pros**: Supports both WinPTY (legacy) and ConPTY. Windows-specific
- **Cons**: WinPTY is legacy (Windows 7/8 compatibility). Low usage. Not part of a larger terminal project. Doesn't handle WSL shell spawning
- **Why rejected**: WinPTY is unnecessary since our minimum target is Windows 10 1809 (ConPTY available). portable-pty is better maintained and more widely used

## Consequences

### Positive
- Production-proven ConPTY handling from WezTerm (5+ years of edge case fixes)
- Clean Rust API: `PtyPair`, `MasterPty`, `Child` with standard Read/Write traits
- Handles shell detection, environment injection, and resize coordination
- 906K downloads/month — large user base catches regressions quickly

### Negative (acknowledged trade-offs)
- Blocking reads require `tokio::task::spawn_blocking`, adding thread pool overhead for PTY I/O
- Tied to WezTerm's maintenance pace — if WezTerm development slows, updates may lag
- No native async — must bridge between blocking PTY I/O and async event loop manually

### Mandatory impact dimensions
- **Security**: portable-pty spawns processes with the user's privileges. Environment variables (WMUX_SOCKET_PATH, etc.) are injected — ensure no sensitive data leaks via env
- **Cost**: $0. MIT licensed
- **Latency**: ConPTY adds ~1-2ms latency vs raw process I/O. Blocking read on dedicated thread adds thread wake-up latency (~0.1ms). Negligible for terminal use

## Revisit Triggers

- If xpty reaches v1.0 with stable async API and production usage, migrate to eliminate `spawn_blocking` overhead
- If portable-pty has no release for 6+ months and ConPTY bugs emerge, consider forking or switching to xpty
- If PTY read latency exceeds 5ms (measurable delay in fast output like `cat` on large files), investigate dedicated I/O thread instead of tokio thread pool
