# Technical Architecture, wmux, Windows Terminal Multiplexer

> **Version**: 3.3 | **Status**: Accepted | **Owner**: wmux team | **Last updated**: 2026-04-19

## 1. Goals and Non-Goals

### Goals
- Reproduce the cmux experience on Windows, a native, GPU-accelerated terminal multiplexer optimized for AI agent workflows (Claude Code, Codex, OpenCode)
- Provide a single-window development environment: split panes, workspaces, integrated browser, CLI/IPC control, notifications
- Achieve ~95% protocol compatibility with cmux so existing AI agents work with minimal adaptation
- Ship as a free, open-source (MIT) desktop application for Windows 10 1809+

### Non-Goals (v1)
- macOS or Linux support, wmux v1 is Windows-only
- Plugin/extension system, no third-party API in v1
- Theme marketplace, themes loaded locally only
- Telemetry or analytics, no data collection in v1
- Full CJK IME support, basic via winit, improvements in v2
- Screen reader accessibility, basic support, incremental in v2
- Microsoft Store distribution, GitHub Releases, winget, Scoop only

### Quality Attributes
| Attribute | Target | Measurement |
|-----------|--------|-------------|
| Input-to-display latency | < 16ms (60fps) | Frame timing via `tracing` spans |
| Render frame budget | < 16ms | GPU profiling (PIX/RenderDoc) |
| Scrollback search | < 100ms on 4K lines | Benchmark test with `criterion` |
| IPC round-trip | < 5ms (Named Pipe local) | End-to-end latency measurement |
| Browser pane open | < 1s | Stopwatch from IPC command to ready |
| Notification delivery | < 2s from OSC/CLI event | Event timestamp delta |
| Session restore | < 3s for 10 workspaces | Startup profiling |
| Crash rate | < 1/week intensive use | User reports + panic handler logs |
| Memory (idle, 1 pane) | < 80MB RSS | Windows Task Manager |
| Memory (10 panes, 3 workspaces) | < 250MB RSS | Windows Task Manager |
| Binary size (wmux-app) | < 15MB (stripped release) | CI artifact size |
| Binary size (wmux-cli) | < 5MB (stripped release) | CI artifact size |

## 2. Stakeholders

| Role | Concern | How this doc serves them |
|------|---------|------------------------|
| AI Agent Developers | IPC protocol compatibility, CLI commands | §5 IPC component, §6 data flow, ADR-0005 |
| End Users (developers) | Performance, reliability, ease of use | §1 quality attributes, §11 failure modes |
| Contributors | Where to start, how crates fit together | §4 C4 diagrams, §5 components, §15 structure |
| Maintainers | Upgrade paths, dependency health | §3 stack, ADRs with revisit triggers |

## 3. Architecture Overview

**Project Type**: Native Windows desktop application (terminal multiplexer)
**Philosophy**: Rebuild cmux's architecture and protocol for Windows using Rust and the Windows platform ecosystem. Start simple (single-pane terminal), layer complexity incrementally (multiplexer, IPC, browser, polish). Prefer battle-tested crates over custom code. Optimize for the AI agent developer workflow.

**Stack Summary**:
| Layer | Choice | Version | Why |
|-------|--------|---------|-----|
| Language | Rust | 1.80+ (edition 2021) | Memory safety, terminal ecosystem (WezTerm, Alacritty, Rio), async (tokio) |
| GPU Rendering | wgpu | 28 | WebGPU to Direct3D 12 on Windows, cross-platform potential, used by WezTerm |
| Text Rendering | glyphon | 0.10 | Standard wgpu text renderer, built on cosmic-text/swash, used by COSMIC Terminal |
| Windowing | winit | 0.30 | Mature cross-platform abstraction over Win32, stable ApplicationHandler API |
| Terminal Parsing | vte | 0.13 | Alacritty's VT escape sequence parser, battle-tested |
| PTY | portable-pty | 0.9 | ConPTY abstraction from WezTerm project, handles Win10 1809+ |
| Async Runtime | tokio | 1.x | De facto Rust async runtime, full-featured (IO, timers, sync) |
| IPC | Named Pipes + JSON-RPC v2 | n/a | Windows equivalent of Unix sockets, cmux protocol compat |
| Browser | WebView2 via webview2-com | 0.39 | Chromium (Edge) pre-installed on Win10/11, full web compat |
| Win32 APIs | windows | 0.62 | Microsoft's official Rust bindings for DWM, Toast, COM |
| CLI Framework | clap | 4 | Standard Rust CLI parser, derive macros |
| Serialization | serde + serde_json | 1 | De facto serialization, JSON-RPC + persistence |
| Logging | tracing | 0.1 | Structured async-aware logging with span-based profiling |
| Error Handling | thiserror (libs) / anyhow (bins) | 2 / 1 | Typed errors in libraries, ergonomic propagation in binaries |
| SSH Daemon | Go (cmuxd-remote) | n/a | Already cross-platform, reused from cmux as-is |

