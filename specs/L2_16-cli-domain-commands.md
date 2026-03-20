---
task_id: L2_16
title: "Implement CLI Domain Commands"
status: pending
priority: P0
estimated_hours: 2.5
wave: 11
prd_features: [F-03]
archi_sections: [ADR-0001, ADR-0005]
depends_on: [L2_15, L2_12, L2_13, L2_14]
blocks: [L4_07]
---

# Task L2_16: Implement CLI Domain Commands

> **Phase**: Core
> **Priority**: P0-Critical
> **Estimated effort**: 2.5 hours
> **Wave**: 11

## Context
With the CLI foundation and all IPC handlers in place, this task implements the complete set of CLI subcommands that wrap the JSON-RPC API. Each subcommand constructs a JSON-RPC request, sends it via the pipe client, and formats the response. PRD §3 defines all API categories.

## Prerequisites
- [ ] Task L2_15: CLI Client Foundation — provides clap structure, pipe client, output formatting
- [ ] Task L2_12: Workspace & Surface IPC Handlers — server-side workspace/surface support
- [ ] Task L2_13: Input & Read IPC Handlers — server-side input/read support
- [ ] Task L2_14: Sidebar Metadata IPC Handlers — server-side sidebar support

## Scope
### Deliverables
- Workspace commands: `wmux workspace list|create|select|close|rename`
- Surface commands: `wmux surface split|list|focus|close|read-text|send-text|send-key`
- Sidebar commands: `wmux sidebar set-status|clear-status|list-status|set-progress|clear-progress|log|clear-log|list-log|state`
- Notify commands: `wmux notify create|list|clear` (stub for Task L3_08)
- System commands: `wmux system ping|capabilities|identify`
- Human-readable output formatting for each command
- `--workspace` and `--surface` resolution for targeting

### Explicitly Out of Scope
- Browser commands (Task L3_07 — separate due to complexity)
- `wmux themes` commands (Task L3_12)
- `wmux ssh` commands (Task L4_03)
- `wmux update` commands (Task L4_04)
- Shell completion generation

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-cli/src/commands/workspace.rs` | Workspace subcommands |
| Create | `wmux-cli/src/commands/surface.rs` | Surface subcommands |
| Create | `wmux-cli/src/commands/sidebar.rs` | Sidebar subcommands |
| Create | `wmux-cli/src/commands/notify.rs` | Notify subcommands |
| Create | `wmux-cli/src/commands/system.rs` | System subcommands |
| Modify | `wmux-cli/src/commands/mod.rs` | Wire all subcommand modules |
| Modify | `wmux-cli/src/main.rs` | Register subcommands in clap |

### Key Decisions
- **Each subcommand is a thin wrapper**: Construct JSON-RPC params from clap args, send via client, format response. No business logic in CLI
- **Human output per command type**: workspace list → table, surface read-text → plain text, sidebar state → formatted metadata display
- **Global --workspace/--surface flags**: Injected into JSON-RPC params if specified. Allows targeting from command line

### Patterns to Follow
- PRD §3: CLI command examples and API table
- `.claude/rules/ipc-protocol.md`: "CLI-to-RPC table" correspondence

### Technical Notes
- `wmux workspace list` → `{"method":"workspace.list","params":{}}` → table: ID | Name | Active | Panes
- `wmux surface split --direction right` → `{"method":"surface.split","params":{"direction":"right"}}`
- `wmux surface read-text --start=-L3_01` → `{"method":"surface.read_text","params":{"start":-L3_01}}`
- `wmux sidebar set-status agent "Needs input" --icon=🔵 --color=blue` → `{"method":"sidebar.set_status","params":{"key":"agent","value":"Needs input","icon":"🔵","color":"blue"}}`
- `wmux sidebar log --level=info --source=claude -- "File created"` → `{"method":"sidebar.log","params":{"level":"info","source":"claude","message":"File created"}}`
- notify commands are stubs (return "not yet implemented" until Task L3_08)

## Success Criteria
- [ ] All workspace commands work end-to-end (create, list, select, close, rename)
- [ ] All surface commands work end-to-end (split, list, focus, close, read-text, send-text, send-key)
- [ ] All sidebar commands work end-to-end (set-status, clear-status, set-progress, log, state)
- [ ] System commands work (ping, capabilities, identify)
- [ ] Human-readable output is well-formatted for each command
- [ ] `--json` flag works for all commands
- [ ] `--workspace`/`--surface` targeting works
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
1. `wmux workspace create --name test` → verify workspace created
2. `wmux workspace list` → verify table output with new workspace
3. `wmux surface split --direction right` → verify pane splits
4. `wmux surface send-text "echo hello\n"` → verify command executes
5. `wmux surface read-text` → verify terminal content returned
6. `wmux sidebar set-status build "OK" --icon=✅` → verify badge in sidebar
7. `wmux sidebar state` → verify full metadata output
### Edge Cases to Test
- Command with no running wmux (clear error message)
- Invalid arguments (clap validation error)
- `--json` with error response (should output JSON error)
- Very long read-text output (should handle large responses)

## Dependencies
**Blocks**: None — this is the final IPC/CLI task. All later features add handlers.

## References
- **PRD**: §3 CLI & API IPC (complete command reference)
- **Architecture**: §5 wmux-cli (commands directory)
- **ADR**: ADR-0005 (JSON-RPC v2)
