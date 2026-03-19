# Task L2_11: Implement IPC Handler Trait, Router, and System Handlers

> **Phase**: Core
> **Priority**: P0-Critical
> **Estimated effort**: 2 hours

## Context
The IPC server needs a clean dispatch mechanism: route incoming JSON-RPC method names to handler implementations. The Handler trait provides the abstraction. System handlers (ping, capabilities, identify) are the first concrete implementations. Architecture §5 (wmux-ipc) describes "Handler trait (one impl per domain)."

## Prerequisites
- [ ] Task L2_09: Named Pipes Server — provides request dispatch point

## Scope
### Deliverables
- `Handler` trait: `async fn handle(&self, method: &str, params: Value, ctx: &ConnectionCtx) -> Result<Value, RpcError>`
- `Router` struct: maps method prefixes (e.g., "workspace.", "surface.") to Handler implementations
- `RpcError` enum with cmux-compatible error codes (parse_error, invalid_request, method_not_found, invalid_params, internal_error)
- `SystemHandler`: system.ping, system.capabilities, system.identify
- system.ping: returns pong (allowed unauthenticated)
- system.capabilities: returns list of supported methods and version
- system.identify: returns wmux version, platform, window info

### Explicitly Out of Scope
- Domain-specific handlers (Tasks L2_12-L2_14)
- Browser handlers (Task L3_07)
- Notification handlers (Task L3_08)

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Modify | `wmux-ipc/src/router.rs` | Router struct, method dispatch |
| Create | `wmux-ipc/src/handler.rs` | Handler trait definition |
| Create | `wmux-ipc/src/handlers/mod.rs` | Handler module organization |
| Create | `wmux-ipc/src/handlers/system.rs` | SystemHandler (ping, capabilities, identify) |
| Modify | `wmux-ipc/src/server.rs` | Wire router into request dispatch |
| Modify | `wmux-ipc/src/error.rs` | Add RpcError variants |

### Key Decisions
- **Prefix-based routing**: Router splits method name on "." → first part selects handler (workspace, surface, sidebar, browser, system, notification). Handler receives the full method name
- **Handler receives AppState channel**: Handlers send AppCommand to AppState actor and await response. This maintains the actor pattern — handlers never directly access state
- **RpcError codes match cmux**: -32700 parse error, -32600 invalid request, -32601 method not found, -32602 invalid params, -32603 internal error

### Patterns to Follow
- Architecture §5 wmux-ipc: "Handler trait (one impl per domain)"
- `.claude/rules/ipc-protocol.md`: Error code format
- ADR-0008: Handlers communicate with AppState via channels

### Technical Notes
- Handler trait must be `Send + Sync` (handlers are shared across connections)
- Router: `HashMap<String, Arc<dyn Handler + Send + Sync>>`
- system.capabilities response: `{ "methods": ["workspace.list", "workspace.create", ...], "version": "0.1.0" }`
- system.identify response: `{ "app": "wmux", "version": "0.1.0", "platform": "windows", "protocol_version": 1 }`
- system.ping response: `{ "pong": true }` — always allowed regardless of auth
- ConnectionCtx passed to handlers for auth-aware behavior

## Success Criteria
- [ ] Handler trait compiles and is implementable
- [ ] Router correctly dispatches methods to appropriate handlers
- [ ] Unknown methods return method_not_found error
- [ ] system.ping returns pong
- [ ] system.capabilities returns method list
- [ ] system.identify returns version info
- [ ] RpcError codes match cmux specification
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
1. Send `{"id":"1","method":"system.ping"}` → verify `{"id":"1","ok":true,"result":{"pong":true}}`
2. Send `{"id":"2","method":"unknown.method"}` → verify method_not_found error
3. Send `{"id":"3","method":"system.capabilities"}` → verify method list
### Edge Cases to Test
- Empty method name (should return invalid_request)
- Method with no dot separator (should return method_not_found)
- Missing params field (should default to empty object)
- Very large params object (should not crash)

## Dependencies
**Blocks**:
- Task L2_12: Workspace & Surface IPC Handlers
- Task L2_13: Input & Read IPC Handlers
- Task L2_14: Sidebar Metadata IPC Handlers
- Task L3_07: Browser IPC Handlers

## References
- **PRD**: §3 CLI & API IPC (API method categories)
- **Architecture**: §5 wmux-ipc (Handler trait, Router)
- **ADR**: ADR-0005 (JSON-RPC v2 protocol), ADR-0008 (actor communication)
