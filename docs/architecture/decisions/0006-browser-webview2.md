# ADR-0006: Browser Integration — WebView2 via webview2-com 0.39

> **Status**: Accepted
> **Date**: 2026-03-19
> **Confidence**: High
> **Deciders**: wmux team

## Context

wmux includes an integrated browser for previewing web apps, inspecting results, and AI agent browser automation — all without leaving the terminal window. cmux uses WebKit (WKWebView) on macOS. wmux needs the Windows equivalent: an embedded Chromium browser in split panes with an automation API (click, fill, eval, screenshot).

## Decision Drivers

- Must be pre-installed on Windows 10/11 (no 100MB+ browser engine to ship)
- Must support JavaScript evaluation, DevTools, cookies/storage control
- Must run in a separate child HWND (never inside the wgpu surface — per project rules)
- Must be controllable via COM for programmatic automation
- Production-proven crate with active maintenance

## Decision

**WebView2** (Microsoft Edge Chromium) via the **webview2-com 0.39** crate. Browser panes are hosted in separate child HWNDs, positioned and sized by the split container layout engine.

## Alternatives Considered

### CEF (Chromium Embedded Framework) via cef-rs
- **Pros**: Full Chromium engine. Used by Electron, Spotify, many desktop apps. More control than WebView2
- **Cons**: Must ship ~150MB of Chromium binaries. cef-rs bindings are experimental. Complex multi-process architecture. Build system integration is painful (prebuilt CEF binaries + Rust FFI)
- **Why rejected**: Shipping 150MB of Chromium defeats the "lightweight native terminal" value proposition. WebView2 uses Edge already installed on the system — zero additional size. CEF's complexity is unnecessary when WebView2 provides the same capabilities

### Servo (Rust browser engine)
- **Pros**: Written in Rust. Could deeply integrate with wgpu pipeline
- **Cons**: Not production-ready for embedding. No stable embedding API. Missing many web platform features. Cannot run real-world web apps reliably
- **Why rejected**: Servo is a research project, not an embeddable browser. Cannot render production web apps. Would need years of work to reach WebView2's compatibility level

### webview2 crate (old, 0.1.4)
- **Pros**: Simpler high-level API
- **Cons**: Last update October 2021. 592 downloads/month. Depends on old `winapi 0.3` (not the modern `windows` crate). Incomplete API coverage. Abandoned
- **Why rejected**: Abandoned for 4+ years. Incompatible with the modern `windows 0.62` crate. `webview2-com` is its maintained successor with 100% API coverage and 1M+ downloads/month

## Consequences

### Positive
- Zero additional binary size — WebView2 runtime is pre-installed on Windows 10 20H2+ and all Windows 11
- Full Chromium compatibility: DevTools, JavaScript, modern web APIs, extensions
- 1M+ downloads/month (webview2-com) — used by Tauri framework, actively maintained
- 100% WebView2 COM API coverage — can implement full browser automation (click, fill, eval, screenshot, PDF)

### Negative (acknowledged trade-offs)
- Requires WebView2 Evergreen Runtime — not available on older Windows 10 builds (pre-20H2). Must detect and prompt for install
- COM-based API requires `unsafe` Rust code — mitigated by RAII wrappers with `Drop` implementation
- WebView2 runs in a separate process (Edge) — cross-process communication adds latency for JavaScript eval (~5-10ms)
- Separate child HWND means z-order management and focus coordination with the wgpu surface

### Mandatory impact dimensions
- **Security**: WebView2 inherits Edge's sandbox model (renderer process is sandboxed). JavaScript eval API must validate the caller is the IPC server — not arbitrary code execution. Cookies/storage are isolated per WebView2 environment
- **Cost**: $0. WebView2 is free. webview2-com is MIT licensed
- **Latency**: WebView2 initialization ~200-500ms (first browser pane). Subsequent panes share the environment (~50ms). JavaScript eval round-trip ~5-10ms (cross-process COM). URL navigation: page-dependent

## Revisit Triggers

- If WebView2 runtime adoption drops below 95% of Windows 10/11 installations, consider bundling the Evergreen Bootstrapper with the MSI installer
- If cross-process latency for JavaScript eval exceeds 50ms consistently, investigate WebView2's `CoreWebView2.CallDevToolsProtocolMethodAsync` for faster evaluation
- If wmux expands to Linux, evaluate WebKitGTK or CEF as the cross-platform browser backend (WebView2 is Windows-only)
