# Changelog

## 2026-04-19

FEATURE: Inherit the calling process's current directory for initial PTY spawns in `wmux-pty` — `SpawnConfig { working_directory: None }` now resolves to `std::env::current_dir()` when it is a user path, falling back to `$HOME` then `"."`. Windows system roots (`C:\Windows\System32`, `SysWOW64`, `C:\Windows`, `C:\`) are filtered so Start Menu/taskbar launches still land in HOME. Unblocks `cargo run -p wmux-cli -- system ping` from a pane of a wmux-app started in a project directory
REFACTOR: Bump vte from 0.13 to 0.15 and pass full byte slice to `Parser::advance` in `Terminal::process` — eliminates per-byte loop and handler reconstruction, picks up SGR perf fix (0.13.1) and grapheme boundary crash fix (0.14.1)
CHORE: Bump rand from 0.8 to 0.9 — fixes RUSTSEC-2026-0097 (unsound with custom logger), renames `thread_rng()` to `rng()` in wmux-ipc auth module
CHORE: Bump toml from 0.8 to 1.1 — major version jump, API remains backward compatible for wmux-config usage
CHORE: Bump compact_str from 0.8 to 0.9
CHORE: Bump patch versions across workspace via global `cargo update`
CHORE: Patch rustls-webpki transitively via Phase 1 update — fixes RUSTSEC-2026-0098/0099 (name constraints for URI names / wildcards)
CHORE: Whitelist RUSTSEC-2024-0436 (paste unmaintained) via `.cargo/audit.toml` — paste is pulled via metal (Apple backend), never linked on Windows
CHORE: Track Cargo.lock for reproducible builds — wmux is a binary application, not a library

## 2026-03-28

