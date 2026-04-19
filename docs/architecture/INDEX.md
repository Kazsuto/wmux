# wmux Architecture Index

> Spine version 3.3, last updated 2026-04-19. See [ARCHITECTURE.md](ARCHITECTURE.md) for full context.

Native Windows terminal multiplexer in Rust. GPU-accelerated, split panes, workspaces, integrated browser (WebView2), CLI/IPC for AI agents.

## Stack

| Layer | Choice | Version |
|-------|--------|---------|
| Language | Rust | 1.80+ (edition 2021) |
| GPU Rendering | wgpu to Direct3D 12 | 28 |
| Text Rendering | glyphon (cosmic-text + swash) | 0.10 |
| Windowing | winit | 0.30 |
| Terminal Parsing | vte (Alacritty's parser) | 0.13 |
| PTY | portable-pty (ConPTY) | 0.9 |
| Async Runtime | tokio | 1.x |
| IPC | Named Pipes + JSON-RPC v2 | — |
| Browser | WebView2 via webview2-com | 0.39 |
| Win32 APIs | windows crate | 0.62 |
| CLI | clap (derive) | 4 |
| SSH Daemon | Go (cmuxd-remote, reused from cmux) | — |

## Crates

| Crate | Responsibility |
|-------|---------------|
| wmux-core | Terminal state, VTE parsing, cell grid, scrollback, domain models, focus routing |
| wmux-pty | ConPTY spawn/resize/I/O via portable-pty, shell detection |
| wmux-render | wgpu surface, glyphon text atlas, QuadPipeline, dirty-row rendering |
| wmux-ui | winit event loop, split pane layout, sidebar, overlays, input dispatch |
| wmux-ipc | Named Pipes server, JSON-RPC v2 protocol, auth, 80+ command handlers |
| wmux-cli | CLI binary (clap), Named Pipe client, human + JSON output |
| wmux-browser | WebView2 COM lifecycle, child HWND, automation API |
| wmux-config | Ghostty-compat config parser, theme engine, locale detection |
| wmux-app | Entry point. Wires all crates, starts IPC, runs event loop |
| wmuxd-remote | Go SSH daemon (planned). PTY relay, browser proxy, CLI relay (reused from cmux) |

## Architecture Documents

| Document | Content | When to read |
|----------|---------|--------------|
| [ARCHITECTURE.md](ARCHITECTURE.md) | Goals, overview, stack, components, ADRs, roadmap, risks | Starting point for any architectural question |
| [system-diagrams.md](system-diagrams.md) | C4 context, container, and component diagrams (Mermaid) | Understanding system boundaries and data flow |
| [data-architecture.md](data-architecture.md) | Data model, IPC/terminal data flows, session schema, security, observability | Working on data, IPC, persistence, or security |
| [dependency-map.md](dependency-map.md) | Full crate/component tree with node types and sub-components | Understanding crate ownership and feature decomposition |
| [component-relations.md](component-relations.md) | 12 relation tables covering all inter-component dependencies | Assessing blast radius of changes |
| [feature-files.md](feature-files.md) | PRD feature → source file mapping with implementation status | Starting work on a specific PRD feature |
| [decisions/](decisions/) | 10 ADRs (MADR format): language, GPU, text, PTY, IPC, browser, windowing, async, persistence, config | Revisiting or challenging an architectural decision |
| [glossary.md](glossary.md) | 25 domain and technical terms | Clarifying project-specific terminology |
