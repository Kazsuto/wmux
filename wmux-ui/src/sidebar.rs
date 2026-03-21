use glyphon::{Attrs, Buffer, Family, Metrics, Shaping, TextArea, TextBounds};
use wmux_core::WorkspaceSnapshot;
use wmux_render::QuadPipeline;

const SIDEBAR_FONT_SIZE: f32 = 12.0;
const SIDEBAR_LINE_HEIGHT: f32 = 16.0;
/// Height of each workspace row in pixels.
pub const ROW_HEIGHT: f32 = 44.0;
const PADDING_X: f32 = 12.0;
const PADDING_Y: f32 = 6.0;
const ACCENT_BAR_WIDTH: f32 = 3.0;
/// Minimum pixel distance before a press becomes a drag.
const DRAG_THRESHOLD: f32 = 5.0;

// Colors
const BG_COLOR: [f32; 4] = [0.12, 0.12, 0.14, 1.0];
const SEPARATOR_COLOR: [f32; 4] = [0.3, 0.3, 0.35, 1.0];
const ACTIVE_BG_COLOR: [f32; 4] = [0.18, 0.20, 0.28, 1.0];
const HOVER_BG_COLOR: [f32; 4] = [0.15, 0.15, 0.18, 1.0];
const DROP_INDICATOR_COLOR: [f32; 4] = [0.4, 0.6, 1.0, 1.0];
const EDIT_BG_COLOR: [f32; 4] = [0.08, 0.08, 0.10, 1.0];
const EDIT_BORDER_COLOR: [f32; 4] = [0.4, 0.6, 1.0, 1.0];
const CURSOR_COLOR: [f32; 4] = [0.8, 0.85, 1.0, 1.0];
const ACCENT_COLOR: [f32; 4] = [0.4, 0.6, 1.0, 1.0];
const TEXT_COLOR: glyphon::Color = glyphon::Color::rgb(210, 210, 215);
const TEXT_DIM_COLOR: glyphon::Color = glyphon::Color::rgb(140, 140, 150);

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
}

/// Sidebar panel state — visibility, width, text buffers, and interaction.
pub struct SidebarState {
    pub visible: bool,
    pub width: f32,
    /// Current mouse interaction state.
    pub interaction: SidebarInteraction,
    /// One glyphon Buffer per workspace row for the name label.
    name_buffers: Vec<Buffer>,
    /// One glyphon Buffer per workspace row for the subtitle (pane count).
    subtitle_buffers: Vec<Buffer>,
    /// Glyphon Buffer for the inline editing text.
    edit_buffer: Option<Buffer>,
    /// Number of workspaces rendered last frame (to detect changes).
    last_workspace_count: usize,
    /// Names hash from last frame (quick change detection).
    last_names_hash: u64,
}

impl SidebarState {
    /// Create a new sidebar with the given width in pixels.
    pub fn new(width: u16) -> Self {
        Self {
            visible: true,
            width: width as f32,
            interaction: SidebarInteraction::Idle,
            name_buffers: Vec::new(),
            subtitle_buffers: Vec::new(),
            edit_buffer: None,
            last_workspace_count: 0,
            last_names_hash: 0,
        }
    }

    /// Toggle sidebar visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
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

    /// Test if a y coordinate hits a workspace row.
    ///
    /// Returns the 0-based workspace index, or `None` if out of range.
    pub fn hit_test_row(&self, y: f32, workspace_count: usize) -> Option<usize> {
        if y < 0.0 {
            return None;
        }
        let index = (y / ROW_HEIGHT) as usize;
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
        let row_f = current_y / ROW_HEIGHT;
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
            let text_width = self.width - PADDING_X * 2.0 - ACCENT_BAR_WIDTH;
            let metrics = Metrics::new(SIDEBAR_FONT_SIZE, SIDEBAR_LINE_HEIGHT);
            let attrs = Attrs::new().family(Family::SansSerif);

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
        let attrs = Attrs::new().family(Family::SansSerif);

        // Resize buffer vecs to match workspace count.
        self.name_buffers
            .resize_with(workspaces.len(), || Buffer::new(font_system, metrics));
        self.subtitle_buffers
            .resize_with(workspaces.len(), || Buffer::new(font_system, metrics));

        for (i, ws) in workspaces.iter().enumerate() {
            // Name buffer
            let buf = &mut self.name_buffers[i];
            buf.set_metrics(font_system, metrics);
            buf.set_size(
                font_system,
                Some(text_width.max(1.0)),
                Some(SIDEBAR_LINE_HEIGHT),
            );
            buf.set_text(font_system, &ws.name, &attrs, Shaping::Advanced, None);
            buf.shape_until_scroll(font_system, false);

            // Subtitle buffer (pane count)
            let sub = &mut self.subtitle_buffers[i];
            sub.set_metrics(font_system, metrics);
            sub.set_size(
                font_system,
                Some(text_width.max(1.0)),
                Some(SIDEBAR_LINE_HEIGHT),
            );
            let subtitle = if ws.pane_count == 1 {
                format!("{} pane", ws.pane_count)
            } else {
                format!("{} panes", ws.pane_count)
            };
            sub.set_text(font_system, &subtitle, &attrs, Shaping::Advanced, None);
            sub.shape_until_scroll(font_system, false);
        }

        // Truncate if workspace count decreased.
        self.name_buffers.truncate(workspaces.len());
        self.subtitle_buffers.truncate(workspaces.len());
    }