REFACTOR: Clean up window module — deduplicate imports, extract reusable text attributes and metrics in render loop
REFACTOR: Unify split shortcuts — replace Ctrl+D/Alt+D direct + Ctrl+K chord with single Ctrl+D → Arrow chord for all 4 directions
REFACTOR: Add missing command palette entries (split left/up, close workspace, browser tab, cycle tabs) and wire command_id_to_action mappings
FEATURE: Add Ctrl+V/C/X (paste/copy/cut) support in browser address bar — previously blocked by generic Ctrl+combo guard
FIX: Dismiss address bar editing on click-away — clicking outside the URL field now cancels editing mode so keyboard input returns to the correct target
FIX: Fix WebView2 stealing keyboard focus permanently — replace per-frame `focus_webview()` with transition-based focus, call `SetFocus(main_hwnd)` when switching away from browser pane
FIX: Prevent panic in scroll_up_in_region when scroll amount covers entire region from row 0 — usize underflow on `bottom - n` caused crash after minimize/restore with active terminals
FEATURE: Center status bar text and add subtle top border separator with connection dot aligned to centered text
FIX: Offset tab inline-edit text past the type indicator icon to prevent overlap during rename
CHORE: Compact tab bar — reduce height 40→34px, increase radius 4→6px, tighten gap 6→3px, shrink action buttons 28→24px
FIX: Move active tab indicator bar from bottom to top, span full tab width
FIX: Rework digital-obsidian accent — vivid blue (#2b7de6) replaces pastel (#a3c9ff) for visible Focus Glow on dark background
FIX: Brighten ANSI 0/8 in digital-obsidian theme — black (#1b1b1c→#484f58), bright-black (#353535→#8b949e) for readability on #131313
FIX: Reduce DIM intensity factor from 0.67 to 0.80 — gentler dimming for ultra-dark backgrounds
FEATURE: Add DIM/faint (SGR 2) text rendering — dim text attenuated to 80% brightness, applied to glyphs and underline/strikethrough decorations
FIX: Change default font-family from "Cascadia Code" to "JetBrainsMono Nerd Font" — enables font fallback chain (NF → Cascadia Code → system monospace)
FIX: Switch terminal text shaping from Basic to Advanced — enable cosmic-text font fallback for Nerd Font glyphs, emoji, and Unicode symbols (fixes □ replacement squares in prompt)
FIX: Suppress fontdb WARN about malformed system fonts via tracing filter (fontdb=error)
REFACTOR: Consolidate render and UI patterns — simplify rendering pipeline and event handling across core/render/ui layers, remove code duplication
REFACTOR: Split automation.rs into 4 sub-modules (navigation, dom, inspect) for wmux-browser
REFACTOR: Split panel.rs into 4 sub-modules (attach, layout, delegation) for wmux-browser
REFACTOR: Remove stale #[allow(dead_code)] on UpdateChecker/UpdateInfo, add justification comments to command palette hit-test methods, pre-allocate command palette rows Vec
REVERT: Remove broken WebView2 environment preload — fire-and-forget COM creates the environment on wrong thread (RPC_E_WRONG_THREAD), restore synchronous lazy creation at first browser panel use
FEATURE: Embed Symbols Nerd Font Mono for automatic powerline/devicon glyph fallback — Nerd Font symbols render with any primary font
FEATURE: Add runtime terminal font detection with fallback chain (JetBrainsMono NF → Cascadia Code → system monospace)
FIX: Fix Nerd Font family name constant — "JetBrainsMono Nerd Font" (v3 naming, no space)
FIX: Add empty font-family config validation — reject blank values, keep default
FIX: Replace all blocking rx.recv() calls with recv_with_pump() in wmux-browser — prevents STA thread deadlocks by pumping Windows messages while waiting for WebView2 COM callbacks
FIX: Add URL scheme validation to browser navigate() — reject javascript:, file://, data: schemes, only allow http(s)
FIX: Apply WebView2 security hardening at creation — disable context menus, status bar, script dialogs; disable DevTools in release builds
FIX: Handle ClientToScreen HRESULT failure — return error instead of silently discarding
FIX: Log WebView2 controller.Close() and DestroyWindow errors in BrowserPanel Drop — detect zombie msedgewebview2.exe processes
FIX: Remove TOCTOU race in ensure_user_data_dir() — call create_dir_all directly without exists() check
FIX: Clear __wmux_errors array after reading in read_errors() — prevent unbounded accumulation
FIX: Improve SAFETY comment on ComGuard::new() — document actual invariants (no prior MTA, balanced CoUninitialize)
FEATURE: Add DPI awareness to browser panel set_bounds() — query and log GetDpiForWindow for diagnostics
CHORE: Add InvalidUrlScheme error variant to BrowserError
CHORE: Document WS_POPUP HWND architecture as deliberate ADR (DXGI flip-model occlusion workaround)
CHORE: Document eval() security model — intentional arbitrary JS execution gated by IPC auth
FIX: Add 30s timeout to recv_with_pump — prevent infinite hang if WebView2 COM callback fails to fire
FIX: Trim URL whitespace before scheme validation in navigate() — normalize input at system boundary
FIX: Disable WebView2 host object injection and web messaging (SetAreHostObjectsAllowed, SetIsWebMessageEnabled) — defense in depth
FIX: Add upper bound check in rect_to_bounds — prevent f32 > i32::MAX saturation producing wrong window coordinates
FIX: Make ComGuard !Send+!Sync via PhantomData — prevent accidental cross-thread COM STA misuse
CHORE: Add Win32_Graphics_Gdi and Win32_UI_HiDpi features to windows crate dependency
FIX: Export COLORTERM=truecolor in PTY environment — enables 24-bit color in Claude Code and other terminal apps, fixes harsh red fallback colors
CHORE: Revert terminal text shaping to Basic — Advanced breaks monospace grid alignment on fallback font glyphs
FIX: Send Shift+Tab as CSI Z (BackTab) to PTY — previously sent as regular Tab, breaking Claude Code permission cycling
FEATURE: Respond to OSC 10/11 foreground/background color queries — allows terminal apps (Claude Code) to detect dark/light theme and adapt colors
FIX: Add 8px inner padding to terminal panes — text no longer starts flush against pane edges
FIX: Compute initial PTY cols/rows from usable terminal area (subtract sidebar, title bar, tab bar, status bar, padding) — prevent wrong dimensions at process startup
FIX: DPI-scale status bar height in surface viewport and rendering — consistent with title bar scaling
FEATURE: Implement CSI X (ECH — Erase Character) escape sequence — erase N chars at cursor without moving it
FEATURE: Render underline and strikethrough text decorations as colored quads — previously stored but never drawn
FIX: Harden wmux-app updater — mandatory SHA-256 checksum verification (refuse unsigned updates), incremental hashing during download (eliminates TOCTOU gap and 200MB memory spike), URL validation (HTTPS + host allowlist), download size limit (200MB), zero-length download rejection, path containment for apply_pending_update, startup recovery from interrupted updates, custom redirect policy blocking HTTP downgrades
FIX: Eliminate unsafe env var manipulation in updater tests — accept disabled flag as constructor parameter instead of reading WMUX_DISABLE_UPDATE
FIX: Remove blocking std::fs calls from async context in updater — store current_exe at construction, use tokio::fs::canonicalize and try_exists
CHORE: Add overflow-checks = true to release profile — prevent silent integer wrapping in release builds
FEATURE: Add custom title bar — replace native Windows title bar with GPU-rendered custom chrome via Win32 SetWindowSubclass, WM_NCCALCSIZE, WM_NCHITTEST; Codicons SVG icons for minimize/maximize/restore/close buttons; theme-driven colors (surface_1 bg, error for close hover); preserves window drag, snap, resize, and DWM shadow
FEATURE: Add 4 chrome button Codicons SVG icons (chrome-close, chrome-minimize, chrome-maximize, chrome-restore) to icon system
CHORE: Add Win32_UI_Controls feature to windows crate for MARGINS/DwmExtendFrameIntoClientArea
FIX: Fix title bar text left-aligned instead of centered — use cosmic_text::Align::Center
FIX: Fix chrome button icons invisible — CustomGlyph positions were in physical pixels but glyphon scales them by TextArea.scale, causing double-scaling; switch to logical pixel coordinates
FIX: Fix double title bar at startup — native Windows title bar remained because WM_NCCALCSIZE was never triggered; add SetWindowPos with SWP_FRAMECHANGED after SetWindowSubclass to force frame recalculation
FIX: Fix title text "wmux" pushed to the right — buffer width was in physical pixels but Align::Center computes position in buffer space which gets re-scaled by TextArea.scale; pass logical width to buffer
FIX: Fix chrome buttons not responding to clicks — WM_NCHITTEST used GetWindowRect (includes invisible DWM borders ~7px) causing button zone coordinates to mismatch client area; split into window-relative coords for resize edges and client-relative coords (via GetClientRect offset) for title bar buttons

## 2026-03-27

FIX: Wire i18n locale system into wmux-ui — replace 12+ hardcoded English strings with locale.t() lookups for notification panel, tab menus, severity labels, toggle labels, and time-ago timestamps
FIX: Add notification.time_* locale keys (en/fr) for relative timestamps with {n} placeholder
FIX: Eliminate per-frame heap allocations in render loop — String::clone for address bar URL, HashSet::new for browser orphan tracking, Vec::new for overlay rects
FIX: RAII-wrap Win32 snapshot handle in port_scanner.rs — OwnedHandle with Drop prevents handle leak on panic
CHORE: Restrict address_bar module visibility from pub to pub(crate)
FIX: Derive overlay_dim and shadow colors from theme background instead of hardcoded black
FIX: Replace hardcoded cursor alpha (0.85) in address bar and tab edit with ui_chrome.cursor_alpha
FIX: Wire is_animations_enabled() to AnimationEngine reduced motion for accessibility
REFACTOR: Remove 5 unused UiChrome fields (accent_hover, accent_pressed, accent_tint, border_strong, info)
FEATURE: Add browser address bar — back/forward buttons, URL text field with click-to-edit, Enter to navigate, Escape to cancel, smart URL handling (auto-https, localhost detection, DuckDuckGo search fallback)
FEATURE: Change browser default page from Google to DuckDuckGo — privacy-respecting default for embedded browser
FEATURE: Add `browser-default-url` config option — configurable default URL for browser panels (default: duckduckgo.com)
FIX: Remove bash from auto-detection shell order — Git Bash doesn't work under ConPTY, detection now follows CLAUDE.md rule (pwsh → powershell → cmd), bash available via explicit config
FEATURE: Add Ctrl+A select-all for all text edit fields — sidebar rename, tab rename, address bar URL; multi-strategy detection (physical key + text field + modifiers), Ctrl+letter guard blocks accidental insertion, visual selection highlight with accent_muted, typing/Backspace replaces selected text
FIX: Fix orphaned WebView2 panel visible after closing browser tab — render loop now removes panels whose surface was deleted from the actor, fixing both the ghost panel and the frozen-on-window-move issue
FIX: Fix keyboard input blocked in address bar and UI overlays — PtyExited event now carries pane_id, process_exited only set for focused pane; address bar handler moved before process_exited guard
FEATURE: Add text caret to browser address bar — visible cursor with proportional positioning, moves with arrow keys
FIX: Fix edit cursor positioning — multiply cursor offset by DPI scale factor to match glyphon's glyph_x × scale rendering; use proportional interpolation for all edit fields (sidebar, tab, address bar)
FIX: Fix sidebar workspace rename edit box misaligned by 22px and too short — remove stale folder icon offset, double edit box height to 2× line height
FIX: Fix overlay menus (context menus, split menu) bleeding through by underlying text — filter base text areas that overlap open menu rects before glyphon prepare
FIX: Fix sidebar edit mode showing subtitle text through edit box — skip info text rendering when workspace row is being edited
FEATURE: Add right-click context menu on tabs — rename and close actions, works on both pill tabs and toggle segments
FEATURE: Add close button to toggle mode — single X button to the right of the shell/browser toggle control
FIX: Reduce max tab width from 220px to 160px — prevent tabs from looking disproportionately wide
FIX: Close last pane properly closes workspace — clear stale tree, emit WorkspaceSwitched/Closed events, auto-correct focused_pane
FIX: Browser panel follows window during drag — add WindowEvent::Moved handler and NotifyParentWindowPositionChanged for WS_POPUP HWND
FEATURE: Allow closing the last shell tab — closes the entire pane instead of blocking with a safety guard
FIX: Navigate browser panel to Google instead of about:blank — blank page was showing when opening browser tab
FEATURE: Add shell/browser segmented toggle in tab bar — centered toggle control replaces individual pills when pane has 1 terminal + 1 browser, with icon + label per segment and accent highlight on active segment
FEATURE: Add sidebar collapsed mode (Ctrl+B) — 48px icon-only column with colored workspace circles, auto-adapting viewport, session persistence
CHORE: Remove folder icons from sidebar workspace cards — text and pills now start flush with accent bar
FIX: Don't show all system ports at startup — return empty when no shell PIDs registered yet (was falling back to unfiltered scan)
FIX: Update notification test signatures to match new `add()` method with severity parameter

## 2026-03-26

FEATURE: Filter sidebar ports by process tree — use `netstat -ano` with PID and `CreateToolhelp32Snapshot` to show only ports owned by workspace shell descendants, not system-wide listeners
FEATURE: Replace sidebar port text with colored pill badges — rounded quads with cycled theme colors (accent, success, warning, purple, cyan) at 15% alpha background, port text centered in each pill
## 2026-03-26

FEATURE: Wire complete notification panel — Ctrl+Shift+I toggles right-side slide-out with header, severity-colored items (category label, title, body, timestamp), scroll, hover, click-to-focus-workspace, Clear All, and close
FEATURE: Add notification badge count text in sidebar — unread count number rendered inside the accent-colored circle badge
FEATURE: Wire Ctrl+Shift+U to jump to last unread notification's source workspace
FEATURE: Add ListNotifications and ClearAllNotifications commands to AppState actor
FEATURE: Infer notification severity from title/body keywords (error/warn/success → colored stripes)
FIX: Force opaque background on notification panel — terminal content was bleeding through 95% alpha surface_overlay
FIX: Filter out Cleared notifications from ListNotifications response — Clear All now has visible effect
FIX: JumpLastUnread fetches from actor directly — works even when panel is closed
FIX: Mutual exclusion between all overlays — command palette and search now close notification panel
FIX: Fix filter tab text clipping — multiply measured line_w by scale_factor (DPI) when computing pill widths, since glyphon layout_runs() returns buffer-coordinate widths but quads/bounds use physical pixels
REFACTOR: Extract PaletteLayout struct — single source of truth for palette position/size math, replaces 3 duplicated inline computations across render_quads/render.rs
REFACTOR: Add dirty tracking for palette search — skip expensive search/set_text/shape_until_scroll when query+filter unchanged between frames (eliminates ~100 per-frame allocations)
FIX: Use text_inverse from UiChrome for active filter tab text instead of hardcoded white — compliant with visual-integrity rules
FIX: Store host HWND immediately after creation in BrowserPanel::attach() — prevents HWND leak if subsequent COM operations fail
FIX: Add tracing::warn when command_id_to_action returns None — unhandled command IDs no longer silently swallowed
REFACTOR: Restrict command_palette module visibility to pub(crate) — internal UI types no longer leak into wmux-ui public API
REFACTOR: Extract SHORTCUT_COL_WIDTH and SHORTCUT_COL_PAD constants from hardcoded magic numbers (120/110/108/100) in palette render code
FEATURE: Wire complete command palette — Ctrl+Shift+P opens overlay with search input, filter tabs, result list with shortcut badges, keyboard navigation (arrows, Tab, Enter, Escape), and command execution via ShortcutAction dispatch
FIX: Eliminate 1-frame palette rendering delay — pre-compute search results and result_count before render_quads so palette height is correct on the first frame
FEATURE: Populate Workspaces and Surfaces filter tabs — Workspaces tab shows workspace names (Enter switches workspace), Surfaces tab shows surface/tab titles (Enter focuses pane + surface), All tab combines commands + workspaces + surfaces
REFACTOR: Replace re-search in Enter handler with palette_actions cache — render path stores PaletteAction per result row, handler reads cached actions instead of re-searching
REFACTOR: Replace .expect() with non-panicking send in COM callbacks (wmux-browser) — prevents potential panic inside STA COM handlers if channel receiver is dropped
FEATURE: Wire focus glow into render loop — active pane now displays animated blue halo (accent_glow from theme, cross-fade via AnimationEngine)
FIX: Render focus glow after pane dimming (z-order 10) instead of before backgrounds (z-order 3) — glow was invisible because adjacent pane backgrounds painted over it
FIX: Focus glow uses transparent inner fill + outer-only halo — inner_color was tinting the entire pane area blue instead of showing just the edge glow per Stitch maquette
FEATURE: Increase pane divider gap 2→8px for focus glow breathing room, add 2px solid accent border on focused pane (matches Stitch maquette)
REFACTOR: Pane tree layout tests use DIVIDER_WIDTH constant instead of hardcoded values
FIX: Add opaque surface_base fill for content area — pane gaps were transparent with Mica, showing light desktop backdrop instead of dark background
REFACTOR: Remove permanent pane divider lines — gaps now show clean dark surface_base, dividers appear only on hover for resize affordance
FIX: Boost focus glow visibility — outer glow alpha 0.30→0.55, border alpha 0.40→0.85, border width 2→3px to match Stitch maquette vivid blue
FEATURE: Add "Stitch Blue" theme — vivid saturated blue accent (#0979d5) with warm orange warnings (#dd8b00), derived from Google Stitch UI redesign maquettes
FEATURE: Add filter tabs (All/Commands/Workspaces/Surfaces) to command palette with pill-style toggles and keyboard cycling
FEATURE: Enhance focus glow — increase radius 10→18px, boost outer alpha 8%→21%, add visible inner ring for stronger active pane indication
REFACTOR: Widen notification panel severity stripe 2→4px, increase item height 72→82px for better visual hierarchy
FEATURE: Add "Digital Obsidian" theme — deep dark glass aesthetic (#131313 base) with electric accent signals, set as new default
REFACTOR: Adapt surface elevation step to base darkness — formula `(L*0.45).clamp(0.030, 0.055)` gives tighter tonal layering for very dark themes while preserving existing theme appearance

## 2026-03-25

FEATURE: Add globe button in tab bar to open browser surface (clickable icon, WebView2 availability-gated)
FIX: Use owned popup HWND (WS_POPUP) for WebView2 panels instead of child HWND — DXGI flip swap chains occlude child windows, popup is a separate DWM visual
FIX: Skip wgpu terminal rendering for browser-active panes — don't render background quad, grid text, or cursor where WebView2 child HWND occupies the area
FIX: Skip pane dimming overlay for browser-active panes (overlay would occlude the WebView2 popup)
FIX: Convert client→screen coordinates in BrowserPanel::set_bounds for popup window positioning
FIX: Use actual pane viewport rect for browser panel creation instead of hardcoded 100x100+800x600
FIX: Skip terminal keyboard input when focused surface is a browser (WebView2 handles its own input)
FIX: Give WebView2 keyboard focus (MoveFocus) when browser surface is active in focused pane
FIX: Prevent panic in compute_ratio when split dimension < 2*MIN_PANE_SIZE (f32::clamp precondition violation)
FIX: Update focused_pane after CloseWorkspace to prevent stale focus (input lost, no cursor)
FIX: Guard menu item index against negative values in top padding click (4 locations)
FIX: Remove dead pid parameter from SetPanePid — renamed to SetPaneInitialCwd with clean API
REFACTOR: Compute pane_count() once in layout_with_dividers instead of traversing tree twice per frame
REFACTOR: Gate sidebar text_areas and status_icons clone behind sidebar.visible check
REFACTOR: Remove no-op render_pane_borders call from render loop
REFACTOR: Apply clean code improvements — #[allow] → #[expect] with reason (4 lints), remove unused import
FEATURE: Add 8 tests for layout_with_dividers and resize_by_split_id (nested trees, clamping, error cases)
FIX: Correct divider drag-to-resize in nested pane trees — add SplitId to PaneTree Split nodes, compute dividers from tree traversal instead of flat layout comparison, resize by split ID to target the correct split node
FIX: Clear mouse state on divider drag release to prevent stale selection activation
REFACTOR: Remove blue focus stripe and glow from focused panes — uniform neutral dividers between all panes
REFACTOR: Replace blue sidebar separator with subtle neutral border_subtle line
FEATURE: Refined pane dividers (2px gap, 1px border_subtle line, 2px border_default hover highlight)
REFACTOR: Remove Claude Code session persistence — delete process_detect module, strip claude_session_id from session schema, remove Claude --resume/--continue restore logic (align with CMX behavior)
FEATURE: Close workspace shortcut (Ctrl+Shift+W) — closes the active workspace and switches to adjacent
FEATURE: Sidebar right-click context menu — "Rename Workspace" and "Close Workspace" items with hover highlight, shadow, rounded popup (follows SplitMenu pattern)
FIX: PowerShell shell integration hook injection — use env var `WMUX_SHELL_HOOK` to pass hook path, avoiding quote_arg mangling that prevented OSC 7 emission after `cd`
FIX: Boost UI chrome text toward white for dark themes — blend foreground 70% toward #ffffff in derive_ui_chrome() (text_primary #d4d4d4→#f2f2f2), terminal foreground unchanged
FIX: Propagate initial pane CWD to workspace metadata — sidebar now shows correct project directory immediately instead of stale home dir
FIX: Use accent_muted (30% alpha) for active workspace card background — visible like CMX instead of nearly invisible accent_tint (8%)
FEATURE: Card-based sidebar design — rounded card backgrounds per workspace (8px radius), accent_muted for active, surface_0 for inactive, surface_2 hover
FEATURE: Display IPC status text in sidebar cards — first StatusEntry value shown as description below workspace name
FEATURE: Display environment info in sidebar cards — git branch, dirty indicator, truncated CWD, and listening ports
REFACTOR: Remove pane count from sidebar info text — workspace cards show only relevant metadata
REFACTOR: Adjust sidebar layout for card margins (6px horizontal, 4px gap) and ROW_HEIGHT 110px
REFACTOR: Position all sidebar elements relative to card geometry (text, icons, badge, edit cursor)
REFACTOR: Extend WorkspaceSnapshot with status_text, ports, and git_dirty fields
FIX: Align edit cursor with workspace icon reserve offset (28px)
FIX: Sort status entries by key for deterministic status_text display in sidebar cards
FIX: Use theme cursor_alpha instead of hardcoded 0.85 for edit cursor color
FIX: Match edit buffer width to card-relative edit box dimensions

## 2026-03-23

FIX: Scale TAB_BAR_HEIGHT by DPI factor in all hit-testing and UI rendering — tab clicks, hover highlights, drag/drop overlays, text bounds, cursor-cell mapping, shadow, and edit pill now use vp.tab_bar_height() or TAB_BAR_HEIGHT * scale_factor instead of the raw 40px constant
FIX: Apply DPI scaling to terminal font metrics — font_size * scale_factor gives physical pixel dimensions, fixing wrong column/row count and garbled TUI display (Claude Code) on high-DPI screens
FIX: Use configured font size for initial terminal metrics — was using hardcoded 20pt instead of config value (default 16pt), causing cell dimension mismatch between layout calculation and actual rendering
FIX: Align build_row_buffers glyph size with metrics — was using hardcoded FONT_SIZE constant instead of the metrics' actual font_size, causing glyph/cell size mismatch
FEATURE: Handle ScaleFactorChanged event — update font metrics, recalculate grid dimensions, and recreate renderers when window moves between monitors with different DPI
FIX: Invalidate stale Claude session UUIDs — clear `pane_claude_sessions` on process exit and when new non-Claude process spawns, prevents saving old UUID when user switches to a different Claude session
FIX: Exact pane→session mapping via WMI command-line query — read `--resume <uuid>` from each Claude process to correlate PIDs to sessions, eliminates wrong-panel assignment
FIX: Remove 5-minute cutoff on session file resolution — active sessions are always the most recently modified, cutoff was too aggressive and caused all UUIDs to resolve as `__continue__`
FIX: Stable pane→session mapping — remember Claude session UUID from `--resume` at restore time, reuse it at subsequent saves instead of re-resolving from filesystem (prevents session swapping between panels)
FIX: Detect Claude in restored panes — `has_claude_descendant` now checks the root PID itself (handles direct Claude spawn without shell), set initial CWD for panes without OSC 7
FIX: Resolve unique Claude Code session UUIDs per pane — multi-pane same-CWD now restores each panel's own session via `claude --resume <uuid>` instead of all panels competing for `--continue`
FEATURE: Detect Claude Code sessions in panes and auto-restore on session restore — process tree detection via ToolHelp32 snapshot, `claude_session_id` field in session schema, filesystem-based UUID resolution, fallback to normal shell if Claude not installed
REFACTOR: Persist window geometry (position, size, maximized state) and sidebar width in session — restore on startup, update when window resizes or sidebar width changes
REFACTOR: Add session restore safety — validate pane tree depth (max 16) to prevent pathological recursion from malformed session files; extract first_leaf helper for simplified restore logic
REFACTOR: Extract DRY helpers in app_state actor (build_workspace_snapshot, mark_active_backing_dirty) and UI handlers (apply_text_edit_key) — eliminate 3 code duplications
FEATURE: Complete session persistence — save per-pane CWD (via OSC 7), window geometry (position/size/maximized), sidebar width; restore full recursive pane tree at arbitrary depth (was depth-1 only); inject saved scrollback text into terminals on restore
FIX: Sanitize scrollback text on session restore — strip VTE escape sequences, normalize \n to \r\n for correct line alignment
FIX: Validate CWD from session.json — reject UNC paths, relative paths, and path traversal to prevent NTLM relay
FIX: Add pane tree depth limit (16) on session load to prevent pathological nesting
REFACTOR: Extract `json_str` helper in wmux-browser automation — DRY 21 repeated `map_err` error conversions
REFACTOR: Simplify sidebar workspace rows — remove subtitle ("> 1 pane"), reduce row height (72→36px)
FIX: Fix rename input box position — offset past icon, properly sized for single-line layout
FIX: Render SVG icons as alpha masks (ContentType::Mask) for theme colorization — icons now visible on dark backgrounds
FIX: Remove permanent blue background on + button — now transparent with hover-only bg (Zed-like)
FIX: Align tab bar surface type icon vertically with text baseline
REFACTOR: Replace all icon font glyphs with Codicons SVGs via CustomGlyph — 18 SVGs embedded, resvg rasterization
FEATURE: Activate CustomGlyph SVG pipeline — prepare_with_custom() + resvg rasterization for SVG icons
FIX: Increase icon font size (14→16px) and buffer bounds to prevent clipping in tab bar
FIX: Increase icon-to-text spacing in sidebar (20→24px) and tab bar (22→24px)
FIX: Correct split button icon codepoint (E73F→E738 ColumnDouble)
FEATURE: Add hover glow effect on + and split tab bar buttons
FEATURE: Render StatusEntry icons in sidebar — status badges from IPC display icon glyphs per workspace
FEATURE: Add Icon::from_name() for IPC icon name resolution and wire StatusEntry::icon to WorkspaceSnapshot
FEATURE: Wire config font-family/font-size to terminal renderer — enables Nerd Fonts and custom monospace fonts
FEATURE: Add NotificationSeverity enum with severity-specific stripe colors in notification panel
FEATURE: Add workspace icon (terminal glyph) to sidebar workspace list
FEATURE: Add search magnifying glass icon to search overlay when icon font is available
REFACTOR: Migrate icon rendering from quad primitives to Segoe Fluent Icons font glyphs with quad fallback
FEATURE: Add pre-shaped icon buffers for UI chrome (terminal, browser, split, search, direction arrows)
FEATURE: Add centralized Icon enum with Segoe Fluent Icons codepoints (17 icons for UI chrome)
FEATURE: Add icon font detection — probe for Segoe Fluent Icons at startup with has_icon_font() accessor

## 2026-03-22

FEATURE: Add sidebar drag-to-resize — click and drag the right edge to adjust width (180-480px range)
FIX: Clamp sidebar width on initialization to prevent unusable layouts from config
FIX: Reset cursor icon to default after sidebar resize release
FIX: Reset sidebar interaction state on toggle to prevent stale ResizeHover
FIX: Reduce sidebar resize hit zone overshoot into content area (5→2px) to avoid divider conflicts
REFACTOR: Improve tab bar styling — reduce corner radius (8→4px), height (44→40px), fix text vertical centering
REFACTOR: Switch default theme palette to VS Code Dark+ inspired colors for better contrast and readability
REFACTOR: Increase sidebar workspace row height (64→72px) for better visual separation
REFACTOR: Increase default sidebar width from 240 to 260px

FEATURE: Add Zed-style chord shortcuts for split — Ctrl+K then Arrow (Right/Left/Up/Down)
FEATURE: Add SplitLeft and SplitUp actions — new pane placed before the original via swap
FEATURE: Add split direction button in tab bar with dropdown menu (4 directions with icons and shortcut hints)
FEATURE: Add chord shortcut state machine with 1s timeout for multi-key sequences

FIX: Switch default theme to cmux Apple System Colors dark — bg=#1e1e1e, fg=#ffffff, accent=#0869cb
FIX: Fix Unicode tofu (square blocks) — use explicit "Segoe UI" font family instead of generic SansSerif for all UI chrome text
FIX: Make shadows visible — increase shadow alpha (0.25→0.45 dark, 0.15→0.30 light) and sigma (2→4 for shadow_sm)
FIX: Make focus glow visible — inner ring alpha 0.0→0.12, outer halo alpha 0.25→0.35
FIX: Reduce plus button background opacity (0.3→0.12) so quad-drawn + icon is clearly visible

FEATURE: Add centralized typography tokens (Title/Body/Caption/Badge) — consistent type scale across all UI chrome
FEATURE: Add horizontal and radial gradient modes to shader — gradient_mode field (0=none, 1=vertical, 2=horizontal, 3=radial) with linear-space interpolation
FEATURE: Add push_horizontal_gradient_quad and push_radial_gradient_quad to QuadPipeline
REFACTOR: Apply typography tokens to tab bar (Body), sidebar (Body/Caption), status bar (Caption), search bar (Caption)

FEATURE: Wire AnimationEngine into render loop — animations now drive continuous redraws, foundation for all UI motion
FEATURE: Animate focus glow cross-fade — "Luminous Void" glow fades in smoothly (MOTION_NORMAL, CubicOut) on pane focus change
FEATURE: Add tab hover background animation — hovered tabs show animated surface_2 highlight (MOTION_FAST, CubicOut)
FEATURE: Add SpringOut easing to AnimationEngine — critically damped spring curve for natural deceleration
FEATURE: Track divider hover state — dividers now trigger redraw on hover change for visual feedback

FEATURE: Add ShadowPipeline with Evan Wallace analytical erf() drop shadows — GPU-native Gaussian-convolved box shadows in a single quad per shadow
FEATURE: Add shadow depth tokens (shadow_sm/md/lg) to UiChrome — sigma and offset_y for 3 elevation levels
FEATURE: Add accent_pressed color to UiChrome — accent darkened by 10% lightness for press states
FEATURE: Replace flat shadow quads with analytical shadows — tab bar, status bar, and sidebar now use ShadowPipeline with soft Gaussian blur

FEATURE: Add per-corner border radius (vec4) to shader SDF — enables asymmetric rounded corners via push_asymmetric_quad([TL, TR, BR, BL])
FEATURE: Add fwidth()-based adaptive anti-aliasing to SDF edges — scale-independent AA, sharper on HiDPI
FEATURE: Add linear-space gradient interpolation — sRGB↔linear conversion for perceptually correct gradients
FEATURE: Add atlas.trim() call per frame to prevent GPU glyph cache memory leak
REFACTOR: Switch terminal text shaping from Advanced to Basic for ~30% faster ASCII rendering
REFACTOR: Add delta time capping (33ms) to AnimationEngine — prevents animation jumps after alt-tab
REFACTOR: Grow QuadInstance from 80 to 96 bytes for per-corner radius support

FEATURE: Add Ctrl+Shift+L shortcut to open a browser tab (WebView2) in the focused pane
FEATURE: Add `wmux browser open/navigate/back/forward/reload/url/eval` CLI commands — fully functional browser control from the shell
FEATURE: Add surface type indicators on tab pills — chevron for terminal, circle for browser
FEATURE: Wire WebView2 browser integration — BrowserManager on UI/STA thread, browser command channel (IPC → EventLoopProxy → UI), browser.open/navigate/back/forward/reload/url/eval/close IPC methods functional, CreateBrowserSurface actor command
FEATURE: Make tab bar always visible — tab bar renders for all panes (even single-tab), "+" button to create new surfaces, click-to-switch works with any tab count
FEATURE: Add FromStr to all ID types (PaneId, SurfaceId, WorkspaceId, WindowId) for string parsing
FIX: Align mouse coordinates and selection highlights with terminal content area by accounting for always-visible tab bar offset

CHORE: Increase all UI element sizes for better readability on high-res displays — terminal font 14→20, tab bar 36→44, sidebar rows 48→56, status bar 28→34, close button 14→18, default font-size 12→16, sidebar width 200→240

FEATURE: Wire Config::load() — load user config from disk (wmux > Ghostty > defaults) instead of hardcoded defaults, enabling theme selection, font, sidebar width, scrollback, inactive pane opacity
FEATURE: Wire StatusBar rendering — display workspace name, pane count, git branch, and connection status dot with pulse animation in the bottom bar
FEATURE: Wire session restore — recreate workspaces, pane trees with split ratios, and PTYs with saved CWDs from session.json on startup
FIX: Expand system.capabilities to list all 26 functional IPC methods across all handlers (was only listing 3 system.* methods)

FEATURE: Add close button (×) on surface tabs — click × to close a surface, hover highlights in red
FEATURE: Add double-click to rename surface tabs — inline editing with Enter/Escape, same UX pattern as sidebar workspace rename
FEATURE: Add RenameSurface command to actor pipeline — rename_surface() on AppStateHandle, surface_ids in PaneRenderData/PaneViewport

FIX: Search bar space key input — winit reports Space as NamedKey::Space, was silently consumed by catch-all
FIX: Search bar cursor position — use glyphon layout_runs().line_w instead of monospace cell_width estimate
FIX: Search bar input blocked after shell exit — move search handler before process_exited check
FIX: Add Shift+Enter for previous match navigation in search bar
REFACTOR: Extract search bar layout constants (SEARCH_BAR_HEIGHT, SEARCH_BAR_PADDING, SEARCH_COUNT_WIDTH)

FIX: Render search bar text (query + match count) via glyphon — search overlay was only drawing background quads without any visible text, making it non-functional

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
