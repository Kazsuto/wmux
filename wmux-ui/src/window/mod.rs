mod event_loop;
mod handlers;
mod render;

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::{
    divider::DragState, effects::EffectResult, event::WmuxEvent, input::InputHandler,
    mouse::MouseHandler, search::SearchState, shortcuts::ShortcutMap, sidebar::SidebarState,
    toast::ToastService, UiError,
};
use tokio::sync::mpsc;
use winit::{
    event_loop::{EventLoop, EventLoopProxy},
    keyboard::ModifiersState,
    window::Window,
};
use wmux_config::{Locale, UiChrome};
use wmux_core::{AppEvent, AppStateHandle, PaneId, SurfaceId, TerminalMode};
use wmux_render::{
    GlyphonRenderer, GpuContext, QuadPipeline, ShadowPipeline, TerminalMetrics, TerminalRenderer,
};

/// UI-thread state created during window initialization.
///
/// Contains only rendering and input state. All terminal/pane state
/// lives in the AppState actor and is accessed via snapshots.
pub(crate) struct UiState<'window> {
    // Rendering
    pub(crate) window: Arc<Window>,
    pub(crate) gpu: GpuContext<'window>,
    pub(crate) quads: QuadPipeline,
    pub(crate) shadows: ShadowPipeline,
    pub(crate) glyphon: GlyphonRenderer,
    /// Per-pane terminal renderers. Created/removed as panes are split/closed.
    pub(crate) renderers: HashMap<PaneId, TerminalRenderer>,
    pub(crate) metrics: TerminalMetrics,

    // Input
    pub(crate) input: InputHandler,
    pub(crate) mouse: MouseHandler,
    pub(crate) shortcuts: ShortcutMap,
    pub(crate) modifiers: ModifiersState,
    pub(crate) cursor_pos: (f64, f64),

    // Notifications
    pub(crate) toast_service: ToastService,

    // Sidebar
    pub(crate) sidebar: SidebarState,
    /// Cached workspace list — refreshed once per frame during render.
    pub(crate) workspace_cache: Vec<wmux_core::WorkspaceSnapshot>,

    // Divider drag
    /// Cached dividers from the last layout — used for hover/drag without blocking.
    pub(crate) dividers: Vec<crate::divider::Divider>,
    /// Active divider drag state, if the user is currently dragging.
    pub(crate) drag_state: Option<DragState>,

    // Active pane tracking
    pub(crate) focused_pane: PaneId,
    pub(crate) cols: u16,
    pub(crate) rows: u16,
    pub(crate) process_exited: bool,
    /// Cached terminal modes from the last render snapshot.
    pub(crate) terminal_modes: TerminalMode,
    /// Cached pane layout from the last render — used for hit-testing on
    /// mouse clicks without blocking on the actor.
    pub(crate) last_layout: Vec<(PaneId, wmux_core::rect::Rect)>,

    // Command palette
    pub(crate) command_palette: crate::command_palette::CommandPalette,
    pub(crate) command_registry: wmux_core::CommandRegistry,
    /// Glyphon text buffer for the palette query / placeholder display.
    pub(crate) palette_query_buffer: glyphon::Buffer,
    /// Glyphon text buffers for the 4 filter tab labels (static text).
    pub(crate) palette_filter_buffers: [glyphon::Buffer; 4],
    /// Pool of glyphon text buffers for result command names (up to 20).
    pub(crate) palette_result_buffers: Vec<glyphon::Buffer>,
    /// Pool of glyphon text buffers for result shortcut badges (up to 20).
    pub(crate) palette_shortcut_buffers: Vec<glyphon::Buffer>,
    /// Pool of glyphon text buffers for per-result descriptions / metadata
    /// (muted line under each name — commands carry a description, workspaces
    /// carry `branch · N panes · path`, surfaces leave it empty).
    pub(crate) palette_desc_buffers: Vec<glyphon::Buffer>,
    /// Pool of glyphon text buffers for the workspace number rendered on the
    /// colored square icon of each workspace result row (14px bold).
    pub(crate) palette_ws_icon_buffers: Vec<glyphon::Buffer>,
    /// Static footer hint text ("↑↓ navigate   ↵ select   Tab filter   ESC close").
    pub(crate) palette_footer_hints_buffer: glyphon::Buffer,
    /// Static footer brand text ("wmux").
    pub(crate) palette_footer_brand_buffer: glyphon::Buffer,
    /// Pre-built 14x14 CustomGlyph arrays for each command row icon, filled
    /// on the dirty-check frame so `text_areas()` can hand out the right
    /// reference per row without fighting lifetimes.
    pub(crate) palette_row_icons: Vec<[glyphon::CustomGlyph; 1]>,
    /// Per-row kind: `None` for regular result rows, `Some(label)` for section
    /// headers rendered inline between groups ("COMMANDS", "WORKSPACES", etc.).
    /// Parallel to `palette_actions` — section rows have `PaletteAction::Command("")`
    /// as a sentinel and are skipped by navigation/Enter.
    pub(crate) palette_row_sections: Vec<Option<String>>,
    /// Actions for the currently visible palette results (set during render, read by Enter handler).
    pub(crate) palette_actions: Vec<crate::command_palette::PaletteAction>,
    /// Last query used for palette search — dirty tracking to skip re-search when unchanged.
    /// `None` means the palette has never rendered (guarantees dirty=true on first open).
    pub(crate) palette_last_query: Option<String>,
    /// Last filter used for palette search — dirty tracking.
    /// `None` means the palette has never rendered (guarantees dirty=true on first open).
    pub(crate) palette_last_filter: Option<crate::command_palette::PaletteFilter>,
    /// Last `scroll_offset` used when populating the visible buffer window.
    /// When the user scrolls, the slot→row mapping changes and we must
    /// re-fill `palette_result_buffers[0..visible]` from the new slice.
    /// `None` means the palette has never rendered.
    pub(crate) palette_last_scroll: Option<usize>,
    /// Last `selected` observed by the render loop. Used to decide whether
    /// to snap `scroll_offset` back onto the selection this frame: on a
    /// selection change we follow; on a pure wheel scroll we leave the
    /// viewport where the user put it.
    /// `None` means the palette has never rendered.
    pub(crate) palette_last_selected: Option<usize>,
    /// Full row list backing the visible window. Stored separately from
    /// `palette_actions` so the renderer can rebuild slot buffers on scroll
    /// changes without re-running the search.
    pub(crate) palette_rows_cache: Vec<crate::command_palette::PaletteRowCache>,

    // Notification panel
    pub(crate) notification_panel: crate::notification_panel::NotificationPanel,
    /// Cached notifications — refreshed each frame when panel is open.
    pub(crate) notification_cache: Vec<wmux_core::Notification>,
    /// Glyphon buffer for the "Notifications" header title.
    pub(crate) notif_header_buffer: glyphon::Buffer,
    /// Glyphon buffer for the "Clear all" header text.
    pub(crate) notif_clear_all_buffer: glyphon::Buffer,
    /// Glyphon buffer for the empty state text.
    pub(crate) notif_empty_buffer: glyphon::Buffer,
    /// Pool of glyphon buffers for notification category labels.
    pub(crate) notif_category_buffers: Vec<glyphon::Buffer>,
    /// Pool of glyphon buffers for notification titles.
    pub(crate) notif_title_buffers: Vec<glyphon::Buffer>,
    /// Pool of glyphon buffers for notification body text.
    pub(crate) notif_body_buffers: Vec<glyphon::Buffer>,
    /// Pool of glyphon buffers for notification timestamps.
    pub(crate) notif_time_buffers: Vec<glyphon::Buffer>,

    // Search overlay
    pub(crate) search: SearchState,
    /// Cached visible rows (scrollback + grid) for the focused pane, used by search.
    /// Updated every frame from the focused pane's render snapshot.
    pub(crate) last_search_rows: Vec<(usize, String)>,
    /// Total visible row count last frame (scrollback_visible + grid_rows).
    pub(crate) last_total_visible_rows: usize,
    /// Glyphon text buffer for the search query display.
    pub(crate) search_query_buffer: glyphon::Buffer,
    /// Glyphon text buffer for the search match count display.
    pub(crate) search_count_buffer: glyphon::Buffer,

    // Tab bar text
    /// Cached glyphon text buffers for tab titles, keyed by layout pane ID.
    pub(crate) tab_title_buffers: HashMap<PaneId, Vec<glyphon::Buffer>>,
    /// Cached glyphon text buffers for toggle labels ("Shell"/"Browser"), keyed by pane ID.
    pub(crate) toggle_label_buffers: HashMap<PaneId, [glyphon::Buffer; 2]>,
    /// Cached viewports from the last render — used for tab bar hit-testing.
    pub(crate) last_viewports: Vec<wmux_render::PaneViewport>,
    /// Active tab drag state for drag-and-drop reordering.
    pub(crate) tab_drag: TabDragState,
    /// Which tab close button is currently hovered: (pane_id, tab_index).
    pub(crate) tab_close_hover: Option<(PaneId, usize)>,
    /// Inline editing state for renaming a surface tab.
    pub(crate) tab_edit: TabEditState,
    /// Glyphon buffer for the tab inline edit text.
    pub(crate) tab_edit_buffer: Option<glyphon::Buffer>,
    // SVG icon rendering — empty buffer + pre-built CustomGlyph arrays.
    /// Empty buffer used as anchor for SVG CustomGlyph TextAreas (no text content).
    pub(crate) icon_empty_buffer: glyphon::Buffer,
    /// Pre-built CustomGlyph arrays for each icon (avoid temporary lifetime issues).
    pub(crate) cg_close: [glyphon::CustomGlyph; 1],
    pub(crate) cg_add: [glyphon::CustomGlyph; 1],
    pub(crate) cg_terminal: [glyphon::CustomGlyph; 1],
    pub(crate) cg_globe: [glyphon::CustomGlyph; 1],
    pub(crate) cg_split: [glyphon::CustomGlyph; 1],
    pub(crate) cg_search: [glyphon::CustomGlyph; 1],
    pub(crate) cg_arrows: [[glyphon::CustomGlyph; 1]; 4],
    /// Pre-built CustomGlyph arrays for status badge icons, keyed by Icon variant.
    pub(crate) status_icon_cgs: HashMap<wmux_render::icons::Icon, [glyphon::CustomGlyph; 1]>,
    /// Pre-built CustomGlyph array for the 3 chrome button icons (minimize, maximize, close).
    pub(crate) cg_chrome_buttons: [glyphon::CustomGlyph; 3],

    // Browser
    /// WebView2 browser panel manager — lives on the UI/STA thread.
    pub(crate) browser_manager: Option<wmux_browser::BrowserManager>,
    /// The main window HWND — needed as parent for WebView2 child HWNDs.
    pub(crate) main_hwnd: windows::Win32::Foundation::HWND,
    /// Kind of the active surface in the focused pane — used to route keyboard input.
    pub(crate) focused_surface_kind: wmux_core::PanelKind,
    /// Address bar state for the focused browser surface.
    pub(crate) address_bar: crate::address_bar::AddressBarState,
    /// Glyphon text buffer for the address bar URL display.
    pub(crate) address_bar_buffer: glyphon::Buffer,
    /// Last known URL per browser surface (avoids per-frame COM calls).
    pub(crate) browser_urls: HashMap<wmux_core::SurfaceId, String>,
    /// Surface ID of the browser that currently has Win32 keyboard focus.
    /// Prevents calling `focus_webview()` every frame and enables reclaiming
    /// focus when switching away from a browser pane.
    pub(crate) browser_focus_target: Option<wmux_core::SurfaceId>,
    /// Config-defined default browser URL.
    pub(crate) browser_default_url: String,

    // Title bar
    /// Custom title bar state (chrome buttons, drag zone, Win32 subclassing).
    pub(crate) titlebar: crate::titlebar::TitleBarState,

    // Status bar
    pub(crate) status_bar: crate::status_bar::StatusBar,
    pub(crate) status_bar_data: crate::status_bar::StatusBarData,
    /// Elapsed time in seconds since window creation — used for status bar pulse animation.
    pub(crate) start_instant: std::time::Instant,

    // Chord shortcuts
    /// State machine for Ctrl+D chord sequences.
    pub(crate) chord_state: ChordState,

    // Split menu
    /// State of the split direction popup menu.
    pub(crate) split_menu: SplitMenuState,
    /// Glyphon text buffers for the 4 split menu items.
    pub(crate) split_menu_buffers: [glyphon::Buffer; 4],
    /// Glyphon text buffers for the 4 split menu shortcut hints.
    pub(crate) split_menu_hint_buffers: [glyphon::Buffer; 4],
    /// Which split menu item is currently hovered (0–3), if any.
    pub(crate) split_menu_hover: Option<usize>,

    // Workspace context menu
    /// State of the workspace context menu (right-click on sidebar).
    pub(crate) workspace_menu: WorkspaceMenuState,
    /// Glyphon text buffers for workspace context menu items.
    pub(crate) workspace_menu_buffers: [glyphon::Buffer; WORKSPACE_MENU_ITEMS],
    /// Which workspace menu item is currently hovered, if any.
    pub(crate) workspace_menu_hover: Option<usize>,

    // Tab context menu
    /// State of the tab context menu (right-click on a tab).
    pub(crate) tab_menu: TabContextMenuState,
    /// Glyphon text buffers for tab context menu items.
    pub(crate) tab_menu_buffers: [glyphon::Buffer; TAB_MENU_ITEMS],
    /// Which tab menu item is currently hovered, if any.
    pub(crate) tab_menu_hover: Option<usize>,

    // Animation
    pub(crate) animation: crate::animation::AnimationEngine,
    /// Animation ID for focus glow fade-in on the newly focused pane.
    pub(crate) focus_glow_anim: Option<u64>,
    /// Which tab is currently hovered: (pane_id, tab_index).
    pub(crate) tab_hover: Option<(PaneId, usize)>,
    /// Animation ID for tab hover background transition.
    pub(crate) tab_hover_anim: Option<u64>,
    /// Index of the currently hovered divider (into self.dividers).
    pub(crate) divider_hover: Option<usize>,

    // Visual theming
    /// UI chrome colors derived from the current theme.
    pub(crate) ui_chrome: UiChrome,
    /// Result of applying Mica/Acrylic effects (determines clear color alpha).
    pub(crate) effect_result: EffectResult,
    /// Theme ANSI palette for terminal renderers.
    pub(crate) theme_ansi: [(u8, u8, u8); 16],
    /// Theme cursor color for terminal renderers.
    pub(crate) theme_cursor: (u8, u8, u8),
    /// Theme foreground color for terminal text default color.
    pub(crate) theme_foreground: (u8, u8, u8),
    /// Opacity for inactive panes (from config). 0.0 = fully dimmed, 1.0 = no dimming.
    pub(crate) inactive_pane_opacity: f32,
    /// Display scale factor (DPI scaling) from the OS window.
    pub(crate) scale_factor: f32,
    /// Resolved terminal font family, or `None` for system monospace fallback.
    pub(crate) terminal_font_family: Option<String>,
    /// User-configured terminal font size.
    pub(crate) terminal_font_size: f32,

    // Localization
    /// Locale instance for i18n string lookups.
    pub(crate) locale: Locale,

    // Reusable allocations (avoid per-frame heap allocation)
    /// Reusable set for tracking live browser surface IDs during render.
    pub(crate) live_browser_sids: HashSet<SurfaceId>,
    /// Scratch map for per-pane render data — cleared and reused each frame (C5).
    pub(crate) pane_render_data_scratch: HashMap<PaneId, wmux_core::PaneRenderData>,
    /// Scratch set for live pane IDs — cleared and reused each frame (C5).
    pub(crate) live_pane_ids_scratch: HashSet<PaneId>,
}

