# Critical Files per Feature

> Part of [wmux Architecture](ARCHITECTURE.md). See also: [Dependency Map](dependency-map.md), [Component Relations](component-relations.md).

Mapping from each **PRD feature** to the **source files** that implement it. Use this to understand which files to read and which files are impacted when working on a specific feature. Verified against filesystem on 2026-04-19.

**Status legend**: `[EXISTS]` file exists with implementation, `[STUB]` file exists but implementation returns a placeholder or error, `[PLANNED]` file does not yet exist.

## PRD 1, Terminal GPU-Accelerated

| Critical File | Crate | Role | Status |
|---------------|-------|------|--------|
| `wmux-core/src/cell.rs` | wmux-core | Cell struct with char + attributes | [EXISTS] |
| `wmux-core/src/color.rs` | wmux-core | Color model (Indexed, Rgb, Named) | [EXISTS] |
| `wmux-core/src/cursor.rs` | wmux-core | Cursor position, shape, visibility | [EXISTS] |
| `wmux-core/src/mode.rs` | wmux-core | Terminal modes (insert, wrap, origin, alt screen) | [EXISTS] |
| `wmux-core/src/types.rs` | wmux-core | Domain ID types (PaneId, SurfaceId, etc.) | [EXISTS] |
| `wmux-core/src/error.rs` | wmux-core | CoreError enum | [EXISTS] |
| `wmux-core/src/terminal.rs` | wmux-core | Terminal state machine | [EXISTS] |
| `wmux-core/src/grid.rs` | wmux-core | Cell grid (contiguous Vec\<Cell\> per row) | [EXISTS] |
| `wmux-core/src/scrollback.rs` | wmux-core | Ring buffer (VecDeque, 4K lines) | [EXISTS] |
| `wmux-core/src/vte_handler.rs` | wmux-core | vte::Perform (CSI, OSC, DSR, DA1) | [EXISTS] |
| `wmux-core/src/event.rs` | wmux-core | TerminalEvent, Hyperlink, PromptMark types | [EXISTS] |
| `wmux-core/src/selection.rs` | wmux-core | Selection model (Normal/Word/Line) + text extraction | [EXISTS] |
| `wmux-pty/src/manager.rs` | wmux-pty | PtyManager (spawn, I/O, resize) | [EXISTS] |
| `wmux-pty/src/conpty.rs` | wmux-pty | ConPTY-specific helpers | [EXISTS] |
| `wmux-pty/src/spawn.rs` | wmux-pty | Process spawn wrapper | [EXISTS] |
| `wmux-pty/src/shell.rs` | wmux-pty | Shell detection (pwsh, powershell, cmd) | [EXISTS] |
| `wmux-pty/src/actor.rs` | wmux-pty | PtyActorHandle async I/O bridge | [EXISTS] |
| `wmux-pty/src/error.rs` | wmux-pty | PtyError enum | [EXISTS] |
| `wmux-render/src/gpu.rs` | wmux-render | GpuContext (wgpu surface, device, queue) | [EXISTS] |
| `wmux-render/src/text.rs` | wmux-render | GlyphonRenderer (text atlas, buffer, render) | [EXISTS] |
| `wmux-render/src/quad.rs` | wmux-render | QuadPipeline (colored rectangles, cursor, selection) | [EXISTS] |
| `wmux-render/src/shader.wgsl` | wmux-render | WGSL shaders for quads | [EXISTS] |
| `wmux-render/src/terminal.rs` | wmux-render | TerminalRenderer (per-frame grid rendering, dirty rows) | [EXISTS] |
| `wmux-render/src/pane.rs` | wmux-render | Per-pane renderer with focus glow | [EXISTS] |
| `wmux-render/src/error.rs` | wmux-render | RenderError enum | [EXISTS] |
| `wmux-ui/src/window/mod.rs` | wmux-ui | UiState + App composition root | [EXISTS] |
| `wmux-ui/src/window/event_loop.rs` | wmux-ui | winit ApplicationHandler impl | [EXISTS] |
| `wmux-ui/src/window/handlers.rs` | wmux-ui | Shortcut + action handler tables | [EXISTS] |
| `wmux-ui/src/window/render.rs` | wmux-ui | Per-frame render orchestration | [EXISTS] |
| `wmux-ui/src/input.rs` | wmux-ui | Keyboard input to VT byte sequences | [EXISTS] |
| `wmux-ui/src/mouse.rs` | wmux-ui | MouseHandler (selection, click, scroll, SGR reporting) | [EXISTS] |
| `wmux-ui/src/event.rs` | wmux-ui | WmuxEvent enum (AppEvent forwarding) | [EXISTS] |
| `wmux-ui/src/error.rs` | wmux-ui | UiError enum | [EXISTS] |

