use wmux_config::UiChrome;
use wmux_core::rect::Rect;
use wmux_render::quad::QuadPipeline;

/// Height of the address bar in logical pixels (before DPI scaling).
pub const ADDRESS_BAR_HEIGHT: f32 = 32.0;

/// Button width for back/forward navigation icons.
const NAV_BUTTON_WIDTH: f32 = 28.0;

/// Horizontal padding inside the URL text field.
const URL_FIELD_PADDING: f32 = 8.0;

/// Result of hit-testing a click against the address bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressBarHit {
    /// Back navigation button.
    Back,
    /// Forward navigation button.
    Forward,
    /// URL text input field.
    UrlField,
    /// Click was not inside the address bar.
    None,
}

/// State for the browser address bar.
///
/// Tracks the displayed URL, edit mode, and cursor position.
/// A single instance exists in `UiState` — it reflects the URL
/// of whichever browser surface is currently focused.
#[derive(Debug)]
pub struct AddressBarState {
    /// The URL text currently shown (or being edited).
    pub url: String,
    /// Whether the user is actively typing in the address bar.
    pub editing: bool,
    /// Char-index cursor position within `url` (not byte offset).
    pub cursor_pos: usize,
    /// Whether all text is selected (Ctrl+A).
    pub selected_all: bool,
    /// Snapshot of the URL before editing started (for Escape revert).
    committed_url: String,
}

impl Default for AddressBarState {
    fn default() -> Self {
        Self::new()
    }
}

/// Check whether a URL uses a dangerous scheme that should be blocked.
fn is_dangerous_scheme(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    let dangerous = ["javascript:", "data:", "vbscript:", "blob:"];
    dangerous.iter().any(|s| lower.starts_with(s))
}

/// Percent-encode a search query string for use in a URL query parameter.
fn encode_query(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 3);
    for b in input.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push(char::from(HEX_UPPER[(b >> 4) as usize]));
                out.push(char::from(HEX_UPPER[(b & 0x0F) as usize]));
            }
        }
    }
    out
}

const HEX_UPPER: [u8; 16] = *b"0123456789ABCDEF";

impl AddressBarState {
    pub fn new() -> Self {
        Self {
            url: String::new(),
            editing: false,
            cursor_pos: 0,
            selected_all: false,
            committed_url: String::new(),
        }
    }

    /// Enter edit mode — select all text for easy replacement.
    pub fn start_editing(&mut self) {
        self.committed_url = self.url.clone();
        self.editing = true;
        self.cursor_pos = self.url.chars().count();
        self.selected_all = true;
    }

    /// Exit edit mode without navigating — revert to the committed URL.
    pub fn cancel_editing(&mut self) {
        self.url = self.committed_url.clone();
        self.editing = false;
        self.cursor_pos = 0;
        self.selected_all = false;
    }

    /// Confirm navigation — returns the URL to navigate to.
    /// Prepends `https://` if no scheme is present.
    /// Blocks dangerous schemes (`javascript:`, `data:`, `vbscript:`).
    pub fn confirm_editing(&mut self) -> String {
        self.editing = false;
        let url = self.url.trim().to_string();
        let navigated = if url.is_empty() {
            self.committed_url.clone()
        } else if is_dangerous_scheme(&url) {
            // Block dangerous schemes — revert to committed URL.
            self.committed_url.clone()
        } else if url.starts_with("https://")
            || url.starts_with("http://")
            || url.starts_with("about:")
            || url.starts_with("file:")
        {
            url
        } else if url.contains('.') || url.starts_with("localhost") {
            format!("https://{url}")
        } else {
            // Treat as a search query with proper percent-encoding.
            let encoded = encode_query(&url);
            format!("https://duckduckgo.com/?q={encoded}")
        };
        self.url = navigated.clone();
        self.committed_url = navigated.clone();
        self.cursor_pos = 0;
        navigated
    }

    /// Update the displayed URL (called after navigation completes).
    pub fn set_url(&mut self, url: &str) {
        if !self.editing {
            self.url = url.to_string();
            self.committed_url = url.to_string();
            self.cursor_pos = 0;
        }
    }