impl UiState<'_> {
    /// Change the focused pane and start a focus glow fade-in animation.
    ///
    /// When switching away from a browser pane, reclaims Win32 keyboard focus
    /// from the WebView2 child HWND back to the main window so that winit
    /// receives subsequent keyboard events.
    pub(crate) fn set_focused_pane(&mut self, pane_id: PaneId) {
        if pane_id == self.focused_pane {
            return;
        }
        self.focused_pane = pane_id;

        // Reclaim Win32 keyboard focus from WebView2 if it currently holds it.
        if self.browser_focus_target.take().is_some() {
            // SAFETY: SetFocus is a standard Win32 call. `main_hwnd` is valid
            // for the lifetime of the window and we are on the UI/STA thread.
            unsafe {
                let _ = windows::Win32::UI::Input::KeyboardAndMouse::SetFocus(Some(self.main_hwnd));
            }
        }

        if let Some(old) = self.focus_glow_anim {
            self.animation.cancel(old);
        }
        self.focus_glow_anim = Some(self.animation.start(
            0.0,
            1.0,
            crate::animation::MOTION_NORMAL,
            crate::animation::Easing::CubicOut,
        ));
    }
}

/// Inline editing state for renaming a surface tab.
#[derive(Debug, Clone)]
pub(crate) enum TabEditState {
    None,
    Editing {
        pane_id: PaneId,
        tab_index: usize,
        surface_id: SurfaceId,
        text: String,
        cursor: usize,
        selected_all: bool,
    },
}

