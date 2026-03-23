use super::{TabDragState, UiState};
use crate::divider::{self, DividerOrientation};
use crate::search;
use std::collections::HashMap;
use wmux_core::{PaneId, PaneRenderData};
use wmux_render::TerminalRenderer;

use crate::typography;

/// Font size for tab bar titles — uses Body token.
const TAB_FONT_SIZE: f32 = typography::BODY_FONT_SIZE;
/// Line height for tab bar titles — uses Body token.
const TAB_LINE_HEIGHT: f32 = typography::BODY_LINE_HEIGHT;
/// Horizontal padding inside each tab (px per side).
const TAB_TEXT_PADDING: f32 = 10.0;
/// Vertical offset to center tab text within the pill.
/// Computed as: pad + (pill_h - LINE_HEIGHT) / 2, where pad=4, pill_h=TAB_BAR_HEIGHT-8.
const TAB_TEXT_TOP_OFFSET: f32 = 10.0;
/// Gap between tab bar start and the first tab (px).
const TAB_GAP: f32 = 4.0;

/// Search bar overlay height (px).
const SEARCH_BAR_HEIGHT: f32 = 38.0;
/// Horizontal padding inside the search bar (px).
const SEARCH_BAR_PADDING: f32 = 12.0;
/// Width of the match count section on the right side (px).
const SEARCH_COUNT_WIDTH: f32 = 140.0;
/// Line height for search bar text — uses Caption token.
const SEARCH_LINE_HEIGHT: f32 = typography::CAPTION_LINE_HEIGHT;
/// Width reserved for the search icon (magnifying glass) when icon font is available.
const SEARCH_ICON_RESERVE: f32 = 22.0;