    /// Push quads for the address bar background and nav buttons.
    ///
    /// `bar_rect` is the full address bar rectangle (below the tab bar).
    pub fn render_quads(
        &self,
        quads: &mut QuadPipeline,
        bar_rect: &Rect,
        chrome: &UiChrome,
        scale: f32,
    ) {
        let h = ADDRESS_BAR_HEIGHT * scale;
        let btn_w = NAV_BUTTON_WIDTH * scale;

        // Address bar background — same elevation as tab bar.
        quads.push_quad(bar_rect.x, bar_rect.y, bar_rect.width, h, chrome.surface_1);

        // Bottom border.
        quads.push_quad(
            bar_rect.x,
            bar_rect.y + h - scale,
            bar_rect.width,
            scale,
            chrome.border_subtle,
        );

        // Back button background (subtle on hover — for now always subtle).
        quads.push_quad(bar_rect.x, bar_rect.y, btn_w, h, chrome.surface_1);

        // Divider after back button.
        quads.push_quad(
            bar_rect.x + btn_w,
            bar_rect.y + 6.0 * scale,
            scale,
            h - 12.0 * scale,
            chrome.border_subtle,
        );

        // Forward button background.
        quads.push_quad(bar_rect.x + btn_w, bar_rect.y, btn_w, h, chrome.surface_1);

        // Divider after forward button.
        quads.push_quad(
            bar_rect.x + 2.0 * btn_w,
            bar_rect.y + 6.0 * scale,
            scale,
            h - 12.0 * scale,
            chrome.border_subtle,
        );

        // URL field background — slightly recessed.
        let url_x = bar_rect.x + 2.0 * btn_w + 4.0 * scale;
        let url_w = bar_rect.width - 2.0 * btn_w - 8.0 * scale;
        let url_y = bar_rect.y + 4.0 * scale;
        let url_h = h - 8.0 * scale;

        quads.push_quad(url_x, url_y, url_w, url_h, chrome.surface_0);

        // Selection highlight — accent at 30% alpha over the URL field.
        if self.selected_all && self.editing {
            let pad = URL_FIELD_PADDING * scale;
            quads.push_quad(
                url_x + pad,
                url_y + 2.0 * scale,
                url_w - 2.0 * pad,
                url_h - 4.0 * scale,
                chrome.accent_muted,
            );
        }

        // Editing cursor indicator.
        if self.editing {
            // Accent bottom border to show focus.
            quads.push_quad(
                url_x,
                url_y + url_h - 2.0 * scale,
                url_w,
                2.0 * scale,
                chrome.accent,
            );
        }
    }

    /// Hit-test a click at `(px, py)` against the address bar.
    ///
    /// `bar_rect` is the full address bar rectangle.
    pub fn hit_test(&self, px: f32, py: f32, bar_rect: &Rect, scale: f32) -> AddressBarHit {
        let h = ADDRESS_BAR_HEIGHT * scale;
        let btn_w = NAV_BUTTON_WIDTH * scale;

        if py < bar_rect.y
            || py >= bar_rect.y + h
            || px < bar_rect.x
            || px >= bar_rect.x + bar_rect.width
        {
            return AddressBarHit::None;
        }

        let rel_x = px - bar_rect.x;
        if rel_x < btn_w {
            AddressBarHit::Back
        } else if rel_x < 2.0 * btn_w {
            AddressBarHit::Forward
        } else {
            AddressBarHit::UrlField
        }
    }

    /// Return the rect for the URL text area (for glyphon TextBounds).
    pub fn url_text_rect(bar_rect: &Rect, scale: f32) -> Rect {
        let btn_w = NAV_BUTTON_WIDTH * scale;
        let pad = URL_FIELD_PADDING * scale;
        let x = bar_rect.x + 2.0 * btn_w + 4.0 * scale + pad;
        let y = bar_rect.y + 4.0 * scale;
        let w = bar_rect.width - 2.0 * btn_w - 8.0 * scale - 2.0 * pad;
        let h = ADDRESS_BAR_HEIGHT * scale - 8.0 * scale;
        Rect::new(x, y, w, h)
    }

    /// Return the center position for the back button icon.
    pub fn back_button_center(bar_rect: &Rect, scale: f32) -> (f32, f32) {
        let btn_w = NAV_BUTTON_WIDTH * scale;
        let h = ADDRESS_BAR_HEIGHT * scale;
        (bar_rect.x + btn_w / 2.0, bar_rect.y + h / 2.0)
    }

