# Task L3_08: Implement Notification Store and OSC Detection

> **Phase**: Integration
> **Priority**: P1-High
> **Estimated effort**: 2 hours

## Context
wmux needs a notification system with a complete lifecycle (received → unread → read → cleared). Notifications come from OSC escape sequences (already parsed in Task L1_04), the CLI, and the IPC API. Architecture §4 shows NotificationManager component. PRD §7 defines the notification lifecycle and suppression rules.

## Prerequisites
- [ ] Task L1_04: OSC Sequence Handlers — provides TerminalEvent::Notification from OSC 9/99/777

## Scope
### Deliverables
- `NotificationStore` in wmux-core: stores notifications with lifecycle state
- `Notification` struct: id, title, body, subtitle, source_workspace, source_surface, timestamp, state
- `NotificationState` enum: Received, Unread, Read, Cleared
- Lifecycle transitions: Received → Unread (after display), Read (workspace visited), Cleared (user action)
- IPC handler: notification.create, notification.list, notification.clear
- OSC notification forwarding: TerminalEvent::Notification → NotificationStore
- Suppression rules: suppress desktop alert when wmux active AND source workspace active, or notification panel open
- Unread count per workspace (for sidebar badges)

### Explicitly Out of Scope
- Visual indicators (rings, badges, panel) — Task L3_09
- Windows Toast notifications — Task L3_10
- Sound playback — Task L3_10
- Custom command on notification — Task L3_10

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-core/src/notification.rs` | NotificationStore, Notification, lifecycle |
| Create | `wmux-ipc/src/handlers/notification.rs` | Notification IPC handlers |
| Modify | `wmux-core/src/app_state.rs` | Add NotificationStore, process terminal events |
| Modify | `wmux-ipc/src/handlers/mod.rs` | Register notification handler |
| Modify | `wmux-core/src/lib.rs` | Export notification module |

### Key Decisions
- **Lifecycle per PRD**: Received → Unread → Read → Cleared. Each state has visual implications (handled by Task L3_09)
- **Suppression**: Desktop alerts suppressed when the source workspace is active AND wmux is focused. This prevents annoying notifications for the workspace you're already looking at
- **Cap**: Store max 200 notifications. Oldest cleared notifications evicted first

### Patterns to Follow
- PRD §7: Notification lifecycle, suppression rules
- `.claude/rules/notifications.md`: "Forward to NotificationStore — NEVER drop silently"
- Architecture §4: NotificationManager component

### Technical Notes
- NotificationStore: `Vec<Notification>` sorted by timestamp. Methods: `add()`, `mark_read(workspace_id)`, `clear(id)`, `clear_all()`, `unread_count(workspace_id)`
- When workspace is selected, all notifications from that workspace transition to Read
- notification.create params: `{ "title": "...", "body": "...", "subtitle": "optional" }` — creates via API (not OSC)
- notification.list params: `{ "state": "optional-filter", "limit": 50 }` → returns notification array
- notification.clear params: `{ "id": "..." }` or `{ "all": true }` → clear specific or all
- OSC notifications carry source_workspace and source_surface from the terminal that emitted them
- NotificationEvent emitted for visual/audio handlers: `{ notification, suppressed: bool }`

## Success Criteria
- [ ] OSC 9/99/777 notifications are stored in NotificationStore
- [ ] notification.create via IPC works
- [ ] Lifecycle transitions (Received → Unread → Read → Cleared) work correctly
- [ ] notification.list returns notifications with correct states
- [ ] notification.clear removes notifications
- [ ] Suppression rules prevent desktop alerts when appropriate
- [ ] Unread count per workspace is accurate
- [ ] Notification cap (200) is enforced
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
1. Trigger OSC 9 notification in terminal → verify stored in NotificationStore
2. Send notification.create via CLI → verify stored
3. Switch to notification's workspace → verify state becomes Read
4. notification.clear → verify removed
### Edge Cases to Test
- 201 notifications (should evict oldest cleared)
- Notification from inactive workspace (should NOT be suppressed)
- Notification from active workspace with wmux focused (SHOULD be suppressed)
- notification.clear with invalid ID (should return error or no-op)

## Dependencies
**Blocks**:
- Task L3_09: Notification Visual Indicators
- Task L3_10: Windows Toast Notifications

## References
- **PRD**: §7 Notifications (lifecycle, suppression, sources)
- **Architecture**: §4 Component Diagram (NotificationManager)
- **ADR**: ADR-0008 (event-driven notification flow)
