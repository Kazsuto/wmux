---
paths:
  - "wmux-core/src/notification*"
  - "wmux-ui/src/notification*"
  - "wmux-ui/src/toast*"
---
# Notification Rules — wmux

## OSC Detection (CRITICAL)
- Detect OSC 9 (iTerm2 notification), OSC 99 (kitty notification), OSC 777 (rxvt notification) in the VTE handler.
- Forward detected notifications to NotificationStore — NEVER drop them silently.
- `wmux notify` via IPC also creates notifications — same code path as OSC.

## Windows Toast
- Use WinRT Toast Notification API via `windows` crate. NEVER use MessageBox or balloon tips.
- Toast must include wmux icon and source pane/workspace name.
- Toast click should focus the relevant workspace and pane.

## Visual Indicators
- Blue ring on pane border when notification pending (rendered in wgpu, not OS-level).
- Badge count on workspace in sidebar. Count resets when workspace is selected.
