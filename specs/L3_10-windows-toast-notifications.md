# Task L3_10: Implement Windows Toast Notifications

> **Phase**: Integration
> **Priority**: P1-High
> **Estimated effort**: 2 hours

## Context
Desktop notifications use Windows Toast (WinRT) to alert users when wmux is not focused. PRD §7 describes Toast integration with actions, sounds, and custom commands. Architecture §3 maps NSUserNotification (macOS) to Windows Toast.

## Prerequisites
- [ ] Task L3_08: Notification Store — provides notification events and suppression state

## Scope
### Deliverables
- Windows Toast notifications via WinRT `Windows.UI.Notifications` API
- AUMID setup for Toast identity (App User Model ID)
- Toast with title, body, wmux icon
- Toast click → focus wmux window + navigate to source workspace
- Sound selection: system sounds, custom .wav, none
- Custom command on notification: execute shell command with env vars (WMUX_NOTIFICATION_TITLE, etc.)
- TTS option via Win32 SAPI

### Explicitly Out of Scope
- Toast action buttons (post-MVP)
- Notification grouping/stacking
- Toast on lock screen

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-ui/src/toast.rs` | Toast notifications via WinRT |
| Modify | `wmux-ui/src/app.rs` | Subscribe to notification events for Toast |
| Modify | `wmux-ui/Cargo.toml` | Add windows crate features for Toast |

### Key Decisions
- **WinRT Toast API** (`.claude/rules/notifications.md`): NEVER MessageBox or balloon tips. Use modern Toast API via `windows` crate
- **AUMID required**: Toast notifications need an App User Model ID. Set via `Shell32::SetCurrentProcessExplicitAppUserModelID("wmux")`
- **Sound via PlaySoundW**: Win32 `PlaySoundW` for .wav files. System sounds via resource names

### Patterns to Follow
- `.claude/rules/notifications.md`: "Use WinRT Toast API — NEVER MessageBox/balloon tips"
- `.claude/rules/notifications.md`: "Shell_NotifyIconW prohibition" — no system tray balloons
- Architecture §3 Adaptation Table: NSUserNotification → Windows Toast

### Technical Notes
- Toast XML template: `<toast><visual><binding><text>Title</text><text>Body</text></binding></visual></toast>`
- ToastNotificationManager: `CreateToastNotifierForApplication(aumid)` → `Show(notification)`
- Toast activation: handle `Activated` event → use `EventLoopProxy` to send focus command to main window
- Custom command: spawn `cmd /c <command>` with env vars. Config setting: `notification_command = "path/to/script"`
- Sound options in config: `notification_sound = "default" | "none" | "path/to/file.wav"`
- TTS: `Add-Type -AssemblyName System.Speech; (New-Object Speech.Synthesis.SpeechSynthesizer).Speak("text")` — executed via PowerShell
- Suppression: check NotificationEvent.suppressed flag before showing Toast

## Success Criteria
- [ ] Toast notification appears on Windows desktop
- [ ] Toast shows correct title and body
- [ ] Clicking Toast focuses wmux and navigates to source workspace
- [ ] Sound plays with notification (configurable)
- [ ] Custom command executes with correct env vars
- [ ] Toast respects suppression rules (not shown when active workspace)
- [ ] AUMID set correctly for Toast identity
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
1. Minimize wmux → trigger notification → verify Toast appears
2. Click Toast → verify wmux focuses and shows source workspace
3. Configure custom sound → verify sound plays
4. Focus wmux on source workspace → trigger notification → verify NO Toast (suppressed)
### Edge Cases to Test
- Toast when wmux is closed (should not crash, just no Toast)
- Very long notification body (should truncate in Toast)
- Custom command that fails (should log error, not crash)
- Rapid notifications (should not spam Toasts — debounce)

## Dependencies
**Blocks**: None — leaf notification feature

## References
- **PRD**: §7 Notifications (Toast, sounds, custom commands)
- **Architecture**: §3 Adaptation Table (Toast)
- **ADR**: ADR-0006 referenced for Windows platform patterns
