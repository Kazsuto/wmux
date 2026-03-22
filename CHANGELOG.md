# Changelog

## 2026-03-22

REFACTOR: Apply clean code improvements to wmux-core — #[allow] → #[expect] with reasons in selection.rs, match → let-else in session.rs/vte_handler.rs, Vec::with_capacity pre-allocation in actor build_surface_list

REFACTOR: Decompose wmux-core app_state.rs (2418 lines) into module directory — mod.rs (types), handle.rs (client API), actor.rs (actor loop + handlers)

REFACTOR: Apply clean code improvements to wmux-core — derive Copy on Attrs (all fields are Copy), replace clone_on_copy in vte_handler, convert manual scrollback loop to iterator chain, simplify filter_map in metadata sweep

REFACTOR: Apply clean code improvements to wmux-config — add Debug derive to ThemeEngine, add PartialEq derive to Config (C-COMMON-TRAITS)

REFACTOR: Apply clean code improvements to wmux-render — DRY tab width/position calculation in PaneRenderer (render_tab_bar now delegates to tab_metrics instead of duplicating formula)

REFACTOR: Decompose wmux-ui window.rs (2148 lines) into 4 submodules — mod.rs (struct defs), render.rs (render pipeline), handlers.rs (shortcut/input handlers), event_loop.rs (ApplicationHandler impl)

REFACTOR: Apply clean code improvements to wmux-ui — extract spawn_split helper (DRY, eliminates SplitRight/SplitDown duplication), avoid surface_titles.clone() in render loop via std::mem::take (performance)

REFACTOR: Apply clean code improvements to wmux-ui — extract shared f32_to_glyphon_color helper (DRY), single-pass escape_xml (performance), #[allow] → #[expect] (compilation), simplify sidebar subtitle allocation

FIX: Fix tab click hitbox misaligned with visual tab position — hit-testing now uses tab_metrics() (gaps + MAX_TAB_WIDTH clamp) instead of naive equal-width division, fixing both click-to-switch and drag-drop reorder

REFACTOR: Clean code improvements in wmux-render/terminal.rs and wmux-ui/window.rs — extract CURSOR_LINE_THICKNESS constant, TAB_FONT_SIZE/TAB_LINE_HEIGHT/TAB_TEXT_PADDING/TAB_GAP/TAB_TEXT_TOP_OFFSET constants, deduplicate blink advance into advance_blink(), extract rgba_to_glyphon() helper, fix misleading _rect variable name

FIX: Fix terminal content not rendering after workspace switch — new TerminalRenderer forces full re-render on first frame via usize::MAX sentinel for last_viewport_offset

FEATURE: Implement all CLI domain commands (L2_16) — workspace (list, create, current, select, close, rename), surface (split, list, focus, close, read-text, send-text, send-key), sidebar (set-status, clear-status, list-status, set-progress, clear-progress, log, clear-log, list-log, state), system (ping, capabilities, identify), notify stubs, global --workspace/--surface flags

FIX: Use palette.selection for selection highlight instead of accent_muted — user theme selection-background color now respected

FIX: Correct cursor z-order — cursor now renders after selection highlights (cursor visible inside selected text, inactive pane cursors correctly hidden by dim overlay)

FIX: Wire EffectResult to clear_color alpha — Mica backdrop now visible on Win11 (transparent clear color when Mica/MicaAlt active, opaque fallback on Win10)

FIX: Replace hardcoded search highlight colors (orange/yellow) with theme-derived UiChrome fields (search_match, search_match_active)

FIX: Replace hardcoded sidebar dot colors (purple/cyan) with theme-derived UiChrome fields (dot_purple, dot_cyan from ANSI palette)

FIX: Replace hardcoded drop shadow colors with theme-adaptive UiChrome.shadow field

FIX: Wire Config.theme to ThemeEngine.set_theme() — user theme choice now applied at startup

FIX: Apply Config.background/foreground/palette overrides on top of loaded theme before derive_ui_chrome()

REFACTOR: Add 7 UiChrome fields (selection_bg, search_match, search_match_active, shadow, dot_purple, dot_cyan, cursor_alpha) to complete theme pipeline

REFACTOR: Extract cursor rendering from TerminalRenderer into public push_cursor() method for z-order control

REFACTOR: Add foreground_color to TerminalRenderer — glyphon default text color now follows loaded theme instead of hardcoded constant

REFACTOR: Remove duplicate DEFAULT_ANSI_PALETTE — single source of truth for palette defaults

FEATURE: Extend shader with gradient + SDF outer glow — QuadInstance 48→80 bytes, vertical gradient (top→bottom color interpolation), shader-based Focus Glow (smoothstep SDF falloff, replaces 4 concentric quads), push_glow_quad/push_gradient_quad API

FEATURE: Implement "Luminous Void" UI design system — extend UiChrome from 13 to 31 tokens (5L surface elevation, accent system with hover/glow/tint, WCAG text alphas 65%/53%/40%, border variants, overlays, semantic muted colors), Focus Glow signature element (shader-based SDF glow on active pane), dual font system (Segoe UI Variable for chrome), status bar component (28px, workspace/pane count/connection/branch/shell), enhanced animation engine (CubicIn/EaseInOut/reduced motion), border-glow luminous separators, surface_base pane dimming

FEATURE: Visual polish pass — permanent pane dividers (border_glow between all pane pairs), tab bar shadows (shadow-sm under tab bar), workspace color dots (8px circles, ANSI palette cycling), sidebar SemiBold titles + Unicode ▸ indicators, status bar integrated in window layout (28px bottom), increased glow visibility (accent_glow 25%, accent_glow_core 60%, border_glow 45%)

REFACTOR: Update sidebar to match "Luminous Void" — row height 52→48px, surface_1 bg, border_glow right edge, section headers, glow-subtle on active row

REFACTOR: Update command palette — overlay_dim+overlay_tint backdrop, surface_overlay bg, border_subtle, INPUT_HEIGHT 44px, narrow window clamp

REFACTOR: Update notification panel — 360px width, surface_overlay bg, severity stripe/tint, empty state

REFACTOR: Update tab bar — MAX_TAB_WIDTH 160px, SurfaceType enum, surface_1/surface_2 colors, inactive pill affordances

