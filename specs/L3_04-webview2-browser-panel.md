---
task_id: L3_04
title: "Implement WebView2 Browser Panel in Split Panes"
status: pending
priority: P1
estimated_hours: 2.5
wave: 8
prd_features: [F-04]
archi_sections: [ADR-0001, ADR-0006]
depends_on: [L3_03, L2_02]
blocks: [L4_07]
---

# Task L3_04: Implement WebView2 Browser Panel in Split Panes

> **Phase**: Integration
> **Priority**: P1-High
> **Estimated effort**: 2.5 hours
> **Wave**: 8

## Context
Browser panels live inside panes as separate child HWNDs (NEVER inside wgpu surface). When a browser surface is created, a child window hosts the WebView2 control, positioned to match the pane's layout rect. Architecture §5 specifies "separate child HWND" and ADR-0006 details the approach.

## Prerequisites
- [ ] Task L3_03: WebView2 COM Initialization — provides WebView2 environment
- [ ] Task L2_02: PaneTree Layout Engine — provides pane layout rects for positioning

## Scope
### Deliverables
- Create child HWND for WebView2 within the main window
- Position and resize HWND to match pane layout rect
- Show/hide HWND on workspace switch (only visible pane's browser is shown)
- URL navigation: open URL in browser surface
- Focus handoff: clicking browser focuses it, clicking terminal returns focus
- DevTools toggle (F12)
- `BrowserPanel` struct tracking HWND + WebView2 controller per browser surface

### Explicitly Out of Scope
- JavaScript eval (Task L3_05)
- DOM automation (Task L3_06)
- IPC handlers (Task L3_07)
- Cookie/storage management (Task L3_06)

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Modify | `wmux-browser/src/manager.rs` | Add create_panel(), resize_panel(), show/hide |
| Create | `wmux-browser/src/panel.rs` | BrowserPanel struct (HWND + controller) |
| Modify | `wmux-ui/src/app.rs` | Create/position browser panels on pane layout change |
| Modify | `wmux-ui/Cargo.toml` | Add wmux-browser dependency |

### Key Decisions
- **Separate child HWND** (ADR-0006): Browser HWND is a child of the main window. Positioned by pane layout. This avoids compositing conflicts with wgpu
- **Show/hide on workspace switch**: When switching workspace, hide all browser HWNDs from old workspace, show HWNDs in new workspace. Prevents z-order issues
- **Focus handoff**: Track whether focus is in terminal or browser. Click events in browser area → browser gets focus. Global shortcuts still intercepted by wmux

### Patterns to Follow
- ADR-0006: "Separate child HWND, NEVER inside wgpu surface"
- `.claude/rules/windows-platform.md`: WebView2 child HWND management

### Technical Notes
- Child HWND: `CreateWindowExW(0, "Static", ..., WS_CHILD | WS_VISIBLE, parent_hwnd)`
- WebView2 controller: `CreateCoreWebView2Controller(hwnd, callback)` — async COM operation
- Resize: `SetWindowPos(hwnd, x, y, width, height)` matching pane rect
- Show/hide: `ShowWindow(hwnd, SW_SHOW/SW_HIDE)`
- HWND extraction from winit: `raw-window-handle` → `Win32WindowHandle` → HWND
- First navigation: `webview.Navigate(url)` after controller creation
- DevTools: `webview.OpenDevToolsWindow()` on F12
- Z-order: browser HWND sits above the wgpu surface in the same pane area. When terminal pane, no browser HWND exists

## Success Criteria
- [ ] Browser panel renders web content in pane area
- [ ] Browser repositions correctly when pane layout changes
- [ ] Workspace switch shows/hides correct browser panels
- [ ] F12 opens DevTools
- [ ] Focus handoff between terminal and browser works
- [ ] Multiple browser panels in different panes work independently
- [ ] Browser panel renders within < 1s of creation
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
1. Create a browser surface → verify web page loads
2. Split with terminal + browser → verify both render in correct areas
3. Resize window → verify browser repositions
4. Switch workspace and back → verify browser reappears correctly
5. F12 → verify DevTools opens
### Edge Cases to Test
- Browser in very small pane (minimum size) — should still render
- Navigate to invalid URL — should show error page
- Close pane with browser — should clean up HWND and WebView2 resources
- Multiple browsers loading simultaneously — no crash

## Dependencies
**Blocks**:
- Task L3_05: Browser JavaScript Eval + Navigation API
- Task L3_07: Browser IPC Handlers

## References
- **PRD**: §4 Navigateur Intégré (WebView2 in panes)
- **Architecture**: §5 wmux-browser (HWND management)
- **ADR**: ADR-0006 (separate child HWND)