    /// Return the center position for the forward button icon.
    pub fn forward_button_center(bar_rect: &Rect, scale: f32) -> (f32, f32) {
        let btn_w = NAV_BUTTON_WIDTH * scale;
        let h = ADDRESS_BAR_HEIGHT * scale;
        (bar_rect.x + btn_w + btn_w / 2.0, bar_rect.y + h / 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_state_defaults() {
        let s = AddressBarState::new();
        assert!(!s.editing);
        assert!(s.url.is_empty());
        assert_eq!(s.cursor_pos, 0);
    }

    #[test]
    fn start_editing_saves_committed_url() {
        let mut s = AddressBarState::new();
        s.set_url("https://example.com");
        s.start_editing();
        assert!(s.editing);
        assert_eq!(s.cursor_pos, s.url.len());
    }

    #[test]
    fn cancel_editing_reverts_url() {
        let mut s = AddressBarState::new();
        s.set_url("https://example.com");
        s.start_editing();
        s.url = "https://changed.com".to_string();
        s.cancel_editing();
        assert!(!s.editing);
        assert_eq!(s.url, "https://example.com");
    }

    #[test]
    fn confirm_prepends_scheme() {
        let mut s = AddressBarState::new();
        s.start_editing();
        s.url = "example.com".to_string();
        let url = s.confirm_editing();
        assert_eq!(url, "https://example.com");
        assert!(!s.editing);
    }

    #[test]
    fn confirm_preserves_existing_scheme() {
        let mut s = AddressBarState::new();
        s.start_editing();
        s.url = "http://example.com".to_string();
        let url = s.confirm_editing();
        assert_eq!(url, "http://example.com");
    }

    #[test]
    fn confirm_localhost_gets_scheme() {
        let mut s = AddressBarState::new();
        s.start_editing();
        s.url = "localhost:3000".to_string();
        let url = s.confirm_editing();
        assert_eq!(url, "https://localhost:3000");
    }

    #[test]
    fn confirm_search_query() {
        let mut s = AddressBarState::new();
        s.start_editing();
        s.url = "rust async tutorial".to_string();
        let url = s.confirm_editing();
        assert!(url.starts_with("https://duckduckgo.com/?q="));
        assert!(url.contains("rust"));
    }

    #[test]
    fn confirm_blocks_javascript_scheme() {
        let mut s = AddressBarState::new();
        s.set_url("https://safe.com");
        s.start_editing();
        s.url = "javascript:alert(1)".to_string();
        let url = s.confirm_editing();
        assert_eq!(url, "https://safe.com"); // reverted to committed
    }

    #[test]
    fn confirm_blocks_data_scheme() {
        let mut s = AddressBarState::new();
        s.set_url("https://safe.com");
        s.start_editing();
        s.url = "data:text/html,<script>alert(1)</script>".to_string();
        let url = s.confirm_editing();
        assert_eq!(url, "https://safe.com");
    }

    #[test]
    fn confirm_encodes_special_chars_in_search() {
        let mut s = AddressBarState::new();
        s.start_editing();
        s.url = "A&B=C".to_string();
        let url = s.confirm_editing();
        assert!(url.contains("A%26B%3DC"));
    }

    #[test]
    fn hit_test_back_button() {
        let bar = Rect::new(100.0, 200.0, 800.0, 32.0);
        let s = AddressBarState::new();
        assert_eq!(s.hit_test(110.0, 210.0, &bar, 1.0), AddressBarHit::Back);
    }

    #[test]
    fn hit_test_forward_button() {
        let bar = Rect::new(100.0, 200.0, 800.0, 32.0);
        let s = AddressBarState::new();
        assert_eq!(s.hit_test(140.0, 210.0, &bar, 1.0), AddressBarHit::Forward);
    }

    #[test]
    fn hit_test_url_field() {
        let bar = Rect::new(100.0, 200.0, 800.0, 32.0);
        let s = AddressBarState::new();
        assert_eq!(s.hit_test(300.0, 210.0, &bar, 1.0), AddressBarHit::UrlField);
    }

    #[test]
    fn hit_test_outside() {
        let bar = Rect::new(100.0, 200.0, 800.0, 32.0);
        let s = AddressBarState::new();
        assert_eq!(s.hit_test(50.0, 210.0, &bar, 1.0), AddressBarHit::None);
        assert_eq!(s.hit_test(300.0, 300.0, &bar, 1.0), AddressBarHit::None);
    }

    #[test]
    fn set_url_when_not_editing() {
        let mut s = AddressBarState::new();
        s.set_url("https://new.com");
        assert_eq!(s.url, "https://new.com");
    }

    #[test]
    fn set_url_ignored_when_editing() {
        let mut s = AddressBarState::new();
        s.set_url("https://old.com");
        s.start_editing();
        s.url = "typing...".to_string();
        s.set_url("https://new.com"); // should be ignored
        assert_eq!(s.url, "typing...");
    }
}