### Cross-Cutting Concerns

| Concern | Approach | Details |
|---------|----------|---------|
| Error handling | `thiserror 2` (libs) / `anyhow 1` (bins) | Typed errors in library crates, `.context()` propagation in binaries. Never bare `unwrap()`, use `expect("reason")` for invariants |
| Logging | `tracing 0.1` structured spans | `RUST_LOG=wmux=debug`. Structured fields, no `println!`. Compatible with `tracy` profiler |
| Icons | Codicons SVG registry | GPU-rendered via `wmux-render/src/svg_icons.rs` + `icons.rs`, converted to glyphon `CustomGlyph`. No bitmap icon fonts. Colorized via theme |
| Custom title bar | GPU-rendered non-client area | `wmux-ui/src/titlebar.rs` uses `WM_NCCALCSIZE`/`WM_NCHITTEST` via `SetWindowSubclass`. Min/max/restore/close drawn as Codicons |
| Internationalization | Locale TOML files | `resources/locales/{en,fr}.toml`. English fallback. System language detection via `GetUserDefaultUILanguage` |
| Clipboard | `arboard 3` | `Ctrl+Shift+C` / `Ctrl+Shift+V` (avoids conflict with terminal `Ctrl+C` SIGINT) |
| Testing | `#[cfg(test)]` + `cargo clippy` | Zero warnings policy. CI gate: clippy, fmt, test, build. See `.claude/rules/testing.md` |

## 4. System Architecture (C4 Model)

C4 context, container, and component diagrams showing system boundaries, external dependencies, and internal component wiring.

> **Full content**: [system-diagrams.md](system-diagrams.md)

## 5. Component Breakdown

### wmux-core, Terminal State & Domain Model
- **Responsibility**: VTE parsing, cell grid, scrollback buffer, cursor/mode state, workspace/pane domain models, notification store, focus routing logic, command registry, sidebar metadata store (statuses/progress/logs), git branch detection, port scanner, session persistence helpers, remote workspace model, surface lifecycle
- **Technology**: vte 0.13, serde 1, thiserror 2, tracing 0.1
- **Interfaces**: Pure Rust library. Key public types: `AppState` actor (`app_state/{mod,actor,handle}.rs`), `PaneTree`, `PaneRegistry`, `SurfaceManager`, `WorkspaceManager`, `NotificationStore`, `MetadataStore`, `CommandRegistry`, `GitDetector`, `PortScanner`, `Session`. Consumed by wmux-render (grid data), wmux-ui (layout/focus), wmux-ipc (command handlers), wmux-app (wiring)
- **Why vte**: Alacritty's battle-tested VT parser. Zero-copy, state-machine based, handles malformed sequences gracefully
- **Design patterns**: State machine (terminal modes), Observer (dirty row flags for renderer), Actor pattern (AppState owns workspace tree, pane registry, focus state; mutations go through bounded channel commands)
- **Trade-off**: vte is lower-level than alternatives (no built-in grid). This requires manual grid/scrollback implementation, and gives full control needed for a terminal multiplexer

### wmux-pty, ConPTY Abstraction
- **Responsibility**: Spawn shell processes via ConPTY, manage I/O pipes, resize, shell detection, environment variable injection
- **Technology**: portable-pty 0.9, tokio (spawn_blocking for PTY reads)
- **Modules**: `manager.rs` (PtyManager spawn/resize), `actor.rs` (async I/O bridge), `conpty.rs` (ConPTY-specific helpers), `spawn.rs` (process spawn wrapper), `shell.rs` (pwsh, powershell, cmd detection)
- **Why portable-pty 0.9**: WezTerm's production-grade ConPTY wrapper. Handles Win10 1809+ quirks. v0.9 has latest fixes
- **Trade-off**: Depends on WezTerm's maintenance pace. Alternative `xpty` adds native async but is too immature (watching for v1.0)

### wmux-render, GPU Rendering Pipeline
- **Responsibility**: wgpu surface management, glyphon text atlas, terminal grid rendering (dirty rows to GPU upload), cursor rendering, UI chrome rendering (sidebar, overlays, titlebar), SVG icon rendering (Codicons), shadow pipeline, pane renderer with focus glow
- **Technology**: wgpu 28, glyphon 0.10, bytemuck 1, tracing 0.1
- **Modules**: `gpu.rs` (surface, device, queue), `text.rs` (GlyphonRenderer), `quad.rs` (QuadPipeline), `pane.rs` (per-pane terminal + chrome render), `terminal.rs` (grid rendering), `icons.rs` + `svg_icons.rs` (Codicons), `shadow.rs` + `shadow.wgsl` (drop shadow pipeline)
- **Why wgpu 28 + glyphon 0.10**: wgpu maps to D3D12 natively on Windows. glyphon is the standard wgpu text renderer (cosmic-text + swash + etagere under the hood). Staying on wgpu 28 because glyphon 0.10 depends on it. Upgrade both together when glyphon publishes a wgpu 29-compatible release
- **Design patterns**: Retained-mode rendering (cache glyph atlas across frames), dirty-flag updates (only upload changed rows)
- **Trade-off**: Custom renderer is more work than iced/egui, and necessary for 60fps terminal grid rendering. Validated by WezTerm, Rio, COSMIC Terminal

