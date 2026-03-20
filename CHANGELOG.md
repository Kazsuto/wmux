# Changelog

## 2026-03-20

REFACTOR: Replace Cell.grapheme String with CompactString (compact_str v0.8) — eliminate heap allocation for graphemes ≤24 bytes, improve cache locality for terminal grid
REFACTOR: Replace CoreError::General(String) catch-all with domain-specific variants — OutOfBounds, InvalidScrollRegion, InvalidConfig
CHORE: Add Eq derive to Cell and SurfaceInfo (all fields are Eq)
CHORE: Add Hash derive to CursorState (all fields are Hash)
CHORE: Add #[must_use] to ID new() and from_uuid() constructors in define_id! macro

CHORE: Update ARCHITECTURE.md v3.0→v3.1 — add 5 missing wmux-core files to project structure tree (color.rs, cursor.rs, mode.rs, types.rs, surface.rs), fix stale crate status note, update specs/ from (planned) to (50 tasks, 5 layers)
CHORE: Move PRD.md to docs/PRD.md, update references in CLAUDE.md and ARCHITECTURE.md
REFACTOR: Split ARCHITECTURE.md (1,254 lines) into modular sub-files — system-diagrams.md, data-architecture.md, dependency-map.md, component-relations.md, feature-files.md — spine retains ~430 lines with stub links
FEATURE: Add docs/architecture/INDEX.md — compact architecture index (~55 lines) for @-import in CLAUDE.md
REFACTOR: Optimize CLAUDE.md — remove redundant tech stack (now in INDEX.md), fix stale counts (14→16 features, 29→50 tasks), add @docs/architecture/INDEX.md import, remove discoverable commands
CHORE: Update ARCHITECTURE.md cross-references to link to extracted sub-files, add sub-files to project structure listing and maintenance section

FEATURE: Add dependency-centric sections support to create-architecture skill — §13 Feature Dependency Map, §14 Inter-Component Relations, §15 Critical Files per Feature with generation guide, template sections, questions, and maintenance workflow for multi-module projects
FEATURE: Add Feature Dependency Map (§12), Inter-Component Relations (§13), and Critical Files per Feature (§14) sections to ARCHITECTURE.md — dependency-centric views covering all 16 PRD features, 9 crates, 12 relation categories, and file-level status tracking
FEATURE: Add domain model types to wmux-core (L0_02) — WindowId, WorkspaceId, PaneId, SurfaceId newtypes (UUID, Copy+Eq+Hash), Cell struct with grapheme cluster model, CellFlags/TerminalMode bitflags with manual serde, Color enum (Named/Indexed/Rgb), CursorShape/CursorState, SplitDirection, PanelKind, SurfaceInfo, Row type alias; 34 unit tests covering serde roundtrips, trait bounds, defaults, edge cases
FIX: Widen CellFlags from u8 to u16 for future extensibility, add BLINK flag (SGR 5)
FIX: Use from_bits_truncate() for CellFlags/TerminalMode serde deserialization (forward-compat with newer versions)
FIX: ID Default returns nil UUID instead of random — deterministic, satisfies principle of least surprise
REFACTOR: Add Default derives to SplitDirection (Horizontal) and PanelKind (Terminal)
CHORE: Add uuid and bitflags workspace dependencies, serde_json dev-dependency for wmux-core

## 2026-03-19

FIX: Replace unchecked index access on surface_caps.formats[0] and alpha_modes[0] with .first().expect() in GpuContext::new()
CHORE: Add Send + Sync compile-time assertion tests to all 7 error types (CoreError, PtyError, IpcError, BrowserError, ConfigError, RenderError, UiError)
FEATURE: Add error types and tracing infrastructure to 6 stub crates — CoreError, PtyError, IpcError, BrowserError, ConfigError (thiserror v2) with General + Io placeholder variants; wmux-cli gets anyhow + tracing-subscriber init with RUST_LOG env filter

REFACTOR: Rename all 50 spec files from gap-based numbering (001, 020, 050...) to layer-prefixed sequential format (L0_01, L1_01, L2_01...) — clearer, no confusing gaps between layers
FEATURE: Generate complete implementation specs — 50 task files across 5 layers (Scaffold, Foundation, Core, Integration, Polish) covering all 16 PRD features and 10 ADRs, with dependency graph, ~111 hours estimated effort
FEATURE: Add ADR-0008 (Async Architecture — Actor Pattern via Bounded Channels), ADR-0009 (Session Persistence — JSON File with Auto-save), ADR-0010 (Config Format — Ghostty-compatible Key-Value)
FEATURE: Add Sidebar Metadata data model (statuses, progress, logs schemas) and sequence diagram to architecture §6
FEATURE: Add Cross-Cutting Concerns table to architecture §3 (error handling, logging, i18n, clipboard, testing)
FIX: Align portable-pty version in Cargo.toml with architecture (0.8 → 0.9)
FIX: Align IPC security mode names with PRD terminology (wmux-only, allowAll, off, password)
CHORE: Add Surface, Panel, Window to architecture glossary (5-level hierarchy from PRD)
CHORE: Add multi-pane quality attribute targets (250MB for 10 panes, CLI binary < 5MB)
CHORE: Mark project structure as "Target" with implementation status note
CHORE: Fix specs/ reference from "39 tasks" to "planned"
CHORE: Add testing strategy reference to architecture Maintenance section
CHORE: Bump architecture document to version 3.0

