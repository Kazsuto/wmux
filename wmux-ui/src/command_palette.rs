use crate::shortcuts::ShortcutAction;
use wmux_config::UiChrome;
use wmux_render::quad::QuadPipeline;

/// Maximum number of visible results in the palette.
pub(crate) const MAX_VISIBLE_RESULTS: usize = 20;

/// Palette overlay dimensions.
pub(crate) const PALETTE_WIDTH: f32 = 600.0;
pub(crate) const INPUT_HEIGHT: f32 = 44.0;
pub(crate) const RESULT_HEIGHT: f32 = 36.0;
pub(crate) const PADDING: f32 = 12.0;
pub(crate) const PALETTE_RADIUS: f32 = 12.0;
pub(crate) const TAB_RADIUS: f32 = 6.0;
const BORDER_WIDTH: f32 = 1.0;
const SHADOW_OFFSET: f32 = 8.0;
/// Height of the filter tab row between input and results.
pub(crate) const FILTER_ROW_HEIGHT: f32 = 42.0;
/// Horizontal padding inside each filter tab pill.
pub(crate) const FILTER_TAB_PAD_X: f32 = 14.0;
/// Vertical padding inside each filter tab pill.
pub(crate) const FILTER_TAB_PAD_Y: f32 = 5.0;
/// Gap between filter tab pills.
pub(crate) const FILTER_TAB_GAP: f32 = 6.0;
/// Inner horizontal padding inside the input field for text.
pub(crate) const INPUT_TEXT_PAD: f32 = 12.0;
/// Width reserved for the shortcut badge column on the right side of each result row.
pub(crate) const SHORTCUT_COL_WIDTH: f32 = 110.0;
/// Horizontal padding between the result name and the shortcut badge.
pub(crate) const SHORTCUT_COL_PAD: f32 = 2.0;

/// Pre-computed layout positions for the command palette.
///
/// Single source of truth — eliminates duplicated layout formulas across
/// `render_quads()`, text buffer updates, and text area collection.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PaletteLayout {
    pub palette_x: f32,
    pub palette_y: f32,
    pub effective_width: f32,
    pub total_height: f32,
    pub input_x: f32,
    pub input_y: f32,
    pub input_w: f32,
    pub filter_y: f32,
    pub results_y: f32,
}

impl PaletteLayout {
    /// Compute layout from surface dimensions and visible result count.
    #[must_use]
    pub fn compute(surface_width: f32, surface_height: f32, visible_results: usize) -> Self {
        let total_height = INPUT_HEIGHT
            + FILTER_ROW_HEIGHT
            + visible_results as f32 * RESULT_HEIGHT
            + 2.0 * PADDING;
        let effective_width = PALETTE_WIDTH.min(surface_width);
        let palette_x = ((surface_width - effective_width) / 2.0).max(0.0);
        let palette_y = (surface_height * 0.2).max(50.0);
        let input_x = palette_x + PADDING;
        let input_y = palette_y + PADDING;
        let input_w = effective_width - 2.0 * PADDING;
        let filter_y = input_y + INPUT_HEIGHT + PADDING / 2.0;
        let results_y = filter_y + FILTER_ROW_HEIGHT;
        Self {
            palette_x,
            palette_y,
            effective_width,
            total_height,
            input_x,
            input_y,
            input_w,
            filter_y,
            results_y,
        }
    }
}

/// An action stored per palette result row, used by the Enter handler.
#[derive(Debug, Clone)]
pub(crate) enum PaletteAction {
    /// Execute a command from the registry (stores the command ID).
    Command(String),
    /// Switch to a workspace by 1-based index.
    SwitchWorkspace(u8),
    /// Focus a pane and switch to the given surface tab index.
    FocusSurface(wmux_core::PaneId, usize),
}

/// Filter category for command palette results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteFilter {
    /// Show all results.
    All,
    /// Show only command actions.
    Commands,
    /// Show only workspace entries.
    Workspaces,
    /// Show only surface/tab entries.
    Surfaces,
}

impl PaletteFilter {
    /// All filter variants in display order.
    pub const ALL: [PaletteFilter; 4] = [
        PaletteFilter::All,
        PaletteFilter::Commands,
        PaletteFilter::Workspaces,
        PaletteFilter::Surfaces,
    ];