### wmux-ui, Window Management & Layout
- **Responsibility**: winit event loop integration, split pane layout engine, sidebar rendering (metadata badges, port pills, collapsed icon-only mode), custom title bar (GPU-rendered non-client area), status bar, command palette overlay (Ctrl+Shift+P), notification panel overlay (Ctrl+Shift+I), search overlay (Ctrl+F), address bar for browser panes, keyboard/mouse input dispatch, drag-and-drop, draggable dividers, central shortcut dispatcher, animation system, window effects (Mica/Acrylic, opaque fallback), Toast Service
- **Technology**: winit 0.30, wmux-render, wmux-core
- **Modules**: `window/{mod,event_loop,handlers,render}.rs` (App implementing winit `ApplicationHandler`, split across event dispatch, handler tables, and rendering), `sidebar.rs`, `titlebar.rs`, `status_bar.rs`, `command_palette.rs`, `notification_panel.rs`, `search.rs`, `address_bar.rs`, `shortcuts.rs` (keybinding table), `input.rs` (keyboard to PTY bytes), `mouse.rs`, `divider.rs` (draggable pane dividers), `effects.rs` (DWM backdrop), `animation.rs`, `toast.rs`, `typography.rs` (design tokens), `event.rs`, `error.rs`
- **Why winit 0.30**: Mature, stable Win32 abstraction. 0.30.x is the stable line (0.31 still beta). `ApplicationHandler` trait is the modern event loop API
- **Trade-off**: winit handles windowing but not UI widgets. All UI (sidebar, overlays, palette, titlebar, status bar) must be custom wgpu-rendered. More work, and no framework lock-in

### wmux-ipc, Named Pipes Server & JSON-RPC v2
- **Responsibility**: Named Pipe server (`\\.\pipe\wmux-*`), JSON-RPC v2 protocol (cmux-compatible), HMAC-SHA256 authentication, security modes, request routing, 80+ command handlers across five domain modules
- **Technology**: tokio (async Named Pipes), serde_json, windows 0.62 (pipe ACLs), thiserror 2
- **Modules**: `server.rs` (listener + per-client loop), `protocol.rs` (JSON-RPC codec), `auth.rs` (HMAC + child-process detection), `router.rs` (method dispatch), `handler.rs` (Handler trait), `handlers/{system,workspace,surface,sidebar,browser}.rs` (domain handlers; notifications route through `sidebar.rs` since the metadata store owns them)
- **Interfaces**: `IpcServer` actor (bounded channel + dedicated tokio task). Receives JSON-RPC requests, dispatches to `Handler` trait implementations, returns responses
- **Why Named Pipes + JSON-RPC v2**: Named Pipes are the Windows equivalent of Unix domain sockets. No port conflicts, ACL security, lower latency than TCP loopback. JSON-RPC v2 matches cmux protocol for AI agent compatibility
- **Design patterns**: Actor pattern (channel-based, not Arc<Mutex>), Handler trait (one impl per domain)
- **Trade-off**: Named Pipes are Windows-only. If wmux ever goes cross-platform, IPC layer needs an abstraction (Unix sockets on Linux/macOS)

### wmux-cli, CLI Client Binary
- **Responsibility**: `wmux.exe` CLI with 80+ commands (list, select, split, send, notify, browser automation, etc.), Named Pipe client, JSON-RPC request construction, human-readable and machine-readable output
- **Technology**: clap 4 (derive), serde_json, tokio (async pipe client), anyhow
- **Modules**: `main.rs` (entry + clap root), `client.rs` (Named Pipe connector), `output.rs` (human + `--json`), `commands/{system,workspace,surface,sidebar,browser,notify,ssh}.rs` (one module per domain; `browser` currently exposes 7 sub-commands out of 30+ IPC methods, `notify` and `ssh` remain stubs pending backlog items)
- **Interfaces**: Standalone binary. Connects to wmux-app via Named Pipe, sends one-shot JSON-RPC requests. Discoverable via `WMUX_SOCKET_PATH` env var
- **Why clap 4**: De facto Rust CLI framework. Derive macros for zero-boilerplate subcommands. Shell completions for free
- **Trade-off**: One-shot connections (connect, send, receive, disconnect) add connection overhead per command, and simplify state management

