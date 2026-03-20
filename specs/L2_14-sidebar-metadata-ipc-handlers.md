---
task_id: L2_14
title: "Implement Sidebar Metadata Store and IPC Handlers"
status: pending
priority: P0
estimated_hours: 2.5
wave: 10
prd_features: [F-03, F-05]
archi_sections: [ADR-0001, ADR-0005, ADR-0008]
depends_on: [L2_11, L2_08]
blocks: [L2_16]
---

# Task L2_14: Implement Sidebar Metadata Store and IPC Handlers

> **Phase**: Core
> **Priority**: P0-Critical
> **Estimated effort**: 2.5 hours
> **Wave**: 10

## Context
The sidebar metadata system is a key differentiator for AI agent workflows. Agents set status badges, progress bars, and log entries via the API to show their state in the sidebar. Architecture §6 defines the MetadataStore and data model. PRD §5 describes the Sidebar Metadata System in detail.

## Prerequisites
- [ ] Task L2_11: IPC Handler Trait + Router — provides Handler dispatch
- [ ] Task L2_08: Sidebar UI Rendering — renders metadata in sidebar

## Scope
### Deliverables
- `MetadataStore` struct: per-workspace storage of statuses, progress, logs
- `SidebarHandler` (IPC): all sidebar.* methods
- sidebar.set_status / clear_status / list_status: key-value badges with icon and color
- sidebar.set_progress / clear_progress: progress bar (0.0-1.0 with label)
- sidebar.log / clear_log / list_log: timestamped log entries with level and source
- sidebar.state: return full metadata state for a workspace
- PID-aware lifecycle: track setter PID, sweep timer (30s) to clear dead process statuses

### Explicitly Out of Scope
- Sidebar rendering updates (Task L2_08 already handles rendering from metadata)
- Notification creation from sidebar events
- Custom sidebar widgets

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-core/src/metadata_store.rs` | MetadataStore, StatusEntry, LogEntry |
| Create | `wmux-ipc/src/handlers/sidebar.rs` | SidebarHandler implementation |
| Modify | `wmux-core/src/app_state.rs` | Add MetadataStore to AppState per workspace |
| Modify | `wmux-ipc/src/handlers/mod.rs` | Register sidebar handler |
| Modify | `wmux-core/src/lib.rs` | Export metadata_store module |

### Key Decisions
- **Per-workspace MetadataStore**: Each workspace has independent statuses, progress, and logs. sidebar.* methods target workspace via params or current
- **PID tracking** (Architecture §6): Each status entry records the PID that set it. A 30-second sweep timer checks PIDs via `GetExitCodeProcess`. Dead PIDs → statuses auto-cleared
- **Logs capped at L3_01** (Architecture §6): Oldest entries evicted when exceeding cap

### Patterns to Follow
- Architecture §6 Sidebar Metadata Model: status/progress/log schemas
- Architecture §6 Data Flow — Sidebar Metadata Update: sequence diagram
- PRD §5: sidebar.* CLI examples and API methods

### Technical Notes
- StatusEntry: `{ key: String, value: String, icon: Option<String>, color: Option<String>, pid: Option<u32> }`
- Statuses stored as `HashMap<String, StatusEntry>` (key = status key like "agent", "build")
- Progress: `Option<ProgressState>` where `ProgressState { value: f32, label: Option<String> }`
- LogEntry: `{ timestamp: DateTime<Utc>, level: LogLevel, source: String, message: String }`
- LogLevel enum: Info, Progress, Success, Warning, Error
- PID sweep timer: `tokio::spawn(async { loop { sleep(Duration::from_secs(30)).await; sweep_dead_pids(); } })`
- PID check on Windows: `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, pid)` → if fails, process is dead
- sidebar.log params: `{ "message": "...", "level": "info", "source": "claude", "workspace_id": "optional" }`

## Success Criteria
- [ ] sidebar.set_status creates/updates a status badge in sidebar
- [ ] sidebar.clear_status removes a specific status
- [ ] sidebar.list_status returns all statuses for workspace
- [ ] sidebar.set_progress sets progress bar value and label
- [ ] sidebar.clear_progress removes progress bar
- [ ] sidebar.log adds a timestamped entry
- [ ] sidebar.list_log returns log entries (with limit support)
- [ ] sidebar.clear_log removes all log entries
- [ ] sidebar.state returns complete metadata
- [ ] PID sweep clears statuses from terminated processes within 30s
- [ ] Log entries capped at L3_01 per workspace
- [ ] `cargo clippy --workspace` zero warnings

## Validation Steps
### Automated Checks
```bash
cargo build --workspace
cargo clippy --workspace -- -W clippy::all
cargo test -p wmux-core -p wmux-ipc
cargo fmt --all -- --check
```
### Manual Verification
1. Send sidebar.set_status → verify badge appears in sidebar
2. Send sidebar.set_progress 0.5 → verify progress bar in sidebar
3. Send sidebar.log "test" → verify log entry in sidebar
4. Kill the process that set a status → wait 30s → verify status cleared
### Edge Cases to Test
- Set status with same key (should update, not duplicate)
- Progress value out of range (< 0.0 or > 1.0) — should clamp
- L3_02 log entries (101st should evict oldest)
- Clear status for non-existent key (should be no-op, not error)
- sidebar.state on workspace with no metadata (should return empty structures)

## Dependencies
**Blocks**:
- Task L2_16: CLI Domain Commands (CLI sidebar commands)

## References
- **PRD**: §5 Sidebar Metadata System (full API description)
- **Architecture**: §6 Sidebar Metadata Model, Data Flow diagram
- **ADR**: ADR-0008 (actor communication for metadata updates)
