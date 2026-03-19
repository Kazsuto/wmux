# Task L3_03: Implement WebView2 COM Initialization and RAII Wrappers

> **Phase**: Integration
> **Priority**: P1-High
> **Estimated effort**: 2.5 hours

## Context
The integrated browser uses Microsoft WebView2 (Chromium/Edge) for rendering web content in terminal panes. This task sets up the COM foundation. Architecture §5 (wmux-browser) specifies webview2-com 0.39. ADR-0006 mandates WebView2 in separate child HWND (NEVER inside wgpu surface).

## Prerequisites
- [ ] Task L0_01: Error Types and Tracing Infrastructure — provides BrowserError enum

## Scope
### Deliverables
- COM initialization (`CoInitializeEx` with apartment threading)
- WebView2 environment creation (`CreateCoreWebView2EnvironmentWithOptions`)
- `BrowserManager` struct with RAII lifecycle
- Safe Rust wrappers around WebView2 COM interfaces
- Runtime detection: check if WebView2 is installed, graceful error if missing
- User data directory setup (`%APPDATA%\wmux\webview2-data`)

### Explicitly Out of Scope
- HWND creation and positioning (Task L3_04)
- Navigation and automation APIs (Tasks L3_05-L3_06)
- IPC handler integration (Task L3_07)

## Implementation Details
### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Create | `wmux-browser/src/manager.rs` | BrowserManager struct, WebView2 env creation |
| Create | `wmux-browser/src/com.rs` | Safe RAII COM wrappers |
| Modify | `wmux-browser/src/lib.rs` | Export modules |
| Modify | `wmux-browser/Cargo.toml` | Add webview2-com, windows deps |

### Key Decisions
- **webview2-com 0.39** (ADR-0006): Maintained, 1M+ downloads, exposes 100% of COM API
- **RAII wrappers with Drop** (`.claude/rules/rust-architecture.md`): All COM pointers wrapped in safe Rust types that release on Drop
- **Apartment-threaded COM**: WebView2 requires STA (Single-Threaded Apartment). COM init on the UI thread

### Patterns to Follow
- ADR-0006: webview2-com 0.39, separate child HWND
- `.claude/rules/rust-architecture.md`: "Wrap all Win32/COM FFI in RAII safe abstractions with Drop"
- `.claude/rules/windows-platform.md`: "All COM calls wrapped in safe Rust abstractions"

### Technical Notes
- COM init: `CoInitializeEx(None, COINIT_APARTMENTTHREADED)` before any WebView2 calls
- Environment creation is async (callback-based in COM). webview2-com provides async wrappers
- User data dir: `%APPDATA%\wmux\webview2-data` — isolates WebView2 cache from Edge profile
- Runtime detection: `GetAvailableCoreWebView2BrowserVersionString` — returns version or error
- If runtime missing: return `BrowserError::RuntimeNotInstalled` with user-facing message
- WebView2 init takes ~200-500ms for first instance. Cache environment for subsequent instances
- All COM operations must happen on the UI thread (STA requirement)

## Success Criteria
- [ ] COM initializes successfully on the UI thread
- [ ] WebView2 environment created with user data directory
- [ ] Runtime detection works (present/absent)
- [ ] Graceful error when WebView2 runtime is not installed
- [ ] RAII wrappers properly release COM resources on drop
- [ ] No unsafe code without `// SAFETY:` comments
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
1. Integration test (`#[ignore]`): create WebView2 environment → verify success
2. Check `%APPDATA%\wmux\webview2-data` directory created
3. Test on system without WebView2 runtime (if possible) → verify error message
### Edge Cases to Test
- Double COM initialization (should be safe — CoInitializeEx is idempotent)
- WebView2 environment creation failure (disk full, permissions) — graceful error
- Drop BrowserManager while WebView2 instances exist — cleanup order

## Dependencies
**Blocks**:
- Task L3_04: WebView2 Browser Panel in Split Panes
- Task L3_05: Browser JavaScript Eval + Navigation API

## References
- **PRD**: §4 Navigateur Intégré (WebView2)
- **Architecture**: §5 wmux-browser, §10 Failure Modes (WebView2 runtime missing)
- **ADR**: ADR-0006 (WebView2 via webview2-com 0.39)