FIX: Sanitize theme name in load_theme() to prevent path traversal (reject `/`, `\`, `..`, null bytes)

FIX: Clamp notification panel and command palette dimensions on narrow windows (< 360px / < 600px) to prevent click-through and rendering overflow

FIX: Wire inactive_pane_opacity config field to UiState (was hardcoded 0.7, now uses Config default ready for config loading)

FIX: Zero-duration animations (reduced motion) now return final value immediately from get() instead of None

REFACTOR: Clean code improvements in wmux-pty — extract shared do_resize() helper to deduplicate ConPTY resize logic, remove duplicate to_wide() function (identical to to_wide_null()), use Cow::into_owned() instead of .to_string() on to_string_lossy() result

REFACTOR: Clean code improvements in wmux-render — replace #[allow] with #[expect] on clippy::too_many_arguments (3 occurrences), use .clamp() instead of .max().min() in quad border radius clamping

REFACTOR: Clean code improvements in wmux-ipc — use impl Into<String> in RpcResponse constructors to avoid unnecessary allocations, pre-allocate HashMap in PID ancestry check, add #[inline] on cross-crate accessor, idiomatic iterator destructuring

REFACTOR: Remove redundant PathBuf clone in wmux-core CwdChanged handler — move cwd into async git detection task instead of cloning twice

REFACTOR: Clean code improvements in wmux-config — eliminate unnecessary HashMap clones in locale load_language, take config overlay by slice reference instead of cloning Vec, flatten nested control flow in theme list_themes

## 2026-03-21

FIX: Fix transparent window (white background) — clear color always opaque, add per-pane background quad before terminal content; connect theme ANSI palette to terminal renderer (named_color_rgb now uses theme colors instead of hardcoded xterm defaults); cursor color from theme; notification ring uses accent color

REFACTOR: Redesign wmux-default theme from VS Code Dark+ (#1e1e1e neutral gray) to GitHub Dark-inspired anthracite (#0d1117 blue-black) with full 16-color ANSI palette; reduce UiChrome surface elevation step from 8% to 5% for subtler nuances across all themes

FEATURE: Add UiChrome color system — derive surface elevation, accent, text hierarchy, and semantic colors from terminal theme palette; all UI components themed consistently across 6 bundled themes

FEATURE: Add SDF rounded rect shader — QuadInstance extended to 48 bytes with border_radius field; fragment shader uses SDF with smoothstep anti-aliasing; zero-cost branch for sharp quads

FEATURE: Modernize sidebar — theme-driven colors via UiChrome, rounded accent bar, notification badges with unread count, row height 44→52px for better spacing

FEATURE: Add pill-style tab bar — rounded tab pills with 6px radius, 4px gap spacing, height 28→36px, accent indicator on active tab

FEATURE: Add pane dimming — inactive panes receive 30% black overlay for clear focus differentiation

FEATURE: Upgrade command palette — rounded corners (12px), drop shadow, fullscreen dimming overlay, theme-driven colors

FEATURE: Upgrade notification panel — rounded items, theme-driven colors from UiChrome

FEATURE: Fix Mica visibility — clear color alpha set to 0.0 when Mica/MicaAlt active, prefer PreMultiplied composite alpha mode

FEATURE: Add animation engine — AnimationEngine with CubicOut/Linear easing, start/update/get/cancel API for UI micro-animations

FEATURE: Enrich WorkspaceSnapshot — add unread_count, cwd, git_branch fields from workspace metadata and notification store

REFACTOR: Remove 25+ hardcoded color constants from sidebar, command palette, notification panel, and window.rs — replaced with UiChrome theme-derived colors

CHORE: Increase MAX_QUADS 4096→8192 for rounded UI elements; add wmux-config dependency to wmux-ui; add inactive_pane_opacity config key

FIX: Fix shutdown race — spawn() returns JoinHandle, main.rs awaits actor completion before process::exit so final session save completes; move std::env::set_var before tokio runtime creation to eliminate UB from multi-threaded env mutation

FIX: Remove JSON serialization from actor loop — SidebarListStatus/SidebarListLog/SidebarState return typed data via oneshot, serialization moved to IPC handler layer; add git detection 500ms debounce per workspace; port scan dedup (skip if in-flight) and interval 5s→15s

FIX: Add IPC string length validation — sidebar handler validates key/value/icon/color/message/source/label lengths, returns invalid_params on overflow; clamp list_log limit; MetadataStore::set_status returns bool for capacity rejection feedback

FIX: Log dark mode DwmSetWindowAttribute HRESULT instead of silently discarding; add unsafe to extern system block for edition 2024 forward compat; add dead_code justification comment

REFACTOR: SearchResult borrows CommandEntry instead of cloning per keystroke; pre-allocate MetadataStore HashMap/VecDeque; detect_git accepts impl AsRef&lt;Path&gt;; add i18n TODO markers on hardcoded command palette strings

CHORE: Update rustls-webpki 0.103.9→0.103.10 (RUSTSEC-2026-0049)

FIX: Track background tasks with JoinSet to prevent orphaned tokio::spawn on shutdown — abort_all on Shutdown, log panicked tasks, RAII handle for OpenProcess, safe u32 PID cast, stable LogLevel Display for IPC

FEATURE: Add MetadataStore for sidebar status badges, progress bars, and activity logs per workspace (L2_14) — in-memory store with PID sweep, IPC handler with 9 sidebar.* methods, app_state actor integration

FEATURE: Add session restore on launch (L3_02) — load_session() reads session.json with version check, corrupt/missing files start fresh (never crash per ADR-0009)

FEATURE: Add notification visual indicators (L3_09) — NotificationPanel overlay, animated blue notification ring on panes with unread notifications, Ctrl+Shift+I toggle panel, Ctrl+Shift+U jump to last unread

FEATURE: Add git branch and port detection for sidebar (L3_14) — async git detection via tokio::process::Command, port scanning via netstat, CWD change triggers re-detection, 5s port scan interval

FEATURE: Add command palette with fuzzy search (L4_01) — CommandRegistry with 16 default commands, prefix/word-start/substring scoring, CommandPalette overlay with keyboard navigation (Ctrl+Shift+P)

FEATURE: Add Mica/Acrylic window effects (L4_05) — DWM API for Win11 backdrop (Mica Alt on 22H2+, Mica on 22000+), opaque fallback on Win10, dark mode title bar support

FIX: IPC method names violate cmux convention — send_text/send_key/read_text were registered under "input.*" instead of "surface.*"; merge InputHandler into SurfaceHandler, delete input.rs, remove "input" router registration; cmux-compatible clients now correctly reach surface.send_text etc.

FIX: Win32 SetHandleInformation errors silently discarded in ConPTY pipe setup — replace `let _ =` with `if let Err(e)` + tracing::warn to surface handle inheritance failures that could cause PTY pipe leaks

FIX: Navigation shortcuts fire on key auto-repeat — holding Ctrl+D/W/N rapidly spawned/closed panes; add event.repeat guard around shortcut matching (terminal text input and sidebar editing still repeat normally)

FIX: Sidebar cursor uses byte length instead of char count — `name.len()` → `name.chars().count()` fixes cursor overshoot with multi-byte UTF-8 workspace names

FIX: auto_save().await blocks actor loop with file I/O — change to fire-and-forget tokio::spawn so the actor never awaits disk writes

FIX: surface.split error leaks internal error details — replace e.to_string() with generic message + tracing::warn

REFACTOR: Eliminate per-frame heap allocations in render hot path — Grid::take_dirty_rows() replaced with take_dirty_rows_into(&mut Vec) + reset_dirty(); TerminalRenderer::text_areas() returns impl Iterator instead of Vec

REFACTOR: Add sRGB view_formats to SurfaceConfiguration and MemoryHints::MemoryUsage to DeviceDescriptor for correct color rendering and conservative GPU memory usage

REFACTOR: Restrict ConPtyHandle::hpcon() and resize_by_hpcon() to pub(crate) with thread-safety documentation

CHORE: Add input validation — MAX_SEND_TEXT_SIZE (64KB) on surface.send_text, MAX_WORKSPACE_NAME_LEN (255) on workspace.create/rename

CHORE: Standardize TODO comments to TODO(spec_id) format across i18n placeholders

CHORE: Revert scrollback initial capacity from 16384 to 4096 to match default max_lines

FIX: Final session save lost on shutdown — auto_save was fire-and-forget but the process exits immediately via std::process::exit(0); add save_session_now() that is awaited on the shutdown path

FIX: Held shortcut keys leak raw control bytes to PTY — Ctrl+D repeat sends EOF (0x04) to newly split pane; fix matches shortcuts on repeat but only executes on first press, consuming the event either way

FIX: ConPTY shell produces no output (blank terminal) — UpdateProcThreadAttribute received a pointer to the HPCON value instead of the HPCON value itself; the kernel interpreted a heap address as an invalid pseudo-console handle, causing the child process to fail console attachment and exit immediately; fix passes hpc.0 directly as lpValue, matching the Microsoft EchoCon sample and Windows Terminal source; also removes the now-unnecessary Box<HPCON> from AttributeList

FIX: Ctrl+T new surface not visible until window click — CreateSurface actor handler was missing PaneNeedsRedraw event emission; the request_redraw() in the UI handler fired before the async spawn completed, so the surface was created after the redraw; now the actor emits PaneNeedsRedraw on success, triggering the UI update when the surface is actually ready

REFACTOR: Apply Rust best practices — remove intermediate HashMap allocation in build_env_block (wmux-pty), replace Vec<char> search matching with str::find (wmux-ui), eliminate per-frame Vec clone in search highlight rendering (wmux-ui), replace #[allow] with #[expect] (wmux-ipc), expand SAFETY comment for transmute (wmux-pty)

FIX: Force-exit process on window close via std::process::exit(0) — spawn_blocking tasks (PTY reader, exit watcher) block on synchronous I/O that cannot be cancelled; without force-exit, the process hangs indefinitely on shutdown because Windows does not kill child processes when the parent exits (standard pattern used by Alacritty/WezTerm)

FIX: Fix dangling HPCON pointer in ConPTY process spawn — UpdateProcThreadAttribute stored a pointer to a stack-local HPCON that became invalid when create_attribute_list returned (struct move invalidated the address); fix uses Box<HPCON> for heap-stable address; this caused child processes to not attach to ConPTY, resulting in blank terminals with no shell output

FEATURE: Replace portable-pty with custom ConPTY wrapper (wmux-pty/src/conpty.rs + spawn.rs) — direct windows crate v0.62 FFI to CreatePseudoConsole with PSEUDOCONSOLE_RESIZE_QUIRK flag (prevents reflow output on resize); proper 24H2+ shutdown via dynamic ReleasePseudoConsole + ClosePseudoConsole; ConPtyHandle RAII with explicit shutdown() in spawn_blocking for clean conhost cleanup; ChildProcess wraps process HANDLE with wait/kill; PtyActorHandle exit watcher now owns ConPtyHandle for correct shutdown ordering; resize handler uses raw HPCON (Copy) to avoid blocking tokio; portable-pty dependency removed entirely

FIX: Clamp PTY resize to minimum 2 columns to prevent ConPTY infinite loop bug #19922 (2-column character on 1-column terminal); applied in both PtyHandle::resize and PtyActorHandle::resize

FIX: Add output flood detection in PTY reader — tracks bytes per second and terminates the reader if output exceeds 10 MB/s, protecting against ConPTY bug #19922 runaway output

REFACTOR: Increase scrollback VecDeque pre-allocation from 4096 to 16384 entries to reduce reallocations for the default 4000-line scrollback

FIX: Tab switching now shows correct terminal content — Grid::mark_all_dirty() forces full re-render when switching surfaces (SwitchSurfaceIndex, CycleSurface, FocusSurface, CloseSurface handlers all mark backing terminal dirty); fixes stale content from previous surface being shown after tab switch

FEATURE: Tab drag-and-drop reordering with visual feedback — TabDragState state machine (None/Pressing/Dragging) with 5px threshold; Grabbing cursor icon during drag; semi-transparent overlay on dragged tab; 2px accent drop indicator bar at target position; cursor restored to Default on release; AppCommand::ReorderSurface wired through actor to SurfaceManager::reorder()

FEATURE: Tab bar mouse click and visual improvements (Phase 3) — click on tabs to switch surfaces via SwitchSurfaceIndex command; tab bar hit-testing before click-to-focus; cached last_viewports for non-blocking mouse interaction

FEATURE: Tab bar text rendering and styling (Phase 2) — glyphon text buffers for tab titles in UiState; tab titles rendered with SansSerif 12px (active=bright, inactive=gray); render_tab_bar improved with full background, active indicator (2px accent bar), vertical separators, bottom border line; TAB_BAR_HEIGHT increased to 28px; terminal text areas offset by tab bar height for correct positioning; PaneViewport derives Clone

FEATURE: Surface-as-Hidden-Pane architecture (Phase 1) — Surface struct gains pane_id field pointing to its backing PaneState; Surface::new/with_kind updated to require PaneId; SurfaceManager gains get_by_index() and reorder() methods; AppStateActor::resolve_terminal_pane() resolves the active surface's backing pane (with fallback); build_render_data, handle_send_input, handle_resize, handle_scroll, ResetViewport, ExtractSelection, and build_read_text all route through resolve_terminal_pane; CreateSurface command gains backing_pane_id field and switches to the new surface after creation; CloseSurface removes the hidden backing pane from the registry; close_pane_internal cleans up all hidden surface panes before removing the layout pane; NewSurface shortcut spawns a real PTY for each new tab; all call sites updated

FIX: Wire surface tab data into render pipeline — PaneRenderData now carries surface_count/surface_titles/active_surface from SurfaceManager; window.rs populates PaneViewport with real tab data and calls render_tab_bar() for multi-surface panes; terminal content area adjusted via terminal_viewport() to avoid tab bar overlap (Ctrl+T/Ctrl+Tab/Ctrl+Shift+Tab now visually functional)

FIX: Clear terminal selection after sidebar click — handle_mouse_press was creating a Selection for click counting that persisted and caused blue selection overlay when moving mouse after double-click rename

FEATURE: Add sidebar mouse interactions — click workspace row to switch, double-click to inline rename, drag & drop to reorder; SidebarInteraction state machine (Idle/Hover/Pressing/Dragging/Editing) with hit testing, hover highlights, drop indicator line, and edit cursor rendering; WorkspaceManager::reorder() for index-based workspace reordering with active_index preservation; AppCommand::ReorderWorkspace wired through actor; handle_sidebar_edit_key for inline rename input (Enter/Escape/Backspace/Delete/Arrows/Home/End); 12 new unit tests

REFACTOR: Clean up GlyphonRenderer — remove dead buffer/set_text/prepare code (only prepare_text_areas is used), remove unused width/height fields, simplify constructor signature, extract DEFAULT_TEXT_COLOR constant to wmux-render lib.rs

FIX: Address adversarial review findings (7 issues) — SSH argument injection prevention via character validation in RemoteConfig::parse; shortcuts now work when focused pane has exited; workspace.select by index validates range before returning success; scrollback truncation uses char count instead of byte count for correct UTF-8 handling; search match positions use char indices for accurate multi-byte highlighting; final session save on actor shutdown prevents data loss; divider drag computes actual container dimension from adjacent pane rects

FEATURE: Add terminal search with match highlighting (L4_02) — wmux-ui/src/search.rs: SearchState (open/close/next_match/prev_match/search/match_count_display), SearchMatch, extract_rows() (grid+scrollback text extraction), render_search_highlights() (QuadPipeline overlay with yellow/orange semi-transparent quads); Ctrl+F toggles search overlay, Escape closes, Backspace edits query, Enter navigates next match; search is case-insensitive by default with optional regex mode (regex crate); search bar rendered as quad at bottom of focused pane with accent line and match count indicator; 16 unit tests for all search/navigation/regex paths; regex = "1" added to workspace Cargo.toml

FEATURE: Add SSH remote support infrastructure (L4_03) — RemoteConfig (parse user@host[:port], ssh_args builder), RemoteConnectionState, RemoteError (thiserror), ReconnectBackoff (exponential 1s..60s) in wmux-core/src/remote.rs; WorkspaceKind enum (Local/Remote) added to workspace.rs with kind() getter; wmux-cli gains `wmux ssh connect <target>` and `wmux ssh disconnect` subcommands (validation-only, daemon integration pending); wmux-core added as wmux-cli dependency; 15 unit tests for parse/backoff

FEATURE: Add draggable dividers and pane resize (L2_05) — divider.rs module with find_dividers (pairwise edge detection), hit_test (8px hit zone), compute_ratio (clamped to MIN_PANE_SIZE=50px); UiState gains dividers cache and drag_state; CursorMoved applies resize_split during drag and sets EwResize/NsResize cursor on hover; MouseInput starts/ends drag on left press/release over divider, double-click resets ratio to 0.5; divider highlighted in accent colour during hover/drag; MouseHandler exposes click_count(); pub mod divider added to lib.rs

FEATURE: Add session auto-save (L3_01) — SessionState/WorkspaceSnapshot/PaneTreeSnapshot/WindowGeometry types with serde; build_session_state() maps WorkspaceManager+PaneRegistry to snapshot with scrollback truncation (4000 lines / 400K chars); save_session() writes atomically via temp file + rename to %APPDATA%/wmux/session.json; AppStateActor run loop restructured from while-let to tokio::select! with 8-second interval timer calling auto_save(); serde_json and dirs added to wmux-core dependencies

FEATURE: Add sidebar panel rendering (L2_08) — SidebarState with toggle/effective_width, render_sidebar pushing background/separator/active-highlight quads via QuadPipeline; terminal viewport adjusted to start after sidebar width; Ctrl+B shortcut toggles sidebar; workspace list refreshed per frame via AppStateHandle::list_workspaces

FEATURE: Add WorkspaceHandler and SurfaceHandler for workspace.* and surface.* JSON-RPC namespaces in wmux-ipc — workspace.list/create/current/select/close/rename and surface.split/list/focus/close wired to AppStateHandle, with unit tests for all error and success paths

FEATURE: Add InputHandler for input.* JSON-RPC namespace (L2_13) — implements send_text (raw bytes to PTY), send_key (key name to VT escape sequence), read_text (terminal grid content); resolves target pane via surface_id or focused pane; full VT key map for Enter/Tab/Escape/arrows/F1-F12/Ctrl+A-Z/PageUp-PageDown/Home/End; 19 unit tests

FEATURE: Add BrowserHandler stub for browser.* JSON-RPC namespace in wmux-ipc — identify returns capability list, all other methods return "not yet wired to COM thread" error pending STA bridge

REFACTOR: Simplify Rust code — extract require_webview()/require_controller() helpers in BrowserPanel (eliminates 33 repetitions of as_ref().ok_or() pattern); convert verbose match→let-else in AppStateActor::handle_navigate_focus, handle_auth_login, and window event handler; remove stale duplicate doc comment on Grid::row_slice

REFACTOR: Apply Rust best practices — add Grid::row_slice() for zero-copy row access and use extend_from_slice in TerminalRenderer::collect_grid_row (memory-reuse-allocations hot path); wrap IPC server auth_secret in Arc<String> to avoid per-connection clone (ownership-avoid-clone); remove redundant tab_count > 0 check in PaneRenderer::render_tab_bar (dead code)

FIX: Render ALL panes with terminal content — replace single TerminalRenderer with HashMap<PaneId, TerminalRenderer>; render loop now iterates all panes (get_render_data + update_from_snapshot for each); per-pane cols/rows computed from rect dimensions with automatic PTY resize; text areas collected from all renderers and prepared in single glyphon call; stale renderers cleaned up on pane close

FIX: Address code review findings (18 issues) — IPC server: add bounded_read_line to prevent OOM via unbounded reads, acquire semaphore before tokio::spawn to prevent task explosion, reject invalid UTF-8 instead of lossy conversion; auth: manual Debug impl for ConnectionCtx to redact session_token; CLI client: add flush after write_all and Take limit on reads; browser: add Drop impl for BrowserPanel calling controller.Close(), add rect_to_bounds validation for f32-to-i32 casts, change is_runtime_available/runtime_version to &self; render: eliminate 3 per-frame Vec allocations (dirty_buf reuse, iterator pass-through to prepare_text_areas and set_rich_text), remove Clone from PaneViewport; core: extract close_pane_internal to deduplicate pane close logic, change Workspace fields to pub(crate), extract hardcoded strings to constants, extract focus nav magic number to FOCUS_NAV_VIEWPORT constant; UI: cache pane layout for non-blocking mouse click hit-testing, rename toggle_devtools to open_devtools

REFACTOR: Apply Rust best practices to Wave 8 code — reuse String buffer in IPC connection loop instead of allocating per request (memory-reuse-allocations); stack-allocate nonce as [u8; 32] instead of Vec<u8> (memory-stack-allocation); wrap check_pid_ancestry in spawn_blocking to avoid blocking tokio runtime (async-no-blocking); accept impl Into<String> in WorkspaceManager::create/rename (ownership-into-string); replace unwrap_or with expect for AuthLoginResponse serialization (error-expect-messages); change handle_auth_login to accept Option<&str> instead of &Option<String> (ownership-accept-slices); add Debug impls for IpcServerHandle and IpcServer (type-common-traits)

FEATURE: Wire SurfaceManager into PaneState and AppState (L2_06) — add `surfaces: SurfaceManager` field to PaneState (initialized with Surface::new("shell") at every construction site); add CreateSurface/CloseSurface/CycleSurface AppCommand variants with Debug impls; add handlers in AppStateActor run() loop (CloseSurface closes pane when last surface removed); add create_surface/close_surface/cycle_surface methods on AppStateHandle; replace NewSurface/CycleSurfaceForward/CycleSurfaceBackward shortcut placeholders in wmux-ui/src/window.rs with live calls to AppStateHandle

FEATURE: Wire wmux-cli main.rs with clap 4 — add clap/serde_json/tokio/wmux-ipc dependencies to Cargo.toml; rewrite main.rs with clap 4 derive (Cli/Commands/SystemCommands structs, --pipe and --json global options, system ping subcommand, stubs for workspace/surface/sidebar/notify/browser); connect IpcClient::new with discovered or overridden pipe name; map ping Result<bool> to i32 exit code; fix redundant closure and add #[allow(dead_code)] on IpcClient::discover

FEATURE: Wire multi-pane splits to UI event loop — extract spawn_pane_pty helper, SplitRight/SplitDown now spawn a real PTY for the new pane and emit WmuxEvent::FocusPane to sync focused_pane on the UI thread; render() uses get_layout() for multi-pane layout with PaneRenderer borders and per-pane origin offset; add WmuxEvent::FocusPane variant to event.rs

FIX: Adversarial review fixes for Wave 8 — clear zoom state on ClosePane and SwitchWorkspace (prevent stale zoomed pane reference); validate zoomed pane existence in GetLayout before returning layout; SurfaceManager active/active_id/active_mut return Option to prevent panic on empty manager; auth.rs generate_auth_secret/load_auth_secret converted from blocking std::fs to async tokio::fs+spawn_blocking; AuthLoginResponse Debug impl redacts session_token; render_tab_bar removes unused glyphon/device/queue stub parameters

FEATURE: Add CLI client foundation (L2_15) — clap 4 derive-based CLI binary with global options (--pipe, --json, --workspace, --surface, --window); IpcClient Named Pipe client with connect/request/30s timeout/pipe discovery via WMUX_SOCKET_PATH; output formatting (human-readable vs JSON); system.ping subcommand; subcommand stubs for workspace/surface/sidebar/notify/browser; 6 unit tests

FEATURE: Add surface tab system (L2_06) — SurfaceManager per pane with Vec of Surface structs and active index tracking; Surface struct (id, title, kind); add/remove/cycle/switch_to/switch_to_id/find/iter methods; active_index adjustment on removal; 17 unit tests

FEATURE: Add focus routing and keyboard shortcut dispatcher (L2_03) — ShortcutMap struct with match_shortcut method covering all PRD shortcuts (Ctrl+D split right, Ctrl+Shift+D split down, Ctrl+W close pane, Ctrl+Shift+Enter zoom toggle, Ctrl+Alt+Arrows focus navigation, Ctrl+N new workspace, Ctrl+1-9 switch workspace, Ctrl+T new surface, Ctrl+Tab cycle surfaces, Ctrl+Shift+C/V clipboard, Ctrl+Shift+P/F/F12 placeholder detectors); ShortcutAction enum (SplitRight/SplitDown/ClosePane/ZoomToggle/FocusUp/Down/Left/Right/NewWorkspace/SwitchWorkspace/NewSurface/CycleSurface*/Copy/Paste/CommandPalette/Find/ToggleDevTools); FocusDirection enum (Up/Down/Left/Right) in wmux-core; AppCommand gains ToggleZoom/NavigateFocus variants; AppStateActor gains zoomed_pane: Option<PaneId> field; GetLayout returns single full-viewport rect when zoomed; NavigateFocus uses nearest-center geometry algorithm on PaneTree layout; AppStateHandle gains close_pane/toggle_zoom/navigate_focus methods; wmux-ui window.rs keyboard handler routes through ShortcutMap before terminal input, replacing hardcoded Ctrl+Shift+C/V block; translate_key in input.rs removes reserved-shortcut filter (now intercepted upstream); 37 new unit tests in shortcuts module

FEATURE: Add workspace model and lifecycle management (L2_07) — Workspace struct (id, name, pane_tree: Option<PaneTree>, metadata, creation_order); WorkspaceMetadata with git_branch/cwd/ports/git_dirty placeholders; WorkspaceManager with Vec<Workspace>/active_index invariant (never empty), create/switch_to_index/switch_to_id/close/rename/by_id/iter methods; close() creates replacement "Workspace 1" when closing last workspace; AppStateActor replaces pane_tree field with workspace_manager — all pane operations (RegisterPane, ClosePane, SplitPane, SwapPanes, GetLayout) route through active workspace's pane_tree; AppCommand gains CreateWorkspace/SwitchWorkspace/CloseWorkspace/RenameWorkspace variants; AppEvent gains WorkspaceCreated/WorkspaceSwitched/WorkspaceClosed variants; AppStateHandle gains create_workspace/switch_workspace/close_workspace/rename_workspace methods; CoreError gains SurfaceNotFound/WorkspaceNotFound/CannotCloseLastWorkspace variants; wmux-ui window.rs updated to handle new AppEvent variants; 35 new unit tests across workspace/workspace_manager/app_state modules

FEATURE: Add IPC Handler trait, Router dispatch system, and system handlers (L2_11) — Handler trait with Pin<Box<dyn Future>> for object safety enabling Arc<dyn Handler> in HashMap; RpcError struct with JSON-RPC v2 numeric codes (-32700 parse, -32600 invalid_request, -32601 method_not_found, -32602 invalid_params, -32603 internal); Router rewritten with HashMap<String, Arc<dyn Handler>> dispatch splitting method on first dot (domain.action); Router::register for extensible handler registration; SystemHandler implements ping ({"pong": true}), capabilities (method list + version), identify (app/version/platform/protocol_version); handlers module with handlers/system.rs; lib.rs exports Handler and RpcError; rpc_error_to_response maps numeric codes to RpcErrorCode for protocol layer; router dispatch passes params as Value::Null when absent; 16 new unit tests across handler/router/system modules

FEATURE: Add IPC authentication and security modes (L2_10) — SecurityMode enum (Off/WmuxOnly/AllowAll/Password) with WmuxOnly as default; ConnectionCtx tracks per-connection auth state (authenticated, mode, session_token, client_pid); generate_auth_secret() writes 256-bit hex secret to %APPDATA%\wmux\auth_secret with owner-only Windows ACL (SDDL-based DACL via ConvertStringSecurityDescriptorToSecurityDescriptorW + SetNamedSecurityInfoW); load_auth_secret() reads existing secret; verify_hmac() constant-time HMAC-SHA256 verification; generate_nonce() 32-byte random nonce; check_pid_ancestry() walks process tree via CreateToolhelp32Snapshot to verify WmuxOnly connections; get_client_pid() via GetNamedPipeClientProcessId; is_unauthenticated_method() allows only system.ping and auth.login; IpcServer::new gains SecurityMode and auth_secret parameters; handle_connection_inner enforces auth per mode; run_connection_loop supports multi-request sessions for challenge-response handshake; handle_auth_login implements two-step nonce/HMAC flow; auth secrets and HMAC tokens never logged; RpcErrorCode gains Unauthorized variant; AuthLoginRequest/AuthLoginResponse protocol types added; Router::dispatch gains &ConnectionCtx parameter; wmux-app updated to use WmuxOnly mode; workspace adds rand/hmac/sha2/hex dependencies; 11 new unit tests

FEATURE: Add multi-pane GPU rendering (L2_04) — PaneRenderer stateless orchestrator with render_pane_borders (4 border quads per pane, accent color for focused pane), render_tab_bar (24px tab bar with per-tab quads when tab_count > 1), terminal_viewport (subtracts tab bar height), scissor_rect (converts Rect to wgpu scissor u32 tuple clamped to surface bounds); PaneViewport struct (pane_id, rect, focused, tab_count, tab_titles, active_tab, zoomed); TerminalRenderer::update_from_snapshot gains pane_origin (f32, f32) parameter for multi-pane surface offsetting; TerminalRenderer::prepare gains pane_origin and pane_rect parameters for per-pane TextBounds clipping; push_background_quads/push_cursor_quad updated with x_off offset; wmux-ui window.rs updated for single-pane compatibility (origin (0,0)); 7 new unit tests in pane module

FEATURE: Extend wmux-browser with browser panel lifecycle management (L3_04) — BrowserPanel gains surface_id field, id()/has_focus()/focus()/blur()/open_devtools() methods (MoveFocus programmatic, OpenDevToolsWindow); BrowserManager gains HashMap<SurfaceId, BrowserPanel> panels field with cached ICoreWebView2Environment, create_panel/resize_panel/show_panel/hide_panel/remove_panel/get_panel/get_panel_mut/resize_all/hide_all/show_all methods; wmux-browser/Cargo.toml adds wmux-core dependency; lib.rs re-exports Rect and SurfaceId; 6 new unit tests

REFACTOR: Apply Rust best practices audit — add manual Debug impl for AppCommand (all 15 variants, finish_non_exhaustive for omitted fields) and PaneRenderData (count-suffixed field names for summarized collections); add #[derive(Debug)] on Router; add SAFETY comments to all unsafe env-var blocks in wmux-ipc tests; convert UpdateChecker::check_pending_update from blocking std::fs to async tokio::fs with resilient loop (log and skip mid-directory I/O errors); add #[must_use] on Scrollback::new

FIX: Harden Wave 7 from adversarial review — PaneTree: single-pass swap_panes (eliminate nil UUID sentinel), close_pane uses mem::replace (no clone), resize_split targets immediate parent only (not topmost ancestor), layout_into avoids intermediate Vec allocations; Rect: clamp negative dimensions to 0.0 on split; AppState: ClosePane now updates pane_tree (sync fix); IPC: bounded read via Take adapter (prevent DoS), connection semaphore (max 64), validate WMUX_SOCKET_PATH pipe prefix; CoreError gains CannotClose variant

FEATURE: Add Named Pipes IPC server and JSON-RPC v2 protocol (L2_09) — RpcRequest/RpcResponse types matching cmux wire format (ok/result/error fields, not standard JSON-RPC), RpcErrorCode enum (ParseError/InvalidRequest/MethodNotFound/InvalidParams/InternalError), IpcServer with Named Pipe accept loop (ServerOptions, pipe-busy fallback to PID-suffixed name), one-shot connection handling (30s read timeout, 1MB size limit, newline-delimited JSON framing), Router with system.ping built-in method, IpcServerHandle for graceful shutdown via bounded channel, pipe_name() with WMUX_SOCKET_PATH env var support; IpcError gains Protocol/PipeBusy/Timeout/RequestTooLarge variants; wmux-app wires IPC server on startup; 16 unit tests
FEATURE: Add PaneTree binary split layout engine (L2_02) — Rect geometry struct with split_horizontal/split_vertical/contains_point; PaneTree recursive binary tree (Leaf/Split) with layout computation, split_pane, close_pane (sibling promotion), swap_panes, resize_split (clamped 0.1--0.9), find_pane, pane_ids, pane_count; CoreError gains PaneNotFound/CannotSplit variants; AppStateActor owns Optional PaneTree initialized on first RegisterPane, handles SplitPane/SwapPanes/GetLayout commands; AppStateHandle gains split_pane/swap_panes/get_layout methods; Serde derives on PaneTree and Rect for session persistence; 28 new unit tests

REFACTOR: Apply Rust best practices to Wave 6 code — fix actor stalling: change handle_send_input/handle_resize from .send().await (blocks entire actor if PTY channel full) to try_send() with warning logs (async-no-blocking rule); add #[must_use] on AppStateHandle::spawn(); pre-allocate scrollback_visible_rows with Vec::with_capacity(sb_rows_shown) in build_render_data hot path, add #[inline] on cross-crate hot-path methods (AppStateHandle::send_input/process_pty_output/scroll_viewport/reset_viewport, PaneRegistry::get/get_mut), derive Debug on AppStateHandle, pre-allocate sanitize_env_value with String::with_capacity

FEATURE: Refactor to AppState actor architecture (L2_01) — AppState runs in dedicated tokio task owning all terminal/pane state via PaneRegistry (HashMap<PaneId, PaneState>); AppCommand enum routes all mutations through bounded(256) channel; PaneRenderData snapshot (cloned Grid + visible scrollback rows + terminal modes) enables UI rendering without shared state; AppEvent channel (PaneNeedsRedraw, NotificationAdded, PaneExited) notifies UI via EventLoopProxy forwarding task; winit event loop refactored to UiState (rendering + input only) sending commands via AppStateHandle; no Arc<Mutex> anywhere; 5 async unit tests
FEATURE: Add Windows Toast notifications (L3_10) — WinRT Toast API via windows 0.62 crate (UI_Notifications, Data_Xml_Dom, Win32_UI_Shell features); AUMID setup via SetCurrentProcessExplicitAppUserModelID; ToastService shows toast with title+body from Notification struct; XML template with entity escaping; custom command execution via WMUX_NOTIFICATION_COMMAND env var with WMUX_NOTIFICATION_TITLE/BODY/WORKSPACE_ID/SURFACE_ID; suppression handled by actor (only unsuppressed notifications forwarded to UI); 4 unit tests (2 ignored for Windows desktop)

REFACTOR: Apply Rust best practices to Wave 5 code — replace fragile filter.is_none()||filter.unwrap() with is_none_or in NotificationStore::list, derive Copy on NotificationSource and Debug on NotificationStore, derive Clone+Copy on InputHandler (ZST), pre-allocate SGR mouse report buffers with Vec::with_capacity(16) and write! (skip String intermediary) in sgr_press/sgr_release/sgr_wheel and window wheel handler, add #[inline] to sgr_press/sgr_release/sgr_wheel hot-path helpers

## 2026-03-20

FIX: Implement DSR (ESC[6n cursor position report) and DA1 (ESC[c device attributes) responses — PowerShell sends DSR on startup and blocks until the terminal replies; without this response the shell hangs after 4 bytes of output
FIX: Add TerminalEvent::PtyWrite variant for terminal-to-PTY write-back responses; create Terminal with event channel and forward PtyWrite events to PTY in render loop
FIX: Sanitize ESC bytes in bracketed paste to prevent paste injection attacks
FIX: Clamp mouse cursor cell coordinates to terminal bounds (prevent out-of-bounds SGR reports and selection)
FIX: Recover GPU surface on Lost/Outdated errors instead of failing all subsequent frames
FIX: Enforce forward-only notification state transitions (Received→Unread→Read→Cleared)
FIX: Clamp NotificationStore::with_capacity(0) to 1 to prevent panic on add()
FIX: Change should_suppress() to accept incoming notification workspace instead of checking last stored

FEATURE: Wire single-pane terminal integration (L1_10) — connect Terminal, PtyActorHandle, TerminalRenderer, InputHandler, MouseHandler into winit event loop; tokio↔winit bridge via bounded channels and EventLoopProxy<WmuxEvent>; PTY output→Terminal::process()→GPU render cycle; keyboard input→VT bytes→PTY write; mouse selection with highlight overlay quads; copy/paste (Ctrl+Shift+C/V); window resize→terminal+PTY+renderer resize; SGR mouse wheel reporting; process exit detection with terminal message; add grid_and_scrollback() split-borrow accessor to Terminal
FEATURE: Add NotificationStore to wmux-core (L3_08) — Notification struct with id/title/body/source/workspace/timestamp, NotificationState lifecycle (Received→Unread→Read→Cleared), NotificationSource (Osc/Api/Internal), NotificationEvent bus (Added/StateChanged/Cleared), NotificationStore with add/transition/mark_workspace_read/clear/clear_all/list/unread_count/should_suppress methods, 200-notification cap with oldest-cleared-first eviction; 12 unit tests

REFACTOR: Apply Rust best practices to Wave 3 code — reusable cell_buf in TerminalRenderer eliminates per-row Vec allocation in render loop, extract resolve_row_cells/collect_grid_row/push_background_quads helpers to reduce update() complexity and remove duplicated grid cell collection, truncate scrollback rows to current column count preventing off-screen quads, pre-allocate Vec with_capacity in selection extract_text, in-place trim_end via truncate (avoid extra String allocation), add Debug/Clone/Copy derives to TerminalMetrics and Debug to MouseHandler/InputHandler, remove identity function in automation.rs

FEATURE: Add mouse selection, copy/paste, and scroll to wmux-core/wmux-ui — Selection model (Normal/Word/Line modes) with text extraction from Grid, word boundary detection, multi-row support; MouseHandler with click-drag selection, double/triple-click detection, SGR mouse reporting (press/release/wheel), viewport scroll handling, arboard clipboard integration (copy/paste with graceful error handling); 27 unit tests (1 ignored clipboard)
FEATURE: Add TerminalRenderer to wmux-render — connects Grid/Scrollback to glyphon+QuadPipeline; TerminalMetrics measures 'M' glyph for cell dimensions; per-row glyphon Buffers with set_rich_text per-cell color spans; dirty-row optimization (only re-shape changed rows); background quads via QuadPipeline; cursor rendering (Block/Underline/Bar) with 500ms blink; scrollback viewport support; xterm-256 color palette; add prepare_text_areas + 5 accessor methods to GlyphonRenderer; add wmux-core dependency to wmux-render; 20 unit tests passing

FEATURE: Add DOM automation to wmux-browser — click/dblclick/hover/focus_element/check/uncheck/scroll_into_view interaction functions, fill/type_text/press_key/select_option form input functions, scroll_page, snapshot (accessibility tree JSON, depth-limited to 10), screenshot stub (returns error pending CapturePreview impl), get_attribute/is_state/find_elements/highlight inspection functions, setup_console_capture/read_console/read_errors console capture; all functions in automation.rs with serde_json-safe selector/value embedding; BrowserPanel delegates all 21 new methods; 7 new panel unit tests
FEATURE: Add shell integration hooks (OSC 7 CWD tracking, OSC 133 prompt marks) — PowerShell/Bash/Zsh scripts embedded via include_str!, written to config_dir/wmux/shell-integration/ on spawn, auto-injected via pwsh -NoExit -ExecutionPolicy Bypass -Command and bash --rcfile wrapper; shell_type_from_path infers ShellType from explicit shell paths; 3 new unit tests (1 ignored filesystem write)
FEATURE: Add InputHandler to wmux-ui — translates winit 0.30 KeyEvent to VT byte sequences for PTY input; handles character keys (UTF-8, Ctrl codes 0x00–0x1F, Alt ESC prefix), named keys (arrows with APPLICATION_CURSOR mode, F1–F12, Home/End, PageUp/PageDown, Insert/Delete, Enter/Backspace/Tab/Escape/Space), bracketed paste wrapping (BRACKETED_PASTE mode); Ctrl+Shift+C/V reserved for copy/paste (returns None); key release events ignored; 41 unit tests

REFACTOR: Apply Rust best practices to Wave 2 code — #[inline] on hot cross-crate accessors (Locale::t, Locale::language, ThemeEngine::current_theme, BrowserPanel::controller/webview), derive Copy+Hash on NavigationState, return &'static str from detect_system_language (avoid String allocation), return &'static [&str] from available_languages (avoid Vec allocation), with_capacity on list_themes Vec, add blocking I/O doc warnings on ThemeEngine::load_theme and list_themes
FEATURE: Add browser navigation and JavaScript eval API to wmux-browser — NavigationState enum (Loading/Complete/Failed), WaitCondition enum (Selector/Text/UrlPattern/LoadState/JsCondition with Display impl), navigate/back/forward/reload/current_url functions (PWSTR CoTaskMemFree), eval via ExecuteScriptCompletedHandler, add_init_script via AddScriptToExecuteOnDocumentCreatedCompletedHandler, focus_webview/is_webview_focused via MoveFocus/IsVisible, wait_for polling at 100ms intervals, BrowserPanel struct wrapping ICoreWebView2Controller+ICoreWebView2 with attach/set_bounds/set_visible and full delegation methods; 16 unit tests (4 ignored COM)
FEATURE: Add Locale to wmux-config — English/French locale files embedded via include_str! (en.toml, fr.toml with ~50 keys across sidebar/palette/notification/terminal/menu/status/dialog/error/browser/settings sections), Locale struct with new/detect/t/language/set_language/available_languages, TOML flattening to dot-notation HashMap, GetUserDefaultUILanguage Win32 detection (primary lang 0x0C=French fallback), English fallback chain (active → en → key), Send+Sync; 18 unit tests
FEATURE: Add ThemeEngine to wmux-config — ColorPalette (16 ANSI + background/foreground/cursor/selection as RGB tuples), Theme struct, ThemeEngine with load_theme/set_theme/list_themes/current_theme/is_dark_mode, Windows registry dark/light mode detection (AppsUseLightTheme DWORD), hex color parser, bundled themes embedded via include_str! (wmux-default, catppuccin-mocha, dracula, nord, gruvbox-dark, one-dark), graceful degradation on invalid colors; 18 unit tests
REFACTOR: Apply Rust best practices fixes across wmux-app, wmux-browser, wmux-config — bounded channel (sync_channel) in BrowserManager, SAFETY comments on unsafe env var ops in updater tests, Debug impl for ComGuard, apply_values takes ownership to eliminate ~10 .clone() calls, with_capacity for parser Vec, max_by replaces sort+first, ensure_dir TOCTOU removal, add #[ignore] to env-var async test
FEATURE: Add WebView2 COM initialization to wmux-browser — ComGuard RAII wrapper (CoInitializeEx STA + CoUninitialize on Drop), BrowserManager with runtime detection (GetAvailableCoreWebView2BrowserVersionString with CoTaskMemFree), environment creation via CreateCoreWebView2EnvironmentCompletedHandler callback pattern, user data dir at %APPDATA%\wmux\webview2-data, BrowserError variants (RuntimeNotInstalled, ComInitFailed, EnvironmentCreationFailed, UserDataDirFailed); 7 tests (4 unit + 3 ignored COM)
REFACTOR: Change config parser from HashMap to Vec<(String, String)> (ParsedConfig) — preserve multi-value keys like keybind; add bounds validation for font_size (4-200), scrollback_limit (cap 1M), sidebar_width (min 1)
FIX: Fix PWSTR memory leak in BrowserManager runtime detection — add CoTaskMemFree after consuming version string
FIX: Improve update apply atomicity — copy to .new first, then rename current to .old, then rename .new to current; clean up staged file after apply; filter pending updates by current version to prevent downgrades
CHORE: Add windows 0.62 crate to workspace dependencies (Win32_System_Com, Win32_Foundation)
FEATURE: Add auto-update module to wmux-app — UpdateChecker with semver version comparison, GitHub Releases API polling, streaming download via reqwest::chunk(), pending-update detection (wmux-app-v*.exe scan), staged apply (rename-to-.old + copy-new + cleanup), WMUX_DISABLE_UPDATE env var support, silent failure on network errors; 12 unit tests (1 ignored network)
FEATURE: Add Ghostty-compatible config parser to wmux-config — Config struct with font_family/font_size/theme/palette/keybindings/scrollback/sidebar_width/language, FromStr impl, Config::load() with priority chain (wmux > ghostty > defaults), Config::merge, ConfigError with ParseError/InvalidValue/ConfigDirNotFound variants, custom key=value parser (BOM strip, quoted strings, inline comment skip), tracing warnings for unknown/invalid keys; 27 unit tests
FEATURE: Add PtyActorHandle with async I/O — spawn_blocking reader/writer/exit-watcher tasks, bounded channels (256 output, 256 write, 4 resize), PtyEvent enum (Output/Exited), graceful shutdown on channel drop, PtyHandle::into_parts for actor ownership transfer
REFACTOR: Preserve error chain in wmux-pty — replace PtyError string-wrapping (.to_string()) with #[source] Box<dyn Error + Send + Sync>, rename catch-all General variant to specific CloneReaderFailed
FEATURE: Add ConPTY shell spawning to wmux-pty — PtyManager with spawn/resize, shell detection (pwsh → powershell → cmd.exe chain via where.exe), SpawnConfig with env injection (TERM, TERM_PROGRAM, WMUX_*), PtyHandle with reader/writer/child/resize, PtyError variants (SpawnFailed, ShellNotFound, ResizeFailed); 10 tests (6 unit + 4 ignored integration)
FEATURE: Add OSC sequence handlers and terminal event bus to wmux-core — OSC 7 (CWD change with file URI parsing and Windows path conversion), OSC 9/99/777 (iTerm2/kitty/rxvt notifications), OSC 133 (shell prompt marks A/B/C/D), OSC 8 (hyperlinks stored per-cell via Arc<Hyperlink>); TerminalEvent enum with bounded tokio mpsc channel (try_send, backpressure-safe); 20 new unit tests
FEATURE: Add Scrollback ring buffer (VecDeque<Row>, configurable max 4000 lines, viewport offset tracking, read_text API) and alternate screen buffer (DECSET 47/1047/1049 enter/exit with grid+cursor save/restore) to wmux-core — Grid::extract_row for row capture, VteHandler pushes evicted rows to scrollback on linefeed/CSI-S, alt screen isolates scrollback; 16 new unit tests
REFACTOR: Add Grid::fill_cells for bulk erase without per-cell clone — eliminates ~1920 CompactString clones per full-screen erase
REFACTOR: Replace .expect() panics with RenderError variants in GpuContext::new — library crate no longer panics on missing GPU formats
REFACTOR: Encapsulate GlyphonRenderer fields (7 pub → private), extract default_attrs() helper
FIX: Add Hash derive to SurfaceInfo for HashMap compatibility (Eq without Hash violated API guidelines)
CHORE: Add #[inline] to VteHandler::erase_cell and param hot-path helpers
FEATURE: Add Terminal struct and VTE parser integration to wmux-core — Terminal owns Grid + vte::Parser, VteHandler implements vte::Perform with character printing (including wide chars), cursor movement (CUU/CUD/CUF/CUB/CUP/HPA/VPA), erase (ED/EL), line ops (IL/DL/ICH/DCH), SGR (16/256/truecolor, bold/italic/underline/inverse/strikethrough), DECSET/DECRST modes, DECSTBM scroll regions, ORIGIN mode support, DECSC/DECRC cursor save/restore; 95 unit tests
FEATURE: Add scroll_up_in_region and scroll_down_in_region to Grid for scroll region support with clone_from_slice optimization
CHORE: Add vte 0.13 and unicode-width 0.2 dependencies to wmux-core
FEATURE: Add Grid struct to wmux-core — flat Vec<Cell> with stride-based indexing, per-row dirty tracking, cursor integration, scroll/resize/insert/delete operations; 21 unit tests
FEATURE: Add QuadPipeline for GPU-accelerated colored rectangle rendering — instanced wgpu pipeline with WGSL shader, batch API (push_quad/prepare/render/clear), 4096-quad capacity, pixel-to-NDC viewport uniform, alpha blending
FIX: Cap QuadPipeline push_quad at buffer capacity to prevent unbounded Vec growth
FIX: Filter NaN/infinity values in push_quad to prevent GPU rendering artifacts
FIX: Guard QuadPipeline::resize against zero dimensions to prevent shader division by zero
REFACTOR: Replace Cell.grapheme String with CompactString (compact_str v0.8) — eliminate heap allocation for graphemes ≤24 bytes
REFACTOR: Replace CoreError::General(String) catch-all with domain-specific variants — OutOfBounds, InvalidScrollRegion, InvalidConfig
FEATURE: Add domain model types to wmux-core — WindowId, WorkspaceId, PaneId, SurfaceId newtypes, Cell struct, CellFlags/TerminalMode bitflags, Color enum, CursorShape/CursorState, SplitDirection, PanelKind, SurfaceInfo; 34 unit tests
FIX: Widen CellFlags from u8 to u16 for future extensibility, add BLINK flag (SGR 5)
FIX: Use from_bits_truncate() for CellFlags/TerminalMode serde deserialization (forward-compat)
FIX: ID Default returns nil UUID instead of random — deterministic
CHORE: Add Eq derive to Cell/SurfaceInfo, Hash derive to CursorState, #[must_use] to ID constructors
CHORE: Add uuid, bitflags, compact_str workspace dependencies

## 2026-03-19

FEATURE: Add error types and tracing infrastructure to 6 stub crates — CoreError, PtyError, IpcError, BrowserError, ConfigError (thiserror v2); wmux-cli gets anyhow + tracing-subscriber with RUST_LOG env filter
REFACTOR: Replace anyhow with thiserror typed errors in wmux-render (RenderError) and wmux-ui (UiError)
REFACTOR: Add .context() to error propagation in wmux-app binary crate
REFACTOR: Add #[inline] to cross-crate hot-path methods in GpuContext and GlyphonRenderer
REFACTOR: Use structured tracing fields instead of format strings in GPU and window logging
FIX: Replace unchecked index access on surface_caps.formats[0] and alpha_modes[0] with .first().expect() in GpuContext::new()
FIX: Replace bare .unwrap() with .expect() documenting invariant in App::render()
CHORE: Add Send + Sync compile-time assertion tests to all 7 error types
CHORE: Add thiserror v2 to workspace dependencies
CHORE: Enhance release profile with strip = "symbols" and panic = "abort"
CHORE: Remove unused anyhow/pollster dependencies from wmux-render