## PRD 2, Multiplexer (Split Panes + Workspaces + Surfaces)

| Critical File | Crate | Role | Status |
|---------------|-------|------|--------|
| `wmux-core/src/pane_tree.rs` | wmux-core | Binary split tree layout engine | [EXISTS] |
| `wmux-core/src/rect.rs` | wmux-core | Rect geometry + split_horizontal/vertical | [EXISTS] |
| `wmux-core/src/app_state/mod.rs` | wmux-core | AppState actor composition + focus routing | [EXISTS] |
| `wmux-core/src/app_state/actor.rs` | wmux-core | Command dispatch loop | [EXISTS] |
| `wmux-core/src/app_state/handle.rs` | wmux-core | Public handle for IPC/UI clients | [EXISTS] |
| `wmux-core/src/pane_registry.rs` | wmux-core | PaneRegistry (PaneId -> PaneState mapping) | [EXISTS] |
| `wmux-core/src/workspace.rs` | wmux-core | Workspace model | [EXISTS] |
| `wmux-core/src/workspace_manager.rs` | wmux-core | Workspace lifecycle (create, select, close, rename) | [EXISTS] |
| `wmux-core/src/surface.rs` | wmux-core | SplitDirection, PanelKind, SurfaceInfo | [EXISTS] |
| `wmux-core/src/surface_manager.rs` | wmux-core | Surface lifecycle (create, close, list) | [EXISTS] |
| `wmux-core/src/types.rs` | wmux-core | PaneId, WorkspaceId, SurfaceId types | [EXISTS] |
| `wmux-ui/src/window/render.rs` | wmux-ui | Multi-pane rendering orchestration | [EXISTS] |
| `wmux-ui/src/divider.rs` | wmux-ui | Draggable pane dividers + hit-testing | [EXISTS] |
| `wmux-ui/src/sidebar.rs` | wmux-ui | Sidebar rendering (workspace list, metadata, port pills, collapsed mode) | [EXISTS] |
| `wmux-ui/src/input.rs` | wmux-ui | Keyboard shortcuts for split/focus/navigate | [EXISTS] |
| `wmux-ui/src/shortcuts.rs` | wmux-ui | Shortcut dispatcher (keybinding table) | [EXISTS] |
| `wmux-render/src/quad.rs` | wmux-render | Divider, selection, and background quads | [EXISTS] |
| `wmux-render/src/gpu.rs` | wmux-render | Multi-pane viewport scissoring | [EXISTS] |
| `wmux-render/src/text.rs` | wmux-render | Per-pane text rendering | [EXISTS] |
| `wmux-render/src/pane.rs` | wmux-render | Per-pane terminal + chrome renderer | [EXISTS] |

## PRD 3, CLI & API IPC

