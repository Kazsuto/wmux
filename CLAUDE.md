# wmux — Windows Terminal Multiplexer

Native Windows terminal multiplexer in Rust, inspired by cmux (macOS). GPU-accelerated terminal with split panes, workspaces, integrated browser (WebView2), and CLI/IPC for AI agents.

## Tech Stack
- Rust (Cargo workspace: wmux-core, wmux-pty, wmux-render, wmux-ui, wmux-ipc, wmux-cli, wmux-app, wmux-browser, wmux-config)
- wgpu (Direct3D 12) for GPU text rendering
- ConPTY via portable-pty
- VTE parsing via vte crate
- WebView2 for integrated browser
- Named Pipes + JSON-RPC v2 for IPC (cmux-compatible protocol)
- tokio for async runtime
- Go for SSH remote daemon (reused from cmux)

## Commands
- `cargo build` - Build all workspace crates
- `cargo run -p wmux-app` - Run main application
- `cargo run -p wmux-cli` - Run CLI client
- `cargo test` - Run all tests

## Important Files
- `PRD.md` - Product requirements (14 features, personas, success metrics)
- `ARCHITECTURE.md` - Full architecture doc with cmux analysis, tech stack, IPC protocol spec
- `specs/README.md` - 29 implementation tasks with dependency map

## Rules
See `.claude/rules/` for detailed rules (10 files). Key constraints:
- Windows 10 1809+ minimum. Mica/Acrylic Win11 only with opaque fallback
- cmux-compatible JSON-RPC v2 over Named Pipes (NEVER TCP)
- Custom wgpu renderer only (NEVER iced/egui for terminal grid)
- WebView2 in separate child HWND (NEVER inside wgpu surface)
- wgpu 28 + glyphon 0.10 — see `.claude/rules/rendering.md` for version-specific API gotchas
