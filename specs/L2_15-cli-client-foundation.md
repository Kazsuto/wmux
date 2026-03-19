# Task L2_15: Build CLI Client Foundation (wmux.exe)

> **Phase**: Core
> **Priority**: P0-Critical
> **Estimated effort**: 2 hours

## Context
The wmux CLI (`wmux.exe`) is how AI agents and users interact with wmux programmatically. It connects to the Named Pipe, sends JSON-RPC requests, and displays results. Architecture §5 (wmux-cli) specifies clap 4 with derive macros. PRD §3 defines global CLI options.

## Prerequisites
- [ ] Task L2_09: Named Pipes Server — provides the pipe to connect to

## Scope
### Deliverables
- clap 4 CLI structure with global options (--pipe, --json, --workspace, --surface, --window)
- Named Pipe client: connect, send JSON-RPC request, receive response, disconnect
- Output formatting: human-readable (default) and JSON (--json flag)
- Pipe discovery: WMUX_SOCKET_PATH → default pipe name
- Error handling and exit codes (0 = success, 1 = error)
- Subcommand stubs for all command groups (workspace, surface, sidebar, notify, browser, system)

### Explicitly Out of Scope
- Individual subcommand implementations (Task L2_16)
- Shell completions (post-MVP)
- Interactive mode (all commands are one-shot)

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Modify | `wmux-cli/src/main.rs` | clap App definition, global options, dispatch |
| Create | `wmux-cli/src/client.rs` | Named Pipe client (connect, send, receive) |
| Create | `wmux-cli/src/output.rs` | Output formatting (human, JSON) |
| Create | `wmux-cli/src/commands/mod.rs` | Subcommand module stubs |
| Modify | `wmux-cli/Cargo.toml` | Add clap, tokio, serde_json, anyhow deps |

### Key Decisions
- **One-shot connections** (Architecture §5 wmux-cli): Connect → send → receive → disconnect. No persistent connection. Simple and stateless
- **Pipe discovery** (`.claude/rules/ipc-protocol.md`): Check `WMUX_SOCKET_PATH` first, fallback to `\\.\pipe\wmux`
- **clap 4 derive** (Architecture §3): Use `#[derive(Parser)]` for zero-boilerplate subcommands

### Patterns to Follow
- Architecture §5 wmux-cli: clap 4, one-shot connections
- `.claude/rules/ipc-protocol.md`: "Exit code 0 success, 1 error", "--json flag for machine-readable output"
- Architecture §3: clap 4 derive macros

### Technical Notes
- Global options struct: `{ pipe: Option<String>, json: bool, workspace: Option<String>, surface: Option<String>, window: Option<String> }`
- Pipe client: `tokio::net::windows::named_pipe::ClientOptions::new().open(pipe_name)?`
- Write request as JSON + newline, read response until newline
- Timeout: 30 seconds for connection + response. `--timeout` flag for custom
- Human output: pretty-print key fields. E.g., workspace.list → table of workspace names
- JSON output: print raw JSON response
- Exit code 0 if `ok: true`, exit code 1 if `ok: false` or connection error

## Success Criteria
- [ ] `wmux --help` shows all subcommands and global options
- [ ] `wmux system ping` connects to pipe and returns pong
- [ ] `--json` flag outputs raw JSON response
- [ ] `--pipe` flag overrides pipe path
- [ ] Exit code 0 on success, 1 on error
- [ ] Connection timeout after 30 seconds
- [ ] Pipe discovery uses WMUX_SOCKET_PATH when set
- [ ] `cargo clippy --workspace` zero warnings

## Validation Steps
### Automated Checks
```bash
cargo build --workspace
cargo clippy --workspace -- -W clippy::all
cargo test -p wmux-cli
cargo fmt --all -- --check
```
### Manual Verification
1. Start wmux-app, then `cargo run -p wmux-cli -- system ping` → verify "pong"
2. `cargo run -p wmux-cli -- --json system ping` → verify JSON output
3. `cargo run -p wmux-cli -- --pipe \\.\pipe\nonexistent system ping` → verify error + exit code 1
### Edge Cases to Test
- wmux-app not running (connection refused → clear error message)
- Invalid pipe path (should show error, not panic)
- Response timeout (should show timeout error after 30s)
- Ctrl+C during command (should exit cleanly)

## Dependencies
**Blocks**:
- Task L2_16: CLI Domain Commands

## References
- **PRD**: §3 CLI & API IPC (CLI options, commands)
- **Architecture**: §5 wmux-cli (clap, one-shot connections)
- **ADR**: ADR-0005 (Named Pipes client)
