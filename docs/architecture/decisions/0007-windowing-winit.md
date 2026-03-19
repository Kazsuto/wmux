# ADR-0007: Windowing — winit 0.30

> **Status**: Accepted
> **Date**: 2026-03-19
> **Confidence**: Medium
> **Deciders**: wmux team

## Context

wmux needs a window on Windows with: wgpu surface attachment, keyboard/mouse event handling, window resize, DPI scaling, and the ability to create child HWNDs for WebView2 panes. The windowing layer is the bridge between the OS and the rendering pipeline.

## Decision Drivers

- Must integrate cleanly with wgpu 28 (raw window handle for surface creation)
- Must support Windows 10 1809+ (our minimum platform target)
- Must provide keyboard events suitable for terminal input (key codes, modifiers, text input)
- Partial IME support for basic CJK input (full IME deferred to v2)
- Used by wgpu tutorials and the Rust GPU ecosystem — path of least resistance

## Decision

**winit 0.30** (stable line, currently 0.30.13). Uses the `ApplicationHandler` trait for event loop integration. Window created on `resumed()` event. wgpu surface attached via `raw-window-handle`.

## Alternatives Considered

### Raw Win32 (CreateWindowEx + message pump)
- **Pros**: Full control over everything. No abstraction overhead. Direct access to Win32 messages (WM_KEYDOWN, WM_CHAR, WM_IME_*). Can create child HWNDs directly
- **Cons**: Massive boilerplate. Manual DPI handling. Manual keyboard layout mapping. No cross-platform potential. Error-prone (Win32 API is C-style, lots of `unsafe`)
- **Why rejected**: Too much boilerplate for the windowing layer, which is not wmux's core value. winit handles DPI, keyboard mapping, and event loop correctly. Child HWNDs for WebView2 can still be created via the `windows` crate alongside winit

### SDL2 (via sdl2-rs)
- **Pros**: Mature, cross-platform. Good controller support. Stable API
- **Cons**: C library dependency (must ship SDL2.dll). Heavier than winit. Less Rust-native (FFI wrapper). wgpu integration is less standard than winit (need manual raw window handle extraction)
- **Why rejected**: External DLL dependency adds distribution complexity. winit is pure Rust and the standard choice for wgpu projects. SDL2's advantages (audio, gamepad) are irrelevant for a terminal

### winit 0.31 (beta)
- **Pros**: Newer API improvements. May fix some IME edge cases
- **Cons**: Still in beta (0.31.0-beta.2, November 2025). Breaking changes possible. Less ecosystem testing
- **Why rejected**: Beta status is too risky for a foundation dependency. 0.30.13 is stable and well-tested. Will upgrade to 0.31 when it reaches stable

### glazier (from Linebender/Xilem)
- **Pros**: Designed for Rust GUI apps. Rich window management. Platform-native feel
- **Cons**: Experimental. Not widely used. Uncertain maintenance trajectory. Limited Windows testing
- **Why rejected**: Too experimental. winit has 100x the usage and testing. glazier may be the future, but it's not ready for production

## Consequences

### Positive
- Standard wgpu integration path — `raw-window-handle` works out of the box
- Pure Rust — no external DLL to ship
- Stable API (0.30.x has 13 patch releases — well-hardened)
- DPI scaling, multi-monitor support, and basic IME handled by the library
- Cross-platform potential if wmux ever expands to Linux/macOS

### Negative (acknowledged trade-offs)
- IME support is partial — complex CJK composition may have edge cases on Windows (TSF/IMM32)
- No built-in child HWND creation — must use `windows` crate for WebView2 HWND (winit provides the parent HWND via raw window handle)
- The `ApplicationHandler` trait is more complex than the old event loop closure API — but it's the modern, supported approach
- winit abstracts away some Win32 message details — may need to hook into the message pump for advanced scenarios (custom title bar, system tray)

### Mandatory impact dimensions
- **Security**: winit handles window input events — must ensure no input injection from other processes (handled by Windows security model, not winit)
- **Cost**: $0. MIT/Apache dual licensed
- **Latency**: winit event dispatch adds < 0.1ms overhead. `WaitUntil` / `Poll` control modes available for frame pacing. Using `RequestRedraw` for on-demand rendering (not polling every frame)

## Revisit Triggers

- If winit 0.31 reaches stable with meaningful improvements (better IME, better Win32 integration), upgrade from 0.30
- If IME issues become blocking for CJK users in v2, investigate supplementing winit with direct Win32 TSF hooks
- If custom title bar or system tray integration is needed, evaluate whether winit's raw window handle gives sufficient access or if glazier/raw Win32 is needed for those features