REFACTOR: Rewrite PRD.md with comprehensive cmux research — add Modele Conceptuel (5-level hierarchy), Sidebar Metadata System (statuts/progress/logs), Read Screen (capture-pane), full browser automation API (50+ subcommands across 8 categories), notification lifecycle with suppression rules and custom commands, access modes (off/wmux-only/allowAll), macOS→Windows adaptation table, keyboard shortcuts mapping, expanded API method reference, session persistence detail (what is/isn't restored), expanded user workflows for multi-agent orchestration

FEATURE: Create complete architecture document (docs/architecture/ARCHITECTURE.md) following C4 model, MADR ADRs, and industry standards — replaces informal root ARCHITECTURE.md
FEATURE: Add 7 Architecture Decision Records (ADR-0001 through ADR-0007) covering language, GPU rendering, text rendering, PTY backend, IPC, browser, and windowing decisions
FEATURE: Add architecture glossary (docs/architecture/glossary.md) with 25 domain and technical terms
CHORE: Identify stack corrections: portable-pty 0.8→0.9, add webview2-com 0.39, add windows 0.62, add clap 4, remove unused dwrote dependency

CHORE: Apply audit corrections to Phase 5 (Tasks 22-29) spec files — add fuzzy-matcher crate and CommandEntry struct to command palette, add Search Model section and alternate screen behavior to terminal search, replace std::process::Command with git2 crate and add actor pattern to git detection, add daemon distribution and SSH transport clarification to SSH remote, add code signing and staged-install pattern to auto-update, add three-tier OS detection table and wgpu alpha compositing to Mica effects, recommend fluent-rs and add AppEvent::LanguageChanged to localization, add WiX 4 and daemon bundling to packaging
CHORE: Update specs/README.md — add Phase 0 section, 10 new intermediate tasks (00a, 00b, 02b, 07b, 08a, 08b, 09a, 12a, 15a, 17a), replace dependency map with updated version showing all new tasks, update total estimate to 120-150 hours for 39 tasks

CHORE: Apply audit corrections to Phase 3 (Tasks 12-15) and Phase 4 (Tasks 16-21) spec files — fix cmux wire format note, pipe name, dependency chain, windows crate features, ConnectionCtx auth hook, thiserror, AppHandle pattern, V1 deferral, connection lifecycle, wmux_only token mechanism, auth.login exchange, DACL file creation, secret rotation, atomic create_new, version pins, clap derive, pipe fallback algorithm, timeout flag, CLI-to-RPC table, current resource resolution, exit codes, task split for 15a/b/c, phase 3 scope reduction, window.* deferral, mock AppHandle tests, SessionState version field, spawn_blocking serialization, MoveFileExW atomic write, browser URL placeholder, COM threading model, HWND extraction, focus handoff, show/hide lifecycle, DevTools clarification, FFI SAFETY comments, CapturePreview screenshot, oneshot bridge pattern, browser.open_split/focus_webview, wait polling, browser.eval security, Toast activation, AUMID requirement, Shell_NotifyIconW prohibition, PlaySoundW sound, single NotificationStore entry point, Ghostty not-TOML parser, ArcSwap/RwLock for live reload, notify crate required, dark/light mode registry/WinRT, thiserror in wmux-config, unknown key forward compat, retroactive task enhancement note, OSC 7/133 ownership split, PowerShell execution policy, fish removal, WSL deferral, GetExtendedTcpTable port scanning
CHORE: Apply audit corrections to Phase 1 (Tasks 01-07) and Phase 2 (Tasks 08-11) spec files — add grapheme cluster Cell model, alternate screen buffer note, VecDeque/Row types, spawn_blocking PTY read pattern, TerminalMetrics export, QuadPipeline note, winit 0.30 text/IME handling, SGR mouse format, PaneArena shape, multi-pane render architecture, WorkspaceEvent broadcast, and Win32 context menu
REFACTOR: Replace anyhow with thiserror typed errors in wmux-render (RenderError) and wmux-ui (UiError) library crates
REFACTOR: Add .context() to error propagation in wmux-app binary crate
FIX: Replace bare .unwrap() with .expect() documenting invariant in App::render()
REFACTOR: Add #[inline] to cross-crate hot-path methods in GpuContext and GlyphonRenderer
REFACTOR: Use structured tracing fields instead of format strings in GPU and window logging
CHORE: Add thiserror v2 to workspace dependencies
CHORE: Enhance release profile with strip = "symbols" and panic = "abort"
CHORE: Remove unused anyhow/pollster dependencies from wmux-render

FIX: Correct async-cancellation-safety rule — replace `read` (cancellation-safe) with `read_exact` (not cancellation-safe) in incorrect example
FIX: Correct memory-stack-allocation rule — main thread stack is OS-dependent (~1MB on Windows, ~8MB on Linux/macOS)
FIX: Correct memory-arena-allocator rule — clarify that bumpalo `allocator_api` does NOT add thread safety, recommend `bumpalo-herd` instead
FIX: Clarify ownership-clone-from rule — precise Clippy `assigning_clones` lint timeline (late 2023, moved to pedantic mid-2024)
FIX: Clarify async-send-bounds rule — async closure Send bounds depend on receiver type for AsyncFn/AsyncFnMut
FIX: Clarify perf-benchmarking rule — replace overstated constant-folding claim with accurate dead-code elimination concern
FIX: Clarify iterator-combinators rule — let chains require edition 2024 as a hard prerequisite, not just Rust 1.88
