mod event_loop;
mod handlers;
mod render;

use crate::divider::DragState;
use crate::effects::EffectResult;
use crate::input::InputHandler;
use crate::mouse::MouseHandler;
use crate::search::SearchState;
use crate::shortcuts::ShortcutMap;
use crate::sidebar::SidebarState;
use crate::toast::ToastService;
use crate::UiError;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use winit::{
    event_loop::{EventLoop, EventLoopProxy},
    keyboard::ModifiersState,
    window::Window,
};
use wmux_config::UiChrome;
use wmux_core::{AppEvent, AppStateHandle, PaneId, TerminalMode};
use wmux_render::{GlyphonRenderer, GpuContext, QuadPipeline, TerminalMetrics, TerminalRenderer};

use crate::event::WmuxEvent;

/// UI-thread state created during window initialization.
///
/// Contains only rendering and input state. All terminal/pane state
/// lives in the AppState actor and is accessed via snapshots.
pub(crate) struct UiState<'window> {
    // Rendering
    pub(crate) window: Arc<Window>,
    pub(crate) gpu: GpuContext<'window>,
    pub(crate) quads: QuadPipeline,
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

    // Search overlay
    pub(crate) search: SearchState,
    /// Cached visible rows (scrollback + grid) for the focused pane, used by search.
    /// Updated every frame from the focused pane's render snapshot.
    pub(crate) last_search_rows: Vec<(usize, String)>,
    /// Total visible row count last frame (scrollback_visible + grid_rows).
    pub(crate) last_total_visible_rows: usize,

    // Tab bar text
    /// Cached glyphon text buffers for tab titles, keyed by layout pane ID.
    pub(crate) tab_title_buffers: HashMap<PaneId, Vec<glyphon::Buffer>>,
    /// Cached viewports from the last render — used for tab bar hit-testing.
    pub(crate) last_viewports: Vec<wmux_render::PaneViewport>,
    /// Active tab drag state for drag-and-drop reordering.
    pub(crate) tab_drag: TabDragState,

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
}

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
}

impl<'window> App<'window> {
    /// Create the event loop and run the application.
    pub fn run(
        rt_handle: tokio::runtime::Handle,
        app_state: AppStateHandle,
        app_event_rx: mpsc::Receiver<AppEvent>,
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
        };
        event_loop.run_app(&mut app)?;
        Ok(())
    }
}