/// Chord (key sequence) state machine for multi-key shortcuts like Ctrl+D → Arrow.
#[derive(Debug, Clone, Default)]
pub(crate) enum ChordState {
    /// No chord in progress.
    #[default]
    Idle,
    /// Ctrl+D was pressed — waiting for the second key within the timeout.
    Pending(std::time::Instant),
}

/// Maximum time between Ctrl+D and the second key for a chord shortcut (ms).
const CHORD_TIMEOUT_MS: u128 = 1000;

impl ChordState {
    /// Check if a chord is pending and still within the timeout window.
    pub(crate) fn is_pending(&self) -> bool {
        matches!(self, Self::Pending(t) if t.elapsed().as_millis() < CHORD_TIMEOUT_MS)
    }
}

/// State for the split direction popup menu.
#[derive(Debug, Clone, Default)]
pub(crate) enum SplitMenuState {
    /// Menu is closed.
    #[default]
    Closed,
    /// Menu is open, anchored at a specific pane's split button.
    Open {
        pane_id: PaneId,
        /// Top-left corner of the menu in logical pixels.
        menu_x: f32,
        menu_y: f32,
    },
}

/// State for the workspace context menu (right-click on sidebar row).
#[derive(Debug, Clone, Default)]
pub(crate) enum WorkspaceMenuState {
    /// Menu is closed.
    #[default]
    Closed,
    /// Menu is open for a specific workspace row.
    Open {
        /// Index of the workspace in the cache.
        workspace_index: usize,
        /// Top-left corner of the menu popup in logical pixels.
        menu_x: f32,
        menu_y: f32,
    },
}

