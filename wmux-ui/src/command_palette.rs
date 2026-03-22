use wmux_config::UiChrome;
use wmux_render::quad::QuadPipeline;

/// Maximum number of visible results in the palette.
const MAX_VISIBLE_RESULTS: usize = 20;

/// Palette overlay dimensions.
const PALETTE_WIDTH: f32 = 600.0;
const INPUT_HEIGHT: f32 = 44.0;
const RESULT_HEIGHT: f32 = 36.0;
const PADDING: f32 = 12.0;
const PALETTE_RADIUS: f32 = 12.0;
const TAB_RADIUS: f32 = 6.0;
const BORDER_WIDTH: f32 = 1.0;
const SHADOW_OFFSET: f32 = 8.0;

/// State for the command palette overlay.
#[derive(Debug)]
pub struct CommandPalette {
    /// Whether the palette is currently visible.
    pub open: bool,
    /// Current search query typed by the user.
    pub query: String,
    /// Index of the currently selected result.
    pub selected: usize,
    /// Number of results from the last search.
    pub result_count: usize,
}

impl Default for CommandPalette {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandPalette {
    /// Create a new hidden command palette.
    #[must_use]
    pub fn new() -> Self {
        Self {
            open: false,
            query: String::new(),
            selected: 0,
            result_count: 0,
        }
    }

    /// Open the palette and reset state.
    pub fn open(&mut self) {
        self.open = true;
        self.query.clear();
        self.selected = 0;
        self.result_count = 0;
        tracing::debug!("command palette opened");
    }

    /// Close the palette.
    pub fn close(&mut self) {
        self.open = false;
        self.query.clear();
        self.selected = 0;
        self.result_count = 0;
        tracing::debug!("command palette closed");
    }

    /// Move selection up.
    pub fn select_prev(&mut self) {
        if self.result_count > 0 {
            self.selected = if self.selected == 0 {
                self.result_count - 1
            } else {
                self.selected - 1
            };
        }
    }

    /// Move selection down.
    pub fn select_next(&mut self) {
        if self.result_count > 0 {
            self.selected = (self.selected + 1) % self.result_count;
        }
    }

    /// Update the result count (called after search).
    pub fn set_result_count(&mut self, count: usize) {
        self.result_count = count;
        if self.selected >= count && count > 0 {
            self.selected = count - 1;
        } else if count == 0 {
            self.selected = 0;
        }
    }

    /// Get the currently selected result index.
    #[must_use]
    pub fn selected_index(&self) -> Option<usize> {
        if self.result_count > 0 {
            Some(self.selected)
        } else {
            None
        }
    }

    /// Render the command palette background and selection highlight.
    ///
    /// Text is rendered separately by the caller using glyphon text areas.
    pub fn render_quads(
        &self,
        quads: &mut QuadPipeline,
        surface_width: f32,
        surface_height: f32,
        ui_chrome: &UiChrome,
    ) {
        if !self.open {
            return;
        }

        let visible_results = self.result_count.min(MAX_VISIBLE_RESULTS);
        let total_height = INPUT_HEIGHT + visible_results as f32 * RESULT_HEIGHT + 2.0 * PADDING;
        let effective_width = PALETTE_WIDTH.min(surface_width);
        let palette_x = ((surface_width - effective_width) / 2.0).max(0.0);
        let palette_y = (surface_height * 0.2).max(50.0);

        // Fullscreen dimming overlay — two layered quads
        // First: overlay_dim (black at 0.5 alpha)
        quads.push_quad(
            0.0,
            0.0,
            surface_width,
            surface_height,
            ui_chrome.overlay_dim,
        );
        // Second: overlay_tint (accent at 8% alpha)
        quads.push_quad(
            0.0,
            0.0,
            surface_width,
            surface_height,
            ui_chrome.overlay_tint,
        );

        // Drop shadow (rounded) — increased offset to 8px
        quads.push_rounded_quad(
            palette_x - SHADOW_OFFSET,
            palette_y - SHADOW_OFFSET,
            effective_width + 2.0 * SHADOW_OFFSET,
            total_height + 2.0 * SHADOW_OFFSET,
            ui_chrome.shadow,
            PALETTE_RADIUS + 2.0,
        );

        // Main background — surface_overlay already has 95% alpha baked in.
        let bg = ui_chrome.surface_overlay;
        quads.push_rounded_quad(
            palette_x,
            palette_y,
            effective_width,
            total_height,
            bg,
            PALETTE_RADIUS,
        );

        // Border (1px) — four thin quads
        let border_color = ui_chrome.border_subtle;
        // Top border
        quads.push_quad(
            palette_x,
            palette_y,
            effective_width,
            BORDER_WIDTH,
            border_color,
        );
        // Bottom border
        quads.push_quad(
            palette_x,
            palette_y + total_height - BORDER_WIDTH,
            effective_width,
            BORDER_WIDTH,
            border_color,
        );
        // Left border
        quads.push_quad(
            palette_x,
            palette_y,
            BORDER_WIDTH,
            total_height,
            border_color,
        );
        // Right border
        quads.push_quad(
            palette_x + effective_width - BORDER_WIDTH,
            palette_y,
            BORDER_WIDTH,
            total_height,
            border_color,
        );

        // Input field (rounded)
        let input_x = palette_x + PADDING;
        let input_y = palette_y + PADDING;
        let input_w = effective_width - 2.0 * PADDING;
        quads.push_rounded_quad(
            input_x,
            input_y,
            input_w,
            INPUT_HEIGHT,
            ui_chrome.surface_0,
            8.0,
        );

        // Selected result highlight (rounded) — use surface_2 with TAB_RADIUS
        if visible_results > 0 {
            let selected_visible = self.selected.min(visible_results - 1);
            let result_y =
                input_y + INPUT_HEIGHT + PADDING + selected_visible as f32 * RESULT_HEIGHT;
            quads.push_rounded_quad(
                input_x,
                result_y,
                input_w,
                RESULT_HEIGHT,
                ui_chrome.surface_2,
                TAB_RADIUS,
            );
        }
    }

