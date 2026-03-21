use crate::divider::{self, DividerOrientation, DragState};
use crate::event::WmuxEvent;
use crate::input::InputHandler;
use crate::mouse::{MouseAction, MouseButton, MouseHandler};
use crate::search::{self, SearchState};
use crate::shortcuts::{ShortcutAction, ShortcutMap};
use crate::sidebar::{SidebarInteraction, SidebarState};
use crate::toast::{self, ToastService};
use crate::UiError;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    keyboard::{Key, ModifiersState, NamedKey},
    window::{Window, WindowAttributes, WindowId},
};
use wmux_core::surface::SplitDirection;
use wmux_core::surface_manager::{Surface, SurfaceManager};
use wmux_core::{
    AppEvent, AppStateHandle, FocusDirection, PaneId, PaneRenderData, PaneState, Terminal,
    TerminalMode,
};
use wmux_pty::{PtyActorHandle, PtyEvent, PtyManager, SpawnConfig};
use wmux_render::{GlyphonRenderer, GpuContext, QuadPipeline, TerminalMetrics, TerminalRenderer};

/// UI-thread state created during window initialization.
///
/// Contains only rendering and input state. All terminal/pane state
/// lives in the AppState actor and is accessed via snapshots.
struct UiState<'window> {
    // Rendering
    window: Arc<Window>,
    gpu: GpuContext<'window>,
    quads: QuadPipeline,
    glyphon: GlyphonRenderer,
    /// Per-pane terminal renderers. Created/removed as panes are split/closed.
    renderers: HashMap<PaneId, TerminalRenderer>,
    metrics: TerminalMetrics,

    // Input
    input: InputHandler,
    mouse: MouseHandler,
    shortcuts: ShortcutMap,
    modifiers: ModifiersState,
    cursor_pos: (f64, f64),

    // Notifications
    toast_service: ToastService,

    // Sidebar
    sidebar: SidebarState,
    /// Cached workspace list — refreshed once per frame during render.
    workspace_cache: Vec<wmux_core::WorkspaceSnapshot>,

    // Divider drag
    /// Cached dividers from the last layout — used for hover/drag without blocking.
    dividers: Vec<divider::Divider>,
    /// Active divider drag state, if the user is currently dragging.
    drag_state: Option<DragState>,

    // Active pane tracking
    focused_pane: PaneId,
    cols: u16,
    rows: u16,
    process_exited: bool,
    /// Cached terminal modes from the last render snapshot.
    terminal_modes: TerminalMode,
    /// Cached pane layout from the last render — used for hit-testing on
    /// mouse clicks without blocking on the actor.
    last_layout: Vec<(PaneId, wmux_core::rect::Rect)>,

    // Search overlay
    search: SearchState,
    /// Cached visible rows (scrollback + grid) for the focused pane, used by search.
    /// Updated every frame from the focused pane's render snapshot.
    last_search_rows: Vec<(usize, String)>,
    /// Total visible row count last frame (scrollback_visible + grid_rows).
    last_total_visible_rows: usize,

    // Tab bar text
    /// Cached glyphon text buffers for tab titles, keyed by layout pane ID.
    tab_title_buffers: HashMap<PaneId, Vec<glyphon::Buffer>>,
    /// Cached viewports from the last render — used for tab bar hit-testing.
    last_viewports: Vec<wmux_render::PaneViewport>,
    /// Active tab drag state for drag-and-drop reordering.
    tab_drag: TabDragState,
}

