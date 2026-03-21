# Inter-Component Relations

> Part of [wmux Architecture](ARCHITECTURE.md). See also: [Dependency Map](dependency-map.md), [Feature Files](feature-files.md).

Exhaustive table of dependencies, data flows, and event triggers between components. Use this to assess **blast radius** when modifying any component.

**Relation types**: `uses` (direct call/import) · `triggers` (event/callback) · `depends-on` (requires at runtime) · `persists-in` (writes to storage) · `consumed-by` (data read by) · `exposes-to` (API surface)

## 13.1 Terminal I/O Relations (Critical Path)

The hot path from keystroke to rendered frame — every millisecond matters here.

| From | Relation | To | Description |
|------|----------|----|-------------|
| winit Event Loop | `triggers` | Keyboard Input | Win32 key events dispatched to input handler |
| Keyboard Input | `uses` | PTY Manager | Translated key bytes written to ConPTY stdin |
| PTY Manager | `uses` | ConPTY (Win32) | portable-pty spawn, resize, I/O via Win32 API |
| PTY Async I/O | `triggers` | VTE Parser | Raw bytes from ConPTY stdout fed to vte::Perform |
| VTE Parser | `uses` | Cell Grid | Escape sequences update cells, cursor, modes |
| Cell Grid | `triggers` | Dirty Row Tracking | Modified rows flagged for GPU upload |
| winit Event Loop | `triggers` | GPU Context | RequestRedraw schedules render pass |
| Terminal Renderer | `consumed-by` | Cell Grid | Reads dirty rows for glyph upload |
| GlyphonRenderer | `uses` | GPU Context | Text atlas rendered to wgpu surface |
| QuadPipeline | `uses` | GPU Context | Colored rectangles (cursor, selection, UI) rendered |

## 13.2 OSC and Event Relations

OSC escape sequences trigger application-level events and UI updates.

| From | Relation | To | Description |
|------|----------|----|-------------|
| VTE Parser | `triggers` | OSC 7 Handler | CWD change → sidebar cwd update |
| VTE Parser | `triggers` | OSC 9/99/777 Handlers | Notification events → NotificationStore |
| VTE Parser | `triggers` | OSC 133 Handler | Shell prompt markers → prompt detection |
| OSC 7 Handler | `triggers` | Sidebar UI | Workspace cwd display refreshed |
| OSC 9/99/777 | `triggers` | Notification Store | New notification created with lifecycle |
| Notification Store | `triggers` | Toast Notifications | Windows Toast API invoked for desktop alerts |
| Notification Store | `triggers` | Sidebar UI | Badge counters updated on workspace entries |
| Notification Store | `triggers` | Visual Indicators | Blue ring on pane, tab glow |
| Shell Integration Hooks | `triggers` | OSC sequences | PowerShell/bash/zsh hooks emit OSC 7/133 |

## 13.3 Multiplexer Layout Relations

The PaneTree drives all layout — changing it affects rendering, browser panels, and dividers.

| From | Relation | To | Description |
|------|----------|----|-------------|
| PaneTree | `uses` | Split nodes | Binary tree of horizontal/vertical splits |
| PaneTree | `consumed-by` | Multi-Pane Rendering | Tree → viewport rects for each pane |
| PaneTree | `consumed-by` | Browser Panel | Browser HWND positioned to match pane rect |
| PaneTree | `consumed-by` | Draggable Dividers | Divider positions derived from split edges |
| Focus Routing | `uses` | PaneTree | Directional nav traverses tree to find neighbor |
| Workspace Manager | `uses` | PaneTree | Each workspace owns one PaneTree |
| Surface Tabs | `uses` | PaneTree leaf | Each leaf pane contains N surfaces (tabs) |
| AppState Actor | `uses` | PaneTree | Mutations (split, close, resize) go through actor |
| AppState Actor | `uses` | PaneRegistry | HashMap lookup for pane state mutations |
| AppState Actor | `uses` | NotificationStore | Centralized notification lifecycle management |

## 13.4 IPC and CLI Relations

The full path from AI agent invocation to state mutation and response.