    /// Get the layout rect for the palette (for hit testing).
    #[must_use]
    pub fn layout_rect(&self, surface_width: f32, surface_height: f32) -> (f32, f32, f32, f32) {
        let visible_results = self.result_count.min(MAX_VISIBLE_RESULTS);
        let total_height = INPUT_HEIGHT + visible_results as f32 * RESULT_HEIGHT + 2.0 * PADDING;
        let effective_width = PALETTE_WIDTH.min(surface_width);
        let x = ((surface_width - effective_width) / 2.0).max(0.0);
        let y = (surface_height * 0.2).max(50.0);
        (x, y, effective_width, total_height)
    }

    /// Check if a screen position is inside the palette.
    #[must_use]
    pub fn contains(&self, x: f32, y: f32, surface_width: f32, surface_height: f32) -> bool {
        if !self.open {
            return false;
        }
        let (px, py, pw, ph) = self.layout_rect(surface_width, surface_height);
        x >= px && x <= px + pw && y >= py && y <= py + ph
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_palette_is_closed() {
        let p = CommandPalette::new();
        assert!(!p.open);
        assert!(p.query.is_empty());
    }

    #[test]
    fn open_resets_state() {
        let mut p = CommandPalette::new();
        p.query = "test".into();
        p.selected = 5;
        p.open();
        assert!(p.open);
        assert!(p.query.is_empty());
        assert_eq!(p.selected, 0);
    }

    #[test]
    fn close_resets_state() {
        let mut p = CommandPalette::new();
        p.open();
        p.query = "test".into();
        p.close();
        assert!(!p.open);
        assert!(p.query.is_empty());
    }

    #[test]
    fn select_next_wraps() {
        let mut p = CommandPalette::new();
        p.result_count = 3;
        p.select_next();
        assert_eq!(p.selected, 1);
        p.select_next();
        assert_eq!(p.selected, 2);
        p.select_next(); // wrap
        assert_eq!(p.selected, 0);
    }

    #[test]
    fn select_prev_wraps() {
        let mut p = CommandPalette::new();
        p.result_count = 3;
        p.select_prev(); // wrap to last
        assert_eq!(p.selected, 2);
        p.select_prev();
        assert_eq!(p.selected, 1);
    }

    #[test]
    fn select_on_empty_results() {
        let mut p = CommandPalette::new();
        p.result_count = 0;
        p.select_next();
        assert_eq!(p.selected, 0);
        p.select_prev();
        assert_eq!(p.selected, 0);
    }

    #[test]
    fn set_result_count_clamps_selected() {
        let mut p = CommandPalette::new();
        p.selected = 10;
        p.set_result_count(3);
        assert_eq!(p.selected, 2);
    }

    #[test]
    fn selected_index_none_when_empty() {
        let p = CommandPalette::new();
        assert_eq!(p.selected_index(), None);
    }

    #[test]
    fn contains_when_closed() {
        let p = CommandPalette::new();
        assert!(!p.contains(500.0, 200.0, 1920.0, 1080.0));
    }
}