/// Tab drag-and-drop state machine.
#[derive(Debug, Clone)]
enum TabDragState {
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

impl UiState<'_> {
    /// Render a frame: get layout from actor, draw borders, render ALL panes.
    fn render(
        &mut self,
        app_state: &AppStateHandle,
        rt: &tokio::runtime::Handle,
    ) -> Result<(), UiError> {
        let surface_width = self.gpu.width();
        let surface_height = self.gpu.height();

        // Reserve space on the left for the sidebar.
        let sidebar_width = self.sidebar.effective_width();
        let surface_viewport = wmux_core::rect::Rect {
            x: sidebar_width,
            y: 0.0,
            width: (surface_width as f32 - sidebar_width).max(1.0),
            height: surface_height as f32,
        };

        // Refresh workspace list once per frame.
        self.workspace_cache = rt.block_on(app_state.list_workspaces());

        // Update sidebar text buffers (only reshapes when data changes).
        self.sidebar
            .update_text(&self.workspace_cache, self.glyphon.font_system());

        // Render sidebar quads (backgrounds + highlights) before pane content.
        self.sidebar.render_quads(
            &self.workspace_cache,
            &mut self.quads,
            surface_height as f32,
        );
        // Render edit cursor overlay when inline renaming.
        self.sidebar.render_edit_cursor(&mut self.quads);

        // Get pane layout from the actor (blocks briefly — acceptable once per frame).
        let layout = rt.block_on(app_state.get_layout(surface_viewport));
        // Cache for non-blocking hit-testing on mouse clicks.
        self.last_layout.clone_from(&layout);
        // Recompute dividers from the current layout.
        self.dividers = divider::find_dividers(&layout);

        // Collect render data for all panes first (surface info needed for viewports).
        let mut render_data_map: HashMap<PaneId, PaneRenderData> =
            HashMap::with_capacity(layout.len());
        for (pane_id, _) in &layout {
            if let Some(data) = rt.block_on(app_state.get_render_data(*pane_id)) {
                render_data_map.insert(*pane_id, data);
            }
        }

        // Build PaneViewport descriptors with real surface data.
        let viewports: Vec<wmux_render::PaneViewport> = layout
            .iter()
            .map(|(id, rect)| {
                let (tab_count, tab_titles, active_tab) = render_data_map
                    .get(id)
                    .map(|d| (d.surface_count, d.surface_titles.clone(), d.active_surface))
                    .unwrap_or((1, vec![], 0));
                wmux_render::PaneViewport {
                    pane_id: *id,
                    rect: *rect,
                    focused: *id == self.focused_pane,
                    tab_count,
                    tab_titles,
                    active_tab,
                    zoomed: false,
                }
            })
            .collect();

        // Draw 1px borders for all panes (focused pane gets accent colour).
        let pane_renderer = wmux_render::PaneRenderer::new();
        pane_renderer.render_pane_borders(
            &mut self.quads,
            &viewports,
            [0.3, 0.3, 0.35, 1.0],
            [0.2, 0.6, 0.9, 1.0],
        );

        // Draw tab bars for panes with multiple surfaces.
        for vp in &viewports {
            if vp.tab_count > 1 {
                let _ = pane_renderer.render_tab_bar(
                    &mut self.quads,
                    vp,
                    [0.12, 0.12, 0.15, 1.0], // inactive tab bg
                    [0.22, 0.22, 0.28, 1.0], // active tab bg
                    [0.2, 0.6, 0.9, 1.0],    // accent color
                );
            }
        }

        // Collect live pane IDs once — used to prune stale tab title buffers and renderers.
        let live_ids: std::collections::HashSet<PaneId> =
            layout.iter().map(|(id, _)| *id).collect();

        // Update tab title text buffers for panes with multiple surfaces.
        {
            let tab_font_size = 12.0_f32;
            let tab_line_height = 16.0_f32;
            let metrics = glyphon::Metrics::new(tab_font_size, tab_line_height);
            let attrs = glyphon::Attrs::new().family(glyphon::Family::SansSerif);

            // Remove buffers for panes that no longer exist.
            self.tab_title_buffers.retain(|id, _| live_ids.contains(id));

            for vp in &viewports {
                if vp.tab_count <= 1 {
                    self.tab_title_buffers.remove(&vp.pane_id);
                    continue;
                }

                let bufs = self.tab_title_buffers.entry(vp.pane_id).or_default();

                // Resize buffer vec to match tab count.
                let tab_width = vp.rect.width / vp.tab_count as f32;
                let text_max_width = (tab_width - 16.0).max(1.0); // 8px padding each side

                bufs.resize_with(vp.tab_count, || {
                    glyphon::Buffer::new(self.glyphon.font_system(), metrics)
                });

                for (i, title) in vp.tab_titles.iter().enumerate() {
                    let buf = &mut bufs[i];
                    buf.set_metrics(self.glyphon.font_system(), metrics);
                    buf.set_size(
                        self.glyphon.font_system(),
                        Some(text_max_width),
                        Some(tab_line_height),
                    );
                    buf.set_text(
                        self.glyphon.font_system(),
                        title,
                        &attrs,
                        glyphon::Shaping::Advanced,
                        None,
                    );
                    buf.shape_until_scroll(self.glyphon.font_system(), false);
                }
            }
        }

        // Cache viewports for mouse hit-testing.
        self.last_viewports.clone_from(&viewports);

        // Tab drag visual feedback: drop indicator line.
        if let TabDragState::Dragging {
            pane_id,
            from_index,
            current_x,
        } = &self.tab_drag
        {
            if let Some(vp) = viewports.iter().find(|v| v.pane_id == *pane_id) {
                let tab_width = vp.rect.width / vp.tab_count as f32;
                let to_index = ((*current_x - vp.rect.x) / tab_width) as usize;
                let to_index = to_index.min(vp.tab_count - 1);

                // Semi-transparent overlay on the dragged tab.
                let from_x = vp.rect.x + *from_index as f32 * tab_width;
                self.quads.push_quad(
                    from_x,
                    vp.rect.y,
                    tab_width,
                    wmux_render::pane::TAB_BAR_HEIGHT,
                    [0.2, 0.6, 0.9, 0.25],
                );

                // Drop indicator: 2px vertical accent bar at the target position.
                if to_index != *from_index {
                    let indicator_x = vp.rect.x
                        + to_index as f32 * tab_width
                        + if to_index > *from_index {
                            tab_width - 1.0
                        } else {
                            0.0
                        };
                    self.quads.push_quad(
                        indicator_x,
                        vp.rect.y,
                        2.0,
                        wmux_render::pane::TAB_BAR_HEIGHT,
                        [0.2, 0.7, 1.0, 0.9],
                    );
                }
            }
        }

        // Remove renderers for panes that no longer exist in the layout.
        self.renderers.retain(|id, _| live_ids.contains(id));

        // Determine the focused pane rect (falls back to full surface for single-pane).
        let focused_rect = viewports
            .iter()
            .find(|vp| vp.focused)
            .map(|vp| vp.rect)
            .unwrap_or(surface_viewport);

        // Render terminal content for ALL panes.
        for (pane_id, _rect) in &layout {
            // When the pane has multiple tabs, exclude the tab bar from terminal area.
            let viewport = viewports.iter().find(|vp| vp.pane_id == *pane_id);
            let terminal_rect = viewport
                .filter(|vp| vp.tab_count > 1)
                .map(wmux_render::PaneRenderer::terminal_viewport)
                .unwrap_or(*_rect);

            // Compute per-pane terminal dimensions from the terminal content rect.
            let pane_cols = ((terminal_rect.width / self.metrics.cell_width)
                .floor()
                .max(1.0) as u32)
                .min(u16::MAX as u32) as u16;
            let pane_rows = ((terminal_rect.height / self.metrics.cell_height)
                .floor()
                .max(1.0) as u32)
                .min(u16::MAX as u32) as u16;

            // Ensure a renderer exists for this pane, creating or resizing as needed.
            if !self.renderers.contains_key(pane_id) {
                let renderer =
                    TerminalRenderer::new(self.glyphon.font_system(), pane_cols, pane_rows);
                self.renderers.insert(*pane_id, renderer);
            }
            let renderer = self.renderers.get_mut(pane_id).expect("just inserted");
            if renderer.cols() != pane_cols || renderer.rows() != pane_rows {
                renderer.resize(pane_cols, pane_rows, self.glyphon.font_system());
                // Notify the actor so the terminal grid and PTY are resized too.
                app_state.resize_pane(*pane_id, pane_cols, pane_rows);
            }

            // Use pre-collected render data for this pane.
            if let Some(data) = render_data_map.remove(pane_id) {
                if *pane_id == self.focused_pane {
                    self.process_exited = data.process_exited;
                    self.terminal_modes = data.modes;
                    // Cache text rows for search (only for focused pane).
                    if self.search.active {
                        self.last_search_rows =
                            search::extract_rows(&data.scrollback_visible_rows, &data.grid);
                        self.last_total_visible_rows =
                            data.scrollback_visible_rows.len() + data.grid.rows() as usize;
                    }
                }
                renderer.update_from_snapshot(
                    &data.grid,
                    &data.dirty_rows,
                    data.viewport_offset,
                    &data.scrollback_visible_rows,
                    self.glyphon.font_system(),
                    &mut self.quads,
                    (terminal_rect.x, terminal_rect.y),
                );
            }
        }

        // Selection highlight overlay on the focused pane only.
        if let Some(sel) = self.mouse.selection() {
            let (start, end) = sel.normalized();
            let pane_cols = (focused_rect.width / self.metrics.cell_width)
                .floor()
                .max(1.0) as usize;
            for row in start.row..=end.row {
                let col_start = if row == start.row { start.col } else { 0 };
                let col_end = if row == end.row {
                    end.col + 1
                } else {
                    pane_cols
                };
                let x = focused_rect.x + col_start as f32 * self.metrics.cell_width;
                let y = focused_rect.y + row as f32 * self.metrics.cell_height;
                let w = col_end.saturating_sub(col_start) as f32 * self.metrics.cell_width;
                let h = self.metrics.cell_height;
                self.quads.push_quad(x, y, w, h, [0.3, 0.5, 0.8, 0.3]);
            }
        }

        // Search match highlights (on focused pane only).
        if self.search.active {
            // Re-run search every render frame when active. The search is O(n*m)
            // but fast enough (<1ms for typical content of 4K lines × 200 cols).
            if !self.search.query.is_empty() && !self.last_search_rows.is_empty() {
                // Temporarily take rows to avoid borrow conflict with `self.search`.
                let rows_snapshot = std::mem::take(&mut self.last_search_rows);
                self.search.search(&rows_snapshot);
                self.last_search_rows = rows_snapshot;
            }

            search::render_search_highlights(
                &self.search,
                &mut self.quads,
                &focused_rect,
                self.metrics.cell_width,
                self.metrics.cell_height,
                self.last_total_visible_rows,
            );

            // Search bar overlay: a background quad at the bottom of the focused pane.
            {
                let bar_height = self.metrics.cell_height + 4.0;
                let bar_y = focused_rect.y + focused_rect.height - bar_height;
                let bar_w = focused_rect.width.max(200.0);

                // Semi-transparent dark background for the search bar.
                let bg_color = if self.search.has_regex_error() {
                    [0.5_f32, 0.1, 0.1, 0.9] // red tint on regex error
                } else {
                    [0.1_f32, 0.1, 0.15, 0.9]
                };
                self.quads
                    .push_quad(focused_rect.x, bar_y, bar_w, bar_height, bg_color);

                // Thin accent line at the top of the search bar.
                self.quads
                    .push_quad(focused_rect.x, bar_y, bar_w, 1.0, [0.2_f32, 0.6, 0.9, 1.0]);

                // Match count indicator on the right side of the bar.
                let count_x = focused_rect.x + bar_w - 120.0;
                if count_x > focused_rect.x {
                    self.quads.push_quad(
                        count_x,
                        bar_y,
                        120.0,
                        bar_height,
                        [0.15_f32, 0.15, 0.2, 0.9],
                    );
                }

                tracing::trace!(
                    query = %self.search.query,
                    matches = self.search.matches.len(),
                    current = self.search.current_match,
                    "search bar rendered"
                );
            }
        }

        // Highlight the hovered or active divider with an accent-coloured quad.
        let cursor_x = self.cursor_pos.0 as f32;
        let cursor_y = self.cursor_pos.1 as f32;
        let hovered_div = match &self.drag_state {
            // During drag, keep the divider highlighted for the dragged one.
            Some(ds) => self
                .dividers
                .iter()
                .find(|d| d.pane_id == ds.pane_id && d.orientation == ds.orientation),
            None => divider::hit_test(&self.dividers, cursor_x, cursor_y),
        };
        if let Some(div) = hovered_div {
            const DIV_HIGHLIGHT_THICKNESS: f32 = 2.0;
            match div.orientation {
                DividerOrientation::Vertical => {
                    let x = div.position - DIV_HIGHLIGHT_THICKNESS / 2.0;
                    let y = div.start;
                    let w = DIV_HIGHLIGHT_THICKNESS;
                    let h = div.end - div.start;
                    self.quads.push_quad(x, y, w, h, [0.2, 0.6, 0.9, 0.7]);
                }
                DividerOrientation::Horizontal => {
                    let x = div.start;
                    let y = div.position - DIV_HIGHLIGHT_THICKNESS / 2.0;
                    let w = div.end - div.start;
                    let h = DIV_HIGHLIGHT_THICKNESS;
                    self.quads.push_quad(x, y, w, h, [0.2, 0.6, 0.9, 0.7]);
                }
            }
        }

        // Upload quad GPU data.
        self.quads.prepare(&self.gpu.queue);

        // Collect text areas from ALL pane renderers + sidebar + tabs, then prepare glyphon once.
        let surface_w = self.gpu.width();
        let surface_h = self.gpu.height();
        let mut all_text_areas: Vec<_> = layout
            .iter()
            .filter_map(|(pane_id, _rect)| {
                let vp = viewports.iter().find(|v| v.pane_id == *pane_id);
                let terminal_rect = vp
                    .filter(|v| v.tab_count > 1)
                    .map(wmux_render::PaneRenderer::terminal_viewport)
                    .unwrap_or(*_rect);
                self.renderers.get(pane_id).map(|r| {
                    r.text_areas(
                        (terminal_rect.x, terminal_rect.y),
                        terminal_rect,
                        surface_w,
                        surface_h,
                    )
                })
            })
            .flatten()
            .collect();

        // Append tab title text areas.
        for vp in &viewports {
            if vp.tab_count > 1 {
                if let Some(bufs) = self.tab_title_buffers.get(&vp.pane_id) {
                    let tab_width = vp.rect.width / vp.tab_count as f32;
                    for (i, buf) in bufs.iter().enumerate() {
                        let tab_x = vp.rect.x + i as f32 * tab_width;
                        let text_color = if i == vp.active_tab {
                            glyphon::Color::rgb(240, 240, 245)
                        } else {
                            glyphon::Color::rgb(150, 150, 165)
                        };
                        all_text_areas.push(glyphon::TextArea {
                            buffer: buf,
                            left: tab_x + 8.0,
                            top: vp.rect.y + 6.0,
                            scale: 1.0,
                            bounds: glyphon::TextBounds {
                                left: (tab_x + 8.0) as i32,
                                top: vp.rect.y as i32,
                                right: (tab_x + tab_width - 8.0) as i32,
                                bottom: (vp.rect.y + wmux_render::pane::TAB_BAR_HEIGHT) as i32,
                            },
                            default_color: text_color,
                            custom_glyphs: &[],
                        });
                    }
                }
            }
        }

        // Append sidebar text areas (workspace names + subtitles).
        all_text_areas.extend(self.sidebar.text_areas(surface_w, surface_h));

        self.glyphon
            .prepare_text_areas(&self.gpu.device, &self.gpu.queue, all_text_areas)?;

        // Render pass.
        let output = self.gpu.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("wmux_encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("wmux_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.12,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            // Backgrounds + borders + cursor + selection first, then text on top.
            self.quads.render(&mut render_pass);
            self.glyphon.render(&mut render_pass)?;
        }

        self.gpu.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        self.quads.clear();

        Ok(())
    }

