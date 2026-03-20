---
task_id: L2_10
title: "Implement IPC Authentication and Security Modes"
status: pending
priority: P1
estimated_hours: 2
wave: 8
prd_features: [F-03]
archi_sections: [ADR-0001, ADR-0005]
depends_on: [L2_09]
blocks: [L4_07]
---

# Task L2_10: Implement IPC Authentication and Security Modes

> **Phase**: Core
> **Priority**: P1-High
> **Estimated effort**: 2 hours
> **Wave**: 8

## Context
The IPC server needs multiple security modes to balance convenience and safety. The default `wmux_only` mode allows only child processes. The `password` mode uses HMAC-SHA256 for external clients. Architecture §7 defines the security modes and auth flow. `.claude/rules/ipc-protocol.md` specifies unauthenticated methods.

## Prerequisites
- [ ] Task L2_09: Named Pipes Server — provides connection handling where auth plugs in

## Scope
### Deliverables
- Security mode configuration: `off`, `wmux_only` (default), `allowAll`, `password`
- `wmux_only` mode: verify caller PID is a descendant of wmux process
- `password` mode: HMAC-SHA256 challenge-response (auth_secret file)
- `ConnectionCtx` struct: tracks auth state per connection
- Auth secret file: auto-generated at `%APPDATA%\wmux\auth_secret` with restricted ACL
- Unauthenticated methods: only `system.ping` and `auth.login`

### Explicitly Out of Scope
- OAuth or network authentication
- Per-method access control (all-or-nothing once authenticated)
- Auth secret rotation (post-MVP)

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-ipc/src/auth.rs` | Auth modes, HMAC-SHA256, ConnectionCtx |
| Modify | `wmux-ipc/src/server.rs` | Integrate auth check before handler dispatch |
| Modify | `wmux-ipc/src/protocol.rs` | Add auth.login method type |

### Key Decisions
- **wmux_only default** (Architecture §7): Child processes inherit WMUX_SOCKET_PATH and are auto-authenticated by PID ancestry check. Most secure default for agent workflows
- **Auth secret auto-generated**: 256-bit random, hex-encoded, written to file with owner-only ACL. NEVER logged
- **HMAC-SHA256 flow** (Architecture §7): Server sends nonce → client computes HMAC(secret, nonce) → server verifies → session token granted

### Patterns to Follow
- Architecture §7: Security modes table, auth flow
- `.claude/rules/ipc-protocol.md`: "Unauthenticated clients can ONLY call system.ping and auth.login"
- `.claude/rules/ipc-protocol.md`: "NEVER log auth secrets/HMAC tokens"

### Technical Notes
- PID ancestry check (wmux_only): Get client PID from Named Pipe (`GetNamedPipeClientProcessId`). Walk process tree upward to check if wmux is an ancestor. Use `windows` crate `CreateToolhelp32Snapshot`
- Auth secret file: `%APPDATA%\wmux\auth_secret`. Create with `OpenOptions::new().write(true).create_new(true)` to avoid race. Set restricted DACL via `windows` crate
- ConnectionCtx: `{ authenticated: bool, mode: SecurityMode, session_token: Option<String> }`
- In `off` mode, pipe is not created at all (handled by server startup)
- In `allowAll` mode, all connections are auto-authenticated
- challenge nonce: 32 random bytes, hex-encoded, fresh per connection

## Success Criteria
- [ ] `wmux_only` mode rejects connections from non-child processes
- [ ] `password` mode HMAC-SHA256 challenge-response works correctly
- [ ] `allowAll` mode accepts all local connections
- [ ] `off` mode prevents pipe creation
- [ ] Auth secret file created with restricted permissions
- [ ] Unauthenticated clients can only call system.ping and auth.login
- [ ] Auth secrets are NEVER logged
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
1. Start wmux in wmux_only mode → connect from external process → verify rejection
2. Start wmux in password mode → perform HMAC auth → verify success
3. Start wmux in allowAll mode → connect → verify immediate access
### Edge Cases to Test
- Invalid HMAC response (should reject)
- Replay of old nonce (should reject — nonces are single-use)
- Auth secret file already exists (should read existing, not overwrite)
- PID ancestry check with deeply nested process (grandchild of wmux)

## Dependencies
**Blocks**:
- Task L2_11: IPC Handler Trait + Router (auth check gates all handlers)

## References
- **PRD**: §3 CLI & API IPC (security modes)
- **Architecture**: §7 Security Architecture (full auth flow)
- **ADR**: ADR-0005 (Named Pipes security)
