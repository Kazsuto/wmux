# Task L3_05: Implement Browser Navigation and JavaScript Eval API

> **Phase**: Integration
> **Priority**: P1-High
> **Estimated effort**: 2 hours

## Context
AI agents need to navigate browser panels and execute JavaScript. This task implements the navigation commands (open, navigate, back, forward, reload) and JavaScript eval with return values. PRD §4 lists 50+ browser commands across 8 categories. Architecture §5 shows automation API exposed via IPC.

## Prerequisites
- [ ] Task L3_03: WebView2 COM Initialization — provides WebView2 environment and controller

## Scope
### Deliverables
- Navigation methods: navigate(url), back(), forward(), reload(), url() (get current)
- Wait methods: wait for selector, text, URL pattern, load state, JS condition
- JavaScript eval: execute JS in page context, return serialized result
- `browser.focus_webview` / `browser.is_webview_focused`: focus management
- Script injection: `addInitScript(js)` for early-load scripts

### Explicitly Out of Scope
- DOM interaction (click, fill, type) — Task L3_06
- Screenshot/snapshot — Task L3_06
- IPC handler wiring — Task L3_07
- Cookie/storage management — Task L3_06

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-browser/src/automation.rs` | Navigation, eval, wait methods |
| Modify | `wmux-browser/src/panel.rs` | Add automation methods to BrowserPanel |
| Modify | `wmux-browser/src/lib.rs` | Export automation module |

### Key Decisions
- **JS eval returns values** (PRD §4): Unlike simple eval, wmux browser.eval must return the result of the expression as a JSON value. WebView2 `ExecuteScriptAsync` returns the result
- **Wait polling**: Wait methods poll at 100ms intervals until condition met or timeout (default 30s). Condition checked via JS eval
- **Security**: JS eval only callable via IPC (authenticated). No user-facing eval input

### Patterns to Follow
- PRD §4: browser.eval returns value, not just "OK"
- Architecture §5 wmux-browser: "Automation API exposed via wmux-ipc handlers"

### Technical Notes
- WebView2 `Navigate(url)`: async navigation. Listen for `NavigationCompleted` event
- `ExecuteScriptAsync(js)`: returns JSON-serialized result. Parse to serde_json::Value
- Back/Forward: `GoBack()` / `GoForward()` on WebView2 controller
- Wait for selector: poll `document.querySelector(selector) !== null`
- Wait for text: poll `document.body.innerText.includes(text)`
- Wait for URL: check `Source` property against pattern
- Wait for load: check `IsDocumentPlayingAudio` or navigation state
- addInitScript: `AddScriptToExecuteOnDocumentCreated(js)` — runs before page scripts
- Timeout: default 30s, configurable via params

## Success Criteria
- [ ] Navigate to URL and get current URL
- [ ] Back/forward navigation works
- [ ] Reload refreshes the page
- [ ] JS eval executes and returns result as JSON value
- [ ] Wait for selector resolves when element appears
- [ ] Wait for text resolves when text appears in page
- [ ] Wait timeout returns error after configured duration
- [ ] addInitScript runs before page load
- [ ] `cargo clippy --workspace` zero warnings

## Validation Steps
### Automated Checks
```bash
cargo build --workspace
cargo clippy --workspace -- -W clippy::all
cargo test -p wmux-browser
cargo fmt --all -- --check
```
### Manual Verification
1. Navigate to https://example.com → verify page loads
2. `browser.eval("document.title")` → verify returns "Example Domain"
3. `browser.back()` then `browser.forward()` → verify navigation
4. `browser.wait` for selector `h1` → verify resolves immediately on example.com
### Edge Cases to Test
- JS eval with syntax error (should return error, not crash)
- Navigate to unreachable URL (should timeout/error gracefully)
- Wait for selector that never appears (should timeout)
- JS eval returning large object (should handle serialization)
- Navigate during JS eval (should not deadlock)

## Dependencies
**Blocks**:
- Task L3_06: Browser DOM Automation
- Task L3_07: Browser IPC Handlers

## References
- **PRD**: §4 Navigateur Intégré (Navigation, JavaScript, Attente categories)
- **Architecture**: §5 wmux-browser (automation.rs)
- **ADR**: ADR-0006 (WebView2 automation capabilities)
