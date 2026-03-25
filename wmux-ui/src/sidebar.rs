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
/// Maximum info lines below the workspace name (status + git + cwd + ports).
const MAX_INFO_LINES: f32 = 4.0;
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
/// Hit zone width for the resize handle on the sidebar right edge.
const RESIZE_HIT_ZONE: f32 = 5.0;
/// Minimum sidebar width when resizing.
pub const MIN_SIDEBAR_WIDTH: f32 = 180.0;
/// Maximum sidebar width when resizing.
pub const MAX_SIDEBAR_WIDTH: f32 = 480.0;

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
    /// Current mouse interaction state.
    pub interaction: SidebarInteraction,
    /// One glyphon Buffer per workspace row for the name label.
    name_buffers: Vec<Buffer>,
    /// One glyphon Buffer per workspace row for environment info (pane count, git, cwd, ports).
    info_buffers: Vec<Buffer>,
    /// Glyphon Buffer for the inline editing text.
    edit_buffer: Option<Buffer>,
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
            interaction: SidebarInteraction::Idle,
            name_buffers: Vec::new(),
            info_buffers: Vec::new(),
            edit_buffer: None,
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

    /// Effective width: actual width when visible, 0 when hidden.
    pub fn effective_width(&self) -> f32 {
        if self.visible {
            self.width
        } else {
            0.0
        }
    }

    /// Test if x is within the resize handle zone at the right edge.
    pub fn hit_test_resize_edge(&self, px: f32) -> bool {
        if !self.visible {
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
        if y < SECTION_HEADER_HEIGHT {
            return None;
        }
        let adjusted_y = y - SECTION_HEADER_HEIGHT;
        let index = (adjusted_y / ROW_HEIGHT) as usize;
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
        let adjusted_y = current_y - SECTION_HEADER_HEIGHT;
        let row_f = adjusted_y / ROW_HEIGHT;
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
        if let SidebarInteraction::Editing { ref text, .. } = self.interaction {
            // Match edit box width from render_quads: card_w - bar - pad - icon - pad
            let card_w = self.width - CARD_MARGIN_X * 2.0 - 1.0;
            let text_width = card_w - ACCENT_BAR_WIDTH - PADDING_X - 22.0 - PADDING_X;
            let metrics = Metrics::new(SIDEBAR_FONT_SIZE, SIDEBAR_LINE_HEIGHT);
            let attrs = Attrs::new().family(Family::Name("Segoe UI"));

            let buf = self
                .edit_buffer
                .get_or_insert_with(|| Buffer::new(font_system, metrics));
            buf.set_metrics(font_system, metrics);
            buf.set_size(
                font_system,
                Some(text_width.max(1.0)),
                Some(SIDEBAR_LINE_HEIGHT),
            );
            buf.set_text(font_system, text, &attrs, Shaping::Advanced, None);
            buf.shape_until_scroll(font_system, false);
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

        // Truncate if workspace count decreased.
        self.name_buffers.truncate(workspaces.len());
        self.info_buffers.truncate(workspaces.len());
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
        scale_factor: f32,
    ) {
        if !self.visible {
            return;
        }

        let _s = scale_factor; // available for future per-element scaling
        let w = self.width;
        let hover_color = [
            ui_chrome.surface_2[0],
            ui_chrome.surface_2[1],
            ui_chrome.surface_2[2],
            0.5,
        ];

        // Background quad
        quad_pipeline.push_quad(0.0, 0.0, w, surface_height, ui_chrome.surface_1);

        // Separator line on right edge (neutral, subtle)
        quad_pipeline.push_quad(w - 1.0, 0.0, 1.0, surface_height, ui_chrome.border_subtle);

        // Section header area
        quad_pipeline.push_quad(0.0, 0.0, w, SECTION_HEADER_HEIGHT, ui_chrome.surface_1);

        let dot_palette = Self::workspace_palette(ui_chrome);

        // Workspace card rows (offset by header height)
        for (i, ws) in workspaces.iter().enumerate() {
            let y = SECTION_HEADER_HEIGHT + i as f32 * ROW_HEIGHT;

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

            // Editing mode: draw input box background + border
            if let SidebarInteraction::Editing { index, .. } = &self.interaction {
                if *index == i {
                    let edit_x = card_x + ACCENT_BAR_WIDTH + PADDING_X + 22.0;
                    let edit_y = card_y + PADDING_Y - 2.0;
                    let edit_w = card_w - ACCENT_BAR_WIDTH - PADDING_X - 22.0 - PADDING_X;
                    let edit_h = SIDEBAR_LINE_HEIGHT + 4.0;
                    quad_pipeline.push_rounded_quad(
                        edit_x,
                        edit_y,
                        edit_w,
                        edit_h,
                        ui_chrome.surface_base,
                        4.0,
                    );
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
                    w - CARD_MARGIN_X * 2.0 - 1.0,
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
        workspace_icon: Option<(&'a glyphon::Buffer, &'a [glyphon::CustomGlyph])>,
        workspace_status_icons: &[Vec<(String, String)>],
        icon_empty: &'a glyphon::Buffer,
        status_icon_cgs: &'a std::collections::HashMap<
            wmux_render::icons::Icon,
            [glyphon::CustomGlyph; 1],
        >,
    ) -> Vec<TextArea<'a>> {
        if !self.visible {
            return Vec::new();
        }

        let w = self.width;
        let bounds = TextBounds {
            left: 0,
            top: 0,
            right: (w - 1.0).max(0.0) as i32,
            bottom: surface_height as i32,
        };

        let text_color = f32_to_glyphon_color(ui_chrome.text_primary);
        let text_dim = f32_to_glyphon_color(ui_chrome.text_secondary);
        let dot_palette = Self::workspace_palette(ui_chrome);
        let text_muted = f32_to_glyphon_color(ui_chrome.text_muted);

        let mut areas = Vec::with_capacity(self.name_buffers.len() * 4 + 1);

        // (workspace icon custom glyph is passed in via workspace_icon parameter)

        // Section header (WORKSPACES)
        if let Some(ref header_buf) = self.header_buffer {
            areas.push(TextArea {
                buffer: header_buf,
                left: CARD_MARGIN_X + PADDING_X,
                top: PADDING_Y,
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
            let y = SECTION_HEADER_HEIGHT + i as f32 * ROW_HEIGHT;
            let card_y = y + CARD_GAP / 2.0;

            // Workspace icon: icon (16px) + gap (12px) = 28px reserve before text.
            let icon_reserve = if workspace_icon.is_some() { 28.0 } else { 0.0 };
            let text_x = CARD_MARGIN_X + ACCENT_BAR_WIDTH + PADDING_X + icon_reserve;

            // Workspace icon — SVG folder icon colored with the workspace identity color.
            if let Some((icon_buf, icon_cg)) = workspace_icon {
                let icon_x = CARD_MARGIN_X + ACCENT_BAR_WIDTH + PADDING_X;
                let ws_color = dot_palette[i % dot_palette.len()];
                let icon_color = f32_to_glyphon_color(ws_color);
                areas.push(TextArea {
                    buffer: icon_buf,
                    left: icon_x,
                    top: card_y + PADDING_Y + 1.0,
                    scale: scale_factor,
                    bounds,
                    default_color: icon_color,
                    custom_glyphs: icon_cg,
                });
            }

            // If this row is being edited, show edit buffer instead of name buffer.
            if editing_index == Some(i) {
                if let Some(ref edit_buf) = self.edit_buffer {
                    areas.push(TextArea {
                        buffer: edit_buf,
                        left: text_x,
                        top: card_y + PADDING_Y,
                        scale: scale_factor,
                        bounds,
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
        }

        areas
    }

    /// Render the text editing cursor as a quad.
    ///
    /// Call after `render_quads` to overlay the cursor on top.
    pub fn render_edit_cursor(&self, quad_pipeline: &mut QuadPipeline, ui_chrome: &UiChrome) {
        if let SidebarInteraction::Editing { index, cursor, .. } = &self.interaction {
            let y = SECTION_HEADER_HEIGHT + *index as f32 * ROW_HEIGHT;
            let card_y = y + CARD_GAP / 2.0;
            // 28px icon reserve (16px icon + 12px gap) — icons are always loaded.
            let text_x = CARD_MARGIN_X + ACCENT_BAR_WIDTH + PADDING_X + 28.0;
            let char_width = SIDEBAR_FONT_SIZE * 0.6;
            let cursor_x = text_x + (*cursor as f32 * char_width);
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

    // Line 4: listening ports (max MAX_DISPLAY_PORTS)
    if !ws.ports.is_empty() {
        let displayed: Vec<String> = ws
            .ports
            .iter()
            .take(MAX_DISPLAY_PORTS)
            .map(|p| format!(":{p}"))
            .collect();
        let mut port_str = displayed.join(" ");
        if ws.ports.len() > MAX_DISPLAY_PORTS {
            port_str.push_str(&format!(" +{}", ws.ports.len() - MAX_DISPLAY_PORTS));
        }
        lines.push(port_str);
    }

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
        assert!(text.contains(":3000"));
        assert!(text.contains(":4723"));
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
    fn build_info_text_limits_ports() {
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
        assert!(text.contains(":3000"));
        assert!(text.contains(":3004"));
        assert!(!text.contains(":3005"));
        assert!(text.contains("+2"));
    }
}
