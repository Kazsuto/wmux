use wmux_core::cell::Row;
use wmux_core::Grid;
use wmux_render::quad::QuadPipeline;

/// A match position in the terminal buffer (scrollback + grid combined).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    /// Absolute row index (0 = first scrollback row, scrollback_len + n = grid row n).
    pub row: usize,
    /// Byte/char column start (inclusive).
    pub col_start: usize,
    /// Byte/char column end (exclusive).
    pub col_end: usize,
}

/// Search overlay state for a single pane.
///
/// Tracks the active query, all match positions, and which match is currently
/// focused for navigation. The `search()` method is called on every keystroke
/// to update match positions incrementally.
#[derive(Debug)]
pub struct SearchState {
    pub active: bool,
    pub query: String,
    pub matches: Vec<SearchMatch>,
    /// Index into `matches` for the currently highlighted ("focused") match.
    pub current_match: usize,
    pub use_regex: bool,
}

impl Default for SearchState {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchState {
    pub fn new() -> Self {
        Self {
            active: false,
            query: String::new(),
            matches: Vec::new(),
            current_match: 0,
            use_regex: false,
        }
    }

    /// Open the search overlay and reset the query.
    pub fn open(&mut self) {
        self.active = true;
        self.query.clear();
        self.matches.clear();
        self.current_match = 0;
    }

    /// Close the search overlay and clear all state.
    pub fn close(&mut self) {
        self.active = false;
        self.query.clear();
        self.matches.clear();
        self.current_match = 0;
    }

    /// Advance to the next match, wrapping around.
    pub fn next_match(&mut self) {
        if !self.matches.is_empty() {
            self.current_match = (self.current_match + 1) % self.matches.len();
        }
    }

    /// Go to the previous match, wrapping around.
    pub fn prev_match(&mut self) {
        if !self.matches.is_empty() {
            self.current_match = if self.current_match == 0 {
                self.matches.len() - 1
            } else {
                self.current_match - 1
            };
        }
    }

    /// Search through grid + scrollback content.
    ///
    /// `rows` is a slice of `(absolute_row_index, row_text)` pairs.
    /// Call this on every query change.
    pub fn search(&mut self, rows: &[(usize, String)]) {
        self.matches.clear();
        self.current_match = 0;

        if self.query.is_empty() {
            return;
        }

        if self.use_regex {
            match regex::Regex::new(&self.query) {
                Ok(re) => {
                    for (row_idx, text) in rows {
                        for mat in re.find_iter(text) {
                            self.matches.push(SearchMatch {
                                row: *row_idx,
                                col_start: mat.start(),
                                col_end: mat.end(),
                            });
                        }
                    }
                }
                Err(e) => {
                    tracing::debug!(query = %self.query, error = %e, "invalid regex in search");
                }
            }
        } else {
            let query_lower = self.query.to_lowercase();
            let query_char_count = query_lower.chars().count();
            for (row_idx, text) in rows {
                // Use str::find for efficient substring search (uses memchr
                // internally), then convert byte offsets to char columns only
                // on matches. Avoids Vec<char> allocations per row.
                let text_lower = text.to_lowercase();
                let mut byte_start = 0;
                while let Some(rel_pos) = text_lower[byte_start..].find(&query_lower) {
                    let abs_byte = byte_start + rel_pos;
                    let col_start = text_lower[..abs_byte].chars().count();
                    self.matches.push(SearchMatch {
                        row: *row_idx,
                        col_start,
                        col_end: col_start + query_char_count,
                    });
                    // Advance past this match (by bytes, at least one char).
                    byte_start = abs_byte + query_lower.len();
                }
            }
        }
    }

    /// Match count display string, e.g. "3/15 matches" or "No matches".
    #[must_use]
    pub fn match_count_display(&self) -> String {
        if self.matches.is_empty() {
            "No matches".to_owned()
        } else {
            format!("{}/{} matches", self.current_match + 1, self.matches.len())
        }
    }

