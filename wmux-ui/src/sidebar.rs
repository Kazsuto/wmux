use glyphon::{Attrs, Buffer, Family, Metrics, Shaping, TextArea, TextBounds};
use wmux_config::UiChrome;
use wmux_core::WorkspaceSnapshot;
use wmux_render::QuadPipeline;

use crate::f32_to_glyphon_color;
use crate::typography;

/// Sidebar workspace names — uses Body token.
const SIDEBAR_FONT_SIZE: f32 = typography::BODY_FONT_SIZE;
const SIDEBAR_LINE_HEIGHT: f32 = typography::BODY_LINE_HEIGHT;
/// Height of each workspace row in pixels (includes card + gap).
pub const ROW_HEIGHT: f32 = 110.0;
const PADDING_X: f32 = 12.0;
const PADDING_Y: f32 = 10.0;
/// Info text below workspace name — uses Caption token.
const INFO_FONT_SIZE: f32 = typography::CAPTION_FONT_SIZE;
const INFO_LINE_HEIGHT: f32 = typography::CAPTION_LINE_HEIGHT;
/// Maximum info lines below the workspace name (status + git + cwd).
const MAX_INFO_LINES: f32 = 3.0;
/// Maximum listening ports displayed per workspace.
const MAX_DISPLAY_PORTS: usize = 5;
const ACCENT_BAR_WIDTH: f32 = 3.0;
/// Card visual design — rounded backgrounds per workspace.
const CARD_CORNER_RADIUS: f32 = 8.0;
const CARD_MARGIN_X: f32 = 6.0;
const CARD_GAP: f32 = 4.0;
/// Height of section header (e.g. "WORKSPACES").
const SECTION_HEADER_HEIGHT: f32 = 42.0;
/// Minimum pixel distance before a press becomes a drag.
const DRAG_THRESHOLD: f32 = 5.0;
/// Notification badge diameter.
const BADGE_SIZE: f32 = 18.0;
/// Port badge pill height.
const PORT_PILL_HEIGHT: f32 = 18.0;
/// Horizontal padding inside a port pill.
const PORT_PILL_PADDING_X: f32 = 6.0;
/// Gap between adjacent port pills.
const PORT_PILL_GAP: f32 = 4.0;
/// Corner radius for port pills (fully rounded).
const PORT_PILL_RADIUS: f32 = PORT_PILL_HEIGHT / 2.0;
/// Background alpha for port pill quads (translucent color wash).
const PORT_PILL_BG_ALPHA: f32 = 0.15;
/// Approximate average glyph advance ratio for Segoe UI at badge font size.
const BADGE_CHAR_WIDTH_RATIO: f32 = 0.62;
/// Hit zone width for the resize handle on the sidebar right edge.
const RESIZE_HIT_ZONE: f32 = 5.0;
/// Minimum sidebar width when resizing.
pub const MIN_SIDEBAR_WIDTH: f32 = 180.0;
/// Maximum sidebar width when resizing.
pub const MAX_SIDEBAR_WIDTH: f32 = 480.0;
/// Fixed width of the sidebar in collapsed (icon-only) mode.
pub const COLLAPSED_WIDTH: f32 = 48.0;
/// Row height in collapsed mode (one icon per workspace).
const COLLAPSED_ROW_HEIGHT: f32 = 48.0;
/// Diameter of the workspace color circle in collapsed mode.
const COLLAPSED_ICON_SIZE: f32 = 28.0;
/// Top padding before first collapsed icon row.
const COLLAPSED_TOP_PADDING: f32 = 8.0;

/// Sidebar interaction state.
#[derive(Debug, Clone)]
pub enum SidebarInteraction {
    /// No interaction.
    Idle,
    /// Mouse hovering over a workspace row.
    Hover(usize),
    /// Mouse pressed, not yet dragging (tracking for click vs drag).
    Pressing { row: usize, start_y: f32 },
    /// Actively dragging a workspace row.
    Dragging { from_row: usize, current_y: f32 },
    /// Inline editing a workspace name.
    Editing {
        index: usize,
        text: String,
        cursor: usize,
        selected_all: bool,
    },
    /// Mouse hovering over the resize edge.
    ResizeHover,
    /// Actively resizing the sidebar width.
    Resizing { start_x: f32, start_width: f32 },
}

/// Sidebar panel state — visibility, width, text buffers, and interaction.
pub struct SidebarState {
    pub visible: bool,
    pub width: f32,
    /// Whether the sidebar is in collapsed (icon-only) mode.
    pub collapsed: bool,
    /// Vertical offset for the sidebar content (title bar height when custom chrome is active).
    pub top_offset: f32,
    /// Current mouse interaction state.
    pub interaction: SidebarInteraction,
    /// One glyphon Buffer per workspace row for the name label.
    name_buffers: Vec<Buffer>,
    /// One glyphon Buffer per workspace row for environment info (pane count, git, cwd, ports).
    info_buffers: Vec<Buffer>,
    /// One glyphon Buffer per workspace row for notification badge count.
    badge_buffers: Vec<Buffer>,
    /// Port pill text buffers — outer Vec per workspace, inner Vec per visible port.
    port_buffers: Vec<Vec<Buffer>>,
    /// Glyphon Buffer for the inline editing text.
    edit_buffer: Option<Buffer>,
    /// Precomputed cursor X offset (relative to text_x) from glyphon layout runs.
    edit_cursor_x_offset: f32,
    /// Glyphon Buffer for the section header label.
    header_buffer: Option<Buffer>,
    /// Number of workspaces rendered last frame (to detect changes).
    last_workspace_count: usize,
    /// Names hash from last frame (quick change detection).
    last_names_hash: u64,
}

