use std::collections::{HashMap, HashSet};

use wmux_core::{PaneId, PaneRenderData};
use wmux_render::TerminalRenderer;

use crate::{
    divider::{self, DividerOrientation},
    search, typography,
};

use super::{TabDragState, UiState};

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

        // Reusable text attributes — avoid recreating per section.
        let ui_attrs = glyphon::Attrs::new().family(glyphon::Family::Name("Segoe UI"));
        let ui_attrs_bold = glyphon::Attrs::new()
            .family(glyphon::Family::Name("Segoe UI"))
            .weight(glyphon::Weight::BOLD);
        let ui_attrs_light = glyphon::Attrs::new()
            .family(glyphon::Family::Name("Segoe UI"))
            .weight(glyphon::Weight::LIGHT);
        let tab_metrics = glyphon::Metrics::new(TAB_FONT_SIZE, TAB_LINE_HEIGHT);

        let surface_width = self.gpu.width();
        let surface_height = self.gpu.height();

        // Reserve space for title bar (top), sidebar (left), and status bar (bottom).
        let titlebar_height = if self.titlebar.custom_chrome_active {
            crate::titlebar::TITLE_BAR_HEIGHT * self.scale_factor
        } else {
            0.0
        };
        let sidebar_width = self.sidebar.effective_width();
        let status_bar_height = crate::status_bar::STATUS_BAR_HEIGHT * self.scale_factor;
        let surface_viewport = wmux_core::rect::Rect {
            x: sidebar_width,
            y: titlebar_height,
            width: (surface_width as f32 - sidebar_width).max(1.0),
            height: (surface_height as f32 - titlebar_height - status_bar_height).max(1.0),
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
            .render_edit_cursor(&mut self.quads, &self.ui_chrome, self.scale_factor);

        // Sidebar separator is rendered by sidebar.render_quads() as a 1px border_glow line.

        // Render custom title bar AFTER sidebar — the title bar's opaque background
        // paints over the sidebar's top region, creating a unified top strip.
        self.titlebar.update_maximized(self.main_hwnd);
        self.titlebar.render_quads(
            &mut self.quads,
            &self.ui_chrome,
            surface_width as f32,
            self.scale_factor,
        );
        // Update chrome button icon positions and store for TextArea borrowing.
        self.cg_chrome_buttons = self.titlebar.chrome_button_glyphs(
            surface_width as f32,
            self.scale_factor,
            &self.ui_chrome,
        );

        // Opaque fill for the content area — ensures pane gaps are dark even when
        // Mica/Acrylic makes the clear color transparent.
        {
            let sb = self.ui_chrome.surface_base;
            self.quads.push_quad(
                surface_viewport.x,
                surface_viewport.y,
                surface_viewport.width,
                surface_viewport.height,
                [sb[0], sb[1], sb[2], 1.0],
            );
        }

        // Get pane layout from the actor (blocks briefly — acceptable once per frame).
        let (layout, layout_dividers) = rt.block_on(app_state.get_layout(surface_viewport));
        // Cache for non-blocking hit-testing on mouse clicks.
        self.last_layout.clone_from(&layout);
        // Convert tree-based dividers to UI dividers.
        self.dividers = layout_dividers
            .into_iter()
            .map(crate::divider::Divider::from)
            .collect();

        // Auto-correct stale focused_pane: if the current focused pane is not in
        // the layout (e.g., workspace was closed), snap to the first available pane.
        if !layout.iter().any(|(id, _)| *id == self.focused_pane) {
            if let Some((first_id, _)) = layout.first() {
                tracing::info!(
                    old = %self.focused_pane,
                    new = %first_id,
                    "focused_pane corrected (stale reference)"
                );
                self.focused_pane = *first_id;
                app_state.focus_pane(*first_id);
            }
        }

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

        let pane_renderer = wmux_render::PaneRenderer::new();

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
                vp.tab_bar_height(),
                0.0,
                sd.sigma,
                0.0,
                sd.offset_y,
                self.ui_chrome.shadow,
            );

            // Tab hover background highlight (animated) — skip in toggle mode.
            if !vp.is_toggle_mode() {
                if let Some((hover_pane, hover_idx)) = self.tab_hover {
                    if hover_pane == vp.pane_id && hover_idx != vp.active_tab {
                        let alpha = self
                            .tab_hover_anim
                            .and_then(|id| self.animation.get(id))
                            .unwrap_or(1.0);
                        let (tw, tx) = wmux_render::pane::PaneRenderer::tab_metrics(vp, hover_idx);
                        let s = self.scale_factor;
                        let pill_y = vp.rect.y + 4.0 * s;
                        let pill_h = vp.tab_bar_height() - 8.0 * s;
                        let s2 = self.ui_chrome.surface_2;
                        let hover_bg = [s2[0], s2[1], s2[2], s2[3] * 0.5 * alpha];
                        self.quads
                            .push_rounded_quad(tx, pill_y, tw, pill_h, hover_bg, 4.0 * s);
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
                        let s = self.scale_factor;
                        let pill_y = vp.rect.y + 4.0 * s;
                        let pill_h = vp.tab_bar_height() - 8.0 * s;
                        // Background fill.
                        self.quads.push_rounded_quad(
                            tx,
                            pill_y,
                            tw,
                            pill_h,
                            self.ui_chrome.surface_base,
                            6.0 * s,
                        );
                        // Accent border (top).
                        self.quads.push_rounded_quad(
                            tx,
                            pill_y,
                            tw,
                            1.0 * s,
                            self.ui_chrome.accent,
                            6.0 * s,
                        );
                        // Accent border (bottom).
                        self.quads.push_rounded_quad(
                            tx,
                            pill_y + pill_h - 1.0 * s,
                            tw,
                            1.0 * s,
                            self.ui_chrome.accent,
                            6.0 * s,
                        );
                    }
                }
            } // end if !vp.is_toggle_mode()
        }

        // Collect live pane IDs once — used to prune stale tab title buffers and renderers.
        let live_ids: HashSet<PaneId> = layout.iter().map(|(id, _)| *id).collect();

        // Update tab title text buffers for panes with multiple surfaces.
        {
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
                    glyphon::Buffer::new(self.glyphon.font_system(), tab_metrics)
                });

                for (i, title) in vp.tab_titles.iter().enumerate() {
                    let buf = &mut bufs[i];
                    buf.set_metrics(self.glyphon.font_system(), tab_metrics);
                    buf.set_size(
                        self.glyphon.font_system(),
                        Some(text_max_width),
                        Some(TAB_LINE_HEIGHT),
                    );
                    buf.set_text(
                        self.glyphon.font_system(),
                        title,
                        &ui_attrs,
                        glyphon::Shaping::Advanced,
                        None,
                    );
                    buf.shape_until_scroll(self.glyphon.font_system(), false);
                }
            }
        }

        // Update toggle label buffers for panes in toggle mode.
        {
            self.toggle_label_buffers
                .retain(|id, _| live_ids.contains(id));

            for vp in &viewports {
                if !vp.is_toggle_mode() {
                    self.toggle_label_buffers.remove(&vp.pane_id);
                    continue;
                }

                let labels = [self.locale.t("tab.shell"), self.locale.t("tab.browser")];
                let bufs = self
                    .toggle_label_buffers
                    .entry(vp.pane_id)
                    .or_insert_with(|| {
                        [
                            glyphon::Buffer::new(self.glyphon.font_system(), tab_metrics),
                            glyphon::Buffer::new(self.glyphon.font_system(), tab_metrics),
                        ]
                    });

                let seg_width = wmux_render::pane::PaneRenderer::toggle_segment_rect(vp, 0)
                    .map(|(_, _, w, _)| w)
                    .unwrap_or(100.0);
                let s = vp.scale;
                let icon_reserve = 30.0 * s; // icon padding (10) + icon gap (20)
                let text_max = (seg_width - icon_reserve - 4.0 * s).max(1.0);

                for (i, label) in labels.iter().enumerate() {
                    bufs[i].set_metrics(self.glyphon.font_system(), tab_metrics);
                    bufs[i].set_size(
                        self.glyphon.font_system(),
                        Some(text_max),
                        Some(TAB_LINE_HEIGHT),
                    );
                    bufs[i].set_text(
                        self.glyphon.font_system(),
                        label,
                        &ui_attrs,
                        glyphon::Shaping::Advanced,
                        None,
                    );
                    bufs[i].shape_until_scroll(self.glyphon.font_system(), false);
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
                self.quads
                    .push_quad(from_tx, vp.rect.y, from_tw, vp.tab_bar_height(), drag_color);

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
                        vp.tab_bar_height(),
                        self.ui_chrome.accent,
                    );
                }
            }
        }

        // Remove renderers for panes that no longer exist in the layout.
        self.renderers.retain(|id, _| live_ids.contains(id));

        // Render address bars and position/show/hide browser panels.
        if let Some(ref mut mgr) = self.browser_manager {
            // Collect all surface IDs still referenced by viewports — used for orphan cleanup.
            self.live_browser_sids.clear();

            // Determine which browser surface (if any) should have Win32 keyboard focus.
            let mut desired_browser_focus: Option<wmux_core::SurfaceId> = None;

            for vp in &viewports {
                // Track all surface IDs in this viewport.
                for sid in &vp.surface_ids {
                    self.live_browser_sids.insert(*sid);
                }

                let active_type = vp
                    .surface_types
                    .get(vp.active_tab)
                    .copied()
                    .unwrap_or(wmux_render::SurfaceType::Terminal);
                let active_sid = vp.surface_ids.get(vp.active_tab).copied();

                if active_type == wmux_render::SurfaceType::Browser {
                    // Render the address bar between tab bar and browser content.
                    let bar_rect = wmux_render::PaneRenderer::address_bar_rect(vp);
                    self.address_bar.render_quads(
                        &mut self.quads,
                        &bar_rect,
                        &self.ui_chrome,
                        vp.scale,
                    );

                    // Address bar text caret (proportional interpolation).
                    if self.address_bar.editing {
                        let url_rect =
                            crate::address_bar::AddressBarState::url_text_rect(&bar_rect, vp.scale);
                        let total_w = self
                            .address_bar_buffer
                            .layout_runs()
                            .next()
                            .map_or(0.0, |run| run.line_w);
                        let char_count = self.address_bar.url.chars().count().max(1) as f32;
                        let cursor_offset =
                            total_w * (self.address_bar.cursor_pos as f32 / char_count);
                        let caret_x = url_rect.x + cursor_offset * vp.scale;
                        let caret_y = url_rect.y + 2.0 * vp.scale;
                        let caret_h = url_rect.height - 4.0 * vp.scale;
                        let caret_color = [
                            self.ui_chrome.text_primary[0],
                            self.ui_chrome.text_primary[1],
                            self.ui_chrome.text_primary[2],
                            self.ui_chrome.cursor_alpha,
                        ];
                        self.quads
                            .push_quad(caret_x, caret_y, 1.5, caret_h, caret_color);
                    }

                    // Update address bar URL for the focused browser pane.
                    if vp.focused {
                        if let Some(sid) = active_sid {
                            if let Some(url) = self.browser_urls.get(&sid) {
                                self.address_bar.set_url(url);
                            }
                        }
                    }

                    if let Some(sid) = active_sid {
                        if mgr.get_panel(sid).is_some() {
                            // Position browser panel below the address bar.
                            let browser_rect = wmux_render::PaneRenderer::browser_viewport(vp);
                            let _ = mgr.resize_panel(sid, &browser_rect);
                            let _ = mgr.show_panel(sid);
                            // Track desired focus — only the focused pane's browser
                            // should receive Win32 keyboard focus (and not while
                            // editing the address bar).
                            if vp.focused && !self.address_bar.editing {
                                desired_browser_focus = Some(sid);
                            }
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

            // Apply browser focus transitions — only on state change, not every frame.
            if desired_browser_focus != self.browser_focus_target {
                if let Some(sid) = desired_browser_focus {
                    if let Some(panel) = mgr.get_panel(sid) {
                        let _ = panel.focus_webview();
                    }
                } else if self.browser_focus_target.is_some() {
                    // Reclaim Win32 keyboard focus from WebView2.
                    // SAFETY: SetFocus is a standard Win32 call. `main_hwnd` is valid
                    // for the lifetime of the window and we are on the UI/STA thread.
                    unsafe {
                        let _ = windows::Win32::UI::Input::KeyboardAndMouse::SetFocus(Some(
                            self.main_hwnd,
                        ));
                    }
                }
                self.browser_focus_target = desired_browser_focus;
            }

            // Remove orphaned browser panels — panels whose surface was closed
            // but whose WebView2 HWND is still alive. Without this cleanup, the
            // WS_POPUP HWND stays visible and frozen at its last position.
            let panel_ids = mgr.panel_ids();
            for sid in panel_ids {
                if !self.live_browser_sids.contains(&sid) {
                    tracing::info!(surface_id = %sid, "removing orphaned browser panel");
                    self.browser_urls.remove(&sid);
                    let _ = mgr.remove_panel(sid);
                }
            }
        }

        // Track the focused pane's active surface kind for keyboard routing.
        if let Some(focused_vp) = viewports.iter().find(|vp| vp.focused) {
            let kind = focused_vp
                .surface_types
                .get(focused_vp.active_tab)
                .copied()
                .unwrap_or(wmux_render::SurfaceType::Terminal);
            self.focused_surface_kind = match kind {
                wmux_render::SurfaceType::Browser => wmux_core::PanelKind::Browser,
                wmux_render::SurfaceType::Terminal => wmux_core::PanelKind::Terminal,
            };
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

        // Render terminal content for each pane (skip browser-active panes).
        for (pane_id, pane_rect) in &layout {
            // Always exclude the tab bar from terminal area (tab bar is always visible).
            let viewport = viewports.iter().find(|vp| vp.pane_id == *pane_id);
            let terminal_rect = viewport
                .map(wmux_render::PaneRenderer::terminal_viewport)
                .unwrap_or(*pane_rect);

            // Check if the active surface is a browser — skip wgpu rendering
            // in this area so the WebView2 child HWND is visible through the
            // transparent swap chain region.
            let active_is_browser = viewport
                .map(|vp| {
                    vp.surface_types
                        .get(vp.active_tab)
                        .copied()
                        .unwrap_or(wmux_render::SurfaceType::Terminal)
                        == wmux_render::SurfaceType::Browser
                })
                .unwrap_or(false);

            if active_is_browser {
                // Still consume render data to update focused pane metadata,
                // but don't render any wgpu content (no background, no text,
                // no cursor) — the WebView2 child HWND occupies this area.
                if let Some(data) = render_data_map.remove(pane_id) {
                    if *pane_id == self.focused_pane {
                        self.process_exited = data.process_exited;
                        self.terminal_modes = data.modes;
                    }
                }
                continue;
            }

            // Opaque background quad for the terminal area (including padding) —
            // one level lighter than the sidebar/chrome for visual hierarchy.
            // Use the pre-padding rect so the background extends behind the padding.
            {
                let tbh = wmux_render::pane::TAB_BAR_HEIGHT * self.scale_factor;
                let bg_rect = viewport
                    .map(|vp| {
                        wmux_core::rect::Rect::new(
                            vp.rect.x,
                            vp.rect.y + tbh,
                            vp.rect.width,
                            (vp.rect.height - tbh).max(0.0),
                        )
                    })
                    .unwrap_or(terminal_rect);
                self.quads.push_quad(
                    bg_rect.x,
                    bg_rect.y,
                    bg_rect.width,
                    bg_rect.height,
                    self.ui_chrome.surface_0,
                );
            }

            // Compute per-pane terminal dimensions from the padded terminal content rect.
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
                    self.terminal_font_family.as_deref(),
                    Some(self.terminal_font_size * self.scale_factor),
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
                    // Skip dimming for browser-active panes — the WebView2 child
                    // HWND manages its own visibility; drawing over it occludes it.
                    let is_browser = vp
                        .surface_types
                        .get(vp.active_tab)
                        .copied()
                        .unwrap_or(wmux_render::SurfaceType::Terminal)
                        == wmux_render::SurfaceType::Browser;
                    if is_browser {
                        continue;
                    }
                    let r = &vp.rect;
                    self.quads.push_quad(r.x, r.y, r.width, r.height, dim_color);
                }
            }
        }

        // Focus Glow — halo around the active pane.
        // Rendered AFTER pane backgrounds + dimming so the outer glow is visible
        // on top of adjacent panes (not hidden underneath their opaque backgrounds).
        if let Some(focused_vp) = viewports.iter().find(|vp| vp.focused) {
            let glow_alpha = self
                .focus_glow_anim
                .and_then(|id| self.animation.get(id))
                .unwrap_or(1.0);
            wmux_render::PaneRenderer::render_focus_glow(
                &mut self.quads,
                &focused_vp.rect,
                self.ui_chrome.accent_glow_core,
                self.ui_chrome.accent_glow,
                glow_alpha,
            );
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
                    &ui_attrs,
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
                    &ui_attrs,
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
            Some(ds) => self.dividers.iter().find(|d| d.split_id == ds.split_id),
            None => divider::hit_test(&self.dividers, cursor_x, cursor_y),
        };
        if let Some(div) = hovered_div {
            const DIV_HIGHLIGHT_THICKNESS: f32 = 2.0;
            // Use border_default (neutral) for hovered dividers.
            let div_color = self.ui_chrome.border_default;
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

        // Pane dividers: no permanent lines — gaps show surface_base.
        // Dividers only appear on hover (above) for resize affordance.

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

            self.status_bar.update_text(
                self.glyphon.font_system(),
                &self.status_bar_data,
                sb_w / self.scale_factor,
            );

            let time_secs = self.start_instant.elapsed().as_secs_f32();

            // Analytical shadow above status bar (shadow-sm, upward)
            {
                let sd = &self.ui_chrome.shadow_sm;
                self.shadows.push_shadow(
                    0.0,
                    sb_y,
                    sb_w,
                    status_bar_height,
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
                self.scale_factor,
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

        // Workspace context menu (renders on top of sidebar).
        if let super::WorkspaceMenuState::Open { menu_x, menu_y, .. } = self.workspace_menu {
            let item_h = 32.0;
            let menu_w = 200.0;
            let menu_items = super::WORKSPACE_MENU_ITEMS as f32;
            let menu_h = item_h * menu_items + 8.0;
            let menu_radius = 8.0;

            // Shadow
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

            // Background
            self.quads.push_rounded_quad(
                menu_x,
                menu_y,
                menu_w,
                menu_h,
                self.ui_chrome.surface_2,
                menu_radius,
            );

            // Border
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

            // Hover highlight
            if let Some(hover_idx) = self.workspace_menu_hover {
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
        }

        // Tab context menu (renders on top of tab bar).
        if let super::TabContextMenuState::Open { menu_x, menu_y, .. } = self.tab_menu {
            let item_h = 32.0;
            let menu_w = 200.0;
            let menu_items = super::TAB_MENU_ITEMS as f32;
            let menu_h = item_h * menu_items + 8.0;
            let menu_radius = 8.0;

            // Shadow
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

            // Background
            self.quads.push_rounded_quad(
                menu_x,
                menu_y,
                menu_w,
                menu_h,
                self.ui_chrome.surface_2,
                menu_radius,
            );

            // Border
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

            // Hover highlight
            if let Some(hover_idx) = self.tab_menu_hover {
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
        }

        // Notification panel overlay (right-side slide-out).
        //
        // Step 1: Fetch notifications from actor (only when panel is open).
        // Step 2: Shape text buffers for visible items.
        // Step 3: Render background/item quads.
        // Text areas are collected later in the text phase.
        if self.notification_panel.open {
            self.notification_cache = rt.block_on(app_state.list_notifications(50));

            let notif_refs: Vec<&wmux_core::Notification> =
                self.notification_cache.iter().collect();

            // Always reshape text buffers — timestamps are time-dependent ("just now" → "1 min ago").
            {
                let text_max_w = crate::notification_panel::PANEL_WIDTH
                    - crate::notification_panel::TEXT_LEFT_OFFSET
                    - 8.0;

                for (i, notif) in notif_refs
                    .iter()
                    .take(crate::notification_panel::MAX_VISIBLE_ITEMS)
                    .enumerate()
                {
                    // Category label (severity name).
                    let category_text = match notif.severity {
                        wmux_core::NotificationSeverity::Info => {
                            self.locale.t("notification.severity_info")
                        }
                        wmux_core::NotificationSeverity::Warning => {
                            self.locale.t("notification.severity_warning")
                        }
                        wmux_core::NotificationSeverity::Error => {
                            self.locale.t("notification.severity_error")
                        }
                        wmux_core::NotificationSeverity::Success => {
                            self.locale.t("notification.severity_success")
                        }
                    };
                    let cat_buf = &mut self.notif_category_buffers[i];
                    cat_buf.set_size(
                        self.glyphon.font_system(),
                        Some(text_max_w),
                        Some(typography::CAPTION_LINE_HEIGHT),
                    );
                    cat_buf.set_text(
                        self.glyphon.font_system(),
                        category_text,
                        &ui_attrs,
                        glyphon::Shaping::Advanced,
                        None,
                    );
                    cat_buf.shape_until_scroll(self.glyphon.font_system(), false);

                    // Title (bold).
                    let title_text = notif.title.as_deref().unwrap_or(&notif.body);
                    let title_buf = &mut self.notif_title_buffers[i];
                    title_buf.set_size(
                        self.glyphon.font_system(),
                        Some(text_max_w),
                        Some(typography::BODY_LINE_HEIGHT),
                    );
                    title_buf.set_text(
                        self.glyphon.font_system(),
                        title_text,
                        &ui_attrs_bold,
                        glyphon::Shaping::Advanced,
                        None,
                    );
                    title_buf.shape_until_scroll(self.glyphon.font_system(), false);

                    // Body text.
                    let body_text = if notif.title.is_some() {
                        notif.body.as_str()
                    } else {
                        notif.subtitle.as_deref().unwrap_or("")
                    };
                    let body_buf = &mut self.notif_body_buffers[i];
                    body_buf.set_size(
                        self.glyphon.font_system(),
                        Some(text_max_w),
                        Some(typography::CAPTION_LINE_HEIGHT),
                    );
                    body_buf.set_text(
                        self.glyphon.font_system(),
                        body_text,
                        &ui_attrs,
                        glyphon::Shaping::Advanced,
                        None,
                    );
                    body_buf.shape_until_scroll(self.glyphon.font_system(), false);

                    // Timestamp.
                    let time_text =
                        crate::notification_panel::format_time_ago(notif.timestamp, &self.locale);
                    let time_buf = &mut self.notif_time_buffers[i];
                    time_buf.set_size(
                        self.glyphon.font_system(),
                        Some(80.0),
                        Some(typography::CAPTION_LINE_HEIGHT),
                    );
                    time_buf.set_text(
                        self.glyphon.font_system(),
                        &time_text,
                        &ui_attrs,
                        glyphon::Shaping::Advanced,
                        None,
                    );
                    time_buf.shape_until_scroll(self.glyphon.font_system(), false);
                }
            }

            // Render panel quads (background, header, items, stripes, hover).
            self.notification_panel.render_quads(
                &mut self.quads,
                &notif_refs,
                surface_width as f32,
                surface_height as f32,
                &self.ui_chrome,
            );
        } else if !self.notification_cache.is_empty() {
            // Panel closed — clear cache.
            self.notification_cache.clear();
        }

        // Command palette overlay (fullscreen dim + palette chrome + text buffers).
        //
        // Step 1: Pre-compute search results + update result_count BEFORE render_quads
        // so the palette height includes the result rows on the very first frame.
        // Step 2: Render background/border/highlight quads.
        // Step 3: Update text buffers + push text cursor quad.
        {
            use crate::command_palette::{
                PaletteAction, PaletteFilter, INPUT_HEIGHT, INPUT_TEXT_PAD, MAX_VISIBLE_RESULTS,
                RESULT_HEIGHT, SHORTCUT_COL_WIDTH,
            };

            if self.command_palette.open {
                let sw = surface_width as f32;
                let sh = surface_height as f32;

                // Dirty check: skip expensive search + buffer updates when unchanged.
                let palette_dirty = self.palette_last_query != self.command_palette.query
                    || self.palette_last_filter != self.command_palette.filter;

                if palette_dirty {
                    self.palette_last_query
                        .clone_from(&self.command_palette.query);
                    self.palette_last_filter = self.command_palette.filter;

                    let ly_tmp = crate::command_palette::PaletteLayout::compute(sw, sh, 0);
                    let input_w = ly_tmp.input_w;

                    let query = &self.command_palette.query;
                    let filter = self.command_palette.filter;
                    let query_lower = self.command_palette.query.to_lowercase();

                    // --- Step 1: Build result rows + actions based on filter ---
                    // Each row: (name, shortcut_hint, action)
                    let mut rows: Vec<(String, String, PaletteAction)> =
                        Vec::with_capacity(MAX_VISIBLE_RESULTS);

                    let push_commands = |rows: &mut Vec<(String, String, PaletteAction)>| {
                        for r in self.command_registry.search(query) {
                            rows.push((
                                r.entry.name.clone(),
                                r.entry.shortcut.clone().unwrap_or_default(),
                                PaletteAction::Command(r.entry.id.clone()),
                            ));
                        }
                    };
                    let push_workspaces = |rows: &mut Vec<(String, String, PaletteAction)>| {
                        for (i, ws) in self.workspace_cache.iter().enumerate() {
                            if query.is_empty() || ws.name.to_lowercase().contains(&query_lower) {
                                let hint = if i < 9 {
                                    format!("Ctrl+{}", i + 1)
                                } else {
                                    String::new()
                                };
                                rows.push((
                                    ws.name.clone(),
                                    hint,
                                    PaletteAction::SwitchWorkspace((i + 1) as u8),
                                ));
                            }
                        }
                    };
                    let push_surfaces = |rows: &mut Vec<(String, String, PaletteAction)>| {
                        for vp in &viewports {
                            for (tab_idx, title) in vp.tab_titles.iter().enumerate() {
                                let display = if title.is_empty() {
                                    format!("Surface {}", tab_idx + 1)
                                } else {
                                    title.clone()
                                };
                                if query.is_empty() || display.to_lowercase().contains(&query_lower)
                                {
                                    rows.push((
                                        display,
                                        String::new(),
                                        PaletteAction::FocusSurface(vp.pane_id, tab_idx),
                                    ));
                                }
                            }
                        }
                    };

                    match filter {
                        PaletteFilter::Commands => push_commands(&mut rows),
                        PaletteFilter::Workspaces => push_workspaces(&mut rows),
                        PaletteFilter::Surfaces => push_surfaces(&mut rows),
                        PaletteFilter::All => {
                            push_commands(&mut rows);
                            push_workspaces(&mut rows);
                            push_surfaces(&mut rows);
                        }
                    }

                    let visible = rows.len().min(MAX_VISIBLE_RESULTS);
                    self.command_palette.set_result_count(visible);

                    // Store actions for the Enter handler.
                    self.palette_actions.clear();
                    self.palette_actions
                        .extend(rows.iter().take(visible).map(|(_, _, a)| a.clone()));

                    // Update text buffers (only when dirty).
                    // TODO(i18n): palette.search_placeholder
                    let query_display = if self.command_palette.query.is_empty() {
                        "Type a command\u{2026}"
                    } else {
                        &self.command_palette.query
                    };
                    let query_w = (input_w - INPUT_TEXT_PAD * 2.0).max(1.0);
                    self.palette_query_buffer.set_size(
                        self.glyphon.font_system(),
                        Some(query_w),
                        Some(INPUT_HEIGHT),
                    );
                    self.palette_query_buffer.set_text(
                        self.glyphon.font_system(),
                        query_display,
                        &ui_attrs,
                        glyphon::Shaping::Advanced,
                        None,
                    );
                    self.palette_query_buffer
                        .shape_until_scroll(self.glyphon.font_system(), false);

                    for (i, (name, shortcut, _)) in rows.iter().enumerate().take(visible) {
                        self.palette_result_buffers[i].set_size(
                            self.glyphon.font_system(),
                            Some((input_w - SHORTCUT_COL_WIDTH).max(1.0)),
                            Some(RESULT_HEIGHT),
                        );
                        self.palette_result_buffers[i].set_text(
                            self.glyphon.font_system(),
                            name,
                            &ui_attrs,
                            glyphon::Shaping::Advanced,
                            None,
                        );
                        self.palette_result_buffers[i]
                            .shape_until_scroll(self.glyphon.font_system(), false);

                        self.palette_shortcut_buffers[i].set_size(
                            self.glyphon.font_system(),
                            Some(SHORTCUT_COL_WIDTH),
                            Some(RESULT_HEIGHT),
                        );
                        self.palette_shortcut_buffers[i].set_text(
                            self.glyphon.font_system(),
                            shortcut,
                            &ui_attrs_light,
                            glyphon::Shaping::Advanced,
                            None,
                        );
                        self.palette_shortcut_buffers[i]
                            .shape_until_scroll(self.glyphon.font_system(), false);
                    }
                } // end dirty check

                // Render quads every frame (uses cached result_count).
                self.command_palette
                    .render_quads(&mut self.quads, sw, sh, &self.ui_chrome);

                // Text cursor in the input field (every frame).
                let ly = crate::command_palette::PaletteLayout::compute(
                    sw,
                    sh,
                    self.command_palette.result_count.min(MAX_VISIBLE_RESULTS),
                );
                {
                    let text_w = if self.command_palette.query.is_empty() {
                        0.0
                    } else {
                        self.palette_query_buffer
                            .layout_runs()
                            .next()
                            .map_or(0.0, |run| run.line_w)
                    };
                    let cursor_x = ly.input_x + INPUT_TEXT_PAD + text_w;
                    let cursor_y_pos = ly.input_y + (INPUT_HEIGHT - SEARCH_LINE_HEIGHT) / 2.0;
                    self.quads.push_quad(
                        cursor_x,
                        cursor_y_pos,
                        1.5,
                        SEARCH_LINE_HEIGHT,
                        self.ui_chrome.text_primary,
                    );
                }
            } else {
                // Palette closed — no overlay to render.
                self.palette_actions.clear();
            }
        }

        // Update address bar text buffer for focused browser pane.
        {
            let text: &str = if self.address_bar.url.is_empty() && !self.address_bar.editing {
                ""
            } else {
                &self.address_bar.url
            };
            // Use focused browser pane width if available, else a reasonable max.
            let buf_width = viewports
                .iter()
                .find(|vp| {
                    vp.focused
                        && vp.surface_types.get(vp.active_tab).copied()
                            == Some(wmux_render::SurfaceType::Browser)
                })
                .map(|vp| vp.rect.width)
                .unwrap_or(800.0);
            self.address_bar_buffer.set_size(
                self.glyphon.font_system(),
                Some(buf_width),
                Some(crate::address_bar::ADDRESS_BAR_HEIGHT),
            );
            self.address_bar_buffer.set_text(
                self.glyphon.font_system(),
                text,
                &ui_attrs,
                glyphon::Shaping::Advanced,
                None,
            );
            self.address_bar_buffer
                .shape_until_scroll(self.glyphon.font_system(), false);
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
            let buf = self.tab_edit_buffer.get_or_insert_with(|| {
                glyphon::Buffer::new(self.glyphon.font_system(), tab_metrics)
            });
            buf.set_metrics(self.glyphon.font_system(), tab_metrics);
            buf.set_size(
                self.glyphon.font_system(),
                Some(120.0),
                Some(TAB_LINE_HEIGHT),
            );
            buf.set_text(
                self.glyphon.font_system(),
                text,
                &ui_attrs,
                glyphon::Shaping::Advanced,
                None,
            );
            buf.shape_until_scroll(self.glyphon.font_system(), false);
        }

        for vp in &viewports {
            // Toggle mode: render segment labels + icons instead of pill tabs.
            if vp.is_toggle_mode() {
                if let Some(bufs) = self.toggle_label_buffers.get(&vp.pane_id) {
                    let active_seg =
                        wmux_render::pane::PaneRenderer::active_toggle_segment(vp).unwrap_or(0);
                    for (seg, label_buf) in bufs.iter().enumerate() {
                        if let Some((sx, sy, sw, sh)) =
                            wmux_render::pane::PaneRenderer::toggle_segment_rect(vp, seg)
                        {
                            let is_active = seg == active_seg;
                            let text_color = if is_active {
                                self.ui_chrome.text_inverse
                            } else {
                                self.ui_chrome.text_secondary
                            };

                            // Icon (terminal or globe).
                            let cg_ref = if seg == 0 {
                                &self.cg_terminal
                            } else {
                                &self.cg_globe
                            };
                            let s = self.scale_factor;
                            let icon_x = sx + 10.0 * s;
                            let icon_top = sy + (sh - TAB_LINE_HEIGHT) / 2.0;
                            let icon_size = 24.0 * s;
                            all_text_areas.push(glyphon::TextArea {
                                buffer: &self.icon_empty_buffer,
                                left: icon_x,
                                top: icon_top,
                                scale: s,
                                bounds: glyphon::TextBounds {
                                    left: (icon_x - 2.0 * s) as i32,
                                    top: (icon_top - 2.0 * s) as i32,
                                    right: (icon_x + icon_size) as i32,
                                    bottom: (icon_top + icon_size) as i32,
                                },
                                default_color: rgba_to_glyphon(text_color),
                                custom_glyphs: cg_ref,
                            });

                            // Label text ("Shell" / "Browser").
                            let text_left = icon_x + 20.0 * s;
                            all_text_areas.push(glyphon::TextArea {
                                buffer: label_buf,
                                left: text_left,
                                top: icon_top,
                                scale: s,
                                bounds: glyphon::TextBounds {
                                    left: text_left as i32,
                                    top: sy as i32,
                                    right: (sx + sw - 4.0 * s) as i32,
                                    bottom: (sy + sh) as i32,
                                },
                                default_color: rgba_to_glyphon(text_color),
                                custom_glyphs: &[],
                            });
                        }
                    }
                }
                continue;
            }
            // Precompute tab edit cursor offset via proportional interpolation.
            let tab_edit_cursor_offset: Option<f32> = if let super::TabEditState::Editing {
                ref text,
                cursor,
                pane_id: edit_pane,
                ..
            } = &self.tab_edit
            {
                if *edit_pane == vp.pane_id {
                    // Measure full text width from the edit buffer, then interpolate.
                    let total_w = self
                        .tab_edit_buffer
                        .as_ref()
                        .and_then(|eb| eb.layout_runs().next())
                        .map_or(0.0, |run| run.line_w);
                    let char_count = text.chars().count().max(1) as f32;
                    Some(total_w * (*cursor as f32 / char_count))
                } else {
                    None
                }
            } else {
                None
            };

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
                                // Match normal tab text offset past the type indicator icon.
                                let icon_reserve = 26.0;
                                let edit_left = tab_x + TAB_TEXT_PADDING + icon_reserve;
                                all_text_areas.push(glyphon::TextArea {
                                    buffer: edit_buf,
                                    left: edit_left,
                                    top: vp.rect.y + TAB_TEXT_TOP_OFFSET,
                                    scale: self.scale_factor,
                                    bounds: glyphon::TextBounds {
                                        left: edit_left as i32,
                                        top: vp.rect.y as i32,
                                        right: (tab_x + tab_width - TAB_TEXT_PADDING) as i32,
                                        bottom: (vp.rect.y + vp.tab_bar_height()) as i32,
                                    },
                                    default_color: rgba_to_glyphon(self.ui_chrome.text_primary),
                                    custom_glyphs: &[],
                                });

                                // Selection highlight when all text is selected.
                                if let super::TabEditState::Editing {
                                    selected_all: true, ..
                                } = &self.tab_edit
                                {
                                    self.quads.push_quad(
                                        edit_left,
                                        vp.rect.y + TAB_TEXT_TOP_OFFSET,
                                        tab_width
                                            - TAB_TEXT_PADDING
                                            - icon_reserve
                                            - TAB_TEXT_PADDING,
                                        TAB_LINE_HEIGHT,
                                        self.ui_chrome.accent_muted,
                                    );
                                }

                                // Edit cursor — use precomputed offset.
                                if let super::TabEditState::Editing { cursor, .. } = &self.tab_edit
                                {
                                    let cursor_offset = tab_edit_cursor_offset
                                        .unwrap_or(*cursor as f32 * TAB_FONT_SIZE * 0.6);
                                    // Multiply by scale — glyphon renders at glyph_x * scale.
                                    let cursor_x = edit_left + cursor_offset * self.scale_factor;
                                    let cursor_y = vp.rect.y + TAB_TEXT_TOP_OFFSET;
                                    let cursor_color = [
                                        self.ui_chrome.text_primary[0],
                                        self.ui_chrome.text_primary[1],
                                        self.ui_chrome.text_primary[2],
                                        self.ui_chrome.cursor_alpha,
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
                                    bottom: (vp.rect.y + vp.tab_bar_height()) as i32,
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
            if vp.is_toggle_mode() {
                // Toggle mode: single close button to the right of the toggle container.
                if let Some((bx, by, _bw, _bh)) =
                    wmux_render::pane::PaneRenderer::toggle_close_button_rect(vp)
                {
                    let is_hovered = self.tab_close_hover.is_some_and(|(hp, _)| hp == vp.pane_id);
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
            } else {
                // Pill mode: one close button per tab.
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
            }

            // "+" button — SVG add icon (skip for toggle-mode panes).
            if !vp.is_toggle_mode() {
                if let Some((px, py, pw, ph)) =
                    wmux_render::pane::PaneRenderer::plus_button_rect(vp)
                {
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
                if let Some((sx, sy, sw, sh)) =
                    wmux_render::pane::PaneRenderer::split_button_rect(vp)
                {
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

                // Globe button — open a new browser surface (only when WebView2 is available).
                if self.browser_manager.is_some() {
                    if let Some((gx, gy, gw, gh)) =
                        wmux_render::pane::PaneRenderer::globe_button_rect(vp)
                    {
                        all_text_areas.push(glyphon::TextArea {
                            buffer: &self.icon_empty_buffer,
                            left: gx + (gw - 16.0) / 2.0,
                            top: gy + (gh - 16.0) / 2.0,
                            scale: self.scale_factor,
                            bounds: glyphon::TextBounds {
                                left: gx as i32,
                                top: gy as i32,
                                right: (gx + gw) as i32,
                                bottom: (gy + gh) as i32,
                            },
                            default_color: rgba_to_glyphon(self.ui_chrome.text_secondary),
                            custom_glyphs: &self.cg_globe,
                        });
                    }
                }
            } // end !is_toggle_mode() guard for +, split, globe buttons
        }

        // Append sidebar text areas (workspace names + subtitles + icons).
        if self.sidebar.visible {
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
                &self.workspace_cache,
                &ws_status_icons,
                &self.icon_empty_buffer,
                &self.status_icon_cgs,
            ));
        }

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

        // Append address bar text area + back/forward icon glyphs for browser panes.
        for vp in &viewports {
            let active_type = vp
                .surface_types
                .get(vp.active_tab)
                .copied()
                .unwrap_or(wmux_render::SurfaceType::Terminal);
            if active_type != wmux_render::SurfaceType::Browser {
                continue;
            }
            let bar_rect = wmux_render::PaneRenderer::address_bar_rect(vp);
            let url_rect = crate::address_bar::AddressBarState::url_text_rect(&bar_rect, vp.scale);

            // URL text.
            let url_color = if self.address_bar.url.is_empty() {
                rgba_to_glyphon(self.ui_chrome.text_muted)
            } else {
                rgba_to_glyphon(self.ui_chrome.text_primary)
            };
            all_text_areas.push(glyphon::TextArea {
                buffer: &self.address_bar_buffer,
                left: url_rect.x,
                top: url_rect.y + (url_rect.height - crate::typography::CAPTION_LINE_HEIGHT) / 2.0,
                scale: vp.scale,
                bounds: glyphon::TextBounds {
                    left: url_rect.x as i32,
                    top: bar_rect.y as i32,
                    right: (url_rect.x + url_rect.width) as i32,
                    bottom: (bar_rect.y + bar_rect.height) as i32,
                },
                default_color: url_color,
                custom_glyphs: &[],
            });

            // Back button icon (arrow-left).
            let (back_cx, back_cy) =
                crate::address_bar::AddressBarState::back_button_center(&bar_rect, vp.scale);
            let icon_size = 14.0 * vp.scale;
            let icon_half = icon_size / 2.0;
            all_text_areas.push(glyphon::TextArea {
                buffer: &self.icon_empty_buffer,
                left: back_cx - icon_half,
                top: back_cy - icon_half,
                scale: vp.scale,
                bounds: glyphon::TextBounds {
                    left: (back_cx - icon_half - 2.0) as i32,
                    top: (back_cy - icon_half - 2.0) as i32,
                    right: (back_cx + icon_half + 2.0) as i32,
                    bottom: (back_cy + icon_half + 2.0) as i32,
                },
                default_color: rgba_to_glyphon(self.ui_chrome.text_secondary),
                custom_glyphs: &self.cg_arrows[1], // ArrowLeft
            });

            // Forward button icon (arrow-right).
            let (fwd_cx, fwd_cy) =
                crate::address_bar::AddressBarState::forward_button_center(&bar_rect, vp.scale);
            all_text_areas.push(glyphon::TextArea {
                buffer: &self.icon_empty_buffer,
                left: fwd_cx - icon_half,
                top: fwd_cy - icon_half,
                scale: vp.scale,
                bounds: glyphon::TextBounds {
                    left: (fwd_cx - icon_half - 2.0) as i32,
                    top: (fwd_cy - icon_half - 2.0) as i32,
                    right: (fwd_cx + icon_half + 2.0) as i32,
                    bottom: (fwd_cy + icon_half + 2.0) as i32,
                },
                default_color: rgba_to_glyphon(self.ui_chrome.text_secondary),
                custom_glyphs: &self.cg_arrows[0], // ArrowRight
            });
        }

        // Append status bar text area.
        all_text_areas.push(self.status_bar.text_area(
            0.0,
            sb_y,
            sb_w,
            &self.ui_chrome,
            self.scale_factor,
        ));

        // Append title bar text areas (title text + chrome button icons).
        if let Some(tb_text) =
            self.titlebar
                .text_area(&self.ui_chrome, surface_width as f32, self.scale_factor)
        {
            all_text_areas.push(tb_text);
            // Chrome button icons via SVG CustomGlyphs.
            all_text_areas.push(glyphon::TextArea {
                buffer: &self.icon_empty_buffer,
                left: 0.0,
                top: 0.0,
                scale: self.scale_factor,
                bounds: glyphon::TextBounds {
                    left: 0,
                    top: 0,
                    right: surface_width as i32,
                    bottom: (crate::titlebar::TITLE_BAR_HEIGHT * self.scale_factor) as i32,
                },
                default_color: crate::f32_to_glyphon_color(self.ui_chrome.text_secondary),
                custom_glyphs: &self.cg_chrome_buttons,
            });
        }

        // ── Overlay text areas (menus) ─────────────────────────────────
        // Track where overlay text starts so we can filter base text that
        // bleeds through opaque menu backgrounds (quad z < glyphon text z).
        let overlay_start = all_text_areas.len();

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

        // Append workspace context menu text areas.
        if let super::WorkspaceMenuState::Open { menu_x, menu_y, .. } = self.workspace_menu {
            let item_h = 32.0;
            let menu_w = 200.0;
            let label_x = menu_x + 12.0;
            let label_color = rgba_to_glyphon(self.ui_chrome.text_primary);
            for i in 0..super::WORKSPACE_MENU_ITEMS {
                let iy = menu_y + 4.0 + i as f32 * item_h;
                let ty = iy + (item_h - 18.0) / 2.0;
                all_text_areas.push(glyphon::TextArea {
                    buffer: &self.workspace_menu_buffers[i],
                    left: label_x,
                    top: ty,
                    scale: self.scale_factor,
                    bounds: glyphon::TextBounds {
                        left: label_x as i32,
                        top: iy as i32,
                        right: (menu_x + menu_w - 8.0) as i32,
                        bottom: (iy + item_h) as i32,
                    },
                    default_color: label_color,
                    custom_glyphs: &[],
                });
            }
        }

        // Append tab context menu text areas.
        if let super::TabContextMenuState::Open { menu_x, menu_y, .. } = self.tab_menu {
            let item_h = 32.0;
            let menu_w = 200.0;
            let label_x = menu_x + 12.0;
            let label_color = rgba_to_glyphon(self.ui_chrome.text_primary);
            for i in 0..super::TAB_MENU_ITEMS {
                let iy = menu_y + 4.0 + i as f32 * item_h;
                let ty = iy + (item_h - 18.0) / 2.0;
                all_text_areas.push(glyphon::TextArea {
                    buffer: &self.tab_menu_buffers[i],
                    left: label_x,
                    top: ty,
                    scale: self.scale_factor,
                    bounds: glyphon::TextBounds {
                        left: label_x as i32,
                        top: iy as i32,
                        right: (menu_x + menu_w - 8.0) as i32,
                        bottom: (iy + item_h) as i32,
                    },
                    default_color: label_color,
                    custom_glyphs: &[],
                });
            }
        }

        // Append command palette text areas (query + filter tabs + results + shortcuts).
        if self.command_palette.open {
            use crate::command_palette::{
                PaletteFilter, PaletteLayout, FILTER_ROW_HEIGHT, FILTER_TAB_GAP, FILTER_TAB_PAD_X,
                FILTER_TAB_PAD_Y, INPUT_HEIGHT, INPUT_TEXT_PAD, MAX_VISIBLE_RESULTS, RESULT_HEIGHT,
                SHORTCUT_COL_PAD, SHORTCUT_COL_WIDTH,
            };

            let sw = surface_w as f32;
            let sh = surface_h as f32;
            let ly = PaletteLayout::compute(
                sw,
                sh,
                self.command_palette.result_count.min(MAX_VISIBLE_RESULTS),
            );

            let caption_lh = typography::CAPTION_LINE_HEIGHT;

            // Query text area (inside the input field).
            let query_text_x = ly.input_x + INPUT_TEXT_PAD;
            let query_text_y = ly.input_y + (INPUT_HEIGHT - caption_lh) / 2.0;
            let query_color = if self.command_palette.query.is_empty() {
                rgba_to_glyphon(self.ui_chrome.text_muted)
            } else {
                rgba_to_glyphon(self.ui_chrome.text_primary)
            };
            all_text_areas.push(glyphon::TextArea {
                buffer: &self.palette_query_buffer,
                left: query_text_x,
                top: query_text_y,
                scale: self.scale_factor,
                bounds: glyphon::TextBounds {
                    left: query_text_x as i32,
                    top: ly.input_y as i32,
                    right: (ly.input_x + ly.input_w - INPUT_TEXT_PAD) as i32,
                    bottom: (ly.input_y + INPUT_HEIGHT) as i32,
                },
                default_color: query_color,
                custom_glyphs: &[],
            });

            // Filter tab labels.
            let mut tab_x = ly.input_x;
            for (i, variant) in PaletteFilter::ALL.iter().enumerate() {
                let pill_w = self.command_palette.measured_pill_width(*variant);
                let pill_h = FILTER_ROW_HEIGHT - 2.0 * FILTER_TAB_PAD_Y;
                let pill_y = ly.filter_y + FILTER_TAB_PAD_Y;

                // Center text inside the pill.
                let text_x = tab_x + FILTER_TAB_PAD_X;
                let text_y = pill_y + (pill_h - caption_lh) / 2.0;

                let tab_color = if *variant == self.command_palette.filter {
                    // Active tab: inverse text on accent background.
                    rgba_to_glyphon(self.ui_chrome.text_inverse)
                } else {
                    rgba_to_glyphon(self.ui_chrome.text_secondary)
                };

                all_text_areas.push(glyphon::TextArea {
                    buffer: &self.palette_filter_buffers[i],
                    left: text_x,
                    top: text_y,
                    scale: self.scale_factor,
                    bounds: glyphon::TextBounds {
                        left: tab_x as i32,
                        top: pill_y as i32,
                        right: (tab_x + pill_w) as i32,
                        bottom: (pill_y + pill_h) as i32,
                    },
                    default_color: tab_color,
                    custom_glyphs: &[],
                });

                tab_x += pill_w + FILTER_TAB_GAP;
            }

            // Result names + shortcut badges.
            let visible = ly.visible_results;
            let name_color = rgba_to_glyphon(self.ui_chrome.text_primary);
            let shortcut_color = rgba_to_glyphon(self.ui_chrome.text_muted);
            for i in 0..visible {
                let row_y = ly.results_y + i as f32 * RESULT_HEIGHT;
                let text_y = row_y + (RESULT_HEIGHT - caption_lh) / 2.0;

                // Command name (left-aligned).
                let shortcut_start = ly.input_x + ly.input_w - SHORTCUT_COL_WIDTH;
                all_text_areas.push(glyphon::TextArea {
                    buffer: &self.palette_result_buffers[i],
                    left: ly.input_x + INPUT_TEXT_PAD,
                    top: text_y,
                    scale: self.scale_factor,
                    bounds: glyphon::TextBounds {
                        left: ly.input_x as i32,
                        top: row_y as i32,
                        right: shortcut_start as i32,
                        bottom: (row_y + RESULT_HEIGHT) as i32,
                    },
                    default_color: name_color,
                    custom_glyphs: &[],
                });

                // Shortcut badge (right-aligned).
                all_text_areas.push(glyphon::TextArea {
                    buffer: &self.palette_shortcut_buffers[i],
                    left: shortcut_start + SHORTCUT_COL_PAD,
                    top: text_y,
                    scale: self.scale_factor,
                    bounds: glyphon::TextBounds {
                        left: shortcut_start as i32,
                        top: row_y as i32,
                        right: (ly.input_x + ly.input_w) as i32,
                        bottom: (row_y + RESULT_HEIGHT) as i32,
                    },
                    default_color: shortcut_color,
                    custom_glyphs: &[],
                });
            }
        }

        // Append notification panel text areas.
        // Buffers struct must outlive all_text_areas, so declare at this scope.
        let notif_buffers = crate::notification_panel::NotificationBuffers {
            header: &self.notif_header_buffer,
            clear_all: &self.notif_clear_all_buffer,
            empty: &self.notif_empty_buffer,
            categories: &self.notif_category_buffers,
            titles: &self.notif_title_buffers,
            bodies: &self.notif_body_buffers,
            timestamps: &self.notif_time_buffers,
        };
        if self.notification_panel.open {
            let refs: Vec<&wmux_core::Notification> = self.notification_cache.iter().collect();
            all_text_areas.extend(self.notification_panel.text_areas(
                &refs,
                surface_w as f32,
                surface_h as f32,
                self.scale_factor,
                &self.ui_chrome,
                &notif_buffers,
            ));
        }

        // Filter base text areas that overlap with open overlay menus.
        // The render pipeline is: quads (painter's algo) → glyphon text (single pass).
        // Menu background quads are opaque, but underlying text (tab titles, sidebar)
        // renders AFTER all quads, bleeding through menu backgrounds.
        // Fix: remove base text areas whose bounds intersect any open menu rect.
        {
            let mut overlay_rects = [(0.0f32, 0.0f32, 0.0f32, 0.0f32); 4];
            let mut overlay_count = 0usize;
            if let super::SplitMenuState::Open { menu_x, menu_y, .. } = self.split_menu {
                let mh = 4.0 * 32.0 + 8.0;
                overlay_rects[overlay_count] = (menu_x, menu_y, 240.0, mh);
                overlay_count += 1;
            }
            if let super::WorkspaceMenuState::Open { menu_x, menu_y, .. } = self.workspace_menu {
                let mh = super::WORKSPACE_MENU_ITEMS as f32 * 32.0 + 8.0;
                overlay_rects[overlay_count] = (menu_x, menu_y, 200.0, mh);
                overlay_count += 1;
            }
            if let super::TabContextMenuState::Open { menu_x, menu_y, .. } = self.tab_menu {
                let mh = super::TAB_MENU_ITEMS as f32 * 32.0 + 8.0;
                overlay_rects[overlay_count] = (menu_x, menu_y, 200.0, mh);
                overlay_count += 1;
            }
            if self.command_palette.open {
                use crate::command_palette::{PaletteLayout, MAX_VISIBLE_RESULTS};
                let ly = PaletteLayout::compute(
                    surface_w as f32,
                    surface_h as f32,
                    self.command_palette.result_count.min(MAX_VISIBLE_RESULTS),
                );
                overlay_rects[overlay_count] = (
                    ly.palette_x,
                    ly.palette_y,
                    ly.effective_width,
                    ly.total_height,
                );
                overlay_count += 1;
            }

            if overlay_count > 0 {
                let active_rects = &overlay_rects[..overlay_count];
                // Terminal panes emit one TextArea per row sharing pane-wide bounds,
                // so `ta.bounds` covers the whole pane. Use `ta.top` + the real cell
                // height for the vertical test so only rows actually under an overlay
                // get dropped (not the whole pane, and not extra rows nearby).
                let row_h = self.metrics.cell_height;
                let mut idx = 0;
                all_text_areas.retain(|ta| {
                    let is_overlay = idx >= overlay_start;
                    idx += 1;
                    if is_overlay {
                        return true; // Keep overlay text (menu labels).
                    }
                    let tl = ta.bounds.left as f32;
                    let tr = ta.bounds.right as f32;
                    let tt = ta.top;
                    let tb = ta.top + row_h;
                    !active_rects
                        .iter()
                        .any(|&(mx, my, mw, mh)| tl < mx + mw && tr > mx && tt < my + mh && tb > my)
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
        let scaled_tbh = wmux_render::pane::TAB_BAR_HEIGHT * self.scale_factor;
        let tab_bar_h = scaled_tbh as f64;
        let (origin_x, origin_y, pane_cols, pane_rows) = self
            .last_layout
            .iter()
            .find(|(id, _)| *id == self.focused_pane)
            .map(|(_, rect)| {
                let content_height = (rect.height - scaled_tbh).max(0.0);
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