### wmux-browser, WebView2 Integration
- **Responsibility**: WebView2 COM initialization (RAII wrappers), child HWND management, URL navigation, JavaScript evaluation, DevTools (F12 handler wiring pending backlog), screenshot/PDF, cookie/storage control, show/hide on workspace switch, accessibility tree snapshot, 30+ automation methods
- **Technology**: webview2-com 0.39, windows 0.62, raw-window-handle
- **Modules**: `manager.rs` (BrowserManager, COM + environment lifecycle), `com.rs` (ComGuard RAII wrapper), `automation/{mod,dom,inspect,navigation}.rs` (DOM interaction, accessibility snapshot, navigation + JS eval), `panel/{mod,attach,delegation,layout}.rs` (child HWND attach, input delegation, size/visibility layout), `error.rs`
- **Why webview2-com 0.39**: 1M+ downloads/month, actively maintained, used by Tauri. Exposes 100% of WebView2 COM API. The older `webview2` crate (0.1.4) is abandoned
- **Design patterns**: Separate child HWND (NEVER inside wgpu surface), RAII Drop for COM cleanup
- **Trade-off**: WebView2 runtime must be installed (pre-installed on Win10 20H2+ and all Win11). Older Win10 builds need the Evergreen Bootstrapper

### wmux-config, Configuration Parsing
- **Responsibility**: Ghostty-compatible config file parsing (`key = value` format), theme loading (8 bundled themes including `stitch-blue`), font configuration (wired inside `config.rs`), dark/light mode detection, locale detection, default config generation
- **Technology**: toml 0.8, serde 1, dirs 6, windows 0.62 (registry for dark/light mode)
- **Modules**: `config.rs` (Config struct with validation, includes font and keybindings as HashMaps), `parser.rs` (Ghostty-compat parser), `theme/{mod,chrome,registry,types}.rs` (ThemeEngine, chrome color tokens, theme registry, typed palettes), `locale.rs` (locale detection + TOML string loading), `error.rs`
- **Why Ghostty-compatible format**: Reuse 50+ existing Ghostty themes. Familiar to cmux users
- **Trade-off**: Not standard TOML semantics. Ghostty uses `key = value` without sections. Requires custom parser layer on top of TOML

### wmux-app, Main Application Binary
- **Responsibility**: Entry point. Wires all crates together: initializes tracing, loads config, starts IPC server, creates window, runs event loop. Graceful shutdown coordination. Hosts the hardened auto-updater (SHA-256 digest verification, HTTPS + host allowlist, 200MB cap, atomic install)
- **Technology**: anyhow, tracing-subscriber, tokio, reqwest, semver, all internal crates
- **Modules**: `main.rs` (entry + `App::run()`), `updater.rs` (UpdateChecker with semver + GitHub Releases)
- **Interfaces**: `main()` to `App::run()`. Owns the tokio runtime and winit event loop
- **Design pattern**: Composition root, no business logic, only wiring

### wmuxd-remote, SSH Remote Daemon (Go)
- **Responsibility**: Runs on remote machines. Manages durable remote sessions, PTY relay, browser proxy (SOCKS5/HTTP CONNECT), CLI relay (reverse TCP forward), multi-client resize coordination
- **Technology**: Go (reused from cmux, already cross-platform)
- **Interfaces**: Bootstrapped by `wmux ssh` command. Communicates with wmux-app over SSH tunnel
- **Why reuse Go daemon**: Already works on Linux/macOS/Windows. ~3K lines. Rewriting in Rust would add months with no functional benefit
- **Status**: Directory not yet in the repo. Planned integration from the cmux source tree as part of Backlog item #7
- **Trade-off**: Two languages in the project (Rust + Go). Go daemon compiled separately, bundled as a binary resource

> **See also**: [Feature Dependency Map](dependency-map.md) for the full component tree with sub-components and node types, [Inter-Component Relations](component-relations.md) for the exhaustive dependency/event table, and [Critical Files per Feature](feature-files.md) for file-level impact mapping.

## 6. Data Architecture · 7. Security · 8. Observability

Data model (JSON session, TOML config, in-memory grid), terminal I/O and IPC data flows, session persistence schema, sidebar metadata model, IPC security modes (wmux-only/password/allowAll/off), HMAC-SHA256 auth flow, and tracing/profiling setup.

> **Full content**: [data-architecture.md](data-architecture.md)

## 9. Infrastructure & Distribution

- **Platform**: Windows 10 1809+ (ConPTY requirement). Windows 11 for Mica/Acrylic effects (opaque fallback on Win10)
- **Build**: `cargo build --release` with LTO, single codegen unit, symbols stripped, panic=abort
- **CI/CD**: GitHub Actions (windows-latest runner). Steps: clippy, fmt, test, build, package
- **Distribution**:
  - MSI installer (via WiX or cargo-wix)
  - winget manifest (Microsoft package manager)
  - Scoop bucket (developer-friendly)
  - Portable .zip (no install required)
- **Auto-update**: GitHub Releases API poll (background, hourly). SHA-256 digest verified before install. HTTPS-only with host allowlist. 200MB download cap. Staged to temp. Notification in title bar. User-initiated install

## 10. Failure Modes & Resilience

