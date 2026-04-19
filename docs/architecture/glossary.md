# Glossary

| Term | Definition | Context |
|------|-----------|---------|
| **cmux** | macOS-native terminal multiplexer by Manaflow AI, built on Swift/AppKit + Ghostty. wmux's reference implementation | System Context, IPC protocol compatibility |
| **ConPTY** | Console Pseudo Terminal — Windows API (since Win10 1809) for pseudo-terminal support, equivalent to Unix `openpty` | wmux-pty, shell spawning |
| **Named Pipes** | Windows IPC mechanism (`\\.\pipe\*`) — equivalent to Unix domain sockets. Supports ACL-based access control | wmux-ipc, CLI communication |
| **JSON-RPC v2** | Lightweight remote procedure call protocol using JSON. wmux uses newline-delimited variant compatible with cmux | wmux-ipc protocol layer |
| **WebView2** | Microsoft's embedded Chromium browser component (based on Edge). Pre-installed on Windows 10 20H2+ and all Windows 11 | wmux-browser, browser panes |
| **wgpu** | Rust implementation of the WebGPU standard. Maps to Direct3D 12 on Windows, Vulkan on Linux, Metal on macOS | wmux-render, GPU rendering |
| **glyphon** | Rust crate for GPU text rendering via wgpu. Built on cosmic-text (layout) + swash (rasterization) + etagere (atlas packing) | wmux-render, text rendering |
| **VTE** | Virtual Terminal Emulator — standard for terminal escape sequences (ANSI, xterm, etc.). The `vte` crate parses these | wmux-core, terminal parsing |
| **OSC** | Operating System Command — a category of terminal escape sequences. OSC 7 = CWD, OSC 9/99/777 = notifications, OSC 133 = prompt marks | wmux-core VTE handler, notifications |
| **Window** | Top-level native Win32 window with its own sidebar. wmux supports multiple windows. Level 1 in the 5-level hierarchy | wmux-ui, Win32 HWND |
| **Workspace** | Named collection of panes — appears as a vertical tab in the sidebar. Shows git branch, CWD, ports, statuses, badges. Level 2 in hierarchy | wmux-core, sidebar UI |
| **Pane** | A split region within a workspace, arranged in a binary split tree. Each pane contains one or more surfaces (tabs). Level 3 in hierarchy | wmux-core, split container |
| **Surface** | An individual tab within a pane, identified by `WMUX_SURFACE_ID`. Can hold a terminal or browser panel. Level 4 in hierarchy | wmux-core, IPC targeting |
| **Panel** | The content inside a surface — either a terminal (ConPTY session) or a browser (WebView2 instance). Level 5 in hierarchy | wmux-core, wmux-pty, wmux-browser |
| **Split Tree** | Binary tree data structure where leaves are panes and internal nodes represent horizontal/vertical splits with a ratio | wmux-core pane_tree |
| **Glyph Atlas** | GPU texture containing rasterized font glyphs, managed by glyphon/etagere. Cached across frames | wmux-render, text rendering |
| **Dirty Row** | A terminal grid row that has changed since the last GPU upload. Only dirty rows are re-uploaded for rendering | wmux-core → wmux-render optimization |
| **HMAC-SHA256** | Hash-based Message Authentication Code using SHA-256. Used for IPC authentication in `password` security mode | wmux-ipc auth |
| **Actor Pattern** | Concurrency pattern: a dedicated async task owns state, receives commands via a bounded channel. Used instead of `Arc<Mutex<T>>` | wmux-ipc server, PTY manager |
| **Mica / Acrylic** | Windows 11 visual effects for translucent backgrounds. Mica uses desktop wallpaper, Acrylic uses blur. Both via DWM API | wmux-ui sidebar, Win11 only |
| **DWM** | Desktop Window Manager — Windows compositor. Provides backdrop effects (Mica, Acrylic), rounded corners | wmux-ui visual effects |
| **DirectWrite** | Windows font rasterization API. Not used directly by wmux — glyphon uses cosmic-text/swash instead | Referenced in alternatives |
| **Ghostty** | GPU-accelerated terminal emulator (Zig/Metal on macOS). cmux embeds Ghostty as its terminal engine. wmux builds its own engine in Rust | cmux analysis, config compatibility |
| **wmuxd-remote** | Go daemon running on remote SSH hosts. Manages remote sessions, browser proxy, CLI relay. Reused from cmux (cmuxd-remote) | SSH remote support |
| **SOCKS5 / HTTP CONNECT** | Proxy protocols used by wmuxd-remote to relay browser traffic from local wmux to remote web servers | SSH browser proxy |
| **Ring Buffer** | Circular buffer (VecDeque) for terminal scrollback. Fixed capacity, oldest lines evicted when full | wmux-core scrollback |
| **Codicons** | VS Code icon font, rendered as SVG and converted to glyphon `CustomGlyph` by `wmux-render/src/svg_icons.rs`. Used for titlebar controls, sidebar icons, palette items. Colorized via theme | wmux-render icons, wmux-ui chrome |
| **Custom Title Bar** | GPU-rendered non-client area implemented in `wmux-ui/src/titlebar.rs`. Uses `WM_NCCALCSIZE` + `WM_NCHITTEST` via `SetWindowSubclass` to draw the whole window frame in wgpu. Min/max/restore/close are Codicons | wmux-ui titlebar |
| **MetadataStore** | Owner of sidebar state per workspace: keyed statuses, progress entries, log entries. Runs a PID sweep every 30s to clear statuses from dead processes. Also owns notifications | wmux-core metadata_store |
| **Focus Glow** | Visual cue on the active pane, rendered by `wmux-render/src/pane.rs::render_focus_glow` and called from the main render loop. Accent color driven by theme | wmux-render pane, theme |
| **Command Registry** | Action catalog consumed by the Command Palette and the shortcut dispatcher. Stores action name, description, localized label, optional shortcut | wmux-core command_registry |
| **Port Scanner** | Listening port detector polling `netstat`/`ss` equivalents, grouped per workspace. Drives the colored port pills in the sidebar | wmux-core port_scanner |
| **Git Detector** | `git rev-parse` + `git status` spawner triggered by OSC 7 CWD change. Populates branch name and dirty flag on the workspace metadata | wmux-core git_detector |
