# wmux — Windows Terminal Multiplexer

Native Windows terminal multiplexer in Rust — GPU-accelerated, split panes, workspaces, integrated browser (WebView2), CLI/IPC for AI agents.

@docs/architecture/INDEX.md

## Commands

### Development
- `cargo run -p wmux-app` — Run main application
- `cargo run -p wmux-cli` — Run CLI client
- `cargo build --workspace` — Build all crates

### Quality (run before every commit)
- `cargo clippy --workspace -- -W clippy::all` — Lint (zero warnings policy)
- `cargo fmt --all` — Format code
- `cargo test --workspace` — Run all unit tests
- `cargo test --workspace -- --ignored` — Run ignored tests (require GPU/PTY)

### Single Crate
- `cargo test -p wmux-core` — Test one crate
- `cargo clippy -p wmux-render` — Lint one crate

## Key Documents

| Document | When to read |
|----------|--------------|
| `docs/PRD.md` | Understanding product requirements (16 features) |
| `specs/README.md` | Finding implementation task specs (50 specs with dependency map) |
| `docs/architecture/feature-files.md` | Starting work on a specific PRD feature |
| `docs/architecture/component-relations.md` | Assessing blast radius of changes |

## Critical Rules (NEVER violate)

Detailed rules in `.claude/rules/` (10 path-scoped files, loaded automatically). Below are the highest-priority constraints that apply everywhere.

### Platform
- **NEVER** use TCP for IPC — Named Pipes only (`\\.\pipe\wmux-*`)
- **NEVER** skip Win10 1809+ fallback — Mica/Acrylic are Win11-only, ALWAYS fallback to opaque
- **NEVER** use MessageBox or balloon tips — WinRT Toast Notification API only

### Rendering
- **NEVER** use iced/egui for terminal grid — custom wgpu renderer only
- **NEVER** place WebView2 inside the wgpu surface — separate child HWND always
- wgpu 28 + glyphon 0.10 have breaking API changes from prior versions — **read `.claude/rules/rendering.md` before touching render code**

### Architecture
- **NEVER** expose `anyhow::Error` in library crate public APIs — use `thiserror` v2
- **NEVER** block tokio with `std::thread::sleep` or `std::fs::*` — use async equivalents
- **NEVER** use unbounded channels — always bounded `mpsc::channel(N)`
- **NEVER** hardcode user-visible strings — all UI text goes through i18n system
- **NEVER** use `println!`/`eprintln!` in library crates — `tracing` crate only

### IPC & Security
- Method names MUST match cmux: `workspace.list`, `surface.send_text`, etc.
- JSON-RPC v2 with newline-delimited messages
- **NEVER** log auth secrets or HMAC tokens — even in debug mode

### Terminal
- **NEVER** write a custom VTE parser — use the `vte` crate
- **NEVER** panic on malformed escape sequences — silently discard
- Grid cells stored contiguously (`Vec<Cell>`) — NEVER `Vec<Vec<Cell>>`

## Conventions

### Error Handling
- Library crates: `thiserror` v2 for typed error enums
- Binary crates (wmux-app, wmux-cli): `anyhow::Result` with `.context()`

### Logging
- `tracing` crate with structured fields: `tracing::info!(workspace_id = %id, "workspace created")`
- Span-based tracing for render loop and IPC request handling

### Unsafe Code
- Every `unsafe` block requires a `// SAFETY:` comment
- Wrap all Win32/COM FFI in RAII safe abstractions with `Drop`

### Testing
- Unit tests in `#[cfg(test)]` modules within source files
- Integration tests in `tests/` directories
- `#[ignore]` for tests requiring real PTY or GPU

## Workflow

### Before Starting a Feature
1. Read `docs/architecture/feature-files.md` to find related specs and files
2. Read the relevant spec in `specs/` for requirements and acceptance criteria
3. Read existing code in the target crate(s) to understand patterns

### After Every Code Change
1. `cargo clippy --workspace -- -W clippy::all` (zero warnings)
2. `cargo fmt --all`
3. `cargo test --workspace`
4. **Update CHANGELOG.md** (only for application code changes — skip docs/specs/architecture/config-only changes)

## Rules Files Reference

Detailed domain rules in `.claude/rules/` (loaded automatically by path scope):

| File | Scope | When critical |
|------|-------|---------------|
| `rust-architecture.md` | `**/*.rs` | Crate boundaries, async, memory, unsafe, logging |
| `rendering.md` | `wmux-render/`, `wmux-ui/` | **Read before touching render code** — wgpu 28 / glyphon 0.10 API gotchas |
| `ipc-protocol.md` | `wmux-ipc/`, `wmux-cli/` | JSON-RPC v2 format, security modes, CLI conventions |
| `windows-platform.md` | `**/*.rs` | Win10/11 compat, Named Pipes, WebView2, ConPTY |
| `testing.md` | `**/*.rs` | Testing strategy, clippy zero-warnings policy |
| `terminal-vte.md` | `wmux-core/` | VTE parsing, grid/scrollback, cursor modes |
| `notifications.md` | `wmux-core/`, `wmux-ui/` | OSC detection, Toast API, visual indicators |
| `persistence.md` | `wmux-core/`, `wmux-config/` | Session save/load, config format, themes |
| `localization.md` | `**/*.rs`, `resources/locales/` | i18n system, locale files, string key conventions |
| `changelog.md` | *(always active)* | Changelog update after every code change |
