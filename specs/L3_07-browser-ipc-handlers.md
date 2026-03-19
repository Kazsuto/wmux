# Task L3_07: Implement Browser IPC Handlers

> **Phase**: Integration
> **Priority**: P1-High
> **Estimated effort**: 2 hours

## Context
All browser automation methods need IPC handlers so AI agents can control browser panels via the CLI or Named Pipe API. This wires the browser.* method family to the IPC router. PRD §3 lists browser methods in the API table.

## Prerequisites
- [ ] Task L2_11: IPC Handler Trait + Router — provides handler dispatch
- [ ] Task L3_05: Browser Navigation and JavaScript Eval — provides navigation/eval methods
- [ ] Task L3_06: Browser DOM Automation — provides DOM interaction methods

## Scope
### Deliverables
- `BrowserHandler` implementing Handler trait for all browser.* methods
- browser.open: create new browser surface in current pane
- browser.open-split: create browser surface in new split pane
- browser.navigate, back, forward, reload, url
- browser.click, dblclick, hover, focus, fill, type, press, select, check, uncheck, scroll
- browser.eval, addinitscript
- browser.snapshot, screenshot, get, is, find, highlight
- browser.wait (all wait variants)
- browser.tab, console, errors
- browser.cookies, storage, state
- browser.identify (browser version info)
- Target browser surface by surface_id

### Explicitly Out of Scope
- CLI browser subcommands (added to Task L2_16 as future extension)
- browser.dialog and browser.download (warn user, post-MVP)

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-ipc/src/handlers/browser.rs` | BrowserHandler implementation |
| Modify | `wmux-ipc/src/handlers/mod.rs` | Register browser handler |
| Modify | `wmux-ipc/src/router.rs` | Add "browser" prefix route |

### Key Decisions
- **Surface targeting**: browser.* methods accept `surface_id` param. If omitted, target the current surface if it's a browser, else error
- **browser.open creates surface**: Creates a new browser surface in the current pane (like Ctrl+T but browser type)
- **browser.open-split**: Creates a split with browser panel (convenience method combining surface.split + browser.open)

### Patterns to Follow
- Architecture §5 wmux-ipc: "One handler per domain"
- PRD §4: All browser categories wired to methods

### Technical Notes
- BrowserHandler routes to BrowserManager via AppState channel
- AppState identifies the target BrowserPanel by surface_id
- browser.cookies: `get_cookies(url)`, `set_cookie(params)`, `delete_cookies(params)` via WebView2 cookie manager
- browser.storage: `eval("localStorage.getItem('key')")` / `eval("localStorage.setItem('key','val')")`
- browser.state: return `{ url, title, loading, can_go_back, can_go_forward }`
- browser.tab: manage multiple browser tabs within a surface (if supported, else error)
- Each method validates params and returns appropriate RpcError for invalid input

## Success Criteria
- [ ] All browser.* methods accessible via IPC
- [ ] browser.open creates browser surface
- [ ] browser.navigate loads URL
- [ ] browser.eval returns JS result
- [ ] browser.click triggers element click
- [ ] browser.screenshot returns image data
- [ ] browser.snapshot returns accessibility tree
- [ ] Surface targeting by ID works
- [ ] Invalid surface_id returns clear error
- [ ] Methods on non-browser surface return clear error
- [ ] `cargo clippy --workspace` zero warnings

## Validation Steps
### Automated Checks
```bash
cargo build --workspace
cargo clippy --workspace -- -W clippy::all
cargo test -p wmux-ipc
cargo fmt --all -- --check
```
### Manual Verification
1. Via CLI: `wmux browser open https://example.com` → verify browser panel appears
2. Via CLI: `wmux browser eval "document.title"` → verify title returned
3. Via CLI: `wmux browser screenshot` → verify PNG data
4. Via CLI: `wmux browser snapshot` → verify tree structure
### Edge Cases to Test
- browser.* on terminal surface (should return error)
- browser.open when WebView2 not installed (should return clear error)
- Multiple browser surfaces with specific targeting
- Rapid sequential browser commands (should not race)

## Dependencies
**Blocks**: None directly — enables browser features through CLI/IPC

## References
- **PRD**: §3 CLI & API IPC (browser methods), §4 Navigateur Intégré
- **Architecture**: §5 wmux-ipc (handlers/browser.rs)
- **ADR**: ADR-0005 (JSON-RPC), ADR-0006 (WebView2)