    /// Convert the current cursor pixel position to pane-local cell coordinates.
    ///
    /// Uses the cached layout to find the focused pane's origin and dimensions,
    /// so that mouse events produce coordinates relative to the pane, not the
    /// full window surface.
    fn cursor_cell(&self) -> (usize, usize) {
        let (origin_x, origin_y, pane_cols, pane_rows) = self
            .last_layout
            .iter()
            .find(|(id, _)| *id == self.focused_pane)
            .map(|(_, rect)| {
                let cols = (rect.width / self.metrics.cell_width).floor().max(1.0) as usize;
                let rows = (rect.height / self.metrics.cell_height).floor().max(1.0) as usize;
                (rect.x as f64, rect.y as f64, cols, rows)
            })
            .unwrap_or((0.0, 0.0, self.cols as usize, self.rows as usize));

        let max_col = pane_cols.saturating_sub(1);
        let max_row = pane_rows.saturating_sub(1);
        let local_x = (self.cursor_pos.0 - origin_x).max(0.0);
        let local_y = (self.cursor_pos.1 - origin_y).max(0.0);
        let col = (local_x as f32 / self.metrics.cell_width) as usize;
        let row = (local_y as f32 / self.metrics.cell_height) as usize;
        (col.min(max_col), row.min(max_row))
    }

    /// Process a mouse action returned by the mouse handler.
    fn handle_mouse_action(&mut self, action: MouseAction, app_state: &AppStateHandle) {
        match action {
            MouseAction::None => {}
            MouseAction::SelectionStarted
            | MouseAction::SelectionUpdated
            | MouseAction::SelectionFinished => {
                self.window.request_redraw();
            }
            MouseAction::Report(bytes) => {
                app_state.send_input(self.focused_pane, bytes);
            }
            MouseAction::Scroll(_) => {
                // Scroll is handled directly in mouse wheel event handler.
                self.window.request_redraw();
            }
        }
    }
}

/// Spawn a new terminal + PTY for `pane_id`, register it with the AppState actor,
/// and start the PTY bridge task.
///
/// This is a fire-and-forget helper: it spawns blocking work inside
/// `rt_handle` and returns immediately. Any spawn failure is logged.
fn spawn_pane_pty(
    pane_id: PaneId,
    cols: u16,
    rows: u16,
    app_state: &AppStateHandle,
    rt_handle: &tokio::runtime::Handle,
) {
    // Create terminal with event channel (owned by actor via PaneState).
    let (terminal, terminal_event_rx) = Terminal::with_event_channel(cols, rows);

    // Bounded bridge channels between PTY actor and AppState actor.
    let (write_tx, mut write_rx) = mpsc::channel::<Vec<u8>>(256);
    let (resize_tx, mut resize_rx) = mpsc::channel::<(u16, u16)>(4);

    // Register pane with the actor before spawning the PTY so that any early
    // output events can be delivered to an already-registered pane.
    let pane_state = PaneState {
        terminal,
        terminal_event_rx,
        pty_write_tx: write_tx,
        pty_resize_tx: resize_tx,
        process_exited: false,
        surfaces: SurfaceManager::new(Surface::new("shell", pane_id)),
    };
    app_state.register_pane(pane_id, pane_state);

    // Spawn PTY process.
    let manager = PtyManager::new();
    let config = SpawnConfig {
        cols,
        rows,
        ..Default::default()
    };
    let handle = match manager.spawn(config) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!(error = %e, pane_id = %pane_id, "failed to spawn PTY for new pane");
            return;
        }
    };

    // PTY bridge task: PTY output → AppState actor, AppState input → PTY.
    let app_state_clone = app_state.clone();
    rt_handle.spawn(async move {
        let mut actor = PtyActorHandle::spawn(handle);
        loop {
            tokio::select! {
                event = actor.next_event() => {
                    match event {
                        Some(PtyEvent::Output(data)) => {
                            app_state_clone.process_pty_output(pane_id, data);
                        }
                        Some(PtyEvent::Exited { success }) => {
                            app_state_clone.mark_exited(pane_id, success);
                            break;
                        }
                        None => break,
                    }
                }
                data = write_rx.recv() => {
                    match data {
                        Some(bytes) => {
                            if actor.write(bytes).await.is_err() {
                                break;
                            }
                        }
                        None => break,
                    }
                }
                size = resize_rx.recv() => {
                    match size {
                        Some((new_rows, new_cols)) => {
                            let _ = actor.resize(new_rows, new_cols).await;
                        }
                        None => break,
                    }
                }
            }
        }
        tracing::info!(pane_id = %pane_id, "PTY bridge task ended");
    });
}