| From | Relation | To | Description |
|------|----------|----|-------------|
| AI Agent / Script | `uses` | wmux-cli | Process invocation (`wmux split --right`) |
| wmux-cli | `uses` | Named Pipe client | Connects to `\\.\pipe\wmux-*` |
| Named Pipe client | `uses` | IPC Server | JSON-RPC v2 request sent over pipe |
| IPC Server | `uses` | Authentication | HMAC-SHA256 or child-process check |
| IPC Server | `uses` | Router | Method string → Handler dispatch |
| Router | `uses` | Workspace Handlers | `workspace.*` methods |
| Router | `uses` | Surface Handlers | `surface.*` methods |
| Router | `uses` | Input/Read Handlers | `surface.send_text`, `surface.read_text` |
| Router | `uses` | Sidebar Handlers | `sidebar.*` methods |
| Router | `uses` | Browser Handlers | `browser.*` methods |
| Router | `uses` | Notification Handlers | `notification.*` methods |
| All Handlers | `uses` | AppState Actor | Commands sent via bounded channel |
| AppState Actor | `exposes-to` | IPC Server | Results returned as JSON-RPC responses |

## 13.5 Browser Relations

WebView2 runs in a separate process — coordination happens via COM interop and HWND management.

| From | Relation | To | Description |
|------|----------|----|-------------|
| Browser Manager | `uses` | WebView2 COM (CoInitializeEx) | COM apartment initialization |
| Browser Manager | `uses` | WebView2 Environment | Edge runtime discovery + creation |
| Browser Manager | `uses` | Child HWND | Separate Win32 window for WebView2 content |
| Browser Panel | `uses` | PaneTree | Rect from PaneTree positions/sizes the HWND |
| Browser Panel | `depends-on` | Workspace switching | Show/hide HWND on workspace change |
| Browser Handlers (IPC) | `uses` | Browser Manager | navigate, eval, click, screenshot via COM API |
| Browser Manager | `depends-on` | WebView2 Runtime | Edge Chromium runtime must be installed |

## 13.6 Notification Relations

Notifications flow from multiple sources through a central store to multiple display targets.

| From | Relation | To | Description |
|------|----------|----|-------------|
| OSC 9/99/777 | `triggers` | Notification Store | Terminal-originated notifications |
| CLI `wmux notify` | `triggers` | Notification Store | CLI-originated notifications |
| IPC `notification.create` | `triggers` | Notification Store | API-originated notifications |
| Notification Store | `triggers` | Windows Toast API | Desktop notification via WinRT |
| Notification Store | `triggers` | Sidebar UI | Badge counters on workspace entries |
| Notification Store | `triggers` | Notification Panel | Overlay list (Ctrl+Shift+I) |
| Notification Store | `triggers` | Pane Visual Indicators | Blue ring, tab glow effects |
| Toast Service (wmux-ui) | `uses` | Windows Toast API (WinRT) | Desktop notification with AUMID setup |
| PID Sweep Timer | `uses` | Notification Store | Clear stale statuses from dead processes (30s) |

## 13.7 Configuration and Theme Relations

Config is read at startup and consumed by nearly every rendering component.

| From | Relation | To | Description |
|------|----------|----|-------------|
| Config Manager | `uses` | Parser | Ghostty-compat key=value parsing |
| Config Manager | `uses` | File system | `%APPDATA%\wmux\config`, `%APPDATA%\ghostty\config` |
| Config Struct | `consumed-by` | GlyphonRenderer | Font family, size, ligatures |
| Config Struct | `consumed-by` | Terminal Engine | Scrollback limit, cursor shape, bell behavior |
| Config Struct | `consumed-by` | Keyboard Input | Keybinding configuration |
| Config Struct | `consumed-by` | Sidebar UI | Sidebar width, locale |
| Theme Engine | `consumed-by` | GPU Context | Color palette (16-color, 256-color, rgb) |
| Theme Engine | `consumed-by` | QuadPipeline | Background, cursor, selection colors |
| Theme Engine | `consumed-by` | Sidebar UI | Sidebar colors, opacity |
| Theme Engine | `uses` | Win32 Registry | Dark/light mode detection (AppsUseLightTheme) |
| Localization | `consumed-by` | Sidebar UI | Translated strings for UI labels |
| Localization | `consumed-by` | Command Palette | Action names in current locale |
| Localization | `uses` | Win32 API | GetUserDefaultUILanguage for auto-detection |