| Failure scenario | Impact | Degradation behavior | Recovery strategy |
|-----------------|--------|---------------------|-------------------|
| ConPTY spawn fails | Single pane broken | Error message in pane area, other panes unaffected | Retry with fallback shell (cmd.exe) |
| GPU adapter unavailable | App cannot start | Log error, show Win32 MessageBox with system requirements | User must update GPU drivers or use software renderer |
| wgpu surface lost (Alt+Tab, sleep) | Frame glitch | Skip frame, reconfigure surface on next redraw | Automatic via wgpu `SurfaceError::Lost` handling |
| WebView2 runtime missing | Browser panes unavailable | Terminal panes work normally. Browser commands return clear error | Prompt user to install Edge WebView2 Evergreen Runtime |
| Named Pipe server bind fails | No IPC/CLI | App runs standalone without programmatic control | Retry with unique pipe name (wmux-{pid}), log warning |
| Session file corrupt | Session not restored | Log warning, start fresh session | Auto-save overwrites corrupt file within 8 seconds |
| PTY process crash (shell exit) | Single pane shows exit code | Display "[Process exited with code N]", pane stays open | User closes or respawns shell in same pane |
| SSH connection drop | Remote workspace frozen | Sidebar shows disconnect icon, auto-reconnect with backoff | Reconnect restores session from remote daemon state |
| Out of memory (massive scrollback) | OOM risk | Scrollback hard-capped at 4K lines / 400K chars per terminal | Oldest lines evicted from ring buffer. Config to reduce limit |
| DWM compositor disabled | Mica/Acrylic broken | Feature-detect, fallback to opaque background | Automatic, no user action needed |
| Auto-update digest mismatch | Update not applied | Log warning, discard staged binary, retry on next poll | User can force check via IPC once patched upstream |

## 11. Architecture Decision Records

ADRs are stored as separate files in `decisions/`. Each follows the MADR template.

| ADR | Title | Status | Confidence |
|-----|-------|--------|------------|
| [ADR-0001](decisions/0001-language-rust.md) | Language: Rust | Accepted | High |
| [ADR-0002](decisions/0002-gpu-rendering-custom-wgpu.md) | GPU Rendering: Custom wgpu pipeline (not iced/egui) | Accepted | High |
| [ADR-0003](decisions/0003-text-rendering-glyphon.md) | Text Rendering: glyphon 0.10 | Accepted | Medium |
| [ADR-0004](decisions/0004-pty-backend-portable-pty.md) | PTY Backend: portable-pty (ConPTY) | Accepted | High |
| [ADR-0005](decisions/0005-ipc-named-pipes-jsonrpc.md) | IPC: Named Pipes + JSON-RPC v2 | Accepted | High |
| [ADR-0006](decisions/0006-browser-webview2.md) | Browser: WebView2 via webview2-com | Accepted | High |
| [ADR-0007](decisions/0007-windowing-winit.md) | Windowing: winit 0.30 | Accepted | Medium |
| [ADR-0008](decisions/0008-async-actor-pattern.md) | Async Architecture: Actor pattern via bounded channels | Accepted | High |
| [ADR-0009](decisions/0009-session-persistence-json.md) | Session Persistence: JSON file with auto-save | Accepted | High |
| [ADR-0010](decisions/0010-config-format-ghostty.md) | Config Format: Ghostty-compatible key-value | Accepted | Medium |

## 12. Feature Dependency Map

Dependency-centric view of every component and PRD feature organized by crate, with sub-component decomposition and cross-crate dependency links.

> **Full content**: [dependency-map.md](dependency-map.md)

## 13. Inter-Component Relations

Exhaustive dependency, data flow, and event trigger tables across 12 categories (Terminal I/O, OSC events, multiplexer layout, IPC/CLI, browser, notifications, config/themes, session persistence, shell/git, auto-update, visual effects, crate dependencies).

> **Full content**: [component-relations.md](component-relations.md)

## 14. Critical Files per Feature

Maps each PRD feature to its implementing source files with implementation status tracking.

> **Full content**: [feature-files.md](feature-files.md)

## 15. Project Structure (Target)

> **Note**: Implementation status as of 2026-04-19: all 9 Rust crates are implemented. 48 of 50 specs delivered (96%). Only Wave 11 (`L2_16` CLI Domain Commands) and Wave 12 (`L4_07` Packaging) remain. See `docs/Backlog.md` for the seven visible tech-debt items (Inter font, progress bar UI, keybindings wiring, F12 DevTools handler, CLI browser/notify completion, SSH daemon Go). See [Critical Files per Feature](feature-files.md) for file-level status per PRD feature.

