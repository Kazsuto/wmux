---
paths:
  - "**/*.rs"
---
# Windows Platform Rules — wmux

## Compatibility (CRITICAL)
- Target **Windows 10 1809+** minimum (ConPTY requirement).
- Mica/Acrylic effects: Win11 only. **ALWAYS** feature-detect and fallback to opaque background on Win10.
- Test on both Win10 and Win11 when touching DWM/visual effects.

## IPC (CRITICAL)
- **NEVER** use TCP for local IPC — always Named Pipes (`\\.\pipe\wmux-*`).
- Protocol: JSON-RPC v2, **~95% compatible** with cmux (same method names, same message structure).
- Pipe path exposed via `WMUX_SOCKET_PATH` environment variable.

## WebView2 (CRITICAL)
- WebView2 hosted in a **separate child HWND** — NEVER inside the wgpu surface.
- Position/size managed by the split container. Show/hide on workspace switch.
- All WebView2 COM calls wrapped in safe Rust abstractions.

## ConPTY
- Spawn shells via `portable-pty` — NEVER raw ConPTY API directly.
- Default shell detection order: pwsh → powershell → cmd.
- Inject `WMUX_WORKSPACE_ID`, `WMUX_SURFACE_ID`, `WMUX_SOCKET_PATH` into shell environment.