/// Number of items in the workspace context menu.
pub(crate) const WORKSPACE_MENU_ITEMS: usize = 2;

/// State for the tab context menu (right-click on a tab pill or toggle segment).
#[derive(Debug, Clone, Default)]
pub(crate) enum TabContextMenuState {
    /// Menu is closed.
    #[default]
    Closed,
    /// Menu is open for a specific tab.
    Open {
        pane_id: PaneId,
        tab_index: usize,
        surface_id: SurfaceId,
        /// Top-left corner of the menu popup in logical pixels.
        menu_x: f32,
        menu_y: f32,
    },
}

/// Number of items in the tab context menu.
pub(crate) const TAB_MENU_ITEMS: usize = 2;

/// Tab drag-and-drop state machine.
#[derive(Debug, Clone)]
pub(crate) enum TabDragState {
    None,
    Pressing {
        pane_id: PaneId,
        tab_index: usize,
        start_x: f32,
    },
    Dragging {
        pane_id: PaneId,
        from_index: usize,
        current_x: f32,
    },
}

/// Main application — owns the winit event loop and AppState handle.
pub struct App<'window> {
    pub(crate) state: Option<UiState<'window>>,
    pub(crate) app_state: AppStateHandle,
    pub(crate) app_event_rx: Option<mpsc::Receiver<AppEvent>>,
    pub(crate) rt_handle: tokio::runtime::Handle,
    pub(crate) proxy: EventLoopProxy<WmuxEvent>,
    /// Saved session to restore on first frame. Consumed once during `resumed()`.
    pub(crate) pending_session: Option<wmux_core::SessionState>,
    /// Browser command receiver — forwarded from IPC handler to UI thread.
    pub(crate) browser_cmd_rx: Option<mpsc::Receiver<wmux_core::BrowserCommand>>,
}

impl<'window> App<'window> {
    /// Create the event loop and run the application.
    pub fn run(
        rt_handle: tokio::runtime::Handle,
        app_state: AppStateHandle,
        app_event_rx: mpsc::Receiver<AppEvent>,
        session: Option<wmux_core::SessionState>,
        browser_cmd_rx: mpsc::Receiver<wmux_core::BrowserCommand>,
    ) -> Result<(), UiError> {
        let event_loop = EventLoop::<WmuxEvent>::with_user_event().build()?;
        let proxy = event_loop.create_proxy();
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
        let mut app = App {
            state: None,
            app_state,
            app_event_rx: Some(app_event_rx),
            rt_handle,
            proxy,
            pending_session: session,
            browser_cmd_rx: Some(browser_cmd_rx),
        };
        event_loop.run_app(&mut app)?;
        Ok(())
    }
}
