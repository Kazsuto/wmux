# Critical Files per Feature

> Part of [wmux Architecture](ARCHITECTURE.md). See also: [Dependency Map](dependency-map.md), [Component Relations](component-relations.md).

Mapping from each **PRD feature** to the **source files** that implement it. Use this to understand which files to read and which files are impacted when working on a specific feature.

**Status**: `[EXISTS]` file exists with implementation · `[STUB]` file exists as placeholder · `[PLANNED]` file does not yet exist

## PRD 1 — Terminal GPU-Accelerated

| Critical File | Crate | Role | Status |
|---------------|-------|------|--------|
| `wmux-core/src/cell.rs` | wmux-core | Cell struct with char + attributes | [EXISTS] |
| `wmux-core/src/color.rs` | wmux-core | Color model (Indexed, Rgb, Named) | [EXISTS] |
| `wmux-core/src/cursor.rs` | wmux-core | Cursor position, shape, visibility | [EXISTS] |
| `wmux-core/src/mode.rs` | wmux-core | Terminal modes (insert, wrap, origin, alt screen) | [EXISTS] |
| `wmux-core/src/types.rs` | wmux-core | Domain ID types (PaneId, SurfaceId, etc.) | [EXISTS] |
| `wmux-core/src/error.rs` | wmux-core | CoreError enum | [EXISTS] |
| `wmux-core/src/terminal.rs` | wmux-core | Terminal state machine | [PLANNED] |
| `wmux-core/src/grid.rs` | wmux-core | Cell grid (contiguous Vec\<Cell\> per row) | [PLANNED] |
| `wmux-core/src/scrollback.rs` | wmux-core | Ring buffer (VecDeque, 4K lines) | [PLANNED] |
| `wmux-core/src/vte_handler.rs` | wmux-core | vte::Perform implementation | [PLANNED] |
| `wmux-pty/src/manager.rs` | wmux-pty | PtyManager (spawn, I/O, resize) | [PLANNED] |
| `wmux-pty/src/shell.rs` | wmux-pty | Shell detection (pwsh → powershell → cmd) | [PLANNED] |
| `wmux-pty/src/error.rs` | wmux-pty | PtyError enum | [EXISTS] |
| `wmux-render/src/gpu.rs` | wmux-render | GpuContext (wgpu surface, device, queue) | [EXISTS] |
| `wmux-render/src/text.rs` | wmux-render | GlyphonRenderer (text atlas, buffer, render) | [EXISTS] |
| `wmux-render/src/quad.rs` | wmux-render | QuadPipeline (colored rectangles) | [PLANNED] |
| `wmux-render/src/shader.wgsl` | wmux-render | WGSL shaders for quads | [PLANNED] |
| `wmux-render/src/error.rs` | wmux-render | RenderError enum | [EXISTS] |
| `wmux-ui/src/window.rs` | wmux-ui | App (winit ApplicationHandler) | [EXISTS] |
| `wmux-ui/src/input.rs` | wmux-ui | Keyboard/mouse event dispatch | [PLANNED] |
| `wmux-ui/src/error.rs` | wmux-ui | UiError enum | [EXISTS] |

## PRD 2 — Multiplexer (Split Panes + Workspaces + Surfaces)

| Critical File | Crate | Role | Status |
|---------------|-------|------|--------|
| `wmux-core/src/pane_tree.rs` | wmux-core | Binary split tree layout engine | [PLANNED] |
| `wmux-core/src/workspace.rs` | wmux-core | Workspace model | [PLANNED] |
| `wmux-core/src/workspace_manager.rs` | wmux-core | Workspace lifecycle (create, select, close) | [PLANNED] |
| `wmux-core/src/focus.rs` | wmux-core | Focus routing logic (directional nav) | [PLANNED] |
| `wmux-core/src/surface.rs` | wmux-core | Surface/tab model per pane | [EXISTS] |
| `wmux-core/src/types.rs` | wmux-core | PaneId, WorkspaceId, SurfaceId types | [EXISTS] |
| `wmux-ui/src/window.rs` | wmux-ui | App with multi-pane rendering | [EXISTS] |
| `wmux-ui/src/split_container.rs` | wmux-ui | Split pane layout + dividers | [PLANNED] |
| `wmux-ui/src/sidebar.rs` | wmux-ui | Sidebar rendering (workspace list) | [PLANNED] |
| `wmux-ui/src/input.rs` | wmux-ui | Keyboard shortcuts for split/focus/navigate | [PLANNED] |
| `wmux-render/src/quad.rs` | wmux-render | Divider, selection, and background quads | [PLANNED] |
| `wmux-render/src/gpu.rs` | wmux-render | Multi-pane viewport scissoring | [EXISTS] |
| `wmux-render/src/text.rs` | wmux-render | Per-pane text rendering | [EXISTS] |

