# Feature Dependency Map

> Part of [wmux Architecture](ARCHITECTURE.md). See also: [Component Relations](component-relations.md), [Feature Files](feature-files.md).

This section provides a **dependency-centric** view of every component and PRD feature, organized by crate. Use this map to understand what each crate owns, how features decompose into sub-components, and where cross-crate dependencies exist.

**Node types**: `[CORE]` foundation component, `[FEATURE]` PRD feature, `[SUB]` sub-component, `[HOOK]` event hook or trigger, `[UTILITY]` shared utility.

```text
wmux
├── wmux-core [CORE], Terminal State & Domain Model
│   ├── Terminal Engine [CORE]
│   │   ├── VTE Parser (vte::Perform) [SUB]
│   │   ├── Cell Grid (contiguous Vec<Cell> per row) [SUB]
│   │   │   ├── Cell struct (char, fg, bg, attrs) [SUB]
│   │   │   ├── Color model (Indexed, Rgb, Named) [SUB]
│   │   │   ├── Cursor state (position, shape, visibility) [SUB]
│   │   │   └── Terminal Modes (insert, wraparound, origin, alt screen) [SUB]
│   │   ├── Dirty Row Tracking [SUB]
│   │   └── OSC Handlers [HOOK]
│   │       ├── OSC 7, CWD change event [HOOK]
│   │       ├── OSC 9, Notification event [HOOK]
│   │       ├── OSC 99, Kitty notification [HOOK]
│   │       ├── OSC 133, Shell prompt markers [HOOK]
│   │       └── OSC 777, RXVT notification [HOOK]
│   ├── Scrollback Ring Buffer (VecDeque, 4K lines) [SUB]
│   ├── Selection Model [SUB]
│   │   ├── Normal / Word / Line selection modes [SUB]
│   │   └── Text extraction with line boundaries [SUB]
│   ├── Event Types [UTILITY]
│   │   ├── TerminalEvent (CwdChanged, Bell, Notification, etc.) [SUB]
│   │   ├── Hyperlink (OSC 8 URI + params) [SUB]
│   │   └── PromptMark (OSC 133 prompt detection) [SUB]
│   ├── AppState Actor [CORE] (split into app_state/{mod,actor,handle}.rs)
│   │   ├── Bounded channel command dispatch [SUB]
│   │   ├── PaneRegistry (PaneId -> Terminal+PTY) [SUB]
│   │   ├── Focus routing (directional nav Up/Down/Left/Right) [SUB]
│   │   └── Focus stack per workspace [SUB]
│   ├── PaneTree (binary split tree) [CORE]
│   │   ├── Split nodes (direction, ratio) [SUB]
│   │   └── Leaf nodes (PaneId) [SUB]
│   ├── Rect Geometry [UTILITY]
│   │   ├── Split horizontal / vertical [SUB]
│   │   └── Contains point / intersection [SUB]
│   ├── Workspace Manager [CORE]
│   │   ├── Workspace lifecycle (create, select, close, rename) [SUB]
│   │   └── Per-workspace PaneTree ownership [SUB]
│   ├── Surface Manager [CORE]
│   │   ├── Surface lifecycle (create, close, list) [SUB]
│   │   └── Tab bar state per pane (Ctrl+T, Ctrl+Tab) [SUB]
│   ├── Notification Store [FEATURE]
│   │   ├── Lifecycle (Received -> Unread -> Read -> Cleared) [SUB]
│   │   └── Workspace badge counters [SUB]
│   ├── MetadataStore [FEATURE]
│   │   ├── Statuses (keyed badges with icon + color) [SUB]
│   │   ├── Progress entries (0.0-1.0 + optional label) [SUB]
│   │   ├── Log entries (info/progress/success/warning/error, capped 100) [SUB]
│   │   └── PID sweep timer (30s, clears stale statuses) [SUB]
│   ├── Command Registry [FEATURE]
│   │   ├── Action catalog (ShortcutAction variants) [SUB]
│   │   └── Palette entries with fuzzy-search tokens [SUB]
│   ├── Git Detector [FEATURE]
│   │   ├── Branch + dirty state (spawned git rev-parse on OSC 7) [SUB]
│   │   └── Workspace-scoped cache [SUB]
│   ├── Port Scanner [FEATURE]
│   │   ├── Listening port detection (netstat/ss) [SUB]
│   │   └── Per-workspace pill list [SUB]
│   ├── Remote Workspace Model [FEATURE]
│   │   ├── SSH remote state (host, connected, reconnect) [SUB]
│   │   └── Sidebar SSH indicator [SUB]
│   ├── Session Persistence [FEATURE]
│   │   ├── Serializable state (serde) [SUB]
│   │   ├── Schema versioning [SUB]
│   │   └── Auto-save orchestration helpers [SUB]
│   ├── Domain ID Types [UTILITY]
│   │   ├── PaneId, WorkspaceId, SurfaceId, WindowId [SUB]
│   │   └── Newtype wrappers (Uuid-based) [SUB]
│   └── Domain Enums [UTILITY]
│       ├── SplitDirection, PanelKind [SUB]
│       └── NotificationLevel, SecurityMode [SUB]
│
├── wmux-pty [CORE], ConPTY Abstraction
│   ├── PTY Manager [CORE]
│   │   ├── Shell Detection (pwsh, powershell, cmd) [SUB]
│   │   ├── ConPTY Spawn (portable-pty) [SUB]
│   │   ├── Env injection (WMUX_SURFACE_ID, etc.) [SUB]
│   │   └── Resize handling [SUB]
│   └── PTY Async I/O [CORE]
│       ├── tokio::spawn_blocking read loop [SUB]
│       ├── Write channel (bytes to ConPTY stdin) [SUB]
│       └── Process spawn wrapper (spawn.rs) [SUB]
│
├── wmux-render [CORE], GPU Rendering Pipeline
│   ├── GPU Context [CORE]
│   │   ├── wgpu Instance/Adapter/Device/Queue [SUB]
│   │   ├── Surface configuration (D3D12) [SUB]
│   │   └── Resize handling [SUB]
│   ├── GlyphonRenderer [CORE]
│   │   ├── cosmic-text FontSystem [SUB]
│   │   ├── Text atlas (etagere packing) [SUB]
│   │   └── Buffer to TextArea rendering [SUB]
│   ├── QuadPipeline [CORE]
│   │   ├── WGSL vertex/fragment shaders [SUB]
│   │   ├── Instance buffer (colored rectangles) [SUB]
│   │   └── Batch rendering [SUB]
│   ├── SVG Icon Renderer [FEATURE]
│   │   ├── Codicons registry (icons.rs) [SUB]
│   │   └── SVG to glyphon CustomGlyph conversion (svg_icons.rs) [SUB]
│   ├── Shadow Pipeline [SUB]
│   │   ├── Drop shadow shader (shadow.wgsl) [SUB]
│   │   └── Per-pane and overlay shadows [SUB]
│   ├── Terminal Renderer [FEATURE]
│   │   ├── Grid cells to glyph upload (dirty rows only) [SUB]
│   │   ├── Cursor rendering (block, underline, bar) [SUB]
│   │   ├── Selection highlight [SUB]
│   │   └── Search match highlight [SUB]
│   └── Pane Renderer [FEATURE]
│       ├── Per-pane terminal rendering [SUB]
│       └── Focus glow effect on active pane [SUB]
│
├── wmux-ui [CORE], Window Management & Layout
│   ├── App (winit ApplicationHandler) [CORE] (split into window/{mod,event_loop,handlers,render}.rs)
│   │   ├── Win32 message pump [SUB]
│   │   ├── RequestRedraw scheduling [SUB]
│   │   ├── Window lifecycle (create, resize, close) [SUB]
│   │   └── Handler tables (shortcut + action) [SUB]
│   ├── Keyboard Input [CORE]
│   │   ├── Shortcut Dispatcher (shortcuts.rs) [SUB]
│   │   ├── Global shortcuts (Ctrl+N, Ctrl+D, Ctrl+Shift+P, Ctrl+F, Ctrl+Shift+I, F12, etc.) [SUB]
│   │   └── Terminal passthrough (raw bytes to PTY) [SUB]
│   ├── Mouse Input [CORE]
│   │   ├── Text selection (click, drag, double-click word) [SUB]
│   │   ├── Scroll (wheel to scrollback / mouse reporting) [SUB]
│   │   └── Divider drag detection [SUB]
│   ├── Multi-Pane Rendering [FEATURE]
│   │   ├── PaneTree to viewport rects [SUB]
│   │   └── Per-pane GPU render pass [SUB]
│   ├── Draggable Dividers [FEATURE] (divider.rs)
│   │   ├── Hit-testing (cursor on divider edge) [SUB]
│   │   └── Ratio adjustment on drag [SUB]
│   ├── Custom Title Bar [FEATURE] (titlebar.rs)
│   │   ├── WM_NCCALCSIZE + WM_NCHITTEST subclass [SUB]
│   │   ├── Min/max/restore/close buttons (Codicons) [SUB]
│   │   └── Theme-driven colors [SUB]
│   ├── Status Bar [FEATURE] (status_bar.rs)
│   ├── Sidebar UI [FEATURE] (sidebar.rs)
│   │   ├── Workspace list rendering [SUB]
│   │   ├── Metadata display (git, ports, status, progress, logs) [SUB]
│   │   ├── Port badges (colored pills, alpha 15% + centered text) [SUB]
│   │   ├── Badge counters (unread notifications) [SUB]
│   │   ├── Collapsed mode (icon-only, 48px) [SUB]
│   │   ├── SSH workspace indicator [SUB]
│   │   └── Drag-and-drop reorder [SUB]
│   ├── Command Palette [FEATURE] (command_palette.rs)
│   │   ├── Overlay rendering (Ctrl+Shift+P) [SUB]
│   │   ├── Fuzzy search over Command Registry [SUB]
│   │   ├── Result buffers + shortcut buffers [SUB]
│   │   └── Action dispatch [SUB]
│   ├── Notification Panel [FEATURE] (notification_panel.rs)
│   │   ├── Overlay list (Ctrl+Shift+I) [SUB]
│   │   ├── Category buffers (titles, bodies, timestamps) [SUB]
│   │   ├── Header + clear-all [SUB]
│   │   └── Jump-to-unread (Ctrl+Shift+U) [SUB]
│   ├── Terminal Search [FEATURE] (search.rs)
│   │   ├── Search overlay (Ctrl+F) [SUB]
│   │   ├── Match navigation (n/N) [SUB]
│   │   └── Regex support [SUB]
│   ├── Address Bar [FEATURE] (address_bar.rs)
│   ├── Toast Service [SUB] (toast.rs)
│   │   ├── Windows Toast Notification API (WinRT) [SUB]
│   │   └── AUMID setup (SetCurrentProcessExplicitAppUserModelID) [SUB]
│   ├── Animation System [SUB] (animation.rs)
│   ├── Typography Tokens [UTILITY] (typography.rs)
│   └── Visual Effects [FEATURE] (effects.rs)
│       ├── Mica/Acrylic (Win11 DWM) [SUB]
│       └── Opaque fallback (Win10) [SUB]
│
├── wmux-ipc [CORE], Named Pipes Server & JSON-RPC v2
│   ├── IPC Server [CORE]
│   │   ├── Named Pipe listener (tokio async) [SUB]
│   │   ├── Connection lifecycle [SUB]
│   │   └── Pipe naming (\\.\pipe\wmux-*) [SUB]
│   ├── JSON-RPC Protocol [CORE]
│   │   ├── Request/Response codec [SUB]
│   │   ├── Error codes (cmux-compatible) [SUB]
│   │   └── Newline-delimited framing [SUB]
│   ├── Authentication [FEATURE]
│   │   ├── HMAC-SHA256 challenge-response [SUB]
│   │   ├── Child-process detection (wmux-only mode) [SUB]
│   │   └── Security modes (off, wmux-only, password, allowAll) [SUB]
│   ├── Handler / Router [CORE]
│   │   ├── Handler trait (one impl per domain) [SUB]
│   │   └── Method dispatch (namespace.method) [SUB]
│   ├── System Handlers [SUB], system.ping, .capabilities, .identify, .tree
│   ├── Workspace Handlers [SUB], workspace.list, .create, .select, .close, .rename
│   ├── Surface Handlers [SUB], surface.split, .list, .focus, .close, .send_text, .send_key, .read_text
│   ├── Sidebar Handlers [SUB], sidebar.set_status, .set_progress, .log, .state, plus notification.create, .list, .clear (notifications owned by metadata store)
│   └── Browser Handlers [SUB], browser.open, .navigate, .click, .fill, .eval, .snapshot, .screenshot, .cookies, .storage, 30+ methods
│
├── wmux-cli [CORE], CLI Client Binary
│   ├── CLI Client [CORE]
│   │   ├── Named Pipe connector (client.rs) [SUB]
│   │   ├── JSON-RPC request builder [SUB]
│   │   └── Output formatter (output.rs, human + --json) [SUB]
│   ├── System Commands [SUB], system.* (ping, capabilities, tree)
│   ├── Workspace Commands [SUB], list, create, select, close, rename
│   ├── Surface Commands [SUB], split, list, focus, close, send-text, send-key, read-text
│   ├── Sidebar Commands [SUB], set-status, set-progress, log, state
│   ├── Browser Commands [SUB], Open/Navigate/Back/Forward/Reload/Url/Eval (7 of 30+ IPC methods, Backlog #5 extends this)
│   ├── Notify Commands [SUB], Create/List/Clear (stub, Backlog #6)
│   └── SSH Commands [SUB], Connect/Disconnect/List (stub, Backlog #7, waits on daemon)
│
├── wmux-browser [FEATURE], WebView2 Integration
│   ├── Browser Manager [CORE] (manager.rs)
│   │   ├── WebView2 COM initialization (CoInitializeEx) [SUB]
│   │   ├── RAII COM wrappers (com.rs, Drop-based cleanup) [SUB]
│   │   └── Environment + Controller lifecycle [SUB]
│   ├── Browser Panel [FEATURE] (panel/{mod,attach,delegation,layout}.rs)
│   │   ├── Child HWND creation + parenting (attach.rs) [SUB]
│   │   ├── Input delegation to WebView2 (delegation.rs) [SUB]
│   │   ├── Position/size sync with PaneTree rects (layout.rs) [SUB]
│   │   ├── DevTools open (layout.rs, F12 wiring pending Backlog #4) [SUB]
│   │   └── Show/hide on workspace switch [SUB]
│   └── Automation [FEATURE] (automation/{mod,dom,inspect,navigation}.rs)
│       ├── Navigation (navigate, back, forward, reload) [SUB]
│       ├── JavaScript evaluation (eval, addscript) [SUB]
│       ├── DOM interaction (click, fill, type, select, check, hover, focus) [SUB]
│       ├── Accessibility tree snapshot (snapshot) [SUB]
│       ├── Query helpers (get, is, find, wait) [SUB]
│       ├── Screenshot / PDF capture [SUB]
│       └── Cookie / storage management [SUB]
│
├── wmux-config [FEATURE], Configuration Parsing
│   ├── Config Manager [CORE] (config.rs)
│   │   ├── File discovery (%APPDATA%\wmux\, %APPDATA%\ghostty\) [SUB]
│   │   ├── Priority merge (wmux > ghostty > defaults) [SUB]
│   │   ├── Font settings (family, size, ligatures) [SUB]
│   │   └── Keybinding config HashMap (parser stores it, wiring pending Backlog #3) [SUB]
│   ├── Parser (Ghostty-compat key=value) [SUB] (parser.rs)
│   ├── Theme Engine [FEATURE] (theme/{mod,chrome,registry,types}.rs)
│   │   ├── Chrome color tokens (chrome.rs) [SUB]
│   │   ├── Theme registry with 8 bundled themes (registry.rs) [SUB]
│   │   ├── Typed palettes (types.rs) [SUB]
│   │   ├── Dark/light mode detection (Win32 registry) [SUB]
│   │   └── Live theme switching (no restart) [SUB]
│   └── Localization [FEATURE] (locale.rs)
│       ├── Locale TOML loading (en.toml, fr.toml) [SUB]
│       ├── System language detection (GetUserDefaultUILanguage) [SUB]
│       └── Runtime language switching [SUB]
│
├── wmux-app [CORE], Main Application Binary
│   ├── Entry Point [CORE] (main.rs, main() to App::run())
│   ├── Config Loading [SUB], wmux-config initialization
│   ├── IPC Spawn [SUB], wmux-ipc server start
│   ├── Actor Spawn [SUB], AppState actor initialization
│   ├── Auto-Updater [FEATURE] (updater.rs)
│   │   ├── Hourly GitHub Releases poll [SUB]
│   │   ├── SHA-256 digest verification [SUB]
│   │   ├── HTTPS + host allowlist [SUB]
│   │   ├── 200MB download cap [SUB]
│   │   └── Atomic install path containment [SUB]
│   └── Graceful Shutdown [SUB], coordinated cleanup
│
└── wmuxd-remote [FEATURE], SSH Remote Daemon (Go, planned)
    ├── Status: directory not yet in repo (Backlog #7)
    ├── PTY relay [SUB]
    ├── Browser proxy (SOCKS5/HTTP CONNECT) [SUB]
    ├── CLI relay (reverse TCP forward) [SUB]
    └── Multi-client resize coordination [SUB]
```