```text
wmux/
├── Cargo.toml                    # Workspace root (9 crates)
├── Cargo.lock
├── CLAUDE.md                     # Claude Code project instructions
├── CHANGELOG.md
├── docs/
│   ├── PRD.md                    # Product requirements (16 features)
│   ├── Backlog.md                # Post-audit 2026-04-19 (15 done + 7 remaining)
│   ├── brief-fonctionnel.md
│   ├── DISTRIBUTION.md
│   └── architecture/
│       ├── ARCHITECTURE.md       # This document (spine)
│       ├── INDEX.md              # Compact index for context import
│       ├── system-diagrams.md    # C4 context/container/component diagrams
│       ├── data-architecture.md  # Data model, flows, security, observability
│       ├── dependency-map.md     # Feature dependency tree by crate
│       ├── component-relations.md # Inter-component relation tables
│       ├── feature-files.md      # PRD feature, source file mapping
│       ├── decisions/            # 10 ADR files (MADR format)
│       └── glossary.md           # Domain & technical terms
├── specs/                        # 50 implementation task specs in 12 waves
│   ├── README.md                 # Task overview + wave-based dependency map
│   └── L{0-4}_*.md               # L0=scaffold, L1=foundation, L2=core, L3=integration, L4=polish
├── resources/
│   ├── locales/                  # i18n strings (en.toml, fr.toml)
│   ├── themes/                   # 8 bundled Ghostty themes (.conf)
│   └── shell-integration/        # wmux.{ps1,bash,zsh} hook scripts
├── wmux-core/                    # Terminal state, VTE, grid, scrollback, domain models, metadata
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── cell.rs               # Cell struct (char + attributes)
│       ├── color.rs              # Color model: Named/Indexed/Rgb
│       ├── cursor.rs             # CursorShape + CursorState
│       ├── mode.rs               # TerminalMode bitflags
│       ├── types.rs              # Domain IDs (PaneId, SurfaceId, etc.)
│       ├── surface.rs            # SplitDirection, PanelKind, SurfaceInfo
│       ├── surface_manager.rs    # Surface lifecycle (create, close, list)
│       ├── error.rs              # CoreError enum
│       ├── terminal.rs           # Terminal state machine
│       ├── grid.rs               # Cell grid (contiguous Vec<Cell> per row)
│       ├── scrollback.rs         # Ring buffer (VecDeque)
│       ├── vte_handler.rs        # vte::Perform implementation
│       ├── event.rs              # TerminalEvent, Hyperlink, PromptMark types
│       ├── selection.rs          # Selection model (Normal/Word/Line)
│       ├── app_state/            # AppState actor (split into 3 files)
│       │   ├── mod.rs
│       │   ├── actor.rs          # Command dispatch loop
│       │   └── handle.rs         # Public handle for IPC/UI clients
│       ├── pane_registry.rs      # PaneRegistry (PaneId -> PaneState)
│       ├── pane_tree.rs          # Binary split tree
│       ├── rect.rs               # Rect geometry + split utilities
│       ├── workspace.rs          # Workspace model
│       ├── workspace_manager.rs  # Workspace lifecycle
│       ├── notification.rs       # NotificationStore
│       ├── metadata_store.rs     # Sidebar statuses/progress/logs + PID sweep
│       ├── command_registry.rs   # Action catalog for Command Palette
│       ├── git_detector.rs       # Branch + dirty state detection
│       ├── port_scanner.rs       # Listening port detection per workspace
│       ├── remote.rs             # Remote workspace model (SSH)
│       └── session.rs            # Session save/load helpers
├── wmux-pty/                     # ConPTY abstraction
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── manager.rs            # PtyManager (spawn, I/O, resize)
│       ├── actor.rs              # PtyActorHandle (async I/O bridge)
│       ├── conpty.rs             # ConPTY-specific helpers
│       ├── spawn.rs              # Process spawn wrapper
│       ├── shell.rs              # Shell detection (pwsh, powershell, cmd)
│       └── error.rs
├── wmux-render/                  # GPU rendering pipeline
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── gpu.rs                # GpuContext (wgpu surface, device, queue)
│       ├── text.rs               # GlyphonRenderer (text atlas, buffer, render)
│       ├── quad.rs               # QuadPipeline (colored rectangles)
│       ├── pane.rs               # Per-pane terminal + chrome renderer with focus glow
│       ├── terminal.rs           # Grid rendering (dirty rows)
│       ├── icons.rs              # Codicons registry
│       ├── svg_icons.rs          # SVG to glyphon CustomGlyph conversion
│       ├── shader.wgsl           # WGSL shaders for quads
│       ├── shadow.rs             # Drop shadow pipeline
│       ├── shadow.wgsl           # Shadow shader
│       └── error.rs              # RenderError enum
├── wmux-ui/                      # Window management, layout, input, overlays
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── window/               # winit App (split into 4 files)
│       │   ├── mod.rs            # UiState + App composition root
│       │   ├── event_loop.rs     # winit ApplicationHandler impl
│       │   ├── handlers.rs       # Shortcut + action handler tables
│       │   └── render.rs         # Per-frame render orchestration
│       ├── input.rs              # Keyboard input to VT byte sequences
│       ├── mouse.rs              # Selection, click, scroll, SGR reporting
│       ├── shortcuts.rs          # Keybinding table (shortcut dispatcher)
│       ├── event.rs              # WmuxEvent enum
│       ├── sidebar.rs            # Workspace list + metadata + port pills + collapsed mode
│       ├── titlebar.rs           # Custom GPU title bar (WM_NCCALCSIZE)
│       ├── status_bar.rs         # Bottom info strip
│       ├── divider.rs            # Draggable pane dividers
│       ├── command_palette.rs    # Ctrl+Shift+P overlay with fuzzy search
│       ├── notification_panel.rs # Ctrl+Shift+I notification list + badge
│       ├── search.rs             # Ctrl+F in-pane search
│       ├── address_bar.rs        # Browser pane URL bar
│       ├── effects.rs            # Mica/Acrylic DWM backdrop
│       ├── animation.rs          # Shared animation helpers
│       ├── typography.rs         # Design tokens (sizes)
│       ├── toast.rs              # Windows Toast via WinRT
│       └── error.rs              # UiError enum
├── wmux-ipc/                     # Named Pipes server, JSON-RPC
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── server.rs             # Named Pipe server (tokio async)
│       ├── protocol.rs           # JSON-RPC v2 codec
│       ├── auth.rs               # HMAC-SHA256 authentication + security modes
│       ├── router.rs             # Method dispatch
│       ├── handler.rs            # Handler trait
│       ├── error.rs              # IpcError + RpcErrorCode
│       └── handlers/             # Five domain modules
│           ├── mod.rs
│           ├── system.rs         # system.* (ping, capabilities, identify, tree)
│           ├── workspace.rs      # workspace.* (list, create, select, close, rename)
│           ├── surface.rs        # surface.* + input/read (split, send_text, read_text)
│           ├── sidebar.rs        # sidebar.* + notification.* (metadata store owns notifications)
│           └── browser.rs        # browser.* (30+ automation methods)
├── wmux-cli/                     # CLI client binary
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs               # clap root + entry
│       ├── client.rs             # Named Pipe client
│       ├── output.rs             # Human + --json formatter
│       └── commands/             # One module per domain
│           ├── mod.rs
│           ├── system.rs
│           ├── workspace.rs
│           ├── surface.rs
│           ├── sidebar.rs
│           ├── browser.rs        # 7 sub-commands (Open/Navigate/Back/Forward/Reload/Url/Eval). 23 IPC methods not yet exposed (Backlog #5)
│           ├── notify.rs         # Stub returning InternalError (Backlog #6)
│           └── ssh.rs            # Stub returning "not yet fully implemented" (Backlog #7)
├── wmux-browser/                 # WebView2 integration
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── com.rs                # ComGuard RAII wrapper for COM STA
│       ├── manager.rs            # BrowserManager (environment + lifecycle)
│       ├── automation/           # 30+ automation methods (split into 4 files)
│       │   ├── mod.rs
│       │   ├── dom.rs            # click, fill, type, select, check, hover, focus
│       │   ├── inspect.rs        # snapshot (a11y tree), get, is, find, wait
│       │   └── navigation.rs     # navigate, back, forward, reload, eval, screenshot
│       ├── panel/                # Child HWND management (split into 4 files)
│       │   ├── mod.rs
│       │   ├── attach.rs         # HWND attach + parent wiring
│       │   ├── delegation.rs     # Input delegation to WebView2
│       │   └── layout.rs         # Bounds/visibility + DevTools open
│       └── error.rs
├── wmux-config/                  # Configuration parsing
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── config.rs             # Config struct (font, keybindings, scrollback, bounds)
│       ├── parser.rs             # Ghostty-compat key=value parser
│       ├── theme/                # Theme engine (split into 4 files)
│       │   ├── mod.rs            # ThemeEngine public API
│       │   ├── chrome.rs         # Chrome color tokens (sidebar, titlebar, overlays)
│       │   ├── registry.rs       # Theme registry (8 bundled + user)
│       │   └── types.rs          # Typed palettes
│       ├── locale.rs             # Locale detection + TOML string loading
│       └── error.rs
├── wmux-app/                     # Main application binary
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs               # Entry point (wiring only)
│       └── updater.rs            # Hardened auto-updater (SHA-256 + HTTPS allowlist + 200MB cap)
└── daemon/                       # Go SSH remote daemon (planned, from cmux source tree)
    └── remote/                   # Integration pending Backlog item #7
        └── cmd/wmuxd-remote/
```