    /// Whether the search bar should display a regex error indicator.
    #[must_use]
    pub fn has_regex_error(&self) -> bool {
        self.use_regex && !self.query.is_empty() && regex::Regex::new(&self.query).is_err()
    }
}

// ─── Grid text extraction ────────────────────────────────────────────────────

/// Extract searchable text rows from a scrollback slice + grid.
///
/// Returns a `Vec<(absolute_row_index, row_text)>` where row 0 is the oldest
/// visible scrollback row. Grid rows follow scrollback rows without a gap.
///
/// The `absolute_row_index` matches the coordinate space used by
/// [`render_search_highlights`].
#[must_use]
pub fn extract_rows(scrollback: &[Row], grid: &Grid) -> Vec<(usize, String)> {
    let mut out = Vec::with_capacity(scrollback.len() + grid.rows() as usize);

    for (i, row) in scrollback.iter().enumerate() {
        let text: String = row.iter().map(|c| c.grapheme.as_str()).collect();
        out.push((i, text));
    }

    let offset = scrollback.len();
    for row_idx in 0..grid.rows() {
        let mut text = String::with_capacity(grid.cols() as usize);
        for col_idx in 0..grid.cols() {
            text.push_str(grid.cell(col_idx, row_idx).grapheme.as_str());
        }
        out.push((offset + row_idx as usize, text));
    }

    out
}

// ─── Highlight rendering ─────────────────────────────────────────────────────

/// Overlay semi-transparent highlight quads for all visible search matches.
///
/// Should be called after terminal content is rendered and before
/// `quads.prepare()` is called (i.e. still within the quad accumulation phase).
///
/// Match row indices use the same local coordinate space produced by
/// [`extract_rows`]: row 0 is the topmost visible line (first scrollback visible
/// row, or first grid row when no scrollback is shown), and row N is the Nth
/// visible line on screen. All matches produced from `extract_rows` data are
/// therefore automatically in-bounds and visible.
///
/// # Parameters
/// - `search` — current search state (matches from [`SearchState::search`])
/// - `quad_pipeline` — mutable quad accumulator
/// - `pane_rect` — screen-space origin and size of the focused pane
/// - `cell_width` / `cell_height` — glyph cell dimensions in pixels
/// - `total_visible_rows` — total visible rows (scrollback_visible.len() + grid_rows)
pub fn render_search_highlights(
    search: &SearchState,
    quad_pipeline: &mut QuadPipeline,
    ui_chrome: &wmux_config::UiChrome,
    pane_rect: &wmux_core::rect::Rect,
    cell_width: f32,
    cell_height: f32,
    total_visible_rows: usize,
) {
    if !search.active || search.matches.is_empty() {
        return;
    }

    for (i, m) in search.matches.iter().enumerate() {
        // Guard: skip any row that falls outside the visible area.
        if m.row >= total_visible_rows {
            continue;
        }
        let x = pane_rect.x + m.col_start as f32 * cell_width;
        let y = pane_rect.y + m.row as f32 * cell_height;
        let w = (m.col_end.saturating_sub(m.col_start)) as f32 * cell_width;

        let color = if i == search.current_match {
            ui_chrome.search_match_active
        } else {
            ui_chrome.search_match
        };

        quad_pipeline.push_quad(x, y, w, cell_height, color);
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(data: &[(usize, &str)]) -> Vec<(usize, String)> {
        data.iter().map(|(i, s)| (*i, s.to_string())).collect()
    }

    #[test]
    fn new_state_is_inactive() {
        let s = SearchState::new();
        assert!(!s.active);
        assert!(s.query.is_empty());
        assert!(s.matches.is_empty());
    }

    #[test]
    fn open_clears_previous_state() {
        let mut s = SearchState::new();
        s.query = "foo".to_owned();
        s.matches.push(SearchMatch {
            row: 0,
            col_start: 0,
            col_end: 3,
        });
        s.open();
        assert!(s.active);
        assert!(s.query.is_empty());
        assert!(s.matches.is_empty());
    }

    #[test]
    fn close_clears_state() {
        let mut s = SearchState::new();
        s.active = true;
        s.query = "x".to_owned();
        s.close();
        assert!(!s.active);
        assert!(s.query.is_empty());
        assert!(s.matches.is_empty());
    }

    #[test]
    fn search_empty_query_produces_no_matches() {
        let mut s = SearchState::new();
        s.search(&rows(&[(0, "hello world")]));
        assert!(s.matches.is_empty());
    }

    #[test]
    fn search_finds_simple_match() {
        let mut s = SearchState::new();
        s.query = "ello".to_owned();
        s.search(&rows(&[(0, "hello world")]));
        assert_eq!(s.matches.len(), 1);
        assert_eq!(s.matches[0].col_start, 1);
        assert_eq!(s.matches[0].col_end, 5);
    }

    #[test]
    fn search_is_case_insensitive() {
        let mut s = SearchState::new();
        s.query = "HELLO".to_owned();
        s.search(&rows(&[(0, "hello world")]));
        assert_eq!(s.matches.len(), 1);
    }

    #[test]
    fn search_finds_multiple_matches_on_same_row() {
        let mut s = SearchState::new();
        s.query = "ab".to_owned();
        s.search(&rows(&[(0, "ababab")]));
        assert_eq!(s.matches.len(), 3);
        assert_eq!(s.matches[0].col_start, 0);
        assert_eq!(s.matches[1].col_start, 2);
        assert_eq!(s.matches[2].col_start, 4);
    }

    #[test]
    fn search_across_multiple_rows() {
        let mut s = SearchState::new();
        s.query = "foo".to_owned();
        s.search(&rows(&[(0, "foo bar"), (1, "baz"), (2, "foo foo")]));
        assert_eq!(s.matches.len(), 3);
        assert_eq!(s.matches[0].row, 0);
        assert_eq!(s.matches[1].row, 2);
        assert_eq!(s.matches[2].row, 2);
    }

    #[test]
    fn search_no_match_returns_empty() {
        let mut s = SearchState::new();
        s.query = "xyz".to_owned();
        s.search(&rows(&[(0, "hello world"), (1, "foo bar")]));
        assert!(s.matches.is_empty());
    }

    #[test]
    fn next_match_wraps_around() {
        let mut s = SearchState::new();
        s.query = "x".to_owned();
        s.search(&rows(&[(0, "x x x")]));
        assert_eq!(s.matches.len(), 3);
        assert_eq!(s.current_match, 0);
        s.next_match();
        assert_eq!(s.current_match, 1);
        s.next_match();
        assert_eq!(s.current_match, 2);
        s.next_match(); // wrap
        assert_eq!(s.current_match, 0);
    }

    #[test]
    fn prev_match_wraps_around() {
        let mut s = SearchState::new();
        s.query = "x".to_owned();
        s.search(&rows(&[(0, "x x x")]));
        assert_eq!(s.current_match, 0);
        s.prev_match(); // wrap to last
        assert_eq!(s.current_match, 2);
        s.prev_match();
        assert_eq!(s.current_match, 1);
    }

    #[test]
    fn next_prev_on_empty_matches_does_not_panic() {
        let mut s = SearchState::new();
        s.next_match();
        s.prev_match();
        assert_eq!(s.current_match, 0);
    }

    #[test]
    fn match_count_display_no_matches() {
        let s = SearchState::new();
        assert_eq!(s.match_count_display(), "No matches");
    }

    #[test]
    fn match_count_display_with_matches() {
        let mut s = SearchState::new();
        s.query = "a".to_owned();
        s.search(&rows(&[(0, "aaa")]));
        assert_eq!(s.match_count_display(), "1/3 matches");
        s.next_match();
        assert_eq!(s.match_count_display(), "2/3 matches");
    }

    #[test]
    fn regex_mode_finds_pattern() {
        let mut s = SearchState::new();
        s.use_regex = true;
        s.query = r"\d+".to_owned();
        s.search(&rows(&[(0, "abc 123 def 456")]));
        assert_eq!(s.matches.len(), 2);
        assert_eq!(s.matches[0].col_start, 4);
        assert_eq!(s.matches[0].col_end, 7);
        assert_eq!(s.matches[1].col_start, 12);
        assert_eq!(s.matches[1].col_end, 15);
    }

    #[test]
    fn regex_invalid_pattern_produces_no_matches() {
        let mut s = SearchState::new();
        s.use_regex = true;
        s.query = r"[invalid".to_owned();
        s.search(&rows(&[(0, "hello world")]));
        assert!(s.matches.is_empty());
    }

    #[test]
    fn has_regex_error_on_invalid_pattern() {
        let mut s = SearchState::new();
        s.use_regex = true;
        s.query = r"[broken".to_owned();
        assert!(s.has_regex_error());
    }

    #[test]
    fn has_regex_error_false_on_valid_pattern() {
        let mut s = SearchState::new();
        s.use_regex = true;
        s.query = r"\w+".to_owned();
        assert!(!s.has_regex_error());
    }

    #[test]
    fn has_regex_error_false_when_not_regex_mode() {
        let mut s = SearchState::new();
        s.use_regex = false;
        s.query = "[broken".to_owned(); // valid literal string
        assert!(!s.has_regex_error());
    }
}