## PRD 3 — CLI & API IPC

| Critical File | Crate | Role | Status |
|---------------|-------|------|--------|
| `wmux-ipc/src/server.rs` | wmux-ipc | Named Pipe server (tokio async) | [PLANNED] |
| `wmux-ipc/src/protocol.rs` | wmux-ipc | JSON-RPC v2 codec | [PLANNED] |
| `wmux-ipc/src/auth.rs` | wmux-ipc | HMAC-SHA256 authentication | [PLANNED] |
| `wmux-ipc/src/router.rs` | wmux-ipc | Method dispatch (Handler trait) | [PLANNED] |
| `wmux-ipc/src/handlers/mod.rs` | wmux-ipc | Handler module re-exports | [PLANNED] |
| `wmux-ipc/src/handlers/system.rs` | wmux-ipc | system.* handlers | [PLANNED] |
| `wmux-ipc/src/handlers/workspace.rs` | wmux-ipc | workspace.* handlers | [PLANNED] |
| `wmux-ipc/src/handlers/surface.rs` | wmux-ipc | surface.* handlers | [PLANNED] |
| `wmux-ipc/src/handlers/browser.rs` | wmux-ipc | browser.* handlers | [PLANNED] |
| `wmux-ipc/src/handlers/notification.rs` | wmux-ipc | notification.* handlers | [PLANNED] |
| `wmux-cli/src/main.rs` | wmux-cli | CLI entry point with clap subcommands | [STUB] |
| `wmux-cli/src/client.rs` | wmux-cli | Named Pipe client connector | [PLANNED] |

## PRD 4 — Integrated Browser (WebView2)

| Critical File | Crate | Role | Status |
|---------------|-------|------|--------|
| `wmux-browser/src/manager.rs` | wmux-browser | BrowserManager (lifecycle, HWND) | [PLANNED] |
| `wmux-browser/src/com.rs` | wmux-browser | Safe RAII wrappers for COM | [PLANNED] |
| `wmux-browser/src/automation.rs` | wmux-browser | click, fill, eval, screenshot | [PLANNED] |
| `wmux-browser/src/error.rs` | wmux-browser | BrowserError enum | [EXISTS] |
| `wmux-ipc/src/handlers/browser.rs` | wmux-ipc | browser.* IPC handlers | [PLANNED] |

## PRD 5 — Sidebar Metadata System

| Critical File | Crate | Role | Status |
|---------------|-------|------|--------|
| `wmux-core/src/notification.rs` | wmux-core | MetadataStore (statuses, progress, logs) | [PLANNED] |
| `wmux-ui/src/sidebar.rs` | wmux-ui | Sidebar metadata rendering | [PLANNED] |
| `wmux-ipc/src/handlers/sidebar.rs` | wmux-ipc | sidebar.* IPC handlers | [PLANNED] |
| `wmux-core/src/types.rs` | wmux-core | StatusEntry, ProgressEntry, LogEntry types | [EXISTS] |

## PRD 6 — Terminal Read (capture-pane)

| Critical File | Crate | Role | Status |
|---------------|-------|------|--------|
| `wmux-core/src/grid.rs` | wmux-core | Grid content extraction for read_text | [PLANNED] |
| `wmux-ipc/src/handlers/surface.rs` | wmux-ipc | surface.read_text handler | [PLANNED] |

## PRD 7 — Notifications

| Critical File | Crate | Role | Status |
|---------------|-------|------|--------|
| `wmux-core/src/notification.rs` | wmux-core | NotificationStore + lifecycle | [PLANNED] |
| `wmux-ui/src/overlay.rs` | wmux-ui | Notification panel overlay (Ctrl+Shift+I) | [PLANNED] |
| `wmux-ipc/src/handlers/notification.rs` | wmux-ipc | notification.* IPC handlers | [PLANNED] |
| `wmux-ui/src/sidebar.rs` | wmux-ui | Badge counters on workspace entries | [PLANNED] |

## PRD 8 — Session Persistence

| Critical File | Crate | Role | Status |
|---------------|-------|------|--------|
| `wmux-core/src/workspace.rs` | wmux-core | Serializable workspace state | [PLANNED] |
| `wmux-core/src/pane_tree.rs` | wmux-core | Serializable pane tree | [PLANNED] |
| `wmux-app/src/main.rs` | wmux-app | Auto-save timer + restore on launch | [EXISTS] |
| `wmux-core/src/types.rs` | wmux-core | Session schema version | [EXISTS] |