/// Main application — owns the winit event loop and AppState handle.
pub struct App<'window> {
    state: Option<UiState<'window>>,
    app_state: AppStateHandle,
    app_event_rx: Option<mpsc::Receiver<AppEvent>>,
    rt_handle: tokio::runtime::Handle,
    proxy: EventLoopProxy<WmuxEvent>,
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

/// Handle a key event when the search overlay is active.
///
/// Intercepts printable characters, Backspace, Enter, and Escape. All other
/// named keys (arrows, F-keys, etc.) are silently consumed — not forwarded to
/// the PTY — while the search overlay is open.
/// Handle keyboard input during sidebar inline editing.
///
/// Enter commits the rename, Escape cancels, Backspace/Delete edit text,
/// and printable characters are inserted at cursor.
fn handle_sidebar_edit_key(state: &mut UiState<'_>, event: &KeyEvent, app_state: &AppStateHandle) {
    // Extract editing state; if not editing, do nothing.
    let (index, text, cursor) = match &mut state.sidebar.interaction {
        SidebarInteraction::Editing {
            index,
            text,
            cursor,
        } => (*index, text, cursor),
        _ => return,
    };

    match &event.logical_key {
        Key::Named(NamedKey::Escape) => {
            // Cancel editing — discard changes.
            state.sidebar.interaction = SidebarInteraction::Idle;
            tracing::debug!(index, "sidebar: editing cancelled");
        }
        Key::Named(NamedKey::Enter) => {
            // Commit the rename.
            let new_name = text.clone();
            if let Some(ws) = state.workspace_cache.get(index) {
                if !new_name.is_empty() && new_name != ws.name {
                    app_state.rename_workspace(ws.id, new_name);
                    tracing::debug!(index, "sidebar: workspace renamed");
                }
            }
            state.sidebar.interaction = SidebarInteraction::Idle;
        }
        Key::Named(NamedKey::Backspace) => {
            if *cursor > 0 {
                // Remove the character before the cursor (byte-aware).
                let byte_pos = text
                    .char_indices()
                    .nth(*cursor - 1)
                    .map(|(pos, _)| pos)
                    .unwrap_or(0);
                let next_byte = text
                    .char_indices()
                    .nth(*cursor)
                    .map(|(pos, _)| pos)
                    .unwrap_or(text.len());
                text.replace_range(byte_pos..next_byte, "");
                *cursor -= 1;
            }
        }
        Key::Named(NamedKey::Delete) => {
            let char_count = text.chars().count();
            if *cursor < char_count {
                let byte_pos = text
                    .char_indices()
                    .nth(*cursor)
                    .map(|(pos, _)| pos)
                    .unwrap_or(text.len());
                let next_byte = text
                    .char_indices()
                    .nth(*cursor + 1)
                    .map(|(pos, _)| pos)
                    .unwrap_or(text.len());
                text.replace_range(byte_pos..next_byte, "");
            }
        }
        Key::Named(NamedKey::ArrowLeft) => {
            if *cursor > 0 {
                *cursor -= 1;
            }
        }
        Key::Named(NamedKey::ArrowRight) => {
            let char_count = text.chars().count();
            if *cursor < char_count {
                *cursor += 1;
            }
        }
        Key::Named(NamedKey::Home) => {
            *cursor = 0;
        }
        Key::Named(NamedKey::End) => {
            *cursor = text.chars().count();
        }
        Key::Character(ch) => {
            let s = ch.as_str();
            // Filter out control characters.
            if s.chars().all(|c| !c.is_control()) {
                // Insert at cursor position (byte-aware).
                let byte_pos = text
                    .char_indices()
                    .nth(*cursor)
                    .map(|(pos, _)| pos)
                    .unwrap_or(text.len());
                text.insert_str(byte_pos, s);
                *cursor += s.chars().count();
            }
        }
        _ => {
            // Other named keys silently consumed.
        }
    }
}

fn handle_search_key(state: &mut UiState<'_>, event: &KeyEvent) {
    match &event.logical_key {
        Key::Named(NamedKey::Escape) => {
            state.search.close();
            tracing::debug!("search closed via Escape");
        }
        Key::Named(NamedKey::Backspace) => {
            state.search.query.pop();
            if state.search.query.is_empty() {
                state.search.matches.clear();
                state.search.current_match = 0;
            }
        }
        Key::Named(NamedKey::Enter) => {
            state.search.next_match();
        }
        Key::Character(ch) => {
            let s = ch.as_str();
            // Filter out control characters (ASCII < 0x20) to avoid injecting
            // non-printable bytes into the search query.
            if s.chars().all(|c| !c.is_control()) {
                state.search.query.push_str(s);
            }
        }
        _ => {
            // Named keys (arrows, Tab, F-keys) are silently consumed.
        }
    }
}