impl UiState<'_> {
    /// Render a frame: get layout from actor, draw borders, render ALL panes.
    pub(super) fn render(
        &mut self,
        app_state: &wmux_core::AppStateHandle,
        rt: &tokio::runtime::Handle,
    ) -> Result<(), crate::UiError> {
        // Advance all UI animations.
        self.animation.update();

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
            self.scale_factor,
        );
        // Render edit cursor overlay when inline renaming.
        self.sidebar
            .render_edit_cursor(&mut self.quads, &self.ui_chrome);

        // Sidebar separator is rendered by sidebar.render_quads() as a 1px border_glow line.

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
                let (tab_count, tab_titles, surface_ids, surface_kinds, active_tab) =
                    render_data_map
                        .get_mut(id)
                        .map(|d| {
                            (
                                d.surface_count,
                                std::mem::take(&mut d.surface_titles),
                                std::mem::take(&mut d.surface_ids),
                                std::mem::take(&mut d.surface_kinds),
                                d.active_surface,
                            )
                        })
                        .unwrap_or((1, vec![], vec![], vec![], 0));
                let surface_types = surface_kinds
                    .iter()
                    .map(|k| match k {
                        wmux_core::PanelKind::Browser => wmux_render::SurfaceType::Browser,
                        wmux_core::PanelKind::Terminal => wmux_render::SurfaceType::Terminal,
                    })
                    .collect::<Vec<_>>();
                // Pad to tab_count if kinds are shorter (e.g., empty on first render).
                let surface_types = if surface_types.len() < tab_count {
                    let mut v = surface_types;
                    v.resize(tab_count, wmux_render::SurfaceType::Terminal);
                    v
                } else {
                    surface_types
                };
                wmux_render::PaneViewport {
                    pane_id: *id,
                    rect: *rect,
                    focused: *id == self.focused_pane,
                    tab_count,
                    tab_titles,
                    surface_ids,
                    active_tab,
                    zoomed: false,
                    surface_types,
                    unsaved: vec![false; tab_count],
                    scale: self.scale_factor,
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
                    self.focus_glow_anim
                        .and_then(|id| self.animation.get(id))
                        .unwrap_or(1.0),
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

        // Draw tab bar for every pane (always visible, even with a single tab).
        for vp in &viewports {
            let _ = pane_renderer.render_tab_bar(
                &mut self.quads,
                vp,
                self.ui_chrome.surface_1,
                self.ui_chrome.surface_2,
                self.ui_chrome.accent,
                (self.cursor_pos.0 as f32, self.cursor_pos.1 as f32),
            );
            // Analytical shadow under tab bar (shadow-sm)
            let r = &vp.rect;
            let sd = &self.ui_chrome.shadow_sm;
            self.shadows.push_shadow(
                r.x,
                r.y,
                r.width,
                wmux_render::pane::TAB_BAR_HEIGHT,
                0.0,
                sd.sigma,
                0.0,
                sd.offset_y,
                self.ui_chrome.shadow,
            );

            // Tab hover background highlight (animated).
            if let Some((hover_pane, hover_idx)) = self.tab_hover {
                if hover_pane == vp.pane_id && hover_idx != vp.active_tab {
                    let alpha = self
                        .tab_hover_anim
                        .and_then(|id| self.animation.get(id))
                        .unwrap_or(1.0);
                    let (tw, tx) = wmux_render::pane::PaneRenderer::tab_metrics(vp, hover_idx);
                    let pill_y = vp.rect.y + 4.0;
                    let pill_h = wmux_render::pane::TAB_BAR_HEIGHT - 8.0;
                    let s2 = self.ui_chrome.surface_2;
                    let hover_bg = [s2[0], s2[1], s2[2], s2[3] * 0.5 * alpha];
                    self.quads
                        .push_rounded_quad(tx, pill_y, tw, pill_h, hover_bg, 4.0);
                }
            }

            // Close button hover highlight (only when ≥ 2 tabs).
            if let Some((hover_pane, hover_idx)) = self.tab_close_hover {
                if hover_pane == vp.pane_id {
                    if let Some((bx, by, bw, bh)) =
                        wmux_render::pane::PaneRenderer::close_button_rect(vp, hover_idx)
                    {
                        let hover_bg = [
                            self.ui_chrome.error[0],
                            self.ui_chrome.error[1],
                            self.ui_chrome.error[2],
                            0.3,
                        ];
                        self.quads.push_rounded_quad(bx, by, bw, bh, hover_bg, 3.0);
                    }
                }
            }

            // Split button icon — rendered as SVG CustomGlyph in the text area section.

            // Tab edit: draw input box background when editing a tab title.
            if let super::TabEditState::Editing {
                pane_id: edit_pane,
                tab_index: edit_idx,
                ..
            } = &self.tab_edit
            {
                if *edit_pane == vp.pane_id {
                    let (tw, tx) = wmux_render::pane::PaneRenderer::tab_metrics(vp, *edit_idx);
                    let pill_y = vp.rect.y + 4.0;
                    let pill_h = wmux_render::pane::TAB_BAR_HEIGHT - 8.0;
                    // Background fill.
                    self.quads.push_rounded_quad(
                        tx,
                        pill_y,
                        tw,
                        pill_h,
                        self.ui_chrome.surface_base,
                        6.0,
                    );
                    // Accent border (top).
                    self.quads
                        .push_rounded_quad(tx, pill_y, tw, 1.0, self.ui_chrome.accent, 6.0);
                    // Accent border (bottom).
                    self.quads.push_rounded_quad(
                        tx,
                        pill_y + pill_h - 1.0,
                        tw,
                        1.0,
                        self.ui_chrome.accent,
                        6.0,
                    );
                }
            }
        }

        // Collect live pane IDs once — used to prune stale tab title buffers and renderers.
        let live_ids: std::collections::HashSet<PaneId> =
            layout.iter().map(|(id, _)| *id).collect();

        // Update tab title text buffers for panes with multiple surfaces.
        {
            let metrics = glyphon::Metrics::new(TAB_FONT_SIZE, TAB_LINE_HEIGHT);
            let attrs = glyphon::Attrs::new().family(glyphon::Family::Name("Segoe UI"));

            // Remove buffers for panes that no longer exist.
            self.tab_title_buffers.retain(|id, _| live_ids.contains(id));

            for vp in &viewports {
                let bufs = self.tab_title_buffers.entry(vp.pane_id).or_default();

                // Use actual rendered tab width (respects gaps + MAX_TAB_WIDTH).
                // Reserve space for close button (14px + 6px padding).
                let (tab_width, _) = wmux_render::PaneRenderer::tab_metrics(vp, 0);
                let close_reserve = 20.0;
                let text_max_width = (tab_width - TAB_TEXT_PADDING * 2.0 - close_reserve).max(1.0);

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

        // Validate tab_edit against current viewports (surface may have been closed via IPC).
        if let super::TabEditState::Editing {
            pane_id,
            tab_index,
            surface_id,
            ..
        } = &self.tab_edit
        {
            let still_valid = viewports.iter().any(|vp| {
                vp.pane_id == *pane_id
                    && *tab_index < vp.tab_count
                    && vp.surface_ids.get(*tab_index) == Some(surface_id)
            });
            if !still_valid {
                self.tab_edit = super::TabEditState::None;
            }
        }

        // Validate tab_close_hover against current viewports.
        if let Some((hover_pane, hover_idx)) = self.tab_close_hover {
            let still_valid = viewports
                .iter()
                .any(|vp| vp.pane_id == hover_pane && hover_idx < vp.tab_count);
            if !still_valid {
                self.tab_close_hover = None;
            }
        }

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

        // Position/show/hide browser panels based on active surface type.
        if let Some(ref mut mgr) = self.browser_manager {
            for vp in &viewports {
                let active_type = vp
                    .surface_types
                    .get(vp.active_tab)
                    .copied()
                    .unwrap_or(wmux_render::SurfaceType::Terminal);
                let active_sid = vp.surface_ids.get(vp.active_tab).copied();

                if active_type == wmux_render::SurfaceType::Browser {
                    if let Some(sid) = active_sid {
                        if mgr.get_panel(sid).is_some() {
                            let terminal_rect = wmux_render::PaneRenderer::terminal_viewport(vp);
                            let _ = mgr.resize_panel(sid, &terminal_rect);
                            let _ = mgr.show_panel(sid);
                        }
                    }
                }

                // Hide panels for non-active browser tabs in this pane.
                for (i, sid) in vp.surface_ids.iter().enumerate() {
                    if i != vp.active_tab && mgr.get_panel(*sid).is_some() {
                        let _ = mgr.hide_panel(*sid);
                    }
                }
            }
        }

        // Determine the focused pane's terminal content area (excludes tab bar).
        let focused_rect = viewports
            .iter()
            .find(|vp| vp.focused)
            .map(wmux_render::PaneRenderer::terminal_viewport)
            .unwrap_or(surface_viewport);

        // Deferred cursor quads — pushed after selection for correct z-order.
        let mut deferred_cursors: Vec<(PaneId, wmux_core::cursor::CursorState, (f32, f32))> =
            Vec::with_capacity(layout.len());

        // Render terminal content for ALL panes.
        for (pane_id, pane_rect) in &layout {
            // Always exclude the tab bar from terminal area (tab bar is always visible).
            let viewport = viewports.iter().find(|vp| vp.pane_id == *pane_id);
            let terminal_rect = viewport
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
                let mut renderer = TerminalRenderer::new(
                    self.glyphon.font_system(),
                    pane_cols,
                    pane_rows,
                    Some(self.terminal_font_family.as_str()),
                    Some(self.terminal_font_size),
                );
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

            // Search bar overlay: background quads + text buffer updates.
            {
                let bar_y = focused_rect.y + focused_rect.height - SEARCH_BAR_HEIGHT;
                let bar_w = focused_rect.width.max(200.0);

                // Semi-transparent background (tinted red on regex error).
                let bg_color = if self.search.has_regex_error() {
                    [
                        self.ui_chrome.error[0],
                        self.ui_chrome.error[1],
                        self.ui_chrome.error[2],
                        0.92,
                    ]
                } else {
                    [
                        self.ui_chrome.surface_base[0],
                        self.ui_chrome.surface_base[1],
                        self.ui_chrome.surface_base[2],
                        0.92,
                    ]
                };
                self.quads
                    .push_quad(focused_rect.x, bar_y, bar_w, SEARCH_BAR_HEIGHT, bg_color);

                // Accent line at the top (2px for visibility).
                self.quads
                    .push_quad(focused_rect.x, bar_y, bar_w, 2.0, self.ui_chrome.accent);

                // Match count section on the right side.
                let count_x = focused_rect.x + bar_w - SEARCH_COUNT_WIDTH;
                if count_x > focused_rect.x {
                    self.quads.push_quad(
                        count_x,
                        bar_y,
                        SEARCH_COUNT_WIDTH,
                        SEARCH_BAR_HEIGHT,
                        self.ui_chrome.surface_0,
                    );
                }

                // Update search text buffers for glyphon rendering (must happen
                // before cursor position query and before text area collection).
                let query_display: String = if self.search.query.is_empty() {
                    "Search\u{2026}".to_owned()
                } else {
                    self.search.query.clone()
                };
                let count_display = self.search.match_count_display();

                let query_w =
                    (bar_w - SEARCH_COUNT_WIDTH - SEARCH_BAR_PADDING * 2.0 - SEARCH_ICON_RESERVE)
                        .max(1.0);
                self.search_query_buffer.set_size(
                    self.glyphon.font_system(),
                    Some(query_w),
                    Some(SEARCH_BAR_HEIGHT),
                );
                self.search_query_buffer.set_text(
                    self.glyphon.font_system(),
                    &query_display,
                    &glyphon::Attrs::new().family(glyphon::Family::Name("Segoe UI")),
                    glyphon::Shaping::Advanced,
                    None,
                );

                self.search_count_buffer.set_size(
                    self.glyphon.font_system(),
                    Some(SEARCH_COUNT_WIDTH),
                    Some(SEARCH_BAR_HEIGHT),
                );
                self.search_count_buffer.set_text(
                    self.glyphon.font_system(),
                    &count_display,
                    &glyphon::Attrs::new().family(glyphon::Family::Name("Segoe UI")),
                    glyphon::Shaping::Advanced,
                    None,
                );

                // Text cursor: positioned using actual rendered text width from glyphon.
                // When the query is empty the buffer contains placeholder text —
                // place the cursor at the start, not after the placeholder.
                {
                    let text_w = if self.search.query.is_empty() {
                        0.0
                    } else {
                        self.search_query_buffer
                            .layout_runs()
                            .next()
                            .map_or(0.0, |run| run.line_w)
                    };
                    let cursor_x =
                        focused_rect.x + SEARCH_BAR_PADDING + SEARCH_ICON_RESERVE + text_w;
                    let cursor_y = bar_y + 7.0;
                    let cursor_h = SEARCH_BAR_HEIGHT - 14.0;
                    self.quads.push_quad(
                        cursor_x,
                        cursor_y,
                        1.5,
                        cursor_h,
                        self.ui_chrome.text_primary,
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

        // Permanent pane dividers — subtle 1px line between all pane pairs.
        for div in &self.dividers {
            match div.orientation {
                DividerOrientation::Vertical => {
                    self.quads.push_quad(
                        div.position - 0.5,
                        div.start,
                        1.0,
                        div.end - div.start,
                        self.ui_chrome.border_subtle,
                    );
                }
                DividerOrientation::Horizontal => {
                    self.quads.push_quad(
                        div.start,
                        div.position - 0.5,
                        div.end - div.start,
                        1.0,
                        self.ui_chrome.border_subtle,
                    );
                }
            }
        }

        // Status bar at the bottom (full window width).
        let sb_y = surface_height as f32 - status_bar_height;
        let sb_w = surface_width as f32;
        {
            // Update status bar data from cached workspace list.
            let active_ws = self.workspace_cache.iter().find(|ws| ws.active);
            self.status_bar_data.workspace_name =
                active_ws.map(|ws| ws.name.clone()).unwrap_or_default();
            self.status_bar_data.pane_count = layout.len();
            self.status_bar_data.branch = active_ws.and_then(|ws| ws.git_branch.clone());

            self.status_bar
                .update_text(self.glyphon.font_system(), &self.status_bar_data, sb_w);

            let time_secs = self.start_instant.elapsed().as_secs_f32();

            // Analytical shadow above status bar (shadow-sm, upward)
            {
                let sd = &self.ui_chrome.shadow_sm;
                self.shadows.push_shadow(
                    0.0,
                    sb_y,
                    sb_w,
                    crate::status_bar::STATUS_BAR_HEIGHT,
                    0.0,
                    sd.sigma,
                    0.0,
                    -sd.offset_y,
                    self.ui_chrome.shadow,
                );
            }

            // Render status bar quads (background + connection dot).
            self.status_bar.render_quads(
                &mut self.quads,
                &self.ui_chrome,
                0.0,
                sb_y,
                sb_w,
                time_secs,
                &self.status_bar_data,
            );
        }

        // Split direction popup menu (renders on top of everything).
        if let super::SplitMenuState::Open { menu_x, menu_y, .. } = self.split_menu {
            let item_h = 32.0;
            let menu_w = 240.0;
            let menu_h = item_h * 4.0 + 8.0; // 4 items + padding
            let menu_radius = 8.0;

            // Menu shadow
            let sd = &self.ui_chrome.shadow_md;
            self.shadows.push_shadow(
                menu_x,
                menu_y,
                menu_w,
                menu_h,
                menu_radius,
                sd.sigma,
                0.0,
                sd.offset_y,
                self.ui_chrome.shadow,
            );

            // Menu background
            self.quads.push_rounded_quad(
                menu_x,
                menu_y,
                menu_w,
                menu_h,
                self.ui_chrome.surface_2,
                menu_radius,
            );

            // Menu border
            self.quads.push_rounded_quad(
                menu_x,
                menu_y,
                menu_w,
                menu_h,
                [
                    self.ui_chrome.border_subtle[0],
                    self.ui_chrome.border_subtle[1],
                    self.ui_chrome.border_subtle[2],
                    0.3,
                ],
                menu_radius,
            );

            // Hover highlight for hovered item
            if let Some(hover_idx) = self.split_menu_hover {
                let hy = menu_y + 4.0 + hover_idx as f32 * item_h;
                let hover_bg = [
                    self.ui_chrome.accent[0],
                    self.ui_chrome.accent[1],
                    self.ui_chrome.accent[2],
                    0.15,
                ];
                self.quads
                    .push_rounded_quad(menu_x + 4.0, hy, menu_w - 8.0, item_h, hover_bg, 4.0);
            }

            // Split direction icons rendered as SVG CustomGlyphs in the text area section.
        }

        // Upload GPU data.
        self.shadows.prepare(&self.gpu.queue);
        self.quads.prepare(&self.gpu.queue);

        // Collect text areas from ALL pane renderers + sidebar + tabs, then prepare glyphon once.
        let surface_w = self.gpu.width();
        let surface_h = self.gpu.height();
        let mut all_text_areas: Vec<_> = layout
            .iter()
            .filter_map(|(pane_id, pane_rect)| {
                let vp = viewports.iter().find(|v| v.pane_id == *pane_id);
                let terminal_rect = vp
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

        // Append tab title text areas + close button × glyphs.
        // Also update the tab edit buffer if editing.
        let close_reserve = 20.0;

        // Update tab edit buffer text if editing.
        if let super::TabEditState::Editing { ref text, .. } = self.tab_edit {
            let edit_metrics = glyphon::Metrics::new(TAB_FONT_SIZE, TAB_LINE_HEIGHT);
            let edit_attrs = glyphon::Attrs::new().family(glyphon::Family::Name("Segoe UI"));
            let buf = self.tab_edit_buffer.get_or_insert_with(|| {
                glyphon::Buffer::new(self.glyphon.font_system(), edit_metrics)
            });
            buf.set_metrics(self.glyphon.font_system(), edit_metrics);
            buf.set_size(
                self.glyphon.font_system(),
                Some(120.0),
                Some(TAB_LINE_HEIGHT),
            );
            buf.set_text(
                self.glyphon.font_system(),
                text,
                &edit_attrs,
                glyphon::Shaping::Advanced,
                None,
            );
            buf.shape_until_scroll(self.glyphon.font_system(), false);
        }

        for vp in &viewports {
            {
                if let Some(bufs) = self.tab_title_buffers.get(&vp.pane_id) {
                    for (i, buf) in bufs.iter().enumerate() {
                        let (tab_width, tab_x) = wmux_render::PaneRenderer::tab_metrics(vp, i);

                        // If this tab is being edited, show the edit buffer instead.
                        let is_editing = matches!(
                            self.tab_edit,
                            super::TabEditState::Editing { pane_id, tab_index, .. }
                            if pane_id == vp.pane_id && tab_index == i
                        );

                        if is_editing {
                            if let Some(ref edit_buf) = self.tab_edit_buffer {
                                all_text_areas.push(glyphon::TextArea {
                                    buffer: edit_buf,
                                    left: tab_x + TAB_TEXT_PADDING,
                                    top: vp.rect.y + TAB_TEXT_TOP_OFFSET,
                                    scale: self.scale_factor,
                                    bounds: glyphon::TextBounds {
                                        left: (tab_x + TAB_TEXT_PADDING) as i32,
                                        top: vp.rect.y as i32,
                                        right: (tab_x + tab_width - TAB_TEXT_PADDING) as i32,
                                        bottom: (vp.rect.y + wmux_render::pane::TAB_BAR_HEIGHT)
                                            as i32,
                                    },
                                    default_color: rgba_to_glyphon(self.ui_chrome.text_primary),
                                    custom_glyphs: &[],
                                });

                                // Edit cursor.
                                if let super::TabEditState::Editing { cursor, .. } = &self.tab_edit
                                {
                                    let char_width = TAB_FONT_SIZE * 0.6;
                                    let cursor_x =
                                        tab_x + TAB_TEXT_PADDING + (*cursor as f32 * char_width);
                                    let cursor_y = vp.rect.y + TAB_TEXT_TOP_OFFSET;
                                    let cursor_color = [
                                        self.ui_chrome.text_primary[0],
                                        self.ui_chrome.text_primary[1],
                                        self.ui_chrome.text_primary[2],
                                        0.85,
                                    ];
                                    self.quads.push_quad(
                                        cursor_x,
                                        cursor_y,
                                        1.5,
                                        TAB_LINE_HEIGHT,
                                        cursor_color,
                                    );
                                }
                            }
                        } else {
                            // Normal tab title.
                            let chrome_color = if i == vp.active_tab {
                                self.ui_chrome.text_primary
                            } else {
                                self.ui_chrome.text_secondary
                            };
                            let text_color = rgba_to_glyphon(chrome_color);
                            // Offset text past the type indicator icon (20px).
                            let icon_reserve = 26.0;
                            let text_left = tab_x + TAB_TEXT_PADDING + icon_reserve;
                            all_text_areas.push(glyphon::TextArea {
                                buffer: buf,
                                left: text_left,
                                top: vp.rect.y + TAB_TEXT_TOP_OFFSET,
                                scale: self.scale_factor,
                                bounds: glyphon::TextBounds {
                                    left: text_left as i32,
                                    top: vp.rect.y as i32,
                                    right: (tab_x + tab_width - TAB_TEXT_PADDING - close_reserve)
                                        as i32,
                                    bottom: (vp.rect.y + wmux_render::pane::TAB_BAR_HEIGHT) as i32,
                                },
                                default_color: text_color,
                                custom_glyphs: &[],
                            });
                        }

                        // Surface type indicator (left side of pill).
                        if let Some((ix, _iy)) =
                            wmux_render::pane::PaneRenderer::tab_type_indicator_pos(vp, i)
                        {
                            let indicator_color = if i == vp.active_tab {
                                self.ui_chrome.text_primary
                            } else {
                                self.ui_chrome.text_muted
                            };
                            let st = vp
                                .surface_types
                                .get(i)
                                .copied()
                                .unwrap_or(wmux_render::SurfaceType::Terminal);

                            {
                                // SVG icon via CustomGlyph for surface type indicator.
                                let cg_ref = match st {
                                    wmux_render::SurfaceType::Terminal => &self.cg_terminal,
                                    wmux_render::SurfaceType::Browser => &self.cg_globe,
                                };
                                // Align icon vertically with tab text baseline.
                                let icon_top = vp.rect.y + TAB_TEXT_TOP_OFFSET;
                                all_text_areas.push(glyphon::TextArea {
                                    buffer: &self.icon_empty_buffer,
                                    left: ix,
                                    top: icon_top,
                                    scale: self.scale_factor,
                                    bounds: glyphon::TextBounds {
                                        left: (ix - 2.0) as i32,
                                        top: (icon_top - 2.0) as i32,
                                        right: (ix + 24.0) as i32,
                                        bottom: (icon_top + 24.0) as i32,
                                    },
                                    default_color: rgba_to_glyphon(indicator_color),
                                    custom_glyphs: cg_ref,
                                });
                            }
                        }
                    }
                }
            }
        }

        // Draw × and + button icons via SVG CustomGlyph.
        for vp in &viewports {
            // × close button — SVG close icon.
            for i in 0..vp.tab_count {
                if let Some((bx, by, _bw, _bh)) =
                    wmux_render::pane::PaneRenderer::close_button_rect(vp, i)
                {
                    let is_hovered = self
                        .tab_close_hover
                        .is_some_and(|(hp, hi)| hp == vp.pane_id && hi == i);
                    let close_color = if is_hovered {
                        rgba_to_glyphon(self.ui_chrome.text_primary)
                    } else {
                        rgba_to_glyphon(self.ui_chrome.text_muted)
                    };
                    all_text_areas.push(glyphon::TextArea {
                        buffer: &self.icon_empty_buffer,
                        left: bx + 2.0,
                        top: by + 2.0,
                        scale: self.scale_factor,
                        bounds: glyphon::TextBounds {
                            left: bx as i32,
                            top: by as i32,
                            right: (bx + 18.0) as i32,
                            bottom: (by + 18.0) as i32,
                        },
                        default_color: close_color,
                        custom_glyphs: &self.cg_close,
                    });
                }
            }

            // "+" button — SVG add icon.
            if let Some((px, py, pw, ph)) = wmux_render::pane::PaneRenderer::plus_button_rect(vp) {
                all_text_areas.push(glyphon::TextArea {
                    buffer: &self.icon_empty_buffer,
                    left: px + (pw - 16.0) / 2.0,
                    top: py + (ph - 16.0) / 2.0,
                    scale: self.scale_factor,
                    bounds: glyphon::TextBounds {
                        left: px as i32,
                        top: py as i32,
                        right: (px + pw) as i32,
                        bottom: (py + ph) as i32,
                    },
                    default_color: rgba_to_glyphon(self.ui_chrome.text_primary),
                    custom_glyphs: &self.cg_add,
                });
            }

            // Split button — SVG split-horizontal icon.
            if let Some((sx, sy, sw, sh)) = wmux_render::pane::PaneRenderer::split_button_rect(vp) {
                all_text_areas.push(glyphon::TextArea {
                    buffer: &self.icon_empty_buffer,
                    left: sx + (sw - 16.0) / 2.0,
                    top: sy + (sh - 16.0) / 2.0,
                    scale: self.scale_factor,
                    bounds: glyphon::TextBounds {
                        left: sx as i32,
                        top: sy as i32,
                        right: (sx + sw) as i32,
                        bottom: (sy + sh) as i32,
                    },
                    default_color: rgba_to_glyphon(self.ui_chrome.text_secondary),
                    custom_glyphs: &self.cg_split,
                });
            }
        }

        // Append sidebar text areas (workspace names + subtitles + icons).
        // Pass empty buffer + pre-built CustomGlyph for sidebar workspace icon.
        let sidebar_icon = Some((&self.icon_empty_buffer, self.cg_workspace.as_slice()));
        let ws_status_icons: Vec<Vec<(String, String)>> = self
            .workspace_cache
            .iter()
            .map(|ws| ws.status_icons.clone())
            .collect();
        all_text_areas.extend(self.sidebar.text_areas(
            surface_w,
            surface_h,
            &self.ui_chrome,
            self.scale_factor,
            sidebar_icon,
            &ws_status_icons,
            &self.icon_empty_buffer,
            &self.status_icon_cgs,
        ));

        // Append search bar text areas (icon + query + match count).
        if self.search.active {
            let bar_y = focused_rect.y + focused_rect.height - SEARCH_BAR_HEIGHT;
            let bar_w = focused_rect.width.max(200.0);
            let text_y = bar_y + (SEARCH_BAR_HEIGHT - SEARCH_LINE_HEIGHT) / 2.0;

            // Search icon (magnifying glass) — SVG CustomGlyph.
            let icon_offset = {
                all_text_areas.push(glyphon::TextArea {
                    buffer: &self.icon_empty_buffer,
                    left: focused_rect.x + SEARCH_BAR_PADDING,
                    top: text_y + 1.0,
                    scale: self.scale_factor,
                    bounds: glyphon::TextBounds {
                        left: focused_rect.x as i32,
                        top: bar_y as i32,
                        right: (focused_rect.x + SEARCH_BAR_PADDING + SEARCH_ICON_RESERVE) as i32,
                        bottom: (bar_y + SEARCH_BAR_HEIGHT) as i32,
                    },
                    default_color: rgba_to_glyphon(self.ui_chrome.text_muted),
                    custom_glyphs: &self.cg_search,
                });
                SEARCH_ICON_RESERVE
            };

            // Query text (after icon) — muted color for placeholder, primary for input.
            let query_x = focused_rect.x + SEARCH_BAR_PADDING + icon_offset;
            let query_color = if self.search.query.is_empty() {
                rgba_to_glyphon(self.ui_chrome.text_muted)
            } else {
                rgba_to_glyphon(self.ui_chrome.text_primary)
            };
            all_text_areas.push(glyphon::TextArea {
                buffer: &self.search_query_buffer,
                left: query_x,
                top: text_y,
                scale: self.scale_factor,
                bounds: glyphon::TextBounds {
                    left: query_x as i32,
                    top: bar_y as i32,
                    right: (focused_rect.x + bar_w - SEARCH_COUNT_WIDTH) as i32,
                    bottom: (bar_y + SEARCH_BAR_HEIGHT) as i32,
                },
                default_color: query_color,
                custom_glyphs: &[],
            });

            // Match count text (right side).
            let count_x = focused_rect.x + bar_w - SEARCH_COUNT_WIDTH;
            if count_x > focused_rect.x {
                all_text_areas.push(glyphon::TextArea {
                    buffer: &self.search_count_buffer,
                    left: count_x + 8.0,
                    top: text_y,
                    scale: self.scale_factor,
                    bounds: glyphon::TextBounds {
                        left: count_x as i32,
                        top: bar_y as i32,
                        right: (focused_rect.x + bar_w) as i32,
                        bottom: (bar_y + SEARCH_BAR_HEIGHT) as i32,
                    },
                    default_color: rgba_to_glyphon(self.ui_chrome.text_secondary),
                    custom_glyphs: &[],
                });
            }
        }

        // Append status bar text area.
        all_text_areas.push(self.status_bar.text_area(
            0.0,
            sb_y,
            sb_w,
            &self.ui_chrome,
            self.scale_factor,
        ));

        // Append split menu text areas (labels + shortcut hints + direction icons).
        if let super::SplitMenuState::Open { menu_x, menu_y, .. } = self.split_menu {
            let item_h = 32.0;
            let label_x = menu_x + 34.0; // after icon
            let hint_x = menu_x + 130.0;
            let label_color = rgba_to_glyphon(self.ui_chrome.text_primary);
            let hint_color = rgba_to_glyphon(self.ui_chrome.text_muted);
            let icon_color = rgba_to_glyphon(self.ui_chrome.text_secondary);
            for i in 0..4 {
                let iy = menu_y + 4.0 + i as f32 * item_h;
                let ty = iy + (item_h - 18.0) / 2.0;

                // Direction arrow icon — SVG CustomGlyph.
                all_text_areas.push(glyphon::TextArea {
                    buffer: &self.icon_empty_buffer,
                    left: menu_x + 10.0,
                    top: ty,
                    scale: self.scale_factor,
                    bounds: glyphon::TextBounds {
                        left: menu_x as i32,
                        top: iy as i32,
                        right: (menu_x + 32.0) as i32,
                        bottom: (iy + item_h) as i32,
                    },
                    default_color: icon_color,
                    custom_glyphs: &self.cg_arrows[i as usize],
                });

                all_text_areas.push(glyphon::TextArea {
                    buffer: &self.split_menu_buffers[i as usize],
                    left: label_x,
                    top: ty,
                    scale: self.scale_factor,
                    bounds: glyphon::TextBounds {
                        left: label_x as i32,
                        top: iy as i32,
                        right: (hint_x - 4.0) as i32,
                        bottom: (iy + item_h) as i32,
                    },
                    default_color: label_color,
                    custom_glyphs: &[],
                });
                all_text_areas.push(glyphon::TextArea {
                    buffer: &self.split_menu_hint_buffers[i as usize],
                    left: hint_x,
                    top: ty,
                    scale: self.scale_factor,
                    bounds: glyphon::TextBounds {
                        left: hint_x as i32,
                        top: iy as i32,
                        right: (menu_x + 240.0) as i32,
                        bottom: (iy + item_h) as i32,
                    },
                    default_color: hint_color,
                    custom_glyphs: &[],
                });
            }
        }

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

            // Shadows first (behind everything), then quads, then text on top.
            self.shadows.render(&mut render_pass);
            self.quads.render(&mut render_pass);
            self.glyphon.render(&mut render_pass)?;
        }

        self.gpu.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        self.quads.clear();
        self.shadows.clear();
        self.glyphon.trim_atlas();

        // Keep rendering while animations are running.
        if self.animation.has_active() {
            self.window.request_redraw();
        }

        Ok(())
    }

    /// Convert the current cursor pixel position to pane-local cell coordinates.
    ///
    /// Uses the cached layout to find the focused pane's origin and dimensions,
    /// so that mouse events produce coordinates relative to the pane, not the
    /// full window surface.
    pub(super) fn cursor_cell(&self) -> (usize, usize) {
        // Use terminal content area (excluding tab bar) for coordinate mapping.
        let tab_bar_h = wmux_render::pane::TAB_BAR_HEIGHT as f64;
        let (origin_x, origin_y, pane_cols, pane_rows) = self
            .last_layout
            .iter()
            .find(|(id, _)| *id == self.focused_pane)
            .map(|(_, rect)| {
                let content_height = (rect.height - wmux_render::pane::TAB_BAR_HEIGHT).max(0.0);
                let cols = (rect.width / self.metrics.cell_width).floor().max(1.0) as usize;
                let rows = (content_height / self.metrics.cell_height).floor().max(1.0) as usize;
                (rect.x as f64, rect.y as f64 + tab_bar_h, cols, rows)
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