| Critical File | Crate | Role | Status |
|---------------|-------|------|--------|
| `wmux-ipc/src/server.rs` | wmux-ipc | Named Pipe server (tokio async, 30s timeout, 1MB limit) | [EXISTS] |
| `wmux-ipc/src/protocol.rs` | wmux-ipc | JSON-RPC v2 codec (cmux-compatible) | [EXISTS] |
| `wmux-ipc/src/auth.rs` | wmux-ipc | HMAC-SHA256 authentication + security modes | [EXISTS] |
| `wmux-ipc/src/router.rs` | wmux-ipc | Method dispatch | [EXISTS] |
| `wmux-ipc/src/handler.rs` | wmux-ipc | Handler trait | [EXISTS] |
| `wmux-ipc/src/error.rs` | wmux-ipc | IpcError + RpcErrorCode | [EXISTS] |
| `wmux-ipc/src/handlers/mod.rs` | wmux-ipc | Handler module re-exports | [EXISTS] |
| `wmux-ipc/src/handlers/system.rs` | wmux-ipc | system.* handlers | [EXISTS] |
| `wmux-ipc/src/handlers/workspace.rs` | wmux-ipc | workspace.* handlers | [EXISTS] |
| `wmux-ipc/src/handlers/surface.rs` | wmux-ipc | surface.* + input/read handlers | [EXISTS] |
| `wmux-ipc/src/handlers/browser.rs` | wmux-ipc | browser.* handlers (30+ methods) | [EXISTS] |
| `wmux-ipc/src/handlers/sidebar.rs` | wmux-ipc | sidebar.* + notification.* handlers | [EXISTS] |
| `wmux-cli/src/main.rs` | wmux-cli | CLI entry point with clap subcommands | [EXISTS] |
| `wmux-cli/src/client.rs` | wmux-cli | Named Pipe client connector | [EXISTS] |
| `wmux-cli/src/output.rs` | wmux-cli | Human + --json formatter | [EXISTS] |
| `wmux-cli/src/commands/mod.rs` | wmux-cli | Command module re-exports | [EXISTS] |

## PRD 4, Integrated Browser (WebView2)