/// Dispatch a matched global shortcut action.
///
/// Takes the app_state handle and rt_handle by reference to avoid borrow
/// conflicts with the mutable UiState borrow in the event handler.
fn handle_shortcut(
    action: ShortcutAction,
    state: &mut UiState<'_>,
    app_state: &AppStateHandle,
    rt_handle: &tokio::runtime::Handle,
    proxy: &EventLoopProxy<WmuxEvent>,
) {
    match action {
        ShortcutAction::SplitRight => {
            let pane_id = state.focused_pane;
            let cols = state.cols;
            let rows = state.rows;
            let app_state_clone = app_state.clone();
            let rt_clone = rt_handle.clone();
            let proxy_clone = proxy.clone();
            rt_handle.spawn(async move {
                match app_state_clone
                    .split_pane(pane_id, SplitDirection::Horizontal)
                    .await
                {
                    Ok(new_id) => {
                        spawn_pane_pty(new_id, cols, rows, &app_state_clone, &rt_clone);
                        app_state_clone.focus_pane(new_id);
                        let _ = proxy_clone.send_event(WmuxEvent::FocusPane(new_id));
                        tracing::info!(
                            pane_id = %pane_id,
                            new_pane = %new_id,
                            "pane split right"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "SplitRight failed");
                    }
                }
            });
            state.window.request_redraw();
        }

        ShortcutAction::SplitDown => {
            let pane_id = state.focused_pane;
            let cols = state.cols;
            let rows = state.rows;
            let app_state_clone = app_state.clone();
            let rt_clone = rt_handle.clone();
            let proxy_clone = proxy.clone();
            rt_handle.spawn(async move {
                match app_state_clone
                    .split_pane(pane_id, SplitDirection::Vertical)
                    .await
                {
                    Ok(new_id) => {
                        spawn_pane_pty(new_id, cols, rows, &app_state_clone, &rt_clone);
                        app_state_clone.focus_pane(new_id);
                        let _ = proxy_clone.send_event(WmuxEvent::FocusPane(new_id));
                        tracing::info!(
                            pane_id = %pane_id,
                            new_pane = %new_id,
                            "pane split down"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "SplitDown failed");
                    }
                }
            });
            state.window.request_redraw();
        }

        ShortcutAction::ClosePane => {
            let closing = state.focused_pane;
            app_state.close_pane(closing);

            // After closing, get the updated layout to find another pane to focus.
            let viewport = wmux_core::rect::Rect::new(
                0.0,
                0.0,
                state.gpu.width() as f32,
                state.gpu.height() as f32,
            );
            let layout = rt_handle.block_on(app_state.get_layout(viewport));
            if let Some((next_id, _)) = layout.first() {
                state.focused_pane = *next_id;
                app_state.focus_pane(*next_id);
            } else {
                // Last pane closed — exit the application.
                tracing::info!("last pane closed, shutting down");
                app_state.shutdown();
                state.window.request_redraw();
            }
            state.window.request_redraw();
        }

        ShortcutAction::ZoomToggle => {
            app_state.toggle_zoom(state.focused_pane);
            state.window.request_redraw();
        }

        ShortcutAction::FocusUp => {
            app_state.navigate_focus(FocusDirection::Up);
            state.window.request_redraw();
        }

        ShortcutAction::FocusDown => {
            app_state.navigate_focus(FocusDirection::Down);
            state.window.request_redraw();
        }

        ShortcutAction::FocusLeft => {
            app_state.navigate_focus(FocusDirection::Left);
            state.window.request_redraw();
        }

        ShortcutAction::FocusRight => {
            app_state.navigate_focus(FocusDirection::Right);
            state.window.request_redraw();
        }

        ShortcutAction::NewWorkspace => {
            let cols = state.cols;
            let rows = state.rows;
            let app_state_clone = app_state.clone();
            let rt_clone = rt_handle.clone();
            let proxy_clone = proxy.clone();
            rt_handle.spawn(async move {
                // 1. Create workspace (actor auto-switches to it)
                let _ws_id = app_state_clone
                    // TODO(L2_16): route through i18n system when wmux-config i18n is implemented.
                    .create_workspace("New Workspace".to_owned())
                    .await;

                // 2. Spawn a pane with PTY in the new (now active) workspace
                let pane_id = PaneId::new();
                spawn_pane_pty(pane_id, cols, rows, &app_state_clone, &rt_clone);

                // 3. Focus the new pane
                app_state_clone.focus_pane(pane_id);
                let _ = proxy_clone.send_event(WmuxEvent::FocusPane(pane_id));

                tracing::info!(pane_id = %pane_id, "new workspace with pane created");
            });
        }

        ShortcutAction::SwitchWorkspace(n) => {
            // n is 1-based; switch_workspace takes 0-based index.
            let index = (n as usize).saturating_sub(1);
            app_state.switch_workspace(index);
            state.window.request_redraw();
        }

        ShortcutAction::NewSurface => {
            let layout_pane_id = state.focused_pane;
            let new_pane_id = PaneId::new();
            let cols = state.cols;
            let rows = state.rows;
            let app_clone = app_state.clone();
            let rt_clone = rt_handle.clone();
            rt_handle.spawn(async move {
                // 1. Spawn PTY (registers backing PaneState in actor).
                spawn_pane_pty(new_pane_id, cols, rows, &app_clone, &rt_clone);
                // 2. Register as surface in the layout pane.
                match app_clone.create_surface(layout_pane_id, new_pane_id).await {
                    Ok(sid) => tracing::info!(
                        surface_id = %sid,
                        backing = %new_pane_id,
                        "new surface created",
                    ),
                    Err(e) => tracing::warn!(error = %e, "create surface failed"),
                }
            });
            state.window.request_redraw();
        }
        ShortcutAction::CycleSurfaceForward => {
            app_state.cycle_surface(state.focused_pane, true);
            state.window.request_redraw();
        }
        ShortcutAction::CycleSurfaceBackward => {
            app_state.cycle_surface(state.focused_pane, false);
            state.window.request_redraw();
        }

        ShortcutAction::Copy => {
            if let Some(sel) = state.mouse.selection() {
                let sel_clone = sel.clone();
                let text =
                    rt_handle.block_on(app_state.extract_selection(state.focused_pane, sel_clone));
                if let Some(text) = text {
                    state.mouse.copy_text_to_clipboard(&text);
                }
            }
        }

        ShortcutAction::Paste => {
            if let Some(text) = state.mouse.paste_from_clipboard() {
                let bytes = state
                    .input
                    .wrap_bracketed_paste(&text, state.terminal_modes);
                app_state.send_input(state.focused_pane, bytes);
                state.window.request_redraw();
            }
        }

        ShortcutAction::ToggleSidebar => {
            state.sidebar.toggle();
            state.window.request_redraw();
        }

        // Placeholders for future tasks.
        ShortcutAction::CommandPalette => {
            tracing::debug!("CommandPalette shortcut (placeholder — Task L4_01)");
        }
        ShortcutAction::Find => {
            if state.search.active {
                state.search.close();
                tracing::debug!("search closed via Ctrl+F toggle");
            } else {
                state.search.open();
                tracing::debug!("search opened via Ctrl+F");
            }
            state.window.request_redraw();
        }
        ShortcutAction::ToggleDevTools => {
            tracing::debug!("ToggleDevTools shortcut (placeholder)");
        }
    }
}