impl SidebarState {
    /// Create a new sidebar with the given width in pixels.
    pub fn new(width: u16) -> Self {
        let clamped = (width as f32).clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH);
        Self {
            visible: true,
            width: clamped,
            collapsed: false,
            top_offset: 0.0,
            interaction: SidebarInteraction::Idle,
            name_buffers: Vec::new(),
            info_buffers: Vec::new(),
            badge_buffers: Vec::new(),
            port_buffers: Vec::new(),
            edit_buffer: None,
            edit_cursor_x_offset: 0.0,
            header_buffer: None,
            last_workspace_count: 0,
            last_names_hash: 0,
        }
    }

    /// Toggle sidebar visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        // Reset interaction to prevent stale states (e.g. ResizeHover) on hidden sidebar.
        self.interaction = SidebarInteraction::Idle;
        tracing::debug!(visible = self.visible, "sidebar toggled");
    }

    /// Toggle between collapsed (icon-only) and expanded modes.
    pub fn toggle_collapsed(&mut self) {
        self.collapsed = !self.collapsed;
        self.interaction = SidebarInteraction::Idle;
        tracing::debug!(collapsed = self.collapsed, "sidebar collapsed toggled");
    }

    /// Effective width: collapsed width when collapsed, full width when expanded, 0 when hidden.
    pub fn effective_width(&self) -> f32 {
        if !self.visible {
            0.0
        } else if self.collapsed {
            COLLAPSED_WIDTH
        } else {
            self.width
        }
    }

    /// Row height: smaller in collapsed mode.
    pub fn row_height(&self) -> f32 {
        if self.collapsed {
            COLLAPSED_ROW_HEIGHT
        } else {
            ROW_HEIGHT
        }
    }

    /// Test if x is within the resize handle zone at the right edge.
    pub fn hit_test_resize_edge(&self, px: f32) -> bool {
        if !self.visible || self.collapsed {
            return false;
        }
        let edge = self.width;
        // Mostly inside sidebar (5px) with minimal overshoot (2px) to avoid
        // conflicting with pane divider hit zones in the content area.
        px >= edge - RESIZE_HIT_ZONE && px <= edge + 2.0
    }

    /// Clamp width to valid range.
    pub fn clamp_width(&mut self) {
        self.width = self.width.clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH);
    }

    /// Test if a y coordinate hits a workspace row.
    ///
    /// Returns the 0-based workspace index, or `None` if out of range.
    pub fn hit_test_row(&self, y: f32, workspace_count: usize) -> Option<usize> {
        let content_top = self.top_offset
            + if self.collapsed {
                COLLAPSED_TOP_PADDING
            } else {
                SECTION_HEADER_HEIGHT
            };
        if y < content_top {
            return None;
        }
        let adjusted_y = y - content_top;
        let rh = self.row_height();
        let index = (adjusted_y / rh) as usize;
        if index < workspace_count {
            Some(index)
        } else {
            None
        }
    }

    /// Compute the drop target index during a drag.
    ///
    /// Returns the index where the dragged workspace would be inserted,
    /// based on whether the cursor is in the top or bottom half of a row.
    pub fn drag_target_index(&self, current_y: f32, workspace_count: usize) -> usize {
        let content_top = self.top_offset
            + if self.collapsed {
                COLLAPSED_TOP_PADDING
            } else {
                SECTION_HEADER_HEIGHT
            };
        let adjusted_y = current_y - content_top;
        let row_f = adjusted_y / self.row_height();
        let row = row_f as usize;
        let frac = row_f - row as f32;
        if row >= workspace_count {
            workspace_count.saturating_sub(1)
        } else if frac > 0.5 {
            (row + 1).min(workspace_count.saturating_sub(1))
        } else {
            row
        }
    }

    /// Check whether a press should transition to a drag.
    pub fn should_start_drag(&self, current_y: f32) -> bool {
        if let SidebarInteraction::Pressing { start_y, .. } = self.interaction {
            (current_y - start_y).abs() >= DRAG_THRESHOLD
        } else {
            false
        }
    }

    /// Returns true if the sidebar is in inline editing mode.
    pub fn is_editing(&self) -> bool {
        matches!(self.interaction, SidebarInteraction::Editing { .. })
    }

    /// Update the edit buffer text for rendering.
    pub fn update_edit_buffer(&mut self, font_system: &mut glyphon::FontSystem) {
        if let SidebarInteraction::Editing {
            ref text, cursor, ..
        } = self.interaction
        {
            // Single-line edit: use unlimited width to prevent wrapping.
            // Visual clipping is handled by the TextArea bounds.
            let metrics = Metrics::new(SIDEBAR_FONT_SIZE, SIDEBAR_LINE_HEIGHT);
            let attrs = Attrs::new().family(Family::Name("Segoe UI"));

            // Set edit_buffer with full text for rendering.
            let buf = self
                .edit_buffer
                .get_or_insert_with(|| Buffer::new(font_system, metrics));
            buf.set_metrics(font_system, metrics);
            buf.set_size(font_system, Some(10000.0), Some(SIDEBAR_LINE_HEIGHT));
            buf.set_text(font_system, text, &attrs, Shaping::Advanced, None);
            buf.shape_until_scroll(font_system, false);

            // Cursor X via proportional interpolation:
            // total_line_width * (cursor_position / char_count).
            // Pure math — cannot get stuck regardless of font shaping quirks.
            let total_w = buf.layout_runs().next().map_or(0.0, |run| run.line_w);
            let char_count = text.chars().count().max(1) as f32;
            self.edit_cursor_x_offset = total_w * (cursor as f32 / char_count);
        }
    }

    /// Update text buffers from the current workspace list.
    ///
    /// Only reshapes buffers when workspace data actually changes (name, count, active state).
    pub fn update_text(
        &mut self,
        workspaces: &[WorkspaceSnapshot],
        font_system: &mut glyphon::FontSystem,
    ) {
        // Also update edit buffer if editing.
        self.update_edit_buffer(font_system);

        // Quick hash to detect changes.
        let hash = compute_hash(workspaces);
        if hash == self.last_names_hash && workspaces.len() == self.last_workspace_count {
            return;
        }
        self.last_names_hash = hash;
        self.last_workspace_count = workspaces.len();

        let text_width = self.width - PADDING_X * 2.0 - ACCENT_BAR_WIDTH;
        let metrics = Metrics::new(SIDEBAR_FONT_SIZE, SIDEBAR_LINE_HEIGHT);
        let attrs_bold = Attrs::new()
            .family(Family::Name("Segoe UI"))
            .weight(glyphon::Weight(600));
        let attrs = Attrs::new().family(Family::Name("Segoe UI"));

        // Section header buffer (small text for "WORKSPACES")
        let header_metrics = Metrics::new(
            typography::CAPTION_FONT_SIZE,
            typography::CAPTION_LINE_HEIGHT,
        );
        let header_buf = self
            .header_buffer
            .get_or_insert_with(|| Buffer::new(font_system, header_metrics));
        header_buf.set_metrics(font_system, header_metrics);
        header_buf.set_size(font_system, Some(text_width.max(1.0)), Some(16.0));
        header_buf.set_text(font_system, "WORKSPACES", &attrs, Shaping::Advanced, None);
        header_buf.shape_until_scroll(font_system, false);

        // Info text metrics (caption size for environment details).
        let info_metrics = Metrics::new(INFO_FONT_SIZE, INFO_LINE_HEIGHT);

        // Resize buffer vecs to match workspace count.
        self.name_buffers
            .resize_with(workspaces.len(), || Buffer::new(font_system, metrics));
        self.info_buffers
            .resize_with(workspaces.len(), || Buffer::new(font_system, info_metrics));

        for (i, ws) in workspaces.iter().enumerate() {
            // Name buffer
            let buf = &mut self.name_buffers[i];
            buf.set_metrics(font_system, metrics);
            buf.set_size(
                font_system,
                Some(text_width.max(1.0)),
                Some(SIDEBAR_LINE_HEIGHT),
            );
            buf.set_text(font_system, &ws.name, &attrs_bold, Shaping::Advanced, None);
            buf.shape_until_scroll(font_system, false);

            // Info buffer (pane count + git branch + cwd + ports)
            let info = &mut self.info_buffers[i];
            info.set_metrics(font_system, info_metrics);
            info.set_size(
                font_system,
                Some(text_width.max(1.0)),
                Some(INFO_LINE_HEIGHT * MAX_INFO_LINES),
            );
            let info_text = build_info_text(ws);
            info.set_text(font_system, &info_text, &attrs, Shaping::Advanced, None);
            info.shape_until_scroll(font_system, false);
        }

        // Badge buffers (notification count per workspace).
        let badge_metrics =
            Metrics::new(typography::BADGE_FONT_SIZE, typography::BADGE_LINE_HEIGHT);
        let badge_attrs = Attrs::new()
            .family(Family::Name("Segoe UI"))
            .weight(glyphon::Weight::BOLD);
        self.badge_buffers
            .resize_with(workspaces.len(), || Buffer::new(font_system, badge_metrics));
        for (i, ws) in workspaces.iter().enumerate() {
            let buf = &mut self.badge_buffers[i];
            if ws.unread_count > 0 {
                let count_text = if ws.unread_count > 99 {
                    "99+".to_string()
                } else {
                    ws.unread_count.to_string()
                };
                buf.set_metrics(font_system, badge_metrics);
                buf.set_size(font_system, Some(BADGE_SIZE), Some(BADGE_SIZE));
                buf.set_text(
                    font_system,
                    &count_text,
                    &badge_attrs,
                    Shaping::Advanced,
                    None,
                );
                buf.shape_until_scroll(font_system, false);
            }
        }

        // Port pill buffers — one Buffer per visible port per workspace.
        let port_attrs = Attrs::new()
            .family(Family::Name("Segoe UI"))
            .weight(glyphon::Weight(600));
        self.port_buffers.resize_with(workspaces.len(), Vec::new);
        for (i, ws) in workspaces.iter().enumerate() {
            let port_count = ws.ports.len().min(MAX_DISPLAY_PORTS);
            let bufs = &mut self.port_buffers[i];
            bufs.resize_with(port_count, || Buffer::new(font_system, badge_metrics));
            for (j, port) in ws.ports.iter().take(MAX_DISPLAY_PORTS).enumerate() {
                let text = format!(":{port}");
                let pill_w = port_pill_width(*port);
                let buf = &mut bufs[j];
                buf.set_metrics(font_system, badge_metrics);
                buf.set_size(font_system, Some(pill_w), Some(PORT_PILL_HEIGHT));
                buf.set_text(font_system, &text, &port_attrs, Shaping::Advanced, None);
                buf.shape_until_scroll(font_system, false);
            }
        }

        // Truncate if workspace count decreased.
        self.name_buffers.truncate(workspaces.len());
        self.info_buffers.truncate(workspaces.len());
        self.badge_buffers.truncate(workspaces.len());
        self.port_buffers.truncate(workspaces.len());
    }

    /// Port badge color palette — cycled per port index.
    fn port_palette(ui_chrome: &UiChrome) -> [[f32; 4]; 5] {
        [
            ui_chrome.accent,     // blue
            ui_chrome.success,    // green
            ui_chrome.warning,    // yellow
            ui_chrome.dot_purple, // purple
            ui_chrome.dot_cyan,   // cyan
        ]
    }

    /// Workspace identity color palette — cycled per workspace index.
    fn workspace_palette(ui_chrome: &UiChrome) -> [[f32; 4]; 6] {
        [
            ui_chrome.accent,     // blue
            ui_chrome.success,    // green
            ui_chrome.warning,    // yellow
            ui_chrome.error,      // red
            ui_chrome.dot_purple, // purple
            ui_chrome.dot_cyan,   // cyan
        ]
    }

    /// Push sidebar quads into the quad pipeline.
    pub fn render_quads(
        &self,
        workspaces: &[WorkspaceSnapshot],
        quad_pipeline: &mut QuadPipeline,
        surface_height: f32,
        ui_chrome: &UiChrome,
        _scale_factor: f32,
    ) {
        if !self.visible {
            return;
        }

        let w = self.effective_width();
        let dot_palette = Self::workspace_palette(ui_chrome);

        let y0 = self.top_offset;

        // ── Collapsed mode: icon-only rendering ─────────────────────────────
        if self.collapsed {
            // Background + separator (starts at top_offset so title bar covers the gap).
            quad_pipeline.push_quad(0.0, y0, w, surface_height - y0, ui_chrome.surface_1);
            quad_pipeline.push_quad(
                w - 1.0,
                y0,
                1.0,
                surface_height - y0,
                ui_chrome.border_subtle,
            );

            let icon_r = COLLAPSED_ICON_SIZE / 2.0;
            let center_x = w / 2.0;

            for (i, ws) in workspaces.iter().enumerate() {
                let row_y = y0 + COLLAPSED_TOP_PADDING + i as f32 * COLLAPSED_ROW_HEIGHT;
                let center_y = row_y + COLLAPSED_ROW_HEIGHT / 2.0;
                let icon_x = center_x - icon_r;
                let icon_y = center_y - icon_r;
                let ws_color = dot_palette[i % dot_palette.len()];

                let is_hover = matches!(self.interaction, SidebarInteraction::Hover(h) if h == i);

                if ws.active {
                    // Active: filled circle with full opacity
                    quad_pipeline.push_rounded_quad(
                        icon_x,
                        icon_y,
                        COLLAPSED_ICON_SIZE,
                        COLLAPSED_ICON_SIZE,
                        ws_color,
                        icon_r,
                    );
                } else if is_hover {
                    // Hover: translucent circle
                    let hover_color = [ws_color[0], ws_color[1], ws_color[2], 0.5];
                    quad_pipeline.push_rounded_quad(
                        icon_x,
                        icon_y,
                        COLLAPSED_ICON_SIZE,
                        COLLAPSED_ICON_SIZE,
                        hover_color,
                        icon_r,
                    );
                } else {
                    // Inactive: dim circle
                    let dim_color = [ws_color[0], ws_color[1], ws_color[2], 0.25];
                    quad_pipeline.push_rounded_quad(
                        icon_x,
                        icon_y,
                        COLLAPSED_ICON_SIZE,
                        COLLAPSED_ICON_SIZE,
                        dim_color,
                        icon_r,
                    );
                }
            }
            return;
        }

        // ── Expanded mode ───────────────────────────────────────────────────
        let hover_color = [
            ui_chrome.surface_2[0],
            ui_chrome.surface_2[1],
            ui_chrome.surface_2[2],
            0.5,
        ];

        // Background quad (starts at top_offset so title bar covers the gap).
        quad_pipeline.push_quad(0.0, y0, w, surface_height - y0, ui_chrome.surface_1);

        // Separator line on right edge (neutral, subtle)
        quad_pipeline.push_quad(
            w - 1.0,
            y0,
            1.0,
            surface_height - y0,
            ui_chrome.border_subtle,
        );

        // Section header area
        quad_pipeline.push_quad(0.0, y0, w, SECTION_HEADER_HEIGHT, ui_chrome.surface_1);

        // Workspace card rows (offset by header height + top_offset)
        for (i, ws) in workspaces.iter().enumerate() {
            let y = y0 + SECTION_HEADER_HEIGHT + i as f32 * ROW_HEIGHT;

            // Card geometry
            let card_x = CARD_MARGIN_X;
            let card_y = y + CARD_GAP / 2.0;
            let card_w = w - CARD_MARGIN_X * 2.0 - 1.0; // -1 for separator
            let card_h = ROW_HEIGHT - CARD_GAP;

            // Workspace identity color (used for icon tint, accent bar).
            let ws_color = dot_palette[i % dot_palette.len()];

            let is_hover = matches!(self.interaction, SidebarInteraction::Hover(h) if h == i)
                && !ws.active
                && !matches!(self.interaction, SidebarInteraction::Editing { index, .. } if index == i);

            if ws.active {
                // Active card: accent-tinted background
                quad_pipeline.push_rounded_quad(
                    card_x,
                    card_y,
                    card_w,
                    card_h,
                    ui_chrome.accent_muted,
                    CARD_CORNER_RADIUS,
                );
                // Accent bar on left edge of card — workspace color
                quad_pipeline.push_rounded_quad(
                    card_x,
                    card_y + 4.0,
                    ACCENT_BAR_WIDTH,
                    card_h - 8.0,
                    ws_color,
                    2.0,
                );
            } else if is_hover {
                // Hover card: surface_2 lift
                quad_pipeline.push_rounded_quad(
                    card_x,
                    card_y,
                    card_w,
                    card_h,
                    hover_color,
                    CARD_CORNER_RADIUS,
                );
            } else {
                // Inactive card: subtle lift above sidebar bg
                quad_pipeline.push_rounded_quad(
                    card_x,
                    card_y,
                    card_w,
                    card_h,
                    ui_chrome.surface_0,
                    CARD_CORNER_RADIUS,
                );
            }

            // Notification badge (right side of card, top-aligned with name)
            if ws.unread_count > 0 {
                let badge_x = card_x + card_w - BADGE_SIZE - PADDING_X;
                let badge_y = card_y + PADDING_Y;
                quad_pipeline.push_rounded_quad(
                    badge_x,
                    badge_y,
                    BADGE_SIZE,
                    BADGE_SIZE,
                    ui_chrome.accent,
                    BADGE_SIZE / 2.0,
                );
            }

            // Port pill badges (bottom of card)
            if !ws.ports.is_empty() {
                let port_colors = Self::port_palette(ui_chrome);
                let pill_start_x = card_x + ACCENT_BAR_WIDTH + PADDING_X;
                let pill_y = card_y + card_h - PADDING_Y - PORT_PILL_HEIGHT;
                let avail_w = card_w - ACCENT_BAR_WIDTH - PADDING_X - PADDING_X;
                let mut cur_x = pill_start_x;
                for (j, port) in ws.ports.iter().take(MAX_DISPLAY_PORTS).enumerate() {
                    let pill_w = port_pill_width(*port);
                    if cur_x + pill_w - pill_start_x > avail_w {
                        break;
                    }
                    let base = port_colors[j % port_colors.len()];
                    let bg = [base[0], base[1], base[2], PORT_PILL_BG_ALPHA];
                    quad_pipeline.push_rounded_quad(
                        cur_x,
                        pill_y,
                        pill_w,
                        PORT_PILL_HEIGHT,
                        bg,
                        PORT_PILL_RADIUS,
                    );
                    cur_x += pill_w + PORT_PILL_GAP;
                }
            }

            // Editing mode: draw input box background + border + selection highlight
            if let SidebarInteraction::Editing {
                index,
                selected_all,
                ..
            } = &self.interaction
            {
                if *index == i {
                    let edit_x = card_x + ACCENT_BAR_WIDTH + PADDING_X;
                    let edit_y = card_y + PADDING_Y - 4.0;
                    let edit_w = card_w - ACCENT_BAR_WIDTH - PADDING_X * 2.0;
                    let edit_h = SIDEBAR_LINE_HEIGHT * 2.0;
                    quad_pipeline.push_rounded_quad(
                        edit_x,
                        edit_y,
                        edit_w,
                        edit_h,
                        ui_chrome.surface_base,
                        4.0,
                    );
                    // Selection highlight when all text is selected.
                    if *selected_all {
                        quad_pipeline.push_quad(
                            edit_x + 4.0,
                            edit_y + 2.0,
                            edit_w - 8.0,
                            edit_h - 4.0,
                            ui_chrome.accent_muted,
                        );
                    }
                    quad_pipeline.push_rounded_quad(
                        edit_x,
                        edit_y,
                        edit_w,
                        1.0,
                        ui_chrome.accent,
                        4.0,
                    );
                    quad_pipeline.push_rounded_quad(
                        edit_x,
                        edit_y + edit_h - 1.0,
                        edit_w,
                        1.0,
                        ui_chrome.accent,
                        4.0,
                    );
                }
            }
        }

        // Drag: drop indicator line (offset by header height)
        if let SidebarInteraction::Dragging {
            current_y,
            from_row,
        } = &self.interaction
        {
            let target = self.drag_target_index(*current_y, workspaces.len());
            if target != *from_row {
                let indicator_y = SECTION_HEADER_HEIGHT + target as f32 * ROW_HEIGHT;
                quad_pipeline.push_quad(
                    CARD_MARGIN_X,
                    indicator_y - 1.0,
                    self.width - CARD_MARGIN_X * 2.0 - 1.0,
                    2.0,
                    ui_chrome.accent,
                );
            }
        }
    }

    /// Produce TextArea descriptors for the sidebar text labels.
    ///
    /// Must be called after `update_text()`. The returned text areas should be
    /// appended to the terminal text areas before calling `prepare_text_areas`.
    #[expect(
        clippy::too_many_arguments,
        reason = "sidebar rendering requires theme, scale, icons, and status data — a config struct would add indirection without benefit"
    )]
    pub fn text_areas<'a>(
        &'a self,
        _surface_width: u32,
        surface_height: u32,
        ui_chrome: &UiChrome,
        scale_factor: f32,
        workspaces: &[WorkspaceSnapshot],
        workspace_status_icons: &[Vec<(String, String)>],
        icon_empty: &'a glyphon::Buffer,
        status_icon_cgs: &'a std::collections::HashMap<
            wmux_render::icons::Icon,
            [glyphon::CustomGlyph; 1],
        >,
    ) -> Vec<TextArea<'a>> {
        if !self.visible || self.collapsed {
            return Vec::new();
        }

        let w = self.width;
        let y0 = self.top_offset;
        let bounds = TextBounds {
            left: 0,
            top: y0 as i32,
            right: (w - 1.0).max(0.0) as i32,
            bottom: surface_height as i32,
        };

        let text_color = f32_to_glyphon_color(ui_chrome.text_primary);
        let text_dim = f32_to_glyphon_color(ui_chrome.text_secondary);
        let text_muted = f32_to_glyphon_color(ui_chrome.text_muted);

        let mut areas = Vec::with_capacity(self.name_buffers.len() * 4 + 1);

        // Section header (WORKSPACES)
        if let Some(ref header_buf) = self.header_buffer {
            areas.push(TextArea {
                buffer: header_buf,
                left: CARD_MARGIN_X + PADDING_X,
                top: y0 + PADDING_Y,
                scale: scale_factor,
                bounds,
                default_color: text_muted,
                custom_glyphs: &[],
            });
        }

        let editing_index = if let SidebarInteraction::Editing { index, .. } = &self.interaction {
            Some(*index)
        } else {
            None
        };

        for (i, name_buf) in self.name_buffers.iter().enumerate() {
            let y = y0 + SECTION_HEADER_HEIGHT + i as f32 * ROW_HEIGHT;
            let card_y = y + CARD_GAP / 2.0;

            let text_x = CARD_MARGIN_X + ACCENT_BAR_WIDTH + PADDING_X;

            // If this row is being edited, show edit buffer instead of name buffer.
            if editing_index == Some(i) {
                if let Some(ref edit_buf) = self.edit_buffer {
                    // Clip text to the edit box area (prevents overflow on long text).
                    let card_w = w - CARD_MARGIN_X * 2.0 - 1.0;
                    let edit_right = (text_x + card_w - ACCENT_BAR_WIDTH - PADDING_X * 2.0) as i32;
                    let edit_bounds = TextBounds {
                        left: text_x as i32,
                        top: card_y as i32,
                        right: edit_right.min(bounds.right),
                        bottom: (card_y + ROW_HEIGHT) as i32,
                    };
                    areas.push(TextArea {
                        buffer: edit_buf,
                        left: text_x,
                        top: card_y + PADDING_Y,
                        scale: scale_factor,
                        bounds: edit_bounds,
                        default_color: text_color,
                        custom_glyphs: &[],
                    });
                }
            } else {
                // Workspace name (primary text)
                areas.push(TextArea {
                    buffer: name_buf,
                    left: text_x,
                    top: card_y + PADDING_Y,
                    scale: scale_factor,
                    bounds,
                    default_color: text_color,
                    custom_glyphs: &[],
                });
            }

            // Info text (status + git + cwd + ports) — caption size, secondary color.
            // Skip when this row is being edited (edit box only covers the name line).
            if editing_index != Some(i) {
                if let Some(info_buf) = self.info_buffers.get(i) {
                    areas.push(TextArea {
                        buffer: info_buf,
                        left: text_x,
                        top: card_y + PADDING_Y + SIDEBAR_LINE_HEIGHT + 2.0,
                        scale: scale_factor,
                        bounds,
                        default_color: text_dim,
                        custom_glyphs: &[],
                    });
                }
            }

            // Status icon from IPC (right side of card, below name).
            if let Some(icons) = workspace_status_icons.get(i) {
                if let Some((_key, icon_name)) = icons.first() {
                    if let Some(icon) = wmux_render::icons::Icon::from_name(icon_name) {
                        if let Some(cg) = status_icon_cgs.get(&icon) {
                            let icon_x = CARD_MARGIN_X + w - CARD_MARGIN_X * 2.0 - PADDING_X - 18.0;
                            areas.push(TextArea {
                                buffer: icon_empty,
                                left: icon_x,
                                top: card_y + PADDING_Y + SIDEBAR_LINE_HEIGHT + 2.0,
                                scale: scale_factor,
                                bounds,
                                default_color: text_dim,
                                custom_glyphs: cg,
                            });
                        }
                    }
                }
            }

            // Badge count text (centered on the accent-colored circle).
            if let Some(ws) = workspaces.get(i) {
                if ws.unread_count > 0 {
                    if let Some(badge_buf) = self.badge_buffers.get(i) {
                        let card_x = CARD_MARGIN_X;
                        let card_w = w - CARD_MARGIN_X * 2.0 - 1.0;
                        let badge_x = card_x + card_w - BADGE_SIZE - PADDING_X;
                        let badge_y = card_y + PADDING_Y;
                        let badge_text_color = f32_to_glyphon_color(ui_chrome.text_inverse);
                        areas.push(TextArea {
                            buffer: badge_buf,
                            left: badge_x,
                            top: badge_y + 2.0,
                            scale: scale_factor,
                            bounds,
                            default_color: badge_text_color,
                            custom_glyphs: &[],
                        });
                    }
                }
            }

            // Port pill text (centered in each pill).
            if let Some(ws) = workspaces.get(i) {
                if !ws.ports.is_empty() {
                    if let Some(port_bufs) = self.port_buffers.get(i) {
                        let port_colors = Self::port_palette(ui_chrome);
                        let card_x = CARD_MARGIN_X;
                        let card_w = w - CARD_MARGIN_X * 2.0 - 1.0;
                        let card_h = ROW_HEIGHT - CARD_GAP;
                        let pill_start_x = card_x + ACCENT_BAR_WIDTH + PADDING_X;
                        let pill_y = card_y + card_h - PADDING_Y - PORT_PILL_HEIGHT;
                        let avail_w = card_w - ACCENT_BAR_WIDTH - PADDING_X - PADDING_X;
                        let mut cur_x = pill_start_x;
                        for (j, (port, buf)) in ws
                            .ports
                            .iter()
                            .take(MAX_DISPLAY_PORTS)
                            .zip(port_bufs.iter())
                            .enumerate()
                        {
                            let pill_w = port_pill_width(*port);
                            if cur_x + pill_w - pill_start_x > avail_w {
                                break;
                            }
                            let color = port_colors[j % port_colors.len()];
                            areas.push(TextArea {
                                buffer: buf,
                                left: cur_x,
                                top: pill_y + 2.0,
                                scale: scale_factor,
                                bounds,
                                default_color: f32_to_glyphon_color(color),
                                custom_glyphs: &[],
                            });
                            cur_x += pill_w + PORT_PILL_GAP;
                        }
                    }
                }
            }
        }

        areas
    }

    /// Render the text editing cursor as a quad.
    ///
    /// Call after `render_quads` to overlay the cursor on top.
    /// `scale` is the DPI scale factor — glyphon multiplies glyph positions
    /// by this value, so the cursor quad must match.
    pub fn render_edit_cursor(
        &self,
        quad_pipeline: &mut QuadPipeline,
        ui_chrome: &UiChrome,
        scale: f32,
    ) {
        if self.collapsed {
            return;
        }
        if let SidebarInteraction::Editing { index, .. } = &self.interaction {
            let y = self.top_offset + SECTION_HEADER_HEIGHT + *index as f32 * ROW_HEIGHT;
            let card_y = y + CARD_GAP / 2.0;
            let text_x = CARD_MARGIN_X + ACCENT_BAR_WIDTH + PADDING_X;
            // Multiply offset by scale — glyphon renders glyphs at (left + glyph_x * scale).
            let cursor_x = text_x + self.edit_cursor_x_offset * scale;
            let cursor_y = card_y + PADDING_Y;
            let cursor_color = [
                ui_chrome.text_primary[0],
                ui_chrome.text_primary[1],
                ui_chrome.text_primary[2],
                ui_chrome.cursor_alpha,
            ];
            quad_pipeline.push_quad(cursor_x, cursor_y, 1.5, SIDEBAR_LINE_HEIGHT, cursor_color);
        }
    }
}