| Critical File | Crate | Role | Status |
|---------------|-------|------|--------|
| `wmux-browser/src/manager.rs` | wmux-browser | BrowserManager (COM init, environment, user data dir) | [EXISTS] |
| `wmux-browser/src/com.rs` | wmux-browser | ComGuard RAII wrapper (CoInitializeEx STA) | [EXISTS] |
| `wmux-browser/src/automation/mod.rs` | wmux-browser | Automation module composition | [EXISTS] |
| `wmux-browser/src/automation/dom.rs` | wmux-browser | DOM interaction (click, fill, type, select, etc.) | [EXISTS] |
| `wmux-browser/src/automation/inspect.rs` | wmux-browser | Accessibility tree snapshot, get, is, find, wait | [EXISTS] |
| `wmux-browser/src/automation/navigation.rs` | wmux-browser | Navigate, back, forward, reload, eval, screenshot | [EXISTS] |
| `wmux-browser/src/panel/mod.rs` | wmux-browser | Panel composition | [EXISTS] |
| `wmux-browser/src/panel/attach.rs` | wmux-browser | Child HWND attach + parent wiring | [EXISTS] |
| `wmux-browser/src/panel/delegation.rs` | wmux-browser | Input delegation to WebView2 | [EXISTS] |
| `wmux-browser/src/panel/layout.rs` | wmux-browser | Bounds/visibility + DevTools open | [EXISTS] |
| `wmux-browser/src/error.rs` | wmux-browser | BrowserError enum | [EXISTS] |
| `wmux-ipc/src/handlers/browser.rs` | wmux-ipc | browser.* IPC handlers (30+ methods) | [EXISTS] |
| `wmux-ui/src/address_bar.rs` | wmux-ui | Browser pane URL bar | [EXISTS] |
| `wmux-ui/src/window/handlers.rs` | wmux-ui | F12 DevTools handler (placeholder, Backlog #4) | [STUB] |

## PRD 5, Sidebar Metadata System

| Critical File | Crate | Role | Status |
|---------------|-------|------|--------|
| `wmux-core/src/metadata_store.rs` | wmux-core | MetadataStore (statuses, progress, logs, PID sweep) | [EXISTS] |
| `wmux-core/src/notification.rs` | wmux-core | NotificationStore (lifecycle, badge counters) | [EXISTS] |
| `wmux-ui/src/sidebar.rs` | wmux-ui | Sidebar metadata rendering + port pills + collapsed mode | [EXISTS] |
| `wmux-ui/src/sidebar.rs` (progress bar UI) | wmux-ui | Progress bar not yet drawn (Backlog #2) | [STUB] |
| `wmux-ipc/src/handlers/sidebar.rs` | wmux-ipc | sidebar.* IPC handlers | [EXISTS] |
| `wmux-core/src/types.rs` | wmux-core | StatusEntry, ProgressEntry, LogEntry types | [EXISTS] |

## PRD 6, Terminal Read (capture-pane)

| Critical File | Crate | Role | Status |
|---------------|-------|------|--------|
| `wmux-core/src/grid.rs` | wmux-core | Grid content extraction for read_text | [EXISTS] |
| `wmux-ipc/src/handlers/surface.rs` | wmux-ipc | surface.read_text handler | [EXISTS] |

## PRD 7, Notifications

| Critical File | Crate | Role | Status |
|---------------|-------|------|--------|
| `wmux-core/src/notification.rs` | wmux-core | NotificationStore + lifecycle | [EXISTS] |
| `wmux-ui/src/toast.rs` | wmux-ui | ToastService (Windows Toast API via WinRT) | [EXISTS] |
| `wmux-ui/src/notification_panel.rs` | wmux-ui | Notification panel overlay (Ctrl+Shift+I, clear-all, jump-to-unread) | [EXISTS] |
| `wmux-ipc/src/handlers/sidebar.rs` | wmux-ipc | notification.* IPC handlers (routed via sidebar handler since metadata store owns them) | [EXISTS] |
| `wmux-ui/src/sidebar.rs` | wmux-ui | Badge counters on workspace entries | [EXISTS] |

## PRD 8, Session Persistence

| Critical File | Crate | Role | Status |
|---------------|-------|------|--------|
| `wmux-core/src/session.rs` | wmux-core | Session save/load helpers + schema version | [EXISTS] |
| `wmux-core/src/workspace.rs` | wmux-core | Serializable workspace state | [EXISTS] |
| `wmux-core/src/pane_tree.rs` | wmux-core | Serializable pane tree | [EXISTS] |
| `wmux-core/src/surface_manager.rs` | wmux-core | Surface restore wiring | [EXISTS] |
| `wmux-app/src/main.rs` | wmux-app | Auto-save timer + restore on launch | [EXISTS] |
| `wmux-core/src/types.rs` | wmux-core | Session schema version | [EXISTS] |

## PRD 9, SSH Remote

| Critical File | Crate | Role | Status |
|---------------|-------|------|--------|
| `daemon/remote/cmd/wmuxd-remote/main.go` | daemon | Go SSH remote daemon entry point (Backlog #7) | [PLANNED] |
| `wmux-cli/src/commands/ssh.rs` | wmux-cli | `wmux ssh` command (returns "not yet fully implemented") | [STUB] |
| `wmux-core/src/remote.rs` | wmux-core | Remote workspace model (SSH icon, reconnect state) | [EXISTS] |
| `wmux-ui/src/sidebar.rs` | wmux-ui | SSH workspace indicator in sidebar | [EXISTS] |

## PRD 10, Themes & Configuration

| Critical File | Crate | Role | Status |
|---------------|-------|------|--------|
| `wmux-config/src/parser.rs` | wmux-config | Ghostty-compat config parser | [EXISTS] |
| `wmux-config/src/config.rs` | wmux-config | Config struct (font, keybindings, scrollback, bounds) | [EXISTS] |
| `wmux-config/src/theme/mod.rs` | wmux-config | ThemeEngine public API | [EXISTS] |
| `wmux-config/src/theme/chrome.rs` | wmux-config | Chrome color tokens | [EXISTS] |
| `wmux-config/src/theme/registry.rs` | wmux-config | Theme registry (8 bundled + user) | [EXISTS] |
| `wmux-config/src/theme/types.rs` | wmux-config | Typed palettes | [EXISTS] |
| `wmux-config/src/error.rs` | wmux-config | ConfigError enum | [EXISTS] |
| `wmux-ui/src/shortcuts.rs` | wmux-ui | Keybinding wiring (parser stores HashMap, apply_custom_keybindings pending Backlog #3) | [STUB] |
| `resources/themes/*.conf` | resources | 8 bundled themes (catppuccin-mocha, digital-obsidian, dracula, gruvbox-dark, nord, one-dark, stitch-blue, wmux-default) | [EXISTS] |
| `wmux-cli/src/commands/` (theme subcommands) | wmux-cli | No dedicated theme command file yet, theme set/list via config | [PLANNED] |
| Inter font bundling (`resources/fonts/`) | resources | Inter Regular + Bold not yet bundled (Backlog #1) | [PLANNED] |

## PRD 11, Command Palette

| Critical File | Crate | Role | Status |
|---------------|-------|------|--------|
| `wmux-ui/src/command_palette.rs` | wmux-ui | Palette overlay, fuzzy search, result + shortcut buffers | [EXISTS] |
| `wmux-core/src/command_registry.rs` | wmux-core | Command registry (actions + palette metadata) | [EXISTS] |
| `wmux-render/src/text.rs` | wmux-render | Palette text rendering (search, results) | [EXISTS] |

## PRD 12, Terminal Search

| Critical File | Crate | Role | Status |
|---------------|-------|------|--------|
| `wmux-ui/src/search.rs` | wmux-ui | Search overlay (Ctrl+F), match navigation | [EXISTS] |
| `wmux-core/src/scrollback.rs` | wmux-core | Searchable scrollback buffer | [EXISTS] |
| `wmux-render/src/text.rs` | wmux-render | Match highlight rendering | [EXISTS] |

## PRD 13, Shell Integration & Git Detection

| Critical File | Crate | Role | Status |
|---------------|-------|------|--------|
| `resources/shell-integration/wmux.ps1` | resources | PowerShell hook script | [EXISTS] |
| `resources/shell-integration/wmux.bash` | resources | Bash hook script | [EXISTS] |
| `resources/shell-integration/wmux.zsh` | resources | Zsh hook script | [EXISTS] |
| `wmux-core/src/vte_handler.rs` | wmux-core | OSC 7/133 sequence processing | [EXISTS] |
| `wmux-core/src/git_detector.rs` | wmux-core | Git branch + dirty state detection | [EXISTS] |
| `wmux-core/src/port_scanner.rs` | wmux-core | Listening port detection per workspace | [EXISTS] |
| `wmux-ui/src/sidebar.rs` | wmux-ui | Git branch + dirty + ports display | [EXISTS] |

## PRD 14, Auto-Update

| Critical File | Crate | Role | Status |
|---------------|-------|------|--------|
| `wmux-app/src/main.rs` | wmux-app | Update timer + download manager | [EXISTS] |
| `wmux-app/src/updater.rs` | wmux-app | Hardened UpdateChecker (SHA-256, HTTPS allowlist, 200MB cap, atomic install) | [EXISTS] |
| `wmux-cli/src/commands/` (update subcommand) | wmux-cli | No dedicated update command file, update check exposed via IPC | [PLANNED] |
| `wmux-ui/src/window/render.rs` | wmux-ui | Update pill/badge in title bar | [EXISTS] |
| `wmux-ui/src/titlebar.rs` | wmux-ui | Custom title bar hosting update indicator | [EXISTS] |

## PRD 15, Windows 11 Visual Effects

| Critical File | Crate | Role | Status |
|---------------|-------|------|--------|
| `wmux-ui/src/effects.rs` | wmux-ui | DWM API calls for Mica/Acrylic + fallback | [EXISTS] |
| `wmux-ui/src/titlebar.rs` | wmux-ui | Non-client area subclass (WM_NCCALCSIZE) | [EXISTS] |
| `wmux-config/src/theme/chrome.rs` | wmux-config | Opacity/alpha values from theme config | [EXISTS] |

## PRD 16, Localization FR/EN

| Critical File | Crate | Role | Status |
|---------------|-------|------|--------|
| `resources/locales/en.toml` | resources | English UI strings | [EXISTS] |
| `resources/locales/fr.toml` | resources | French UI strings | [EXISTS] |
| `wmux-config/src/locale.rs` | wmux-config | Locale detection + TOML string loading + system language | [EXISTS] |
| `wmux-ui/src/notification_panel.rs` + `sidebar.rs` | wmux-ui | 12+ strings wired via `locale.t()` | [EXISTS] |
