# System Architecture (C4 Model)

> Part of [wmux Architecture](ARCHITECTURE.md). See also: [Component Breakdown](ARCHITECTURE.md#5-component-breakdown), [Component Relations](component-relations.md).

## Level 1: System Context

```mermaid
C4Context
    title System Context — wmux

    Person(dev, "Developer", "Uses wmux as primary terminal<br/>on Windows for AI-assisted coding")
    Person(agent, "AI Agent", "Claude Code, Codex, OpenCode<br/>controls wmux programmatically")

    System(wmux, "wmux", "Native Windows terminal multiplexer<br/>with GPU rendering, split panes,<br/>integrated browser, and IPC API")

    System_Ext(shell, "Shell Processes", "PowerShell, cmd, bash,<br/>WSL distributions")
    System_Ext(ssh, "SSH Remote Hosts", "Remote machines with<br/>wmuxd-remote daemon (Go)")
    System_Ext(github, "GitHub Releases", "Auto-update checks<br/>and binary downloads")
    System_Ext(webview2rt, "WebView2 Runtime", "Chromium (Edge) for<br/>integrated browser panes")

    Rel(dev, wmux, "Uses keyboard, mouse,<br/>command palette")
    Rel(agent, wmux, "Controls via Named Pipes<br/>JSON-RPC v2 IPC")
    Rel(wmux, shell, "Spawns via ConPTY,<br/>reads/writes I/O")
    Rel(wmux, ssh, "SSH tunnel +<br/>reverse CLI relay")
    Rel(wmux, github, "Checks for updates<br/>HTTPS REST")
    Rel(wmux, webview2rt, "Hosts browser panes<br/>COM interop")
```

## Level 2: Container Diagram

```mermaid
C4Container
    title Container Diagram — wmux

    Person(dev, "Developer")
    Person(agent, "AI Agent")

    Container_Boundary(wmux_system, "wmux System") {
        Container(app, "wmux-app", "Rust binary", "Main application: window,<br/>GPU rendering, multiplexer,<br/>terminal engine, IPC server")
        Container(cli, "wmux-cli", "Rust binary", "CLI client: 80+ commands<br/>for programmatic control")
        Container(daemon, "wmuxd-remote", "Go binary", "SSH remote daemon:<br/>session relay, browser proxy")
        ContainerDb(session, "Session File", "JSON", "Auto-saved layout,<br/>scrollback, browser URLs<br/>(%APPDATA%\\wmux\\session.json)")
        ContainerDb(config, "Config Files", "Ghostty-compat", "Themes, fonts, keybindings<br/>(%APPDATA%\\wmux\\config)")
    }

    System_Ext(shell, "Shell Processes")
    System_Ext(webview2, "WebView2 Runtime")
    System_Ext(ssh_host, "SSH Remote Host")

    Rel(dev, app, "Keyboard, mouse,<br/>window events", "Win32")
    Rel(agent, cli, "Invokes commands", "Process exec")
    Rel(cli, app, "JSON-RPC v2", "Named Pipe")
    Rel(app, shell, "Spawns, I/O", "ConPTY")
    Rel(app, webview2, "Browser panes", "COM/HWND")
    Rel(app, session, "Read/Write", "tokio::fs")
    Rel(app, config, "Read", "std::fs")
    Rel(app, daemon, "Tunnel", "SSH + reverse TCP")
    Rel(daemon, ssh_host, "PTY relay", "SSH")
```

## Level 3: Component Diagram — wmux-app

```mermaid
C4Component
    title Component Diagram — wmux-app (Main Application)

    Container_Boundary(app, "wmux-app") {

        Component(event_loop, "Event Loop", "winit ApplicationHandler", "Win32 message pump,<br/>input dispatch, redraw")
        Component(gpu_ctx, "GPU Context", "wgpu + glyphon", "D3D12 surface, text atlas,<br/>glyph rendering pipeline")
        Component(ui_layer, "UI Layer", "Custom wgpu rendering", "Sidebar, overlays,<br/>command palette, search")
        Component(mux, "Multiplexer", "PaneTree + WorkspaceManager", "Binary split tree,<br/>focus routing, workspace lifecycle")
        Component(terminal, "Terminal Engine", "vte + Grid + Scrollback", "VTE parsing, cell grid,<br/>ring buffer scrollback,<br/>mode/cursor state")
        Component(pty_mgr, "PTY Manager", "portable-pty + tokio", "ConPTY spawn, I/O pipes,<br/>shell detection, env injection")
        Component(ipc_srv, "IPC Server", "Named Pipes + JSON-RPC v2", "Request dispatch, auth,<br/>80+ command handlers")
        Component(browser, "Browser Manager", "webview2-com + HWND", "WebView2 lifecycle,<br/>automation API, DevTools")
        Component(notif, "Notification Manager", "windows crate Toast API", "OSC detection, Toast,<br/>visual badges, panel")
        Component(persist, "Session Persistence", "serde_json + tokio::fs", "Auto-save 8s interval,<br/>restore on launch")
        Component(config_mgr, "Config Manager", "toml + dirs", "Ghostty-compat parsing,<br/>theme engine, dark/light detect")
    }

    ContainerDb(session_file, "Session JSON")
    ContainerDb(config_files, "Config Files")
    System_Ext(conpty, "ConPTY")
    System_Ext(wv2, "WebView2 Runtime")
    System_Ext(pipe_client, "CLI / AI Agent")

    Rel(event_loop, gpu_ctx, "Resize, redraw")
    Rel(event_loop, ui_layer, "Input events")
    Rel(ui_layer, mux, "Split, focus,<br/>workspace ops")
    Rel(mux, terminal, "Route PTY output<br/>to active grid")
    Rel(terminal, pty_mgr, "Read/Write bytes")
    Rel(pty_mgr, conpty, "Spawn, resize, I/O")
    Rel(gpu_ctx, terminal, "Read dirty rows<br/>for rendering")
    Rel(pipe_client, ipc_srv, "JSON-RPC v2")
    Rel(ipc_srv, mux, "Execute commands")
    Rel(ipc_srv, browser, "Browser automation")
    Rel(browser, wv2, "COM calls")
    Rel(terminal, notif, "OSC 9/99/777 events")
    Rel(persist, session_file, "Read/Write JSON")
    Rel(config_mgr, config_files, "Read TOML/themes")
```
