use wmux_render::quad::QuadPipeline;

/// Maximum number of visible results in the palette.
const MAX_VISIBLE_RESULTS: usize = 20;

/// Palette overlay dimensions.
const PALETTE_WIDTH: f32 = 600.0;
const INPUT_HEIGHT: f32 = 40.0;
const RESULT_HEIGHT: f32 = 36.0;
const PADDING: f32 = 8.0;

/// Background colors.
const BG_COLOR: [f32; 4] = [0.1, 0.1, 0.15, 0.97];
const INPUT_BG: [f32; 4] = [0.15, 0.15, 0.2, 1.0];
const SELECTED_BG: [f32; 4] = [0.2, 0.3, 0.5, 0.8];
const BORDER_COLOR: [f32; 4] = [0.3, 0.5, 1.0, 0.5];

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
    pub fn render_quads(&self, quads: &mut QuadPipeline, surface_width: f32, surface_height: f32) {
        if !self.open {
            return;
        }

        let visible_results = self.result_count.min(MAX_VISIBLE_RESULTS);
        let total_height = INPUT_HEIGHT + visible_results as f32 * RESULT_HEIGHT + 2.0 * PADDING;
        let palette_x = (surface_width - PALETTE_WIDTH) / 2.0;
        let palette_y = (surface_height * 0.2).max(50.0); // 20% from top

        // Drop shadow (subtle)
        quads.push_quad(
            palette_x - 2.0,
            palette_y - 2.0,
            PALETTE_WIDTH + 4.0,
            total_height + 4.0,
            [0.0, 0.0, 0.0, 0.3],
        );

        // Main background
        quads.push_quad(palette_x, palette_y, PALETTE_WIDTH, total_height, BG_COLOR);

        // Border
        let bw = 1.0;
        quads.push_quad(palette_x, palette_y, PALETTE_WIDTH, bw, BORDER_COLOR); // top
        quads.push_quad(
            palette_x,
            palette_y + total_height - bw,
            PALETTE_WIDTH,
            bw,
            BORDER_COLOR,
        ); // bottom
        quads.push_quad(palette_x, palette_y, bw, total_height, BORDER_COLOR); // left
        quads.push_quad(
            palette_x + PALETTE_WIDTH - bw,
            palette_y,
            bw,
            total_height,
            BORDER_COLOR,
        ); // right

        // Input field background
        let input_x = palette_x + PADDING;
        let input_y = palette_y + PADDING;
        let input_w = PALETTE_WIDTH - 2.0 * PADDING;
        quads.push_quad(input_x, input_y, input_w, INPUT_HEIGHT, INPUT_BG);

        // Selected result highlight
        if visible_results > 0 {
            let selected_visible = self.selected.min(visible_results - 1);
            let result_y =
                input_y + INPUT_HEIGHT + PADDING + selected_visible as f32 * RESULT_HEIGHT;
            quads.push_quad(input_x, result_y, input_w, RESULT_HEIGHT, SELECTED_BG);
        }
    }

    /// Get the layout rect for the palette (for hit testing).
    #[must_use]
    pub fn layout_rect(&self, surface_width: f32, surface_height: f32) -> (f32, f32, f32, f32) {
        let visible_results = self.result_count.min(MAX_VISIBLE_RESULTS);
        let total_height = INPUT_HEIGHT + visible_results as f32 * RESULT_HEIGHT + 2.0 * PADDING;
        let x = (surface_width - PALETTE_WIDTH) / 2.0;
        let y = (surface_height * 0.2).max(50.0);
        (x, y, PALETTE_WIDTH, total_height)
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