    /// Push sidebar quads into the quad pipeline.
    pub fn render_quads(
        &self,
        workspaces: &[WorkspaceSnapshot],
        quad_pipeline: &mut QuadPipeline,
        surface_height: f32,
    ) {
        if !self.visible {
            return;
        }

        let w = self.width;

        // Background quad — dark panel.
        quad_pipeline.push_quad(0.0, 0.0, w, surface_height, BG_COLOR);

        // Separator line on right edge.
        quad_pipeline.push_quad(w - 1.0, 0.0, 1.0, surface_height, SEPARATOR_COLOR);

        // Workspace entry rows.
        for (i, ws) in workspaces.iter().enumerate() {
            let y = i as f32 * ROW_HEIGHT;

            // Hover highlight (non-active, non-editing rows only).
            if matches!(self.interaction, SidebarInteraction::Hover(h) if h == i)
                && !ws.active
                && !matches!(self.interaction, SidebarInteraction::Editing { index, .. } if index == i)
            {
                quad_pipeline.push_quad(0.0, y, w - 1.0, ROW_HEIGHT, HOVER_BG_COLOR);
            }

            if ws.active {
                // Highlight background for active workspace.
                quad_pipeline.push_quad(0.0, y, w - 1.0, ROW_HEIGHT, ACTIVE_BG_COLOR);
                // Accent bar on left edge.
                quad_pipeline.push_quad(
                    0.0,
                    y + 4.0,
                    ACCENT_BAR_WIDTH,
                    ROW_HEIGHT - 8.0,
                    ACCENT_COLOR,
                );
            }

            // Editing mode: draw input box background + border.
            if let SidebarInteraction::Editing { index, .. } = &self.interaction {
                if *index == i {
                    let edit_x = ACCENT_BAR_WIDTH + PADDING_X - 2.0;
                    let edit_y = y + PADDING_Y - 2.0;
                    let edit_w = w - edit_x - PADDING_X;
                    let edit_h = SIDEBAR_LINE_HEIGHT + 4.0;
                    // Background
                    quad_pipeline.push_quad(edit_x, edit_y, edit_w, edit_h, EDIT_BG_COLOR);
                    // Top border
                    quad_pipeline.push_quad(edit_x, edit_y, edit_w, 1.0, EDIT_BORDER_COLOR);
                    // Bottom border
                    quad_pipeline.push_quad(
                        edit_x,
                        edit_y + edit_h - 1.0,
                        edit_w,
                        1.0,
                        EDIT_BORDER_COLOR,
                    );
                    // Left border
                    quad_pipeline.push_quad(edit_x, edit_y, 1.0, edit_h, EDIT_BORDER_COLOR);
                    // Right border
                    quad_pipeline.push_quad(
                        edit_x + edit_w - 1.0,
                        edit_y,
                        1.0,
                        edit_h,
                        EDIT_BORDER_COLOR,
                    );
                }
            }
        }

        // Drag: drop indicator line.
        if let SidebarInteraction::Dragging {
            current_y,
            from_row,
        } = &self.interaction
        {
            let target = self.drag_target_index(*current_y, workspaces.len());
            if target != *from_row {
                let indicator_y = target as f32 * ROW_HEIGHT;
                quad_pipeline.push_quad(
                    ACCENT_BAR_WIDTH,
                    indicator_y - 1.0,
                    w - ACCENT_BAR_WIDTH - 1.0,
                    2.0,
                    DROP_INDICATOR_COLOR,
                );
            }
        }
    }