/// Compute the pixel width of a port pill badge from the port number.
fn port_pill_width(port: u16) -> f32 {
    let text_len = if port >= 10000 {
        6 // ":XXXXX"
    } else if port >= 1000 {
        5 // ":XXXX"
    } else if port >= 100 {
        4 // ":XXX"
    } else if port >= 10 {
        3 // ":XX"
    } else {
        2 // ":X"
    };
    PORT_PILL_PADDING_X * 2.0
        + text_len as f32 * typography::BADGE_FONT_SIZE * BADGE_CHAR_WIDTH_RATIO
}

/// Simple hash for quick change detection on workspace data.
fn compute_hash(workspaces: &[WorkspaceSnapshot]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    workspaces.len().hash(&mut hasher);
    for ws in workspaces {
        ws.name.hash(&mut hasher);
        ws.active.hash(&mut hasher);
        ws.pane_count.hash(&mut hasher);
        ws.unread_count.hash(&mut hasher);
        ws.cwd.hash(&mut hasher);
        ws.git_branch.hash(&mut hasher);
        ws.git_dirty.hash(&mut hasher);
        ws.ports.hash(&mut hasher);
        ws.status_text.hash(&mut hasher);
    }
    hasher.finish()
}

/// Build multi-line info text from workspace metadata.
///
/// Lines only included when data is available:
/// - Line 1: status text from IPC (if set)
/// - Line 2: git branch + dirty indicator
/// - Line 3: truncated CWD path
/// - Line 4: listening ports
fn build_info_text(ws: &WorkspaceSnapshot) -> String {
    let mut lines = Vec::with_capacity(4);

    // Line 1: status text from IPC (e.g. "Claude is waiting for your input")
    if let Some(ref text) = ws.status_text {
        let max_chars = 40;
        let char_count = text.chars().count();
        if char_count > max_chars {
            let truncated: String = text.chars().take(max_chars).collect();
            lines.push(format!("{truncated}..."));
        } else {
            lines.push(text.clone());
        }
    }

    // Line 2: git branch + dirty indicator
    if let Some(ref branch) = ws.git_branch {
        let mut line = branch.clone();
        if ws.git_dirty {
            line.push_str(" \u{2022}");
        }
        lines.push(line);
    }

    // Line 3: CWD (truncated from the left if too long)
    if let Some(ref cwd) = ws.cwd {
        let max_chars = 30;
        let char_count = cwd.chars().count();
        if char_count > max_chars {
            let truncated: String = cwd.chars().skip(char_count - max_chars).collect();
            lines.push(format!("...{truncated}"));
        } else {
            lines.push(cwd.clone());
        }
    }

    // Ports are rendered as pill badges (not text) — see render_quads() / text_areas().

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_sidebar_is_visible() {
        let s = SidebarState::new(220);
        assert!(s.visible);
        assert_eq!(s.effective_width(), 220.0);
    }

    #[test]
    fn toggle_hides_and_shows() {
        let mut s = SidebarState::new(220);
        s.toggle();
        assert!(!s.visible);
        assert_eq!(s.effective_width(), 0.0);
        s.toggle();
        assert!(s.visible);
        assert_eq!(s.effective_width(), 220.0);
    }

    #[test]
    fn compute_hash_changes_on_data() {
        let ws1 = vec![WorkspaceSnapshot {
            id: wmux_core::WorkspaceId::new(),
            name: "ws1".into(),
            active: true,
            pane_count: 1,
            unread_count: 0,
            cwd: None,
            git_branch: None,
            ports: Vec::new(),
            git_dirty: false,
            status_text: None,
            status_icons: Vec::new(),
        }];
        let ws2 = vec![WorkspaceSnapshot {
            id: wmux_core::WorkspaceId::new(),
            name: "ws2".into(),
            active: true,
            pane_count: 1,
            unread_count: 0,
            cwd: None,
            git_branch: None,
            ports: Vec::new(),
            git_dirty: false,
            status_text: None,
            status_icons: Vec::new(),
        }];
        assert_ne!(compute_hash(&ws1), compute_hash(&ws2));
    }

    #[test]
    fn compute_hash_changes_on_metadata() {
        let ws1 = vec![WorkspaceSnapshot {
            id: wmux_core::WorkspaceId::new(),
            name: "ws1".into(),
            active: true,
            pane_count: 1,
            unread_count: 0,
            cwd: Some("/foo".into()),
            git_branch: Some("main".into()),
            ports: vec![3000],
            git_dirty: false,
            status_text: None,
            status_icons: Vec::new(),
        }];
        let ws2 = vec![WorkspaceSnapshot {
            id: wmux_core::WorkspaceId::new(),
            name: "ws1".into(),
            active: true,
            pane_count: 1,
            unread_count: 0,
            cwd: Some("/foo".into()),
            git_branch: Some("main".into()),
            ports: vec![3000],
            git_dirty: true,
            status_text: None,
            status_icons: Vec::new(),
        }];
        assert_ne!(compute_hash(&ws1), compute_hash(&ws2));
    }

    #[test]
    fn hit_test_row_returns_correct_index() {
        let s = SidebarState::new(220);
        // With SECTION_HEADER_HEIGHT=42.0, ROW_HEIGHT=110.0
        // Row 0 is at y=42..152, Row 1 at y=152..262, Row 2 at y=262..372
        assert_eq!(s.hit_test_row(0.0, 3), None); // Above header
        assert_eq!(s.hit_test_row(42.0, 3), Some(0)); // Start of row 0
        assert_eq!(s.hit_test_row(151.0, 3), Some(0)); // End of row 0
        assert_eq!(s.hit_test_row(152.0, 3), Some(1)); // Start of row 1
        assert_eq!(s.hit_test_row(261.0, 3), Some(1)); // End of row 1
        assert_eq!(s.hit_test_row(262.0, 3), Some(2)); // Start of row 2
        assert_eq!(s.hit_test_row(372.0, 3), None); // Beyond row 2
    }

    #[test]
    fn hit_test_row_negative_returns_none() {
        let s = SidebarState::new(220);
        assert_eq!(s.hit_test_row(-1.0, 3), None);
    }

    #[test]
    fn drag_target_index_top_half() {
        let s = SidebarState::new(220);
        // SECTION_HEADER_HEIGHT=42.0, ROW_HEIGHT=110.0
        // y=42+12=54 is in top quarter of row 0 → target = 0
        assert_eq!(s.drag_target_index(54.0, 3), 0);
    }

    #[test]
    fn drag_target_index_bottom_half() {
        let s = SidebarState::new(220);
        // y=42+65=107 is past 50% of row 0 (110*0.5=55 + 42=97) → target = 1
        assert_eq!(s.drag_target_index(107.0, 3), 1);
    }

    #[test]
    fn drag_target_clamps_to_last() {
        let s = SidebarState::new(220);
        // With large y value, should clamp to last workspace (index 2)
        assert_eq!(s.drag_target_index(500.0, 3), 2);
    }

    #[test]
    fn should_start_drag_below_threshold() {
        let mut s = SidebarState::new(220);
        s.interaction = SidebarInteraction::Pressing {
            row: 0,
            start_y: 10.0,
        };
        assert!(!s.should_start_drag(12.0)); // 2px < 5px threshold
    }

    #[test]
    fn should_start_drag_above_threshold() {
        let mut s = SidebarState::new(220);
        s.interaction = SidebarInteraction::Pressing {
            row: 0,
            start_y: 10.0,
        };
        assert!(s.should_start_drag(16.0)); // 6px >= 5px threshold
    }

    #[test]
    fn is_editing_returns_true_when_editing() {
        let mut s = SidebarState::new(220);
        assert!(!s.is_editing());
        s.interaction = SidebarInteraction::Editing {
            index: 0,
            text: "test".into(),
            cursor: 4,
            selected_all: false,
        };
        assert!(s.is_editing());
    }

    #[test]
    fn build_info_text_full() {
        let ws = WorkspaceSnapshot {
            id: wmux_core::WorkspaceId::new(),
            name: "test".into(),
            active: true,
            pane_count: 2,
            unread_count: 0,
            cwd: Some("F:/Workspaces/wmux".into()),
            git_branch: Some("main".into()),
            ports: vec![3000, 4723],
            git_dirty: true,
            status_text: Some("Building project...".into()),
            status_icons: Vec::new(),
        };
        let text = build_info_text(&ws);
        assert!(text.contains("Building project..."));
        assert!(text.contains("main"));
        assert!(text.contains("\u{2022}")); // dirty indicator
        assert!(text.contains("F:/Workspaces/wmux"));
        // Ports are rendered as pill badges, not in info text.
        assert!(!text.contains(":3000"));
        // Pane count should NOT be present
        assert!(!text.contains("pane"));
    }

    #[test]
    fn build_info_text_minimal() {
        let ws = WorkspaceSnapshot {
            id: wmux_core::WorkspaceId::new(),
            name: "test".into(),
            active: false,
            pane_count: 1,
            unread_count: 0,
            cwd: None,
            git_branch: None,
            ports: Vec::new(),
            git_dirty: false,
            status_text: None,
            status_icons: Vec::new(),
        };
        let text = build_info_text(&ws);
        assert!(text.is_empty());
    }

    #[test]
    fn build_info_text_with_status() {
        let ws = WorkspaceSnapshot {
            id: wmux_core::WorkspaceId::new(),
            name: "test".into(),
            active: false,
            pane_count: 1,
            unread_count: 0,
            cwd: None,
            git_branch: Some("feat/sidebar".into()),
            ports: Vec::new(),
            git_dirty: false,
            status_text: Some("Claude is waiting for your input".into()),
            status_icons: Vec::new(),
        };
        let text = build_info_text(&ws);
        assert!(text.starts_with("Claude is waiting"));
        assert!(text.contains("feat/sidebar"));
    }

    #[test]
    fn build_info_text_truncates_long_cwd() {
        let long_cwd = "F:/Very/Long/Path/That/Exceeds/Thirty/Characters/Easily/Here";
        let ws = WorkspaceSnapshot {
            id: wmux_core::WorkspaceId::new(),
            name: "test".into(),
            active: false,
            pane_count: 1,
            unread_count: 0,
            cwd: Some(long_cwd.into()),
            git_branch: None,
            ports: Vec::new(),
            git_dirty: false,
            status_text: None,
            status_icons: Vec::new(),
        };
        let text = build_info_text(&ws);
        assert!(text.contains("..."));
        assert!(!text.contains(long_cwd));
    }

    #[test]
    fn build_info_text_excludes_ports() {
        let ws = WorkspaceSnapshot {
            id: wmux_core::WorkspaceId::new(),
            name: "test".into(),
            active: false,
            pane_count: 1,
            unread_count: 0,
            cwd: None,
            git_branch: None,
            ports: vec![3000, 3001, 3002, 3003, 3004, 3005, 3006],
            git_dirty: false,
            status_text: None,
            status_icons: Vec::new(),
        };
        let text = build_info_text(&ws);
        // Ports are rendered as pill badges, not in info text.
        assert!(!text.contains(":3000"));
        assert!(text.is_empty());
    }

    #[test]
    fn edit_cursor_offset_increases_monotonically() {
        let mut font_system = glyphon::FontSystem::new();
        let mut s = SidebarState::new(250);
        let text = "Workspace 1";
        let char_count = text.chars().count(); // 11

        let mut offsets = Vec::new();
        for cursor in 0..=char_count {
            s.interaction = SidebarInteraction::Editing {
                index: 0,
                text: text.to_string(),
                cursor,
                selected_all: false,
            };
            s.update_edit_buffer(&mut font_system);
            offsets.push((cursor, s.edit_cursor_x_offset));
        }

        // Print all offsets for debugging.
        for (c, o) in &offsets {
            eprintln!("cursor={c:>2} offset={o:.2}");
        }

        // Cursor 0 should be at 0.
        assert!(
            offsets[0].1.abs() < 0.001,
            "cursor 0 should be at offset 0, got {}",
            offsets[0].1
        );
        // Last cursor should be > 0 (non-empty text has width).
        assert!(
            offsets[char_count].1 > 10.0,
            "cursor at end should have positive offset, got {}",
            offsets[char_count].1
        );
        // Offsets should be strictly increasing.
        for i in 1..offsets.len() {
            assert!(
                offsets[i].1 > offsets[i - 1].1,
                "offset must increase: cursor {} ({:.2}) should be > cursor {} ({:.2})",
                i,
                offsets[i].1,
                i - 1,
                offsets[i - 1].1
            );
        }
    }
}