## 16. Implementation Status (Waves)

> **See also**: [Feature Dependency Map](dependency-map.md) for the full crate/component tree, and `specs/README.md` for the complete wave-indexed task list. Execution is wave-based: all tasks within a wave run in parallel, waves execute sequentially.

Progress as of 2026-04-19: **48/50 specs delivered (96%)**. Waves 0 through 10 complete, Wave 11 and Wave 12 remain.

### Waves 0-5, Scaffold and Foundation, COMPLETE
Error types, tracing, domain model types, QuadPipeline, ConPTY spawn/async I/O, WebView2 COM init, Ghostty config parser, auto-update, terminal cell grid, browser navigation + JS eval, theme engine + dark/light, localization FR/EN, VTE parser integration, scrollback ring buffer, terminal grid GPU rendering, keyboard input to PTY dispatch, mouse selection + copy/paste, browser DOM automation, shell integration hooks, OSC sequence handlers, single-pane terminal integration, notification store + OSC detection.

**Milestone reached**: functional single-pane terminal with full VT parsing, scrollback, mouse, and theme support.

### Waves 6-8, Multiplexer + IPC Backbone, COMPLETE
AppState actor + multi-pane architecture, Windows Toast notifications, PaneTree binary split layout, Named Pipes server + JSON-RPC v2, focus routing + keyboard shortcuts, multi-pane GPU rendering, surface tab system, workspace lifecycle, IPC authentication, IPC Handler/Router, CLI client foundation, WebView2 browser panel.

