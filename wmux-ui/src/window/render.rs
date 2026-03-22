use super::{TabDragState, UiState};
use crate::divider::{self, DividerOrientation};
use crate::search;
use std::collections::HashMap;
use wmux_core::{PaneId, PaneRenderData};
use wmux_render::TerminalRenderer;

/// Font size for tab bar titles (px).
const TAB_FONT_SIZE: f32 = 12.0;
/// Line height for tab bar titles (px).
const TAB_LINE_HEIGHT: f32 = 16.0;
/// Horizontal padding inside each tab (px per side).
const TAB_TEXT_PADDING: f32 = 8.0;
/// Vertical offset to center tab text within the tab bar (px).
const TAB_TEXT_TOP_OFFSET: f32 = 6.0;
/// Gap between tab bar start and the first tab (px).
const TAB_GAP: f32 = 4.0;

impl UiState<'_> {
    /// Render a frame: get layout from actor, draw borders, render ALL panes.
    pub(super) fn render(
        &mut self,
        app_state: &wmux_core::AppStateHandle,
        rt: &tokio::runtime::Handle,
    ) -> Result<(), crate::UiError> {
        let surface_width = self.gpu.width();
        let surface_height = self.gpu.height();

        // Reserve space for sidebar (left) and status bar (bottom).
        let sidebar_width = self.sidebar.effective_width();
        let status_bar_height = crate::status_bar::STATUS_BAR_HEIGHT;
        let surface_viewport = wmux_core::rect::Rect {
            x: sidebar_width,
            y: 0.0,
            width: (surface_width as f32 - sidebar_width).max(1.0),
            height: (surface_height as f32 - status_bar_height).max(1.0),
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
            &self.ui_chrome,
        );
        // Render edit cursor overlay when inline renaming.
        self.sidebar
            .render_edit_cursor(&mut self.quads, &self.ui_chrome);

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
                    .get_mut(id)
                    .map(|d| {
                        (
                            d.surface_count,
                            std::mem::take(&mut d.surface_titles),
                            d.active_surface,
                        )
                    })
                    .unwrap_or((1, vec![], 0));
                wmux_render::PaneViewport {
                    pane_id: *id,
                    rect: *rect,
                    focused: *id == self.focused_pane,
                    tab_count,
                    tab_titles,
                    active_tab,
                    zoomed: false,
                    surface_types: vec![wmux_render::SurfaceType::Terminal; tab_count],
                    unsaved: vec![false; tab_count],
                }
            })
            .collect();

        // Draw Focus Glow behind the active pane (signature "Luminous Void" element).
        let pane_renderer = wmux_render::PaneRenderer::new();
        for vp in &viewports {
            if vp.focused {
                wmux_render::PaneRenderer::render_focus_glow(
                    &mut self.quads,
                    &vp.rect,
                    self.ui_chrome.accent_glow_core,
                    self.ui_chrome.accent_glow,
                    1.0, // full glow alpha (no cross-fade yet)
                );
            }
        }

        // Draw focus indicators (accent stripe) for all panes.
        pane_renderer.render_pane_borders(
            &mut self.quads,
            &viewports,
            self.ui_chrome.border_default,
            self.ui_chrome.accent,
        );

        // Draw tab bars for panes with multiple surfaces, with shadow underneath.
        for vp in &viewports {
            if vp.tab_count > 1 {
                let _ = pane_renderer.render_tab_bar(
                    &mut self.quads,
                    vp,
                    self.ui_chrome.surface_1,
                    self.ui_chrome.surface_2,
                    self.ui_chrome.accent,
                );
                // Shadow under tab bar (shadow-sm)
                let r = &vp.rect;
                self.quads.push_quad(
                    r.x,
                    r.y + wmux_render::pane::TAB_BAR_HEIGHT,
                    r.width,
                    3.0,
                    self.ui_chrome.shadow,
                );
            }
        }

        // Collect live pane IDs once — used to prune stale tab title buffers and renderers.
        let live_ids: std::collections::HashSet<PaneId> =
            layout.iter().map(|(id, _)| *id).collect();

        // Update tab title text buffers for panes with multiple surfaces.
        {
            let metrics = glyphon::Metrics::new(TAB_FONT_SIZE, TAB_LINE_HEIGHT);
            let attrs = glyphon::Attrs::new().family(glyphon::Family::SansSerif);

            // Remove buffers for panes that no longer exist.
            self.tab_title_buffers.retain(|id, _| live_ids.contains(id));

            for vp in &viewports {
                if vp.tab_count <= 1 {
                    self.tab_title_buffers.remove(&vp.pane_id);
                    continue;
                }

                let bufs = self.tab_title_buffers.entry(vp.pane_id).or_default();

                // Use actual rendered tab width (respects gaps + MAX_TAB_WIDTH).
                let (tab_width, _) = wmux_render::PaneRenderer::tab_metrics(vp, 0);
                let text_max_width = (tab_width - TAB_TEXT_PADDING * 2.0).max(1.0);

                bufs.resize_with(vp.tab_count, || {
                    glyphon::Buffer::new(self.glyphon.font_system(), metrics)
                });

                for (i, title) in vp.tab_titles.iter().enumerate() {
                    let buf = &mut bufs[i];
                    buf.set_metrics(self.glyphon.font_system(), metrics);
                    buf.set_size(
                        self.glyphon.font_system(),
                        Some(text_max_width),
                        Some(TAB_LINE_HEIGHT),
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
                let (from_tw, from_tx) = wmux_render::PaneRenderer::tab_metrics(vp, *from_index);
                // Approximate target index from cursor position using tab metrics.
                let first_tab_x = vp.rect.x + TAB_GAP;
                let to_index = ((*current_x - first_tab_x) / (from_tw + TAB_GAP)).max(0.0) as usize;
                let to_index = to_index.min(vp.tab_count - 1);

                // Semi-transparent overlay on the dragged tab.
                let drag_color = [
                    self.ui_chrome.accent[0],
                    self.ui_chrome.accent[1],
                    self.ui_chrome.accent[2],
                    0.25,
                ];
                self.quads.push_quad(
                    from_tx,
                    vp.rect.y,
                    from_tw,
                    wmux_render::pane::TAB_BAR_HEIGHT,
                    drag_color,
                );

                // Drop indicator: 2px vertical accent bar at the target position.
                if to_index != *from_index {
                    let (_, to_tx) = wmux_render::PaneRenderer::tab_metrics(vp, to_index);
                    let indicator_x = to_tx
                        + if to_index > *from_index {
                            from_tw - 1.0
                        } else {
                            0.0
                        };
                    self.quads.push_quad(
                        indicator_x,
                        vp.rect.y,
                        2.0,
                        wmux_render::pane::TAB_BAR_HEIGHT,
                        self.ui_chrome.accent,
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

        // Deferred cursor quads — pushed after selection for correct z-order.
        let mut deferred_cursors: Vec<(PaneId, wmux_core::cursor::CursorState, (f32, f32))> =
            Vec::with_capacity(layout.len());

        // Render terminal content for ALL panes.
        for (pane_id, pane_rect) in &layout {
            // When the pane has multiple tabs, exclude the tab bar from terminal area.
            let viewport = viewports.iter().find(|vp| vp.pane_id == *pane_id);
            let terminal_rect = viewport
                .filter(|vp| vp.tab_count > 1)
                .map(wmux_render::PaneRenderer::terminal_viewport)
                .unwrap_or(*pane_rect);

            // Opaque background quad for the terminal area — one level
            // lighter than the sidebar/chrome for visual hierarchy.
            self.quads.push_quad(
                terminal_rect.x,
                terminal_rect.y,
                terminal_rect.width,
                terminal_rect.height,
                self.ui_chrome.surface_0,
            );

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
                let mut renderer =
                    TerminalRenderer::new(self.glyphon.font_system(), pane_cols, pane_rows);
                renderer.set_palette(
                    self.theme_ansi,
                    self.theme_cursor,
                    self.theme_foreground,
                    self.ui_chrome.cursor_alpha,
                );
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
                // Defer cursor rendering — will be pushed after selection for correct z-order.
                deferred_cursors.push((
                    *pane_id,
                    *data.grid.cursor(),
                    (terminal_rect.x, terminal_rect.y),
                ));
            }
        }

        // Pane dimming: surface_base overlay on inactive panes (text stays readable).
        {
            let dim_alpha = 1.0 - self.inactive_pane_opacity;
            let sb = self.ui_chrome.surface_base;
            let dim_color = [sb[0], sb[1], sb[2], dim_alpha];
            for vp in &viewports {
                if !vp.focused {
                    let r = &vp.rect;
                    self.quads.push_quad(r.x, r.y, r.width, r.height, dim_color);
                }
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
                self.quads
                    .push_quad(x, y, w, h, self.ui_chrome.selection_bg);
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
                &self.ui_chrome,
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
                    [
                        self.ui_chrome.error[0],
                        self.ui_chrome.error[1],
                        self.ui_chrome.error[2],
                        0.9,
                    ]
                } else {
                    [
                        self.ui_chrome.surface_base[0],
                        self.ui_chrome.surface_base[1],
                        self.ui_chrome.surface_base[2],
                        0.9,
                    ]
                };
                self.quads
                    .push_quad(focused_rect.x, bar_y, bar_w, bar_height, bg_color);

                // Thin accent line at the top of the search bar.
                self.quads
                    .push_quad(focused_rect.x, bar_y, bar_w, 1.0, self.ui_chrome.accent);

                // Match count indicator on the right side of the bar.
                let count_x = focused_rect.x + bar_w - 120.0;
                if count_x > focused_rect.x {
                    self.quads.push_quad(
                        count_x,
                        bar_y,
                        120.0,
                        bar_height,
                        self.ui_chrome.surface_0,
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

        // Cursor quad — pushed after selection/search for correct z-order.
        // Only render cursor for the focused pane; inactive pane cursors are
        // covered by the dimming overlay (drawn earlier), which is correct behavior.
        for (pane_id, cursor_state, origin) in &deferred_cursors {
            if *pane_id == self.focused_pane {
                if let Some(renderer) = self.renderers.get(pane_id) {
                    renderer.push_cursor(cursor_state, &mut self.quads, *origin);
                }
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
            // Use border_glow (luminous accent separator) for hovered dividers.
            let div_color = self.ui_chrome.border_glow;
            match div.orientation {
                DividerOrientation::Vertical => {
                    let x = div.position - DIV_HIGHLIGHT_THICKNESS / 2.0;
                    let y = div.start;
                    let w = DIV_HIGHLIGHT_THICKNESS;
                    let h = div.end - div.start;
                    self.quads.push_quad(x, y, w, h, div_color);
                }
                DividerOrientation::Horizontal => {
                    let x = div.start;
                    let y = div.position - DIV_HIGHLIGHT_THICKNESS / 2.0;
                    let w = div.end - div.start;
                    let h = DIV_HIGHLIGHT_THICKNESS;
                    self.quads.push_quad(x, y, w, h, div_color);
                }
            }
        }

        // Permanent pane dividers (1px border_glow between all pane pairs).
        for div in &self.dividers {
            match div.orientation {
                DividerOrientation::Vertical => {
                    self.quads.push_quad(
                        div.position - 0.5,
                        div.start,
                        1.0,
                        div.end - div.start,
                        self.ui_chrome.border_glow,
                    );
                }
                DividerOrientation::Horizontal => {
                    self.quads.push_quad(
                        div.start,
                        div.position - 0.5,
                        div.end - div.start,
                        1.0,
                        self.ui_chrome.border_glow,
                    );
                }
            }
        }

        // Status bar at the bottom (full window width).
        {
            let sb_y = surface_height as f32 - status_bar_height;
            let sb_w = surface_width as f32;
            // Shadow above status bar
            self.quads
                .push_quad(0.0, sb_y - 1.0, sb_w, 1.0, self.ui_chrome.shadow);
            // Status bar background
            self.quads
                .push_quad(0.0, sb_y, sb_w, status_bar_height, self.ui_chrome.surface_1);
        }

        // Upload quad GPU data.
        self.quads.prepare(&self.gpu.queue);

        // Collect text areas from ALL pane renderers + sidebar + tabs, then prepare glyphon once.
        let surface_w = self.gpu.width();
        let surface_h = self.gpu.height();
        let mut all_text_areas: Vec<_> = layout
            .iter()
            .filter_map(|(pane_id, pane_rect)| {
                let vp = viewports.iter().find(|v| v.pane_id == *pane_id);
                let terminal_rect = vp
                    .filter(|v| v.tab_count > 1)
                    .map(wmux_render::PaneRenderer::terminal_viewport)
                    .unwrap_or(*pane_rect);
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
                    for (i, buf) in bufs.iter().enumerate() {
                        let (tab_width, tab_x) = wmux_render::PaneRenderer::tab_metrics(vp, i);
                        let chrome_color = if i == vp.active_tab {
                            self.ui_chrome.text_primary
                        } else {
                            self.ui_chrome.text_secondary
                        };
                        let text_color = rgba_to_glyphon(chrome_color);
                        all_text_areas.push(glyphon::TextArea {
                            buffer: buf,
                            left: tab_x + TAB_TEXT_PADDING,
                            top: vp.rect.y + TAB_TEXT_TOP_OFFSET,
                            scale: 1.0,
                            bounds: glyphon::TextBounds {
                                left: (tab_x + TAB_TEXT_PADDING) as i32,
                                top: vp.rect.y as i32,
                                right: (tab_x + tab_width - TAB_TEXT_PADDING) as i32,
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
        all_text_areas.extend(
            self.sidebar
                .text_areas(surface_w, surface_h, &self.ui_chrome),
        );

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
                        load: wgpu::LoadOp::Clear({
                            let s = self.ui_chrome.surface_base;
                            let clear_alpha = match self.effect_result {
                                crate::effects::EffectResult::MicaAlt
                                | crate::effects::EffectResult::Mica => 0.0,
                                crate::effects::EffectResult::Opaque => 1.0,
                            };
                            wgpu::Color {
                                r: s[0] as f64,
                                g: s[1] as f64,
                                b: s[2] as f64,
                                a: clear_alpha,
                            }
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
    pub(super) fn cursor_cell(&self) -> (usize, usize) {
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
    pub(super) fn handle_mouse_action(
        &mut self,
        action: crate::mouse::MouseAction,
        app_state: &wmux_core::AppStateHandle,
    ) {
        match action {
            crate::mouse::MouseAction::None => {}
            crate::mouse::MouseAction::SelectionStarted
            | crate::mouse::MouseAction::SelectionUpdated
            | crate::mouse::MouseAction::SelectionFinished => {
                self.window.request_redraw();
            }
            crate::mouse::MouseAction::Report(bytes) => {
                app_state.send_input(self.focused_pane, bytes);
            }
            crate::mouse::MouseAction::Scroll(_) => {
                // Scroll is handled directly in mouse wheel event handler.
                self.window.request_redraw();
            }
        }
    }
}

/// Convert a normalized `[f32; 4]` RGBA color to a glyphon `Color`.
#[inline]
pub(super) fn rgba_to_glyphon(c: [f32; 4]) -> glyphon::Color {
    glyphon::Color::rgba(
        (c[0] * 255.0) as u8,
        (c[1] * 255.0) as u8,
        (c[2] * 255.0) as u8,
        (c[3] * 255.0) as u8,
    )
}
