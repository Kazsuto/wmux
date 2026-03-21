---
task_id: L2_09
title: "Implement Named Pipes Server and JSON-RPC v2 Protocol"
status: done
priority: P0
estimated_hours: 3
wave: 7
prd_features: [F-03]
archi_sections: [ADR-0001, ADR-0005, ADR-0008]
depends_on: [L2_01]
blocks: [L2_10, L2_11, L2_15, L4_03]
---

# Task L2_09: Implement Named Pipes Server and JSON-RPC v2 Protocol

> **Phase**: Core
> **Priority**: P0-Critical
> **Estimated effort**: 3 hours
> **Wave**: 7

## Context
The IPC server enables programmatic control of wmux by AI agents and the CLI. It uses Windows Named Pipes with JSON-RPC v2 protocol, matching cmux's protocol for compatibility. Architecture §5 (wmux-ipc) describes the server. ADR-0005 mandates Named Pipes (NEVER TCP). PRD §3 specifies the complete API.

## Prerequisites
- [ ] Task L2_01: AppState Actor — IPC commands route to AppState

## Scope
### Deliverables
- Named Pipe server (`\\.\pipe\wmux`) using tokio async Named Pipes
- JSON-RPC v2 codec: parse requests, format responses, error codes
- Request/Response types matching cmux protocol
- Connection lifecycle: accept → read → dispatch → write → close (one-shot)
- Newline-delimited framing (one JSON object per line)
- Pipe name: `\\.\pipe\wmux` (release), `\\.\pipe\wmux-debug` (debug), or `WMUX_SOCKET_PATH`
- RpcError enum matching cmux error codes

### Explicitly Out of Scope
- Authentication (Task L2_10)
- Handler implementations (Tasks L2_11-L2_14)
- CLI client (Task L2_15)

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-ipc/src/server.rs` | Named Pipe server, connection handling |
| Create | `wmux-ipc/src/protocol.rs` | JSON-RPC v2 codec, Request/Response types |
| Create | `wmux-ipc/src/router.rs` | Method dispatch stub (filled by Task L2_11) |
| Modify | `wmux-ipc/src/lib.rs` | Export modules, public API |
| Modify | `wmux-ipc/Cargo.toml` | Add tokio, serde_json, windows, tracing deps |
| Modify | `wmux-app/src/main.rs` | Spawn IPC server task |
| Modify | `wmux-app/Cargo.toml` | Add wmux-ipc dependency |

### Key Decisions
- **One-shot connections** (Architecture §5 wmux-cli): Connect → send request → receive response → disconnect. Simple, stateless, matches cmux
- **Newline-delimited JSON** (`.claude/rules/ipc-protocol.md`): Each message is a single JSON object terminated by `\n`. Read until newline, parse, dispatch
- **Pipe security**: Default DACL restricts to current user SID. Uses `windows` crate for pipe creation with security attributes
- **Response format** (`.claude/rules/ipc-protocol.md`): Success: `{"id":"...", "ok":true, "result":{...}}`. Error: `{"id":"...", "ok":false, "error":{"code":"...", "message":"..."}}`

### Patterns to Follow
- ADR-0005: Named Pipes, NEVER TCP
- `.claude/rules/ipc-protocol.md`: JSON-RPC v2 wire format, response structure
- `.claude/rules/windows-platform.md`: Named Pipes for IPC
- ADR-0008: IPC Server actor with bounded channel

### Technical Notes
- tokio Named Pipe: `tokio::net::windows::named_pipe::ServerOptions::new().create(pipe_name)?`
- Connection loop: `loop { server.connect().await?; tokio::spawn(handle_connection(server)); server = create_next_pipe(); }`
- Pipe name discovery: check `WMUX_SOCKET_PATH` env → fallback to `\\.\pipe\wmux` (release) or `\\.\pipe\wmux-debug` (debug build)
- If pipe name is taken (another wmux instance), fall back to `\\.\pipe\wmux-{pid}`
- Read timeout: 30 seconds per connection to prevent hung clients
- Request parsing: validate JSON, extract `id`, `method`, `params`. Missing `method` = parse error response
- cmux response format includes `"ok"` boolean field — NOT standard JSON-RPC v2 (which uses result/error). We match cmux

## Success Criteria
- [ ] Named Pipe server starts and accepts connections
- [ ] JSON-RPC requests are correctly parsed
- [ ] Responses are correctly formatted (ok:true/false with result/error)
- [ ] Newline-delimited framing works for request/response
- [ ] Pipe name respects WMUX_SOCKET_PATH environment variable
- [ ] Connection timeout prevents hung clients
- [ ] Multiple concurrent connections handled correctly
- [ ] Pipe ACL restricts to current user
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
1. Start wmux-app, use PowerShell to connect to pipe: `[System.IO.Pipes.NamedPipeClientStream]::new(".", "wmux", "InOut")` → send `{"id":"1","method":"system.ping"}\n` → verify response
2. Verify pipe appears in Windows pipe list
3. Test with malformed JSON → verify error response
### Edge Cases to Test
- Malformed JSON input (should return parse error, not crash)
- Missing `method` field (should return invalid request error)
- Very large request (> 1MB) — should reject with size limit error
- Client disconnects mid-request (should not crash server)
- Multiple simultaneous connections (should handle all)

## Dependencies
**Blocks**:
- Task L2_10: IPC Authentication
- Task L2_11: IPC Handler Trait + Router
- Task L2_15: CLI Client Foundation

## References
- **PRD**: §3 CLI & API IPC (Named Pipe paths, JSON-RPC v2)
- **Architecture**: §5 wmux-ipc, §6 Data Flow — IPC Command, §7 Security
- **ADR**: ADR-0005 (Named Pipes + JSON-RPC v2), ADR-0008 (actor pattern)