impl<'window> ApplicationHandler<WmuxEvent> for App<'window> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let attrs = WindowAttributes::default()
            .with_title("wmux")
            .with_inner_size(winit::dpi::LogicalSize::new(1200, 800));

        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("failed to create window"),
        );

        let gpu =
            pollster::block_on(GpuContext::new(window.clone())).expect("failed to initialize GPU");

        let quads = QuadPipeline::new(
            &gpu.device,
            &gpu.queue,
            gpu.format,
            gpu.width(),
            gpu.height(),
        );

        let mut glyphon = GlyphonRenderer::new(&gpu.device, &gpu.queue, gpu.format);
        glyphon.resize(&gpu.queue, gpu.width(), gpu.height());

        // Compute terminal dimensions from window size and font metrics
        let metrics = TerminalMetrics::new(glyphon.font_system());
        let cols = ((gpu.width() as f32) / metrics.cell_width).floor().max(1.0) as u32;
        let rows = ((gpu.height() as f32) / metrics.cell_height)
            .floor()
            .max(1.0) as u32;
        let cols = cols.min(u16::MAX as u32) as u16;
        let rows = rows.min(u16::MAX as u32) as u16;

        // Per-pane renderers are created lazily in the render loop.
        let renderers: HashMap<PaneId, TerminalRenderer> = HashMap::new();

        // Initialize Windows Toast notification support.
        toast::init_aumid();
        let toast_service = ToastService::new();

        // Generate pane ID and delegate PTY spawn + registration to shared helper.
        let pane_id = PaneId::new();
        spawn_pane_pty(pane_id, cols, rows, &self.app_state, &self.rt_handle);

        // Event forwarding task: reads AppEvent → sends WmuxEvent via EventLoopProxy.
        if let Some(mut event_rx) = self.app_event_rx.take() {
            let proxy_fwd = self.proxy.clone();
            self.rt_handle.spawn(async move {
                while let Some(event) = event_rx.recv().await {
                    match event {
                        AppEvent::PaneNeedsRedraw(_) => {
                            let _ = proxy_fwd.send_event(WmuxEvent::PtyOutput);
                        }
                        AppEvent::NotificationAdded {
                            notification,
                            suppressed,
                        } => {
                            if !suppressed {
                                let _ = proxy_fwd.send_event(WmuxEvent::ShowToast(notification));
                            }
                        }
                        AppEvent::PaneExited { success, .. } => {
                            let _ = proxy_fwd.send_event(WmuxEvent::PtyExited { success });
                        }
                        AppEvent::FocusChanged { pane_id } => {
                            let _ = proxy_fwd.send_event(WmuxEvent::FocusPane(pane_id));
                        }
                        // Workspace events are handled by the sidebar (Task L2_08).
                        AppEvent::WorkspaceCreated { .. }
                        | AppEvent::WorkspaceSwitched { .. }
                        | AppEvent::WorkspaceClosed { .. } => {}
                    }
                }
                tracing::info!("event forwarding task ended");
            });
        }

        tracing::info!(
            cols,
            rows,
            width = gpu.width(),
            height = gpu.height(),
            format = ?gpu.format,
            pane_id = %pane_id,
            "terminal initialized (actor pattern)",
        );

        self.state = Some(UiState {
            window,
            gpu,
            quads,
            glyphon,
            renderers,
            metrics,
            input: InputHandler::new(),
            mouse: MouseHandler::new(),
            shortcuts: ShortcutMap::new(),
            modifiers: ModifiersState::default(),
            cursor_pos: (0.0, 0.0),
            toast_service,
            sidebar: SidebarState::new(220),
            workspace_cache: Vec::new(),
            dividers: Vec::new(),
            drag_state: None,
            focused_pane: pane_id,
            cols,
            rows,
            process_exited: false,
            terminal_modes: TerminalMode::empty(),
            last_layout: Vec::new(),
            search: SearchState::new(),
            last_search_rows: Vec::new(),
            last_total_visible_rows: 0,
            tab_title_buffers: HashMap::new(),
            last_viewports: Vec::new(),
            tab_drag: TabDragState::None,
        });
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: WmuxEvent) {
        match event {
            WmuxEvent::PtyOutput => {
                if let Some(state) = self.state.as_ref() {
                    state.window.request_redraw();
                }
            }
            WmuxEvent::PtyExited { success } => {
                if let Some(state) = self.state.as_mut() {
                    state.process_exited = true;
                    state.window.request_redraw();
                    tracing::info!(success, "shell process exited");
                }
            }
            WmuxEvent::ShowToast(notification) => {
                if let Some(state) = self.state.as_ref() {
                    state.toast_service.show(&notification);
                }
            }
            WmuxEvent::FocusPane(pane_id) => {
                if let Some(state) = self.state.as_mut() {
                    state.focused_pane = pane_id;
                    state.window.request_redraw();
                }
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = self.state.as_mut() else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => {
                tracing::info!("window close requested");
                self.app_state.shutdown();
                event_loop.exit();
            }

            WindowEvent::Resized(physical_size) => {
                let w = physical_size.width;
                let h = physical_size.height;
                if w > 0 && h > 0 {
                    // GPU resize
                    state.gpu.resize(w, h);
                    state.quads.resize(&state.gpu.queue, w, h);
                    state.glyphon.resize(&state.gpu.queue, w, h);

                    // Terminal resize
                    let new_cols = ((w as f32) / state.metrics.cell_width).floor().max(1.0) as u32;
                    let new_rows = ((h as f32) / state.metrics.cell_height).floor().max(1.0) as u32;
                    let new_cols = new_cols.min(u16::MAX as u32) as u16;
                    let new_rows = new_rows.min(u16::MAX as u32) as u16;

                    if new_cols != state.cols || new_rows != state.rows {
                        state.cols = new_cols;
                        state.rows = new_rows;
                        // Per-pane renderer + PTY resizing is handled in render()
                        // based on each pane's actual rect dimensions.
                    }

                    state.window.request_redraw();
                }
            }

            WindowEvent::RedrawRequested => match state.render(&self.app_state, &self.rt_handle) {
                Ok(()) => {}
                Err(UiError::Surface(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated)) => {
                    let w = state.gpu.width();
                    let h = state.gpu.height();
                    state.gpu.resize(w, h);
                    state.window.request_redraw();
                }
                Err(e) => {
                    tracing::error!(error = %e, "render failed");
                }
            },

            WindowEvent::ModifiersChanged(modifiers) => {
                state.modifiers = modifiers.state();
            }

            WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } => {
                if event.state != ElementState::Pressed {
                    return;
                }

                // Priority 0: sidebar inline editing — intercept all keys when renaming.
                if state.sidebar.is_editing() {
                    handle_sidebar_edit_key(state, &event, &self.app_state);
                    state.window.request_redraw();
                    return;
                }

                // Priority 1: global shortcuts — intercepted before terminal input.
                // Shortcuts must work even when the focused pane's process has exited
                // (so the user can close panes, switch workspaces, etc.).
                // Match the shortcut regardless of repeat state, but only execute
                // the action on the first press. Repeated keys are consumed (return)
                // to prevent shortcut key combos from leaking to the PTY as raw
                // control bytes (e.g. Ctrl+D → 0x04 EOF sent to the shell).
                if let Some(action) = state.shortcuts.match_shortcut(
                    &event.logical_key,
                    event.physical_key,
                    &state.modifiers,
                ) {
                    if !event.repeat {
                        handle_shortcut(
                            action,
                            state,
                            &self.app_state,
                            &self.rt_handle,
                            &self.proxy,
                        );
                    }
                    return;
                }

                // Don't send input to a dead process.
                if state.process_exited {
                    return;
                }

                // Priority 1.5: search overlay input — intercepted when search is active.
                if state.search.active {
                    handle_search_key(state, &event);
                    state.window.request_redraw();
                    return;
                }

                // Priority 2: regular key input → actor → PTY
                if let Some(bytes) =
                    state
                        .input
                        .handle_key_event(&event, &state.modifiers, state.terminal_modes)
                {
                    self.app_state.reset_viewport(state.focused_pane);
                    state.mouse.clear_selection();
                    self.app_state.send_input(state.focused_pane, bytes);
                    state.window.request_redraw();
                }
            }

            WindowEvent::MouseInput {
                state: elem_state,
                button,
                ..
            } => {
                let px = state.cursor_pos.0 as f32;
                let py = state.cursor_pos.1 as f32;

                // Sidebar mouse interaction — click to select, drag to reorder, double-click to rename.
                if state.sidebar.visible && px < state.sidebar.effective_width() {
                    if elem_state == ElementState::Pressed
                        && button == winit::event::MouseButton::Left
                    {
                        // Track click count for double-click detection.
                        let mouse_mode =
                            state.terminal_modes.contains(TerminalMode::MOUSE_REPORTING);
                        let shift = state.modifiers.shift_key();
                        let (col, row) = state.cursor_cell();
                        let _ = state.mouse.handle_mouse_press(
                            col,
                            row,
                            MouseButton::Left,
                            shift,
                            mouse_mode,
                        );
                        // We only needed click counting — discard the terminal
                        // selection that handle_mouse_press created.
                        state.mouse.clear_selection();

                        if let Some(row_index) =
                            state.sidebar.hit_test_row(py, state.workspace_cache.len())
                        {
                            if state.mouse.click_count() >= 2 {
                                // Double-click: start inline editing.
                                let name = state
                                    .workspace_cache
                                    .get(row_index)
                                    .map(|ws| ws.name.clone())
                                    .unwrap_or_default();
                                let cursor = name.chars().count();
                                state.sidebar.interaction = SidebarInteraction::Editing {
                                    index: row_index,
                                    text: name,
                                    cursor,
                                };
                                tracing::debug!(row_index, "sidebar: started inline editing");
                            } else {
                                // Single press: start tracking for click vs drag.
                                state.sidebar.interaction = SidebarInteraction::Pressing {
                                    row: row_index,
                                    start_y: py,
                                };
                            }
                        }
                    } else if elem_state == ElementState::Released
                        && button == winit::event::MouseButton::Left
                    {
                        match state.sidebar.interaction.clone() {
                            SidebarInteraction::Pressing { row, .. } => {
                                // Click completed without drag → switch workspace.
                                self.app_state.switch_workspace(row);
                                tracing::debug!(row, "sidebar: workspace selected via click");
                                state.sidebar.interaction = SidebarInteraction::Idle;
                            }
                            SidebarInteraction::Dragging {
                                from_row,
                                current_y,
                            } => {
                                // Drag completed → reorder workspace.
                                let target = state
                                    .sidebar
                                    .drag_target_index(current_y, state.workspace_cache.len());
                                if target != from_row {
                                    self.app_state.reorder_workspace(from_row, target);
                                    tracing::debug!(
                                        from_row,
                                        target,
                                        "sidebar: workspace reordered via drag"
                                    );
                                }
                                state.sidebar.interaction = SidebarInteraction::Idle;
                            }
                            _ => {
                                // Release in other states (Editing, Idle, Hover) — no-op.
                            }
                        }
                    }

                    // Click in sidebar area — cancel editing if clicking outside edit row.
                    if elem_state == ElementState::Pressed {
                        if let SidebarInteraction::Editing {
                            index, ref text, ..
                        } = state.sidebar.interaction
                        {
                            if let Some(row_index) =
                                state.sidebar.hit_test_row(py, state.workspace_cache.len())
                            {
                                if row_index != index {
                                    // Commit the edit on click-away.
                                    if let Some(ws) = state.workspace_cache.get(index) {
                                        if !text.is_empty() && *text != ws.name {
                                            self.app_state.rename_workspace(ws.id, text.clone());
                                        }
                                    }
                                    state.sidebar.interaction = SidebarInteraction::Idle;
                                }
                            }
                        }
                    }

                    state.window.request_redraw();
                    return;
                }

                // Click outside sidebar — cancel any sidebar editing.
                if elem_state == ElementState::Pressed {
                    if let SidebarInteraction::Editing {
                        index, ref text, ..
                    } = state.sidebar.interaction
                    {
                        if let Some(ws) = state.workspace_cache.get(index) {
                            if !text.is_empty() && *text != ws.name {
                                self.app_state.rename_workspace(ws.id, text.clone());
                            }
                        }
                        state.sidebar.interaction = SidebarInteraction::Idle;
                        state.window.request_redraw();
                    }
                }

                // Left-button press on a divider: start drag or reset on double-click.
                if elem_state == ElementState::Pressed && button == winit::event::MouseButton::Left
                {
                    if let Some(div) = divider::hit_test(&state.dividers, px, py) {
                        // Detect double-click via the mouse handler's click count.
                        // We call handle_mouse_press so it tracks timing; then inspect count.
                        let mouse_mode =
                            state.terminal_modes.contains(TerminalMode::MOUSE_REPORTING);
                        let shift = state.modifiers.shift_key();
                        let (col, row) = state.cursor_cell();
                        let _ = state.mouse.handle_mouse_press(
                            col,
                            row,
                            MouseButton::Left,
                            shift,
                            mouse_mode,
                        );

                        if state.mouse.click_count() >= 2 {
                            // Double-click: reset split ratio to equal halves.
                            tracing::debug!(
                                pane_id = %div.pane_id,
                                "divider double-click — resetting ratio to 0.5"
                            );
                            self.app_state.resize_split(div.pane_id, 0.5);
                            state.window.request_redraw();
                        } else {
                            // Single press: start drag.
                            // Compute actual container dimension from adjacent pane rects
                            // for correct ratio calculation in multi-level splits.
                            let (split_start, split_dimension) = {
                                let pos = div.position;
                                let layout = &state.last_layout;
                                match div.orientation {
                                    DividerOrientation::Vertical => {
                                        // Find panes immediately left and right of divider
                                        let left = layout.iter().find(|(_, r)| {
                                            (r.x + r.width - pos).abs() < 4.0
                                                && r.y < div.end
                                                && (r.y + r.height) > div.start
                                        });
                                        let right = layout.iter().find(|(_, r)| {
                                            (r.x - pos).abs() < 4.0
                                                && r.y < div.end
                                                && (r.y + r.height) > div.start
                                        });
                                        match (left, right) {
                                            (Some((_, l)), Some((_, r))) => {
                                                (l.x, l.width + r.width)
                                            }
                                            _ => {
                                                let sw = state.sidebar.effective_width();
                                                (sw, state.gpu.width() as f32 - sw)
                                            }
                                        }
                                    }
                                    DividerOrientation::Horizontal => {
                                        let above = layout.iter().find(|(_, r)| {
                                            (r.y + r.height - pos).abs() < 4.0
                                                && r.x < div.end
                                                && (r.x + r.width) > div.start
                                        });
                                        let below = layout.iter().find(|(_, r)| {
                                            (r.y - pos).abs() < 4.0
                                                && r.x < div.end
                                                && (r.x + r.width) > div.start
                                        });
                                        match (above, below) {
                                            (Some((_, a)), Some((_, b))) => {
                                                (a.y, a.height + b.height)
                                            }
                                            _ => (0.0, state.gpu.height() as f32),
                                        }
                                    }
                                }
                            };
                            let start_cursor = match div.orientation {
                                DividerOrientation::Vertical => px,
                                DividerOrientation::Horizontal => py,
                            };
                            // Derive start_ratio from current divider position.
                            let start_ratio = if split_dimension > 0.0 {
                                (div.position - split_start) / split_dimension
                            } else {
                                0.5
                            };
                            state.drag_state = Some(DragState {
                                pane_id: div.pane_id,
                                orientation: div.orientation,
                                split_dimension,
                                split_start,
                                start_cursor,
                                start_ratio,
                            });
                            tracing::debug!(
                                pane_id = %div.pane_id,
                                start_ratio,
                                "divider drag started"
                            );
                        }
                        return;
                    }
                }

                // Tab drag release: reorder tabs if dragging.
                if elem_state == ElementState::Released && button == winit::event::MouseButton::Left
                {
                    if let TabDragState::Dragging {
                        pane_id,
                        from_index,
                        current_x,
                    } = state.tab_drag
                    {
                        if let Some(vp) = state.last_viewports.iter().find(|v| v.pane_id == pane_id)
                        {
                            let tab_width = vp.rect.width / vp.tab_count as f32;
                            let to_index = ((current_x - vp.rect.x) / tab_width) as usize;
                            let to_index = to_index.min(vp.tab_count - 1);
                            if from_index != to_index {
                                self.app_state
                                    .reorder_surface(pane_id, from_index, to_index);
                            }
                        }
                        state.tab_drag = TabDragState::None;
                        state.window.set_cursor(winit::window::CursorIcon::Default);
                        state.window.request_redraw();
                        return;
                    }
                    state.tab_drag = TabDragState::None;
                }

                // Left-button release: end any active divider drag.
                if elem_state == ElementState::Released
                    && button == winit::event::MouseButton::Left
                    && state.drag_state.is_some()
                {
                    tracing::debug!("divider drag ended");
                    state.drag_state = None;
                    state.window.request_redraw();
                    return;
                }

                // Tab bar: click to switch + initiate drag.
                if elem_state == ElementState::Pressed && button == winit::event::MouseButton::Left
                {
                    let mut tab_clicked = false;
                    for vp in &state.last_viewports {
                        if vp.tab_count <= 1 {
                            continue;
                        }
                        let tab_bar_bottom = vp.rect.y + wmux_render::pane::TAB_BAR_HEIGHT;
                        if px >= vp.rect.x
                            && px < vp.rect.x + vp.rect.width
                            && py >= vp.rect.y
                            && py < tab_bar_bottom
                        {
                            let tab_width = vp.rect.width / vp.tab_count as f32;
                            let tab_index = ((px - vp.rect.x) / tab_width) as usize;
                            let tab_index = tab_index.min(vp.tab_count - 1);

                            if vp.pane_id != state.focused_pane {
                                state.focused_pane = vp.pane_id;
                                self.app_state.focus_pane(vp.pane_id);
                            }
                            if tab_index != vp.active_tab {
                                self.app_state.cycle_surface_to_index(vp.pane_id, tab_index);
                            }
                            state.tab_drag = TabDragState::Pressing {
                                pane_id: vp.pane_id,
                                tab_index,
                                start_x: px,
                            };
                            tab_clicked = true;
                            state.window.request_redraw();
                            break;
                        }
                    }
                    if tab_clicked {
                        return;
                    }
                }

                // Click-to-focus: on any press, check if the click landed in a
                // different pane and switch focus to it.
                if elem_state == ElementState::Pressed {
                    // Use cached layout from the last render frame instead of
                    // blocking on the actor, which could cause UI freezes.
                    for (pane_id, rect) in &state.last_layout {
                        if rect.contains_point(px, py) && *pane_id != state.focused_pane {
                            state.focused_pane = *pane_id;
                            self.app_state.focus_pane(*pane_id);
                            state.window.request_redraw();
                            break;
                        }
                    }
                }

                let btn = match button {
                    winit::event::MouseButton::Left => MouseButton::Left,
                    winit::event::MouseButton::Right => MouseButton::Right,
                    winit::event::MouseButton::Middle => MouseButton::Middle,
                    _ => return,
                };

                let mouse_mode = state.terminal_modes.contains(TerminalMode::MOUSE_REPORTING);
                let shift = state.modifiers.shift_key();
                let (col, row) = state.cursor_cell();

                let action = match elem_state {
                    ElementState::Pressed => state
                        .mouse
                        .handle_mouse_press(col, row, btn, shift, mouse_mode),
                    ElementState::Released => {
                        state.mouse.handle_mouse_release(col, row, btn, mouse_mode)
                    }
                };

                state.handle_mouse_action(action, &self.app_state);
            }

            WindowEvent::CursorMoved { position, .. } => {
                state.cursor_pos = (position.x, position.y);
                let px = position.x as f32;
                let py = position.y as f32;

                // Tab drag: transition Pressing → Dragging on threshold.
                match state.tab_drag {
                    TabDragState::Pressing {
                        pane_id,
                        tab_index,
                        start_x,
                    } => {
                        if (px - start_x).abs() > 5.0 {
                            state.tab_drag = TabDragState::Dragging {
                                pane_id,
                                from_index: tab_index,
                                current_x: px,
                            };
                            state.window.set_cursor(winit::window::CursorIcon::Grabbing);
                            state.window.request_redraw();
                        }
                    }
                    TabDragState::Dragging {
                        ref mut current_x, ..
                    } => {
                        *current_x = px;
                        state.window.request_redraw();
                    }
                    TabDragState::None => {}
                }

                // Sidebar interactions: hover highlighting and drag-to-reorder.
                if state.sidebar.visible {
                    let in_sidebar = px < state.sidebar.effective_width();
                    let ws_count = state.workspace_cache.len();

                    // Check if we should transition from Pressing to Dragging.
                    if state.sidebar.should_start_drag(py) {
                        if let SidebarInteraction::Pressing { row, .. } = state.sidebar.interaction
                        {
                            state.sidebar.interaction = SidebarInteraction::Dragging {
                                from_row: row,
                                current_y: py,
                            };
                            state.window.set_cursor(winit::window::CursorIcon::Grabbing);
                            state.window.request_redraw();
                            return;
                        }
                    }

                    // Update drag position.
                    if let SidebarInteraction::Dragging {
                        ref mut current_y, ..
                    } = state.sidebar.interaction
                    {
                        *current_y = py;
                        state.window.set_cursor(winit::window::CursorIcon::Grabbing);
                        state.window.request_redraw();
                        return;
                    }

                    // Hover: update when inside sidebar and not dragging/editing/pressing.
                    if in_sidebar
                        && !matches!(
                            state.sidebar.interaction,
                            SidebarInteraction::Dragging { .. }
                                | SidebarInteraction::Editing { .. }
                                | SidebarInteraction::Pressing { .. }
                        )
                    {
                        let new_hover = state.sidebar.hit_test_row(py, ws_count);
                        let old_hover =
                            if let SidebarInteraction::Hover(h) = state.sidebar.interaction {
                                Some(h)
                            } else {
                                None
                            };
                        if new_hover != old_hover {
                            state.sidebar.interaction = match new_hover {
                                Some(idx) => SidebarInteraction::Hover(idx),
                                None => SidebarInteraction::Idle,
                            };
                            state.window.request_redraw();
                        }
                        // Pointer cursor in sidebar over workspace rows.
                        if new_hover.is_some() {
                            state.window.set_cursor(winit::window::CursorIcon::Pointer);
                        }
                        return;
                    } else if !in_sidebar
                        && matches!(state.sidebar.interaction, SidebarInteraction::Hover(_))
                    {
                        // Moved out of sidebar — clear hover.
                        state.sidebar.interaction = SidebarInteraction::Idle;
                        state.window.request_redraw();
                    }
                }

                // If a divider drag is active, compute the new ratio and resize.
                if let Some(ref drag) = state.drag_state {
                    let cursor = match drag.orientation {
                        DividerOrientation::Vertical => px,
                        DividerOrientation::Horizontal => py,
                    };
                    let new_ratio = divider::compute_ratio(drag, cursor);
                    self.app_state.resize_split(drag.pane_id, new_ratio);
                    state.window.request_redraw();
                    return;
                }

                // Change cursor icon based on divider hover (skip during tab drag).
                if matches!(state.tab_drag, TabDragState::None) {
                    let icon = match divider::hit_test(&state.dividers, px, py)
                        .map(|d| d.orientation)
                    {
                        Some(DividerOrientation::Vertical) => winit::window::CursorIcon::EwResize,
                        Some(DividerOrientation::Horizontal) => winit::window::CursorIcon::NsResize,
                        None => winit::window::CursorIcon::Default,
                    };
                    state.window.set_cursor(icon);
                }

                let (col, row) = state.cursor_cell();
                let mouse_mode = state.terminal_modes.contains(TerminalMode::MOUSE_REPORTING);
                let action = state.mouse.handle_mouse_motion(col, row, mouse_mode);
                state.handle_mouse_action(action, &self.app_state);
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let mouse_mode = state.terminal_modes.contains(TerminalMode::MOUSE_REPORTING);

                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y as f64,
                    MouseScrollDelta::PixelDelta(pos) => pos.y / state.metrics.cell_height as f64,
                };

                if mouse_mode {
                    let (col, row) = state.cursor_cell();
                    let button: u8 = if lines > 0.0 { 64 } else { 65 };
                    let report = {
                        use std::io::Write;
                        let mut buf = Vec::with_capacity(16);
                        let _ = write!(buf, "\x1b[<{};{};{}M", button, col + 1, row + 1);
                        buf
                    };
                    self.app_state.send_input(state.focused_pane, report);
                } else {
                    // Scroll viewport via actor (3 lines per scroll notch).
                    const SCROLL_LINES: i32 = 3;
                    let delta = if lines > 0.0 {
                        (lines.ceil() as i32) * SCROLL_LINES
                    } else {
                        (lines.floor() as i32) * SCROLL_LINES
                    };
                    if delta != 0 {
                        self.app_state.scroll_viewport(state.focused_pane, delta);
                        state.window.request_redraw();
                    }
                }
            }

            _ => {}
        }
    }
}