    /// Display label for this filter.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            PaletteFilter::All => "All",
            PaletteFilter::Commands => "Commands",
            PaletteFilter::Workspaces => "Workspaces",
            PaletteFilter::Surfaces => "Surfaces",
        }
    }

    /// Approximate pixel width of this filter's pill (label width + padding).
    #[must_use]
    pub fn pill_width(self) -> f32 {
        // Approximate character width for Segoe UI at CAPTION_FONT_SIZE (13px).
        // Worst-case proportional chars ("m", "W") are ~10px at 13px.
        // Using 10px to guarantee no clipping at any DPI scale.
        let char_w = 10.0;
        self.label().len() as f32 * char_w + 2.0 * FILTER_TAB_PAD_X
    }
}

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
    /// Active filter tab.
    pub filter: PaletteFilter,
    /// Measured pill widths for each filter tab (text_width + 2*padding).
    /// Set once during init from actual glyphon layout measurements.
    pub(crate) filter_pill_widths: [f32; 4],
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
        // Default pill widths from estimate — overridden by measured values during init.
        let default_widths = std::array::from_fn(|i| PaletteFilter::ALL[i].pill_width());
        Self {
            open: false,
            query: String::new(),
            selected: 0,
            result_count: 0,
            filter: PaletteFilter::All,
            filter_pill_widths: default_widths,
        }
    }

    /// Get the measured pill width for a filter variant.
    #[must_use]
    pub(crate) fn measured_pill_width(&self, filter: PaletteFilter) -> f32 {
        let idx = PaletteFilter::ALL
            .iter()
            .position(|&f| f == filter)
            .unwrap_or(0);
        self.filter_pill_widths[idx]
    }

    /// Open the palette and reset state.
    pub fn open(&mut self) {
        self.open = true;
        self.query.clear();
        self.selected = 0;
        self.result_count = 0;
        self.filter = PaletteFilter::All;
        tracing::debug!("command palette opened");
    }

    /// Close the palette.
    pub fn close(&mut self) {
        self.open = false;
        self.query.clear();
        self.selected = 0;
        self.result_count = 0;
        self.filter = PaletteFilter::All;
        tracing::debug!("command palette closed");
    }

    /// Cycle to the next filter tab.
    pub fn next_filter(&mut self) {
        let variants = PaletteFilter::ALL;
        let current = variants.iter().position(|&f| f == self.filter).unwrap_or(0);
        self.filter = variants[(current + 1) % variants.len()];
        self.selected = 0;
        self.result_count = 0;
        tracing::debug!(filter = ?self.filter, "palette filter changed");
    }

    /// Cycle to the previous filter tab.
    pub fn prev_filter(&mut self) {
        let variants = PaletteFilter::ALL;
        let current = variants.iter().position(|&f| f == self.filter).unwrap_or(0);
        self.filter = if current == 0 {
            variants[variants.len() - 1]
        } else {
            variants[current - 1]
        };
        self.selected = 0;
        self.result_count = 0;
        tracing::debug!(filter = ?self.filter, "palette filter changed");
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

        let ly = PaletteLayout::compute(
            surface_width,
            surface_height,
            self.result_count.min(MAX_VISIBLE_RESULTS),
        );

        // Fullscreen dimming overlay — two layered quads
        // First: overlay_dim (bg-tinted at 0.5 alpha)
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
            ly.palette_x - SHADOW_OFFSET,
            ly.palette_y - SHADOW_OFFSET,
            ly.effective_width + 2.0 * SHADOW_OFFSET,
            ly.total_height + 2.0 * SHADOW_OFFSET,
            ui_chrome.shadow,
            PALETTE_RADIUS + 2.0,
        );

        // Main background — surface_overlay already has 95% alpha baked in.
        let bg = ui_chrome.surface_overlay;
        quads.push_rounded_quad(
            ly.palette_x,
            ly.palette_y,
            ly.effective_width,
            ly.total_height,
            bg,
            PALETTE_RADIUS,
        );

        // Border (1px) — four thin quads
        let border_color = ui_chrome.border_subtle;
        quads.push_quad(
            ly.palette_x,
            ly.palette_y,
            ly.effective_width,
            BORDER_WIDTH,
            border_color,
        );
        quads.push_quad(
            ly.palette_x,
            ly.palette_y + ly.total_height - BORDER_WIDTH,
            ly.effective_width,
            BORDER_WIDTH,
            border_color,
        );
        quads.push_quad(
            ly.palette_x,
            ly.palette_y,
            BORDER_WIDTH,
            ly.total_height,
            border_color,
        );
        quads.push_quad(
            ly.palette_x + ly.effective_width - BORDER_WIDTH,
            ly.palette_y,
            BORDER_WIDTH,
            ly.total_height,
            border_color,
        );

        // Input field (rounded)
        quads.push_rounded_quad(
            ly.input_x,
            ly.input_y,
            ly.input_w,
            INPUT_HEIGHT,
            ui_chrome.surface_0,
            8.0,
        );

        // Filter tabs row — pill-shaped tabs below the input
        let mut tab_x = ly.input_x;
        for variant in PaletteFilter::ALL {
            let pill_w = self.measured_pill_width(variant);
            let pill_h = FILTER_ROW_HEIGHT - 2.0 * FILTER_TAB_PAD_Y;
            let pill_y = ly.filter_y + FILTER_TAB_PAD_Y;

            if variant == self.filter {
                // Active tab: accent background
                quads.push_rounded_quad(
                    tab_x,
                    pill_y,
                    pill_w,
                    pill_h,
                    ui_chrome.accent,
                    TAB_RADIUS,
                );
            } else {
                // Inactive tab: subtle border pill
                quads.push_rounded_quad(
                    tab_x,
                    pill_y,
                    pill_w,
                    pill_h,
                    ui_chrome.surface_0,
                    TAB_RADIUS,
                );
                // 1px border around inactive pill
                quads.push_rounded_quad(
                    tab_x,
                    pill_y,
                    pill_w,
                    BORDER_WIDTH,
                    ui_chrome.border_subtle,
                    TAB_RADIUS,
                );
            }
            tab_x += pill_w + FILTER_TAB_GAP;
        }

        // Selected result highlight (rounded) — use surface_2 with TAB_RADIUS
        let visible_results = self.result_count.min(MAX_VISIBLE_RESULTS);
        if visible_results > 0 {
            let selected_visible = self.selected.min(visible_results - 1);
            let result_y = ly.results_y + selected_visible as f32 * RESULT_HEIGHT;
            quads.push_rounded_quad(
                ly.input_x,
                result_y,
                ly.input_w,
                RESULT_HEIGHT,
                ui_chrome.surface_2,
                TAB_RADIUS,
            );
        }
    }

    /// Get the layout rect for the palette (for hit testing).
    #[must_use]
    #[allow(dead_code)] // Used in tests, not yet in production hit-testing.
    pub(crate) fn layout_rect(
        &self,
        surface_width: f32,
        surface_height: f32,
    ) -> (f32, f32, f32, f32) {
        let ly = PaletteLayout::compute(
            surface_width,
            surface_height,
            self.result_count.min(MAX_VISIBLE_RESULTS),
        );
        (
            ly.palette_x,
            ly.palette_y,
            ly.effective_width,
            ly.total_height,
        )
    }

    /// Check if a screen position is inside the palette.
    #[must_use]
    #[allow(dead_code)] // Used in tests, not yet in production hit-testing.
    pub(crate) fn contains(&self, x: f32, y: f32, surface_width: f32, surface_height: f32) -> bool {
        if !self.open {
            return false;
        }
        let (px, py, pw, ph) = self.layout_rect(surface_width, surface_height);
        x >= px && x <= px + pw && y >= py && y <= py + ph
    }
}

