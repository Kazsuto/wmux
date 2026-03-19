# ADR-0005: IPC — Named Pipes + JSON-RPC v2

> **Status**: Accepted
> **Date**: 2026-03-19
> **Confidence**: High
> **Deciders**: wmux team

## Context

wmux must expose a programmatic API for AI agents (Claude Code, Codex, OpenCode) and the `wmux` CLI to control the application: create workspaces, split panes, send text, open browser URLs, trigger notifications. The IPC mechanism must be cmux-compatible (~95%) so existing AI agent integrations work with minimal adaptation.

## Decision Drivers

- cmux uses Unix domain sockets with JSON-RPC v2 — wmux must match the protocol semantics
- AI agents discover the socket via `WMUX_SOCKET_PATH` environment variable
- No port conflicts (multiple wmux instances must coexist)
- Security: restrict access to authorized processes by default
- Lower latency than TCP loopback

## Decision

**Named Pipes** (`\\.\pipe\wmux-{session_id}`) as transport, **JSON-RPC v2** (newline-delimited) as protocol. Implemented with tokio async Named Pipe APIs from the `tokio` and `windows` crates.

Protocol format (cmux-compatible):
```json
{"id":"abc123","method":"workspace.list","params":{}}
{"id":"abc123","ok":true,"result":[{"id":"ws1","name":"dev"}]}
```

## Alternatives Considered

### TCP loopback (localhost:port)
- **Pros**: Cross-platform. Easy to implement. Works with any HTTP/JSON client
- **Cons**: Port conflicts when running multiple instances. Visible to network firewalls. Higher latency than Named Pipes. No inherent access control (any process on the machine can connect)
- **Why rejected**: Project rules explicitly state "NEVER use TCP for local IPC." Named Pipes provide better security (ACL-based), no port conflicts, and lower latency. TCP would be a departure from cmux's Unix socket model

### AF_UNIX sockets on Windows
- **Pros**: Identical to cmux's mechanism. Windows 10 1803+ supports AF_UNIX sockets
- **Cons**: Windows AF_UNIX implementation has limitations (no `SO_PEERCRED` equivalent for peer authentication, no file-system permissions like Unix). Less ecosystem support on Windows. Named Pipes are the native Windows IPC pattern
- **Why rejected**: AF_UNIX on Windows is a compatibility layer, not a native feature. Named Pipes offer better security (DACL), better tooling support (PowerShell, VS Code use them), and are the idiomatic Windows choice. The JSON-RPC protocol layer makes the transport transparent to AI agents

### gRPC (protobuf over HTTP/2)
- **Pros**: Typed API contracts. Streaming support. Code generation. Used by many microservices
- **Cons**: Heavy dependency (tonic + prost). Overkill for local IPC. Not compatible with cmux's JSON-RPC protocol. Binary format harder to debug. Requires HTTP/2 transport
- **Why rejected**: cmux uses JSON-RPC v2 — switching to gRPC would break AI agent compatibility, which is the primary use case. JSON-RPC is simpler, human-readable, and sufficient for the command set

## Consequences

### Positive
- ~95% compatible with cmux protocol — AI agents (Claude Code) work with minimal adaptation
- Named Pipes: no port conflicts, ACL security, lower latency than TCP (~0.1ms vs ~1ms)
- JSON-RPC is human-readable — easy to debug with `wmux --json` or pipe to `jq`
- newline-delimited framing is simple to implement (no content-length headers, no framing complexity)

### Negative (acknowledged trade-offs)
- Named Pipes are Windows-only — if wmux goes cross-platform, needs transport abstraction (Unix sockets on Linux/macOS)
- JSON-RPC has no schema enforcement — must validate requests manually (or with serde)
- JSON serialization adds overhead vs binary protocols (~10μs per message — negligible for IPC command frequency)

### Mandatory impact dimensions
- **Security**: Default mode (`wmux_only`) restricts to child processes. HMAC-SHA256 challenge-response for `password` mode. Named Pipe DACL restricts to current user. Auth secrets stored in separate file with restricted permissions, never logged
- **Cost**: $0. tokio Named Pipe support is built-in. serde_json handles serialization
- **Latency**: Named Pipe round-trip ~0.1ms local. JSON parse + serialize ~10μs. Total IPC command latency < 5ms including handler execution

## Revisit Triggers

- If wmux expands to Linux/macOS, implement a transport abstraction layer (Named Pipes on Windows, Unix sockets on Unix) behind a common trait
- If AI agents require streaming responses (e.g., live terminal output streaming), evaluate adding JSON-RPC notifications (server → client push) over the same pipe
- If IPC throughput becomes a bottleneck (> 10K commands/second), evaluate binary framing (MessagePack) while keeping JSON as human-readable fallback
