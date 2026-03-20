---
task_id: L3_09
title: "Implement Notification Visual Indicators"
status: pending
priority: P1
estimated_hours: 2.5
wave: 10
prd_features: [F-07]
archi_sections: [ADR-0001, ADR-0002]
depends_on: [L3_08, L2_08]
blocks: [L4_07]
---

# Task L3_09: Implement Notification Visual Indicators

> **Phase**: Integration
> **Priority**: P1-High
> **Estimated effort**: 2.5 hours
> **Wave**: 10

## Context
Notifications need visual representation: blue rings around panes, badges on workspaces in the sidebar, and a notification panel overlay. PRD §7 describes rings, badges, panel, and navigation shortcuts (Ctrl+Shift+I for panel, Ctrl+Shift+U for jump to unread).

## Prerequisites
- [ ] Task L3_08: Notification Store — provides notification data and unread counts
- [ ] Task L2_08: Sidebar UI Rendering — provides sidebar to render badges on

## Scope
### Deliverables
- Blue ring rendering on pane borders when notification pending (wgpu QuadPipeline)
- Badge count on workspace rows in sidebar
- Notification panel overlay (Ctrl+Shift+I toggle)
- Panel: scrollable list of notifications with title, body, timestamp, source
- Click notification in panel → navigate to source workspace/surface
- Ctrl+Shift+U → jump to most recent unread notification's workspace
- Ring animation (pulse/glow effect)

### Explicitly Out of Scope
- Windows Toast notifications (Task L3_10)
- Sound playback (Task L3_10)
- Custom notification commands (Task L3_10)

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-ui/src/notification_panel.rs` | Notification panel overlay |
| Modify | `wmux-ui/src/sidebar.rs` | Add badge rendering on workspace rows |
| Modify | `wmux-render/src/pane.rs` | Add ring rendering on pane borders |
| Modify | `wmux-ui/src/shortcuts.rs` | Add Ctrl+Shift+I and Ctrl+Shift+U |

### Key Decisions
- **Rings via QuadPipeline**: Blue semi-transparent quads around pane border. Animated brightness pulsing (sine wave over 2s cycle)
- **Badge as colored circle with number**: Small circle with unread count text, rendered in sidebar workspace row
- **Panel as overlay**: Full-height overlay on right side (like VS Code notifications). Rendered above all panes via wgpu

### Patterns to Follow
- PRD §7: Ring, badge, panel descriptions
- `.claude/rules/notifications.md`: "Blue ring on pane border when notification pending"

### Technical Notes
- Ring rendering: draw 4 quads (top/bottom/left/right edges) with animated alpha. Alpha = 0.3 + 0.2 * sin(time * π)
- Badge: render in sidebar at top-right of workspace row. Circle (quad with same w/h) + centered text
- Panel: list of notification items. Each item: title (bold), body, timestamp, source workspace name. Click handler → AppCommand::SelectWorkspace
- Ctrl+Shift+U: find most recent notification with state=Unread → navigate to its workspace
- Panel state: open/closed boolean in AppState. When open, suppresses desktop alerts
- Ring clears when workspace is selected (notification transitions to Read)
- Panel scrolling: track scroll offset, render visible items only

## Success Criteria
- [ ] Blue ring appears on pane when notification pending
- [ ] Ring pulses/animates
- [ ] Badge count shows on workspace in sidebar
- [ ] Ctrl+Shift+I toggles notification panel
- [ ] Panel shows notification history
- [ ] Click notification → navigates to source workspace
- [ ] Ctrl+Shift+U jumps to latest unread
- [ ] Ring and badge clear when workspace visited
- [ ] `cargo clippy --workspace` zero warnings

## Validation Steps
### Automated Checks
```bash
cargo build --workspace
cargo clippy --workspace -- -W clippy::all
cargo test --workspace
cargo fmt --all -- --check
```
### Manual Verification
1. Trigger notification → verify blue ring appears on source pane
2. Check sidebar → verify badge count on workspace
3. Ctrl+Shift+I → verify panel opens with notification
4. Click notification → verify workspace switches
5. Ctrl+Shift+U → verify jumps to unread notification workspace
### Edge Cases to Test
- Many notifications (100+) → panel should scroll smoothly
- Notification from closed workspace → badge appears but click does nothing
- No unread notifications → Ctrl+Shift+U is no-op
- Panel open while new notification arrives → should appear in panel immediately

## Dependencies
**Blocks**: None — visual leaf task

## References
- **PRD**: §7 Notifications (visual indicators, panel, navigation)
- **Architecture**: §4 Component Diagram (NotificationManager)