/// Map a `CommandRegistry` entry ID to a `ShortcutAction`.
///
/// Returns `None` for unrecognised IDs (e.g. workspace/surface entries
/// that don't map to a shortcut action).
#[must_use]
pub(crate) fn command_id_to_action(id: &str) -> Option<ShortcutAction> {
    match id {
        "split_right" => Some(ShortcutAction::SplitRight),
        "split_left" => Some(ShortcutAction::SplitLeft),
        "split_down" => Some(ShortcutAction::SplitDown),
        "split_up" => Some(ShortcutAction::SplitUp),
        "close_pane" => Some(ShortcutAction::ClosePane),
        "zoom_toggle" => Some(ShortcutAction::ZoomToggle),
        "focus_up" => Some(ShortcutAction::FocusUp),
        "focus_down" => Some(ShortcutAction::FocusDown),
        "focus_left" => Some(ShortcutAction::FocusLeft),
        "focus_right" => Some(ShortcutAction::FocusRight),
        "new_workspace" => Some(ShortcutAction::NewWorkspace),
        "close_workspace" => Some(ShortcutAction::CloseWorkspace),
        "new_surface" => Some(ShortcutAction::NewSurface),
        "new_browser_surface" => Some(ShortcutAction::NewBrowserSurface),
        "cycle_surface_forward" => Some(ShortcutAction::CycleSurfaceForward),
        "cycle_surface_backward" => Some(ShortcutAction::CycleSurfaceBackward),
        "toggle_sidebar" => Some(ShortcutAction::ToggleSidebar),
        "copy" => Some(ShortcutAction::Copy),
        "paste" => Some(ShortcutAction::Paste),
        "find" => Some(ShortcutAction::Find),
        "toggle_notification_panel" => Some(ShortcutAction::NotificationPanelToggle),
        "jump_last_unread" => Some(ShortcutAction::JumpLastUnread),
        _ => None,
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
        p.filter = PaletteFilter::Workspaces;
        p.open();
        assert!(p.open);
        assert!(p.query.is_empty());
        assert_eq!(p.selected, 0);
        assert_eq!(p.filter, PaletteFilter::All);
    }

    #[test]
    fn close_resets_state() {
        let mut p = CommandPalette::new();
        p.open();
        p.query = "test".into();
        p.filter = PaletteFilter::Commands;
        p.close();
        assert!(!p.open);
        assert!(p.query.is_empty());
        assert_eq!(p.filter, PaletteFilter::All);
    }

    #[test]
    fn next_filter_cycles() {
        let mut p = CommandPalette::new();
        assert_eq!(p.filter, PaletteFilter::All);
        p.next_filter();
        assert_eq!(p.filter, PaletteFilter::Commands);
        p.next_filter();
        assert_eq!(p.filter, PaletteFilter::Workspaces);
        p.next_filter();
        assert_eq!(p.filter, PaletteFilter::Surfaces);
        p.next_filter(); // wrap
        assert_eq!(p.filter, PaletteFilter::All);
    }

    #[test]
    fn prev_filter_cycles() {
        let mut p = CommandPalette::new();
        p.prev_filter(); // wrap to last
        assert_eq!(p.filter, PaletteFilter::Surfaces);
        p.prev_filter();
        assert_eq!(p.filter, PaletteFilter::Workspaces);
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
