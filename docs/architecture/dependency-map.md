# Feature Dependency Map

> Part of [wmux Architecture](ARCHITECTURE.md). See also: [Component Relations](component-relations.md), [Feature Files](feature-files.md).

This section provides a **dependency-centric** view of every component and PRD feature, organized by crate. Use this map to understand what each crate owns, how features decompose into sub-components, and where cross-crate dependencies exist.

**Node types**: `[CORE]` foundation component · `[FEATURE]` PRD feature · `[SUB]` sub-component · `[HOOK]` event hook / trigger · `[UTILITY]` shared utility

```
wmux
├── wmux-core [CORE] — Terminal State & Domain Model
│   ├── Terminal Engine [CORE]
│   │   ├── VTE Parser (vte::Perform) [SUB]
│   │   ├── Cell Grid (contiguous Vec<Cell> per row) [SUB]
│   │   │   ├── Cell struct (char, fg, bg, attrs) [SUB]
│   │   │   ├── Color model (Indexed, Rgb, Named) [SUB]
│   │   │   ├── Cursor state (position, shape, visibility) [SUB]
│   │   │   └── Terminal Modes (insert, wraparound, origin, alt screen) [SUB]
│   │   ├── Dirty Row Tracking [SUB]
│   │   └── OSC Handlers [HOOK]
│   │       ├── OSC 7  → CWD change event [HOOK]
│   │       ├── OSC 9  → Notification event [HOOK]
│   │       ├── OSC 99 → Kitty notification [HOOK]
│   │       ├── OSC 133 → Shell prompt markers [HOOK]
│   │       └── OSC 777 → RXVT notification [HOOK]
│   ├── Scrollback Ring Buffer (VecDeque, 4K lines) [SUB]
│   ├── AppState Actor [CORE]
│   │   ├── Bounded channel command dispatch [SUB]
│   │   └── PaneRegistry (PaneId → Terminal+PTY) [SUB]
│   ├── PaneTree (binary split tree) [CORE]
│   │   ├── Split nodes (direction, ratio) [SUB]
│   │   └── Leaf nodes (PaneId) [SUB]
│   ├── Focus Routing [SUB]
│   │   ├── Directional navigation (Up/Down/Left/Right) [SUB]
│   │   └── Focus stack per workspace [SUB]
│   ├── Surface Tabs [FEATURE]
│   │   └── Tab bar per pane (Ctrl+T, Ctrl+Tab) [SUB]
│   ├── Notification Store [FEATURE]
│   │   ├── Lifecycle (Received → Unread → Read → Cleared) [SUB]
│   │   └── Workspace badge counters [SUB]
│   ├── Session Persistence [FEATURE]
│   │   ├── Serializable state (serde) [SUB]
│   │   └── Schema versioning [SUB]
│   ├── Command Registry [SUB]
│   ├── Domain ID Types [UTILITY]
│   │   ├── PaneId, WorkspaceId, SurfaceId, WindowId [SUB]
│   │   └── Newtype wrappers (Uuid-based) [SUB]
│   └── Domain Enums [UTILITY]
│       ├── SplitDirection, PanelKind [SUB]
│       └── NotificationLevel, SecurityMode [SUB]
│
├── wmux-pty [CORE] — ConPTY Abstraction
│   ├── PTY Manager [CORE]
│   │   ├── Shell Detection (pwsh → powershell → cmd) [SUB]
│   │   ├── ConPTY Spawn (portable-pty) [SUB]
│   │   ├── Env injection (WMUX_SURFACE_ID, etc.) [SUB]
│   │   └── Resize handling [SUB]
│   └── PTY Async I/O [CORE]
│       ├── tokio::spawn_blocking read loop [SUB]
│       └── Write channel (bytes → ConPTY stdin) [SUB]
│
├── wmux-render [CORE] — GPU Rendering Pipeline
│   ├── GPU Context [CORE]
│   │   ├── wgpu Instance/Adapter/Device/Queue [SUB]
│   │   ├── Surface configuration (D3D12) [SUB]
│   │   └── Resize handling [SUB]
│   ├── GlyphonRenderer [CORE]
│   │   ├── cosmic-text FontSystem [SUB]
│   │   ├── Text atlas (etagere packing) [SUB]
│   │   └── Buffer → TextArea rendering [SUB]
│   ├── QuadPipeline [CORE]
│   │   ├── WGSL vertex/fragment shaders [SUB]
│   │   ├── Instance buffer (colored rectangles) [SUB]
│   │   └── Batch rendering [SUB]
│   └── Terminal Renderer [FEATURE]
│       ├── Grid cells → glyph upload (dirty rows only) [SUB]
│       ├── Cursor rendering (block, underline, bar) [SUB]
│       ├── Selection highlight [SUB]
│       └── Search match highlight [SUB]
│
├── wmux-ui [CORE] — Window Management & Layout
│   ├── App (winit ApplicationHandler) [CORE]
│   │   ├── Win32 message pump [SUB]
│   │   ├── RequestRedraw scheduling [SUB]
│   │   └── Window lifecycle (create, resize, close) [SUB]
│   ├── Keyboard Input [CORE]
│   │   ├── Shortcut priority dispatcher [SUB]
│   │   ├── Global shortcuts (Ctrl+N, Ctrl+D, etc.) [SUB]
│   │   └── Terminal passthrough (raw bytes → PTY) [SUB]
│   ├── Mouse Input [CORE]
│   │   ├── Text selection (click, drag, double-click word) [SUB]
│   │   ├── Scroll (wheel → scrollback / mouse reporting) [SUB]
│   │   └── Divider drag detection [SUB]
│   ├── Multi-Pane Rendering [FEATURE]
│   │   ├── PaneTree → viewport rects [SUB]
│   │   └── Per-pane GPU render pass [SUB]
│   ├── Draggable Dividers [FEATURE]
│   │   ├── Hit-testing (cursor on divider edge) [SUB]
│   │   └── Ratio adjustment on drag [SUB]
│   ├── Sidebar UI [FEATURE]
│   │   ├── Workspace list rendering [SUB]
│   │   ├── Metadata display (git, status, progress, logs) [SUB]
│   │   ├── Badge counters [SUB]
│   │   └── Drag-and-drop reorder [SUB]
│   ├── Command Palette [FEATURE]
│   │   ├── Overlay rendering (Ctrl+Shift+P) [SUB]
│   │   ├── Fuzzy search engine [SUB]
│   │   └── Action dispatch [SUB]
│   ├── Terminal Search [FEATURE]
│   │   ├── Search overlay (Ctrl+F) [SUB]
│   │   ├── Match navigation (n/N) [SUB]
│   │   └── Regex support [SUB]
│   └── Visual Effects [FEATURE]
│       ├── Mica/Acrylic (Win11 DWM) [SUB]
│       └── Opaque fallback (Win10) [SUB]
│
├── wmux-ipc [CORE] — Named Pipes Server & JSON-RPC v2
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
│   ├── System Handlers [SUB] — system.ping, .capabilities, .identify, .tree
│   ├── Workspace Handlers [SUB] — workspace.list, .create, .select, .close, .rename
│   ├── Surface Handlers [SUB] — surface.split, .list, .focus, .close
│   ├── Input/Read Handlers [SUB] — surface.send_text, .send_key, .read_text
│   ├── Sidebar Handlers [SUB] — sidebar.set_status, .set_progress, .log, .state
│   ├── Browser Handlers [SUB] — browser.open, .navigate, .click, .fill, .eval, .snapshot, .screenshot
│   └── Notification Handlers [SUB] — notification.create, .list, .clear
│
├── wmux-cli [CORE] — CLI Client Binary
│   ├── CLI Client [CORE]
│   │   ├── Named Pipe connector [SUB]
│   │   ├── JSON-RPC request builder [SUB]
│   │   └── Output formatter (human + --json) [SUB]
│   ├── Workspace Commands [SUB] — list, create, select, close, rename
│   ├── Surface Commands [SUB] — split, list, focus, close, read-text
│   ├── Input Commands [SUB] — send-text, send-key
│   ├── Sidebar Commands [SUB] — set-status, set-progress, log, state
│   ├── Browser Commands [SUB] — open, navigate, click, fill, eval, snapshot, screenshot
│   ├── Notification Commands [SUB] — create, list, clear
│   ├── SSH Commands [SUB] — connect, disconnect, list
│   ├── Update Commands [SUB] — check, install
│   └── Theme Commands [SUB] — list, set, clear
│
├── wmux-browser [FEATURE] — WebView2 Integration
│   ├── Browser Manager [CORE]
│   │   ├── WebView2 COM initialization (CoInitializeEx) [SUB]
│   │   ├── RAII COM wrappers (Drop-based cleanup) [SUB]
│   │   └── Environment + Controller lifecycle [SUB]
│   ├── Browser Panel [FEATURE]
│   │   ├── Child HWND creation + parenting [SUB]
│   │   ├── Position/size sync with PaneTree rects [SUB]
│   │   └── Show/hide on workspace switch [SUB]
│   └── Navigation / JS Eval / DOM Automation [FEATURE]
│       ├── URL navigation + history (back, forward, reload) [SUB]
│       ├── JavaScript evaluation (eval, addscript) [SUB]
│       ├── DOM interaction (click, fill, type, select) [SUB]
│       ├── Accessibility tree snapshot [SUB]
│       ├── Screenshot / PDF capture [SUB]
│       └── Cookie / storage management [SUB]
│
├── wmux-config [FEATURE] — Configuration Parsing
│   ├── Config Manager [CORE]
│   │   ├── File discovery (%APPDATA%\wmux\, %APPDATA%\ghostty\) [SUB]
│   │   └── Priority merge (wmux > ghostty > defaults) [SUB]
│   ├── Parser (Ghostty-compat key=value) [SUB]
│   ├── Config Struct [SUB]
│   │   ├── Terminal settings (scrollback, cursor, bell) [SUB]
│   │   ├── Font settings (family, size, ligatures) [SUB]
│   │   └── Keybinding configuration [SUB]
│   ├── Theme Engine [FEATURE]
│   │   ├── Color palette loading (16 + 256 + rgb) [SUB]
│   │   ├── Dark/light mode detection (Win32 registry) [SUB]
│   │   └── Live theme switching (no restart) [SUB]
│   └── Localization [FEATURE]
│       ├── Locale TOML loading (en.toml, fr.toml) [SUB]
│       ├── System language detection (GetUserDefaultUILanguage) [SUB]
│       └── Runtime language switching [SUB]
│
├── wmux-app [CORE] — Main Application Binary
│   ├── Entry Point [CORE] — main() → App::run()
│   ├── Config Loading [SUB] — wmux-config initialization
│   ├── IPC Spawn [SUB] — wmux-ipc server start
│   ├── Actor Spawn [SUB] — AppState actor initialization
│   ├── Updater [FEATURE] — background update check
│   └── Graceful Shutdown [SUB] — coordinated cleanup
│
└── daemon/ [FEATURE] — SSH Remote Daemon (Go)
    └── wmuxd-remote [CORE]
        ├── PTY relay [SUB]
        ├── Browser proxy (SOCKS5/HTTP CONNECT) [SUB]
        ├── CLI relay (reverse TCP forward) [SUB]
        └── Multi-client resize coordination [SUB]
```