## PRD 9 — SSH Remote

| Critical File | Crate | Role | Status |
|---------------|-------|------|--------|
| `daemon/remote/cmd/wmuxd-remote/main.go` | daemon | Go SSH remote daemon entry point | [PLANNED] |
| `wmux-cli/src/commands/ssh.rs` | wmux-cli | `wmux ssh` command implementation | [PLANNED] |
| `wmux-core/src/workspace.rs` | wmux-core | Remote workspace model (SSH icon, reconnect state) | [PLANNED] |
| `wmux-ui/src/sidebar.rs` | wmux-ui | SSH workspace indicator in sidebar | [PLANNED] |

## PRD 10 — Themes & Configuration

| Critical File | Crate | Role | Status |
|---------------|-------|------|--------|
| `wmux-config/src/parser.rs` | wmux-config | Ghostty-compat config parser | [PLANNED] |
| `wmux-config/src/theme.rs` | wmux-config | Theme/color palette loading | [PLANNED] |
| `wmux-config/src/font.rs` | wmux-config | Font configuration (DirectWrite) | [PLANNED] |
| `wmux-config/src/keymap.rs` | wmux-config | Keybinding configuration | [PLANNED] |
| `wmux-config/src/error.rs` | wmux-config | ConfigError enum | [EXISTS] |
| `resources/themes/` | resources | Bundled Ghostty themes (.conf files) | [PLANNED] |
| `wmux-cli/src/commands/theme.rs` | wmux-cli | `wmux themes` list/set/clear commands | [PLANNED] |

## PRD 11 — Command Palette

| Critical File | Crate | Role | Status |
|---------------|-------|------|--------|
| `wmux-ui/src/overlay.rs` | wmux-ui | Palette overlay rendering (Ctrl+Shift+P) | [PLANNED] |
| `wmux-core/src/lib.rs` | wmux-core | Command registry (actions + metadata) | [STUB] |
| `wmux-render/src/text.rs` | wmux-render | Palette text rendering (search, results) | [EXISTS] |

## PRD 12 — Terminal Search

| Critical File | Crate | Role | Status |
|---------------|-------|------|--------|
| `wmux-ui/src/overlay.rs` | wmux-ui | Search overlay rendering (Ctrl+F) | [PLANNED] |
| `wmux-core/src/scrollback.rs` | wmux-core | Searchable scrollback buffer | [PLANNED] |
| `wmux-render/src/text.rs` | wmux-render | Match highlight rendering | [EXISTS] |

## PRD 13 — Shell Integration & Git Detection

| Critical File | Crate | Role | Status |
|---------------|-------|------|--------|
| `resources/shell-integration/wmux.ps1` | resources | PowerShell hook script | [PLANNED] |
| `resources/shell-integration/wmux.bash` | resources | Bash hook script | [PLANNED] |
| `resources/shell-integration/wmux.zsh` | resources | Zsh hook script | [PLANNED] |
| `wmux-core/src/vte_handler.rs` | wmux-core | OSC 7/133 sequence processing | [PLANNED] |
| `wmux-ui/src/sidebar.rs` | wmux-ui | Git branch + dirty + ports display | [PLANNED] |
| `wmux-core/src/lib.rs` | wmux-core | Git/port detection logic | [STUB] |

## PRD 14 — Auto-Update

| Critical File | Crate | Role | Status |
|---------------|-------|------|--------|
| `wmux-app/src/main.rs` | wmux-app | Update timer + download manager | [EXISTS] |
| `wmux-cli/src/commands/update.rs` | wmux-cli | `wmux update check/install` commands | [PLANNED] |
| `wmux-ui/src/window.rs` | wmux-ui | Update pill/badge in title bar | [EXISTS] |

## PRD 15 — Windows 11 Visual Effects

| Critical File | Crate | Role | Status |
|---------------|-------|------|--------|
| `wmux-ui/src/window.rs` | wmux-ui | DWM API calls for Mica/Acrylic + fallback | [EXISTS] |
| `wmux-config/src/theme.rs` | wmux-config | Opacity/alpha values from theme config | [PLANNED] |

## PRD 16 — Localization FR/EN

| Critical File | Crate | Role | Status |
|---------------|-------|------|--------|
| `resources/locales/en.toml` | resources | English UI strings | [PLANNED] |
| `resources/locales/fr.toml` | resources | French UI strings | [PLANNED] |
| `wmux-config/src/lib.rs` | wmux-config | Locale detection + string loading | [STUB] |