## 13.8 Session Persistence Relations

Auto-save and restore cycle — writes every 8 seconds, reads on launch.

| From | Relation | To | Description |
|------|----------|----|-------------|
| Auto-Save Timer (8s) | `uses` | AppState Actor | Serialize workspace/pane/surface state |
| Auto-Save Timer | `persists-in` | Session JSON | `%APPDATA%\wmux\session.json` |
| App Launch | `uses` | Session JSON | Read and deserialize saved state |
| Session Restore | `uses` | Workspace Manager | Recreate workspaces with saved names/order |
| Session Restore | `uses` | PaneTree | Rebuild split layout from saved tree |
| Session Restore | `uses` | PTY Manager | Spawn shells in saved CWDs |
| Session Restore | `uses` | Browser Manager | Reopen browser panes with saved URLs |
| Session Restore | `uses` | Scrollback | Restore saved scrollback lines (best-effort) |

## 13.9 Shell Integration and Git Relations

Shell hooks provide real-time context to the sidebar via OSC sequences.

| From | Relation | To | Description |
|------|----------|----|-------------|
| Shell Hook Scripts | `triggers` | OSC 7 | CWD change reported to terminal |
| Shell Hook Scripts | `triggers` | OSC 133 | Prompt start/end markers |
| OSC 7 CWD event | `triggers` | Git Detection | Spawn `git rev-parse` in new CWD |
| Git Detection | `triggers` | Sidebar UI | Branch name + dirty status displayed |
| Port Detection | `uses` | netstat/ss | Periodic scan for listening ports |
| Port Detection | `triggers` | Sidebar UI | Port list displayed per workspace |
| Env Injection | `uses` | PTY Manager | WMUX_SURFACE_ID, WMUX_WORKSPACE_ID injected |

## 13.10 Auto-Update Relations

Background check → staged download → user-initiated install on next launch.

| From | Relation | To | Description |
|------|----------|----|-------------|
| Update Timer (hourly) | `uses` | GitHub Releases API | HTTPS check for newer version |
| Update Check | `triggers` | Download Manager | Background download to temp directory |
| Download Manager | `persists-in` | Temp directory | Staged binary for next launch |
| Update Notification | `triggers` | Title Bar | Update pill/badge displayed |
| CLI `wmux update` | `uses` | Update Check | Manual check + install trigger |

## 13.11 Visual Effects Relations

OS version detection gates visual effects — Win11 gets Mica/Acrylic, Win10 gets opaque fallback.

| From | Relation | To | Description |
|------|----------|----|-------------|
| OS Version Detection | `triggers` | DWM API | DwmSetWindowAttribute for Mica/Acrylic (Win11) |
| OS Version Detection | `triggers` | Opaque Fallback | Solid background color (Win10) |
| DWM API | `uses` | Window HWND | Backdrop effect applied to main window |
| Theme Engine | `consumed-by` | Visual Effects | Alpha/opacity values from theme |

## 13.12 Crate Dependency Relations

Inter-crate Cargo dependencies (compile-time).

| From | Relation | To | Description |
|------|----------|----|-------------|
| wmux-app | `depends-on` | wmux-ui | Window creation, event loop |
| wmux-app | `depends-on` | wmux-ipc | IPC server spawn |
| wmux-app | `depends-on` | wmux-config | Config loading at startup |
| wmux-app | `depends-on` | wmux-core | Domain types, AppState |
| wmux-ui | `depends-on` | wmux-render | GPU rendering pipeline |
| wmux-ui | `depends-on` | wmux-core | Grid data, layout, focus |
| wmux-ui | `depends-on` | wmux-pty | PTY spawn and I/O |
| wmux-ui | `depends-on` | wmux-browser | Browser panel management |
| wmux-render | `depends-on` | wmux-core | Cell grid data for rendering |
| wmux-ipc | `depends-on` | wmux-core | Domain types, AppState actor |
| wmux-ipc | `depends-on` | wmux-browser | Browser automation handlers |
| wmux-cli | `depends-on` | wmux-ipc | JSON-RPC protocol types |
| wmux-config | `depends-on` | wmux-core | Config consumed by domain types |