**Milestone reached**: native terminal multiplexer (tmux-like) with functional IPC and CLI bootstrap.

### Wave 9, Sidebar + Domain Handlers + Session + Browser, COMPLETE
Draggable dividers + pane resize, sidebar UI rendering, workspace/surface IPC handlers, input/read IPC handlers, session auto-save (8s interval), browser IPC handlers, terminal search (Ctrl+F), SSH remote scaffolding.

### Wave 10, Metadata + Session Restore + Polish, COMPLETE
Sidebar metadata store + IPC (statuses, progress, logs), session restore, notification visual indicators, git branch + port detection, command palette (Ctrl+Shift+P), Mica/Acrylic effects.

**Milestone reached**: ~95% feature parity with cmux plus AI-agent IPC surface, auto-update, custom title bar, localized UI.

### Wave 11, CLI Domain Commands, PENDING (`L2_16`)
Finish CLI coverage across workspace/surface/sidebar/browser/notify domains. Current gap: `wmux-cli/src/commands/browser.rs` exposes 7 sub-commands of 30+ IPC methods (Backlog #5), `notify.rs` is a stub (Backlog #6).

### Wave 12, Packaging + Distribution, PENDING (`L4_07`)
`.ico` asset, `app.manifest`, `build.rs`, GitHub Actions CI, MSI installer via cargo-wix, winget + Scoop manifests, portable .zip. Last barrier before the first public GitHub Releases drop.

## 17. Risks & Mitigations

| Risk | Impact | Likelihood | Mitigation |
|------|--------|-----------|------------|
| GPU text rendering complexity (glyph atlas, ligatures, emoji) | High | Medium | Study WezTerm (MIT) and Rio (MIT) renderers. glyphon handles atlas management. Start with ASCII-only, add Unicode incrementally |
| ConPTY edge cases (resize races, escape sequence differences vs Unix PTY) | Medium | Medium | portable-pty 0.9 handles known quirks. Extensive VTE conformance testing against vttest |
| WebView2 + wgpu HWND coordination (z-order, focus, resize sync) | Medium | Medium | Separate child HWND architecture avoids compositing conflicts. Tauri validates this approach |
| winit IME support on Windows (CJK input) | Medium | Medium | winit 0.30.x has partial IME via TSF. Defer full CJK to v2. Basic Latin input works |
| wgpu/glyphon version coupling | Low | High | Pin both versions together. Upgrade only when glyphon publishes matching release |
| cmux protocol drift (cmux evolves, wmux must track) | Medium | Medium | Abstract protocol layer. Track cmux releases. Maintain compatibility test suite |
| Single-developer bus factor | High | Medium | MIT license, thorough documentation, clean crate boundaries for contributor onboarding |
| WebView2 runtime not installed (old Win10) | Low | Low | Detect at startup, show install prompt. Browser features degrade gracefully |
| Auto-update supply chain | High | Low | SHA-256 digest verification, HTTPS-only with host allowlist, 200MB cap, atomic install, signed releases planned |

## 18. Maintenance & Change Management

- **Documentation ownership**: wmux core team. Architecture docs live in `docs/architecture/` and are versioned with the code. See [INDEX.md](INDEX.md) for the document map
- **Review cadence**: Quarterly review, or when a major feature changes the architecture
- **Change process**: New architectural decision gets a draft ADR (Proposed), then PR review, then merge (Accepted). Update main ARCHITECTURE.md to reflect new ADR. Keep sub-files ([system-diagrams.md](system-diagrams.md), [data-architecture.md](data-architecture.md), [dependency-map.md](dependency-map.md), [component-relations.md](component-relations.md), [feature-files.md](feature-files.md)) in sync
- **Dependency policy**: Patch updates freely. Minor/major updates require testing + CHANGELOG entry. Quarterly dependency audit (`cargo audit`, `cargo outdated`)
- **Testing strategy**: Unit tests in `#[cfg(test)]` modules. Zero clippy warnings policy. CI gate: clippy, fmt, test, build. Detailed rules in `.claude/rules/testing.md`
- **Versioning**: SemVer. Pre-1.0 breaking changes allowed between minor versions. CHANGELOG.md updated with every change