    /// Produce TextArea descriptors for the sidebar text labels.
    ///
    /// Must be called after `update_text()`. The returned text areas should be
    /// appended to the terminal text areas before calling `prepare_text_areas`.
    pub fn text_areas(&self, _surface_width: u32, surface_height: u32) -> Vec<TextArea<'_>> {
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

        let mut areas = Vec::with_capacity(self.name_buffers.len() * 2);

        let editing_index = if let SidebarInteraction::Editing { index, .. } = &self.interaction {
            Some(*index)
        } else {
            None
        };

        for (i, (name_buf, sub_buf)) in self
            .name_buffers
            .iter()
            .zip(self.subtitle_buffers.iter())
            .enumerate()
        {
            let y = i as f32 * ROW_HEIGHT;
            let text_x = ACCENT_BAR_WIDTH + PADDING_X;

            // If this row is being edited, show edit buffer instead of name buffer.
            if editing_index == Some(i) {
                if let Some(ref edit_buf) = self.edit_buffer {
                    areas.push(TextArea {
                        buffer: edit_buf,
                        left: text_x,
                        top: y + PADDING_Y,
                        scale: 1.0,
                        bounds,
                        default_color: TEXT_COLOR,
                        custom_glyphs: &[],
                    });
                }
            } else {
                // Workspace name (primary text)
                areas.push(TextArea {
                    buffer: name_buf,
                    left: text_x,
                    top: y + PADDING_Y,
                    scale: 1.0,
                    bounds,
                    default_color: TEXT_COLOR,
                    custom_glyphs: &[],
                });
            }

            // Subtitle (dimmed, below name)
            areas.push(TextArea {
                buffer: sub_buf,
                left: text_x,
                top: y + PADDING_Y + SIDEBAR_LINE_HEIGHT + 2.0,
                scale: 1.0,
                bounds,
                default_color: TEXT_DIM_COLOR,
                custom_glyphs: &[],
            });
        }

        // Render cursor for editing mode.
        // The cursor is rendered as a thin quad in render_quads, but we need
        // the text area to position it. We handle cursor rendering via quads
        // since glyphon doesn't provide cursor positioning directly.

        areas
    }

    /// Render the text editing cursor as a quad.
    ///
    /// Call after `render_quads` to overlay the cursor on top.
    pub fn render_edit_cursor(&self, quad_pipeline: &mut QuadPipeline) {
        if let SidebarInteraction::Editing { index, cursor, .. } = &self.interaction {
            let y = *index as f32 * ROW_HEIGHT;
            let text_x = ACCENT_BAR_WIDTH + PADDING_X;
            // Approximate cursor x position: each character is roughly
            // SIDEBAR_FONT_SIZE * 0.6 wide for sans-serif at this size.
            let char_width = SIDEBAR_FONT_SIZE * 0.6;
            let cursor_x = text_x + (*cursor as f32 * char_width);
            let cursor_y = y + PADDING_Y;
            quad_pipeline.push_quad(cursor_x, cursor_y, 1.5, SIDEBAR_LINE_HEIGHT, CURSOR_COLOR);
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
    }
    hasher.finish()
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
        }];
        let ws2 = vec![WorkspaceSnapshot {
            id: wmux_core::WorkspaceId::new(),
            name: "ws2".into(),
            active: true,
            pane_count: 1,
        }];
        assert_ne!(compute_hash(&ws1), compute_hash(&ws2));
    }

    #[test]
    fn hit_test_row_returns_correct_index() {
        let s = SidebarState::new(220);
        assert_eq!(s.hit_test_row(0.0, 3), Some(0));
        assert_eq!(s.hit_test_row(43.0, 3), Some(0));
        assert_eq!(s.hit_test_row(44.0, 3), Some(1));
        assert_eq!(s.hit_test_row(88.0, 3), Some(2));
        assert_eq!(s.hit_test_row(132.0, 3), None);
    }

    #[test]
    fn hit_test_row_negative_returns_none() {
        let s = SidebarState::new(220);
        assert_eq!(s.hit_test_row(-1.0, 3), None);
    }

    #[test]
    fn drag_target_index_top_half() {
        let s = SidebarState::new(220);
        // y=10 is in top half of row 0 → target = 0
        assert_eq!(s.drag_target_index(10.0, 3), 0);
    }

    #[test]
    fn drag_target_index_bottom_half() {
        let s = SidebarState::new(220);
        // y=30 is in bottom half of row 0 → target = 1
        assert_eq!(s.drag_target_index(30.0, 3), 1);
    }

    #[test]
    fn drag_target_clamps_to_last() {
        let s = SidebarState::new(220);
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
}
