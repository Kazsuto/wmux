# Task L4_03: Implement SSH Remote Support

> **Phase**: Polish
> **Priority**: P2-Medium
> **Estimated effort**: 3 hours

## Context
SSH remote lets users create durable workspaces on remote machines. A Go daemon (wmuxd-remote, reused from cmux) runs on the remote. PRD §9 describes the workflow. Architecture §5 specifies Go daemon reuse.

## Prerequisites
- [ ] Task L2_09: Named Pipes Server — provides IPC for remote CLI relay
- [ ] Task L2_07: Workspace Lifecycle — provides workspace model for remote workspaces

## Scope
### Deliverables
- `wmux ssh user@host` command in CLI
- Go daemon (wmuxd-remote) compilation and bundling
- SSH connection with daemon provisioning (upload if absent)
- Remote workspace creation (SSH icon in sidebar)
- PTY relay over SSH tunnel
- Reverse TCP relay for CLI control from remote
- Reconnection with exponential backoff
- `wmux ssh disconnect` command

### Explicitly Out of Scope
- Browser proxy (SOCKS5/HTTP CONNECT) — post-MVP
- SSH key management (use system SSH agent)
- Remote session persistence (daemon handles this)

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-cli/src/commands/ssh.rs` | SSH subcommands |
| Create | `wmux-core/src/remote.rs` | Remote workspace model |
| Create | `daemon/remote/cmd/wmuxd-remote/` | Go daemon (reuse from cmux) |
| Modify | `wmux-core/src/workspace.rs` | Add remote workspace type |
| Modify | `wmux-ui/src/sidebar.rs` | SSH icon for remote workspaces |

### Key Decisions
- **Reuse Go daemon** (Architecture §5): Already cross-platform, ~3K lines. Compile separately, bundle as resource binary
- **SSH via system ssh**: Use `ssh` command (not a Rust SSH library). Simpler, uses existing SSH config and agent
- **Exponential backoff**: On disconnect, retry at 1s, 2s, 4s, 8s, 16s, max 60s intervals

### Patterns to Follow
- Architecture §5 wmuxd-remote: "Bootstrapped by wmux ssh command"
- PRD §9: SSH workflow steps

### Technical Notes
- Workflow: (1) `wmux ssh user@host` → (2) SSH connect → (3) check for wmuxd-remote on remote → (4) upload if absent → (5) start daemon → (6) create tunnel → (7) create local workspace proxying to remote
- Daemon distribution: compile Go binary for target platform. Bundle linux-amd64 and linux-arm64 variants
- Tunnel: SSH port forwarding for PTY I/O and CLI relay
- Sidebar: remote workspaces show SSH icon (🔗 or similar) and connection status
- Disconnect: kill SSH process, mark workspace as disconnected, attempt reconnect

## Success Criteria
- [ ] `wmux ssh user@host` creates remote workspace
- [ ] Remote shell is accessible through wmux pane
- [ ] SSH icon visible in sidebar for remote workspaces
- [ ] Reconnection works after network interruption
- [ ] `wmux ssh disconnect` cleanly closes connection
- [ ] Daemon provisioned automatically on first connect
- [ ] `cargo clippy --workspace` zero warnings (Rust parts)

## Validation Steps
### Automated Checks
```bash
cargo build --workspace
cargo clippy --workspace -- -W clippy::all
cargo test --workspace
cargo fmt --all -- --check
```
### Manual Verification
1. `wmux ssh user@localhost` (test with local SSH) → verify remote workspace
2. Kill SSH → verify reconnection attempt
3. `wmux ssh disconnect` → verify cleanup
### Edge Cases to Test
- Host not reachable (should error, not hang)
- SSH auth failure (should show clear error)
- Daemon already running on remote (should reuse, not duplicate)
- Network drops during active session (should reconnect and resume)

## Dependencies
**Blocks**: None — leaf feature

## References
- **PRD**: §9 Support SSH Remote
- **Architecture**: §5 wmuxd-remote
