# Task L1_10: Wire Single-Pane Terminal Integration

> **Phase**: Foundation
> **Priority**: P0-Critical
> **Estimated effort**: 2.5 hours

## Context

This is the Phase 1 integration milestone: combining all foundation components into a working single-pane terminal. The event loop receives keyboard/mouse input, routes to PTY, reads PTY output, parses through VTE into the grid, and renders to GPU. This is the "minimal Alacritty" milestone from Architecture §13. All prior Layer 1 tasks are prerequisites.

## Prerequisites

- [ ] Task L1_02: VTE Parser Integration — VTE → grid mutations
- [ ] Task L1_03: Scrollback Ring Buffer — scrollback and alt screen
- [ ] Task L1_04: OSC Sequence Handlers — terminal event bus
- [ ] Task L1_06: PTY Async I/O — async read/write channels
- [ ] Task L1_07: Terminal Grid GPU Rendering — grid → GPU
- [ ] Task L1_08: Keyboard Input → PTY Dispatch — key events → VT bytes
- [ ] Task L1_09: Mouse Selection, Copy/Paste — mouse interaction

## Scope

### Deliverables
- Updated `wmux-app/src/main.rs`: create Terminal + PtyActor, wire into event loop
- Updated `wmux-ui/src/app.rs`: integrate terminal rendering into existing App struct
- tokio runtime integration with winit event loop
- PTY output → Terminal::process() → render cycle
- Keyboard → PTY write pipeline
- Mouse → selection/scroll/reporting pipeline
- Resize → PTY resize + grid resize + GPU viewport update
- Process exit handling (display "[Process exited with code N]")

### Explicitly Out of Scope
- Multiple panes (Task L2_01+)
- Sidebar (Task L2_08)
- IPC (Task L2_09+)
- Any overlay UI (palette, search, notifications)

## Implementation Details

### Files to Create/Modify
| Action | Path | Purpose |
|--------|------|---------|
| Modify | `wmux-app/src/main.rs` | Wire tokio runtime + winit event loop + terminal |
| Modify | `wmux-ui/src/app.rs` | Integrate Terminal + PtyActor + TerminalRenderer |
| Modify | `wmux-ui/Cargo.toml` | Add wmux-core, wmux-pty dependencies |
| Modify | `wmux-app/Cargo.toml` | Add wmux-core, wmux-pty, tokio dependencies |

### Key Decisions
- **tokio + winit threading**: winit event loop runs on the main thread. tokio runtime runs on a separate thread pool. Communication via channels. Use `winit::event_loop::EventLoopProxy` to wake the event loop from tokio tasks
- **Render cycle**: On each `RedrawRequested`: (1) drain PTY output channel → Terminal::process(), (2) take dirty rows → TerminalRenderer::update(), (3) render frame
- **Redraw trigger**: PTY output arriving should trigger `EventLoopProxy::send_event()` to request a redraw

### Patterns to Follow
- Architecture §6 Data Flow — Terminal I/O (full sequence diagram)
- Architecture §13 Phase 1: "Milestone: A functional terminal (like minimal Alacritty)"
- `.claude/rules/rust-architecture.md`: Actor pattern, bounded channels

### Technical Notes
- winit 0.30 + tokio: Create tokio runtime before winit event loop. Spawn tokio tasks for PtyActor. Use EventLoopProxy to bridge async → sync
- Frame pacing: `PresentMode::AutoVsync` handles 60fps cap. Don't request redraws faster than needed
- Resize handling order: (1) update wgpu surface config, (2) update terminal grid dimensions, (3) send resize to PTY, (4) update renderer metrics
- Process exit: when PtyActor emits Exited event, display message in terminal area. Don't close the pane automatically — user decides
- Initial terminal size: derive from window size and TerminalMetrics (cols = window_width / cell_width, rows = window_height / cell_height)
- Window title: set to "wmux" or optionally include shell name

## Success Criteria

- [ ] Application opens a window with a working terminal (shell prompt visible)
- [ ] Keyboard input works (type commands, see output)
- [ ] Terminal correctly renders colored output (ls, git status, etc.)
- [ ] Scrollback works (scroll wheel through history)
- [ ] Copy/paste works (Ctrl+Shift+C/V)
- [ ] Window resize correctly adjusts terminal dimensions
- [ ] TUI applications work (vim, htop, less — alt screen + mouse)
- [ ] Process exit displays exit code message
- [ ] Performance: < 16ms frame time for typical terminal output
- [ ] No panics or crashes during normal usage

## Validation Steps

### Automated Checks
```bash
cargo build --workspace
cargo clippy --workspace -- -W clippy::all
cargo test --workspace
cargo fmt --all -- --check
```

### Manual Verification
1. Run `cargo run -p wmux-app` — verify terminal opens with shell prompt
2. Type `echo hello world` — verify output appears
3. Run `ls --color` (or PowerShell equivalent) — verify colored output
4. Run `vim` or `less` — verify alt screen works, exit restores main screen
5. Scroll up through history — verify scrollback content visible
6. Select text, Ctrl+Shift+C, open notepad, Ctrl+V — verify clipboard
7. Resize window — verify terminal reflows (or truncates cleanly)
8. Type `exit` — verify "[Process exited]" message

### Edge Cases to Test
- Very fast output (e.g., `cat /dev/urandom | xxd | head -1000`) — verify no crash, frames may drop
- Empty terminal (no output yet) — should show clean background with cursor
- Unicode output (emoji, CJK characters) — should render or show fallback glyph
- Multiple rapid resizes — should not crash or deadlock

## Dependencies

**Blocks**:
- Task L2_01: AppState Actor + Multi-Pane Architecture

## References
- **PRD**: §1 Terminal GPU-Acceleré (all requirements)
- **Architecture**: §6 Data Flow — Terminal I/O, §13 Implementation Roadmap Phase 1
- **ADR**: ADR-0001 through ADR-0004, ADR-0007, ADR-0008
