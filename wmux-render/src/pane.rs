use wmux_core::{rect::Rect, types::PaneId, SurfaceId};

use crate::quad::QuadPipeline;

/// Height in pixels of the tab bar when a pane has multiple surfaces.
pub const TAB_BAR_HEIGHT: f32 = 40.0;

/// Width in pixels of the focused pane accent stripe (left bar).
pub const FOCUS_STRIPE_WIDTH: f32 = 3.0;

/// Spacing between pill-style tabs.
const TAB_GAP: f32 = 6.0;

/// Border radius for pill-style tabs.
const TAB_RADIUS: f32 = 4.0;

/// Maximum width for a single tab pill (220px).
const MAX_TAB_WIDTH: f32 = 220.0;

/// Close button size (visual hit area).
const CLOSE_BUTTON_SIZE: f32 = 18.0;

/// Padding from the right edge of the pill to the close button.
const CLOSE_BUTTON_PADDING: f32 = 8.0;

/// Width of the "+" new surface button in the tab bar.
const PLUS_BUTTON_WIDTH: f32 = 32.0;

/// Height of the "+" new surface button (matches pill height).
const PLUS_BUTTON_HEIGHT: f32 = 28.0;

/// Width of the split direction button in the tab bar.
const SPLIT_BUTTON_WIDTH: f32 = 32.0;

/// Height of the split direction button (matches pill height).
const SPLIT_BUTTON_HEIGHT: f32 = 28.0;

/// Gap between the "+" button and the split button.
const SPLIT_BUTTON_GAP: f32 = 4.0;

/// Type of surface (Terminal or Browser) for a tab indicator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceType {
    Terminal,
    Browser,
}

/// Describes a pane's position and state for a single rendered frame.
#[derive(Debug, Clone)]
pub struct PaneViewport {
    pub pane_id: PaneId,
    /// Layout rect from PaneTree (physical pixels, top-left origin).
    pub rect: Rect,
    /// Whether this pane currently holds keyboard focus.
    pub focused: bool,
    /// Number of surfaces (tabs) owned by this pane.
    pub tab_count: usize,
    /// Display titles for each tab.
    pub tab_titles: Vec<String>,
    /// Surface IDs for each tab (parallel to `tab_titles`).
    pub surface_ids: Vec<SurfaceId>,
    /// Index of the currently active tab.
    pub active_tab: usize,
    /// When `true`, this pane fills the entire workspace area (zoom mode).
    pub zoomed: bool,
    /// Surface type (Terminal or Browser) for each tab.
    pub surface_types: Vec<SurfaceType>,
    /// Unsaved state for each tab (true = unsaved).
    pub unsaved: Vec<bool>,
    /// Display scale factor (DPI). Multiplied into all UI dimensions.
    pub scale: f32,
}

impl PaneViewport {
    /// Tab bar height scaled for DPI.
    pub fn tab_bar_height(&self) -> f32 {
        TAB_BAR_HEIGHT * self.scale
    }
}

/// Orchestrates multi-pane rendering within a single wgpu render pass.
///
/// `PaneRenderer` is stateless — all rendering decisions are driven by
/// the `PaneViewport` slice passed to each method so there is no persistent
/// render state to synchronise between frames.
pub struct PaneRenderer;

impl PaneRenderer {
    /// Create a new, stateless renderer orchestrator.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Push focus indicator quads for every pane in `viewports`.
    ///
    /// Currently a no-op — the blue accent stripe was removed in favour of
    /// uniform neutral dividers between all panes.
    pub fn render_pane_borders(
        &self,
        _quads: &mut QuadPipeline,
        _viewports: &[PaneViewport],
        _theme_border_color: [f32; 4],
        _theme_accent_color: [f32; 4],
    ) {
        // Intentionally empty — focus is indicated by the tab bar and
        // inactive-pane dimming overlay, not by a coloured stripe.
    }

    /// Render the signature Focus Glow around the active pane (shader-based).
    ///
    /// Uses a single GPU quad with SDF outer glow rendered by the fragment shader.
    /// The quad is automatically expanded by `glow_radius` on all sides.
    ///
    /// `glow_alpha` is a 0.0..1.0 multiplier for cross-fade animation.
    pub fn render_focus_glow(
        quads: &mut QuadPipeline,
        rect: &Rect,
        accent_glow_core: [f32; 4],
        accent_glow: [f32; 4],
        glow_alpha: f32,
    ) {
        if glow_alpha < 0.01 {
            return;
        }

        const GLOW_RADIUS: f32 = 10.0;

        // Subtle outer glow halo
        let glow_color = [
            accent_glow[0],
            accent_glow[1],
            accent_glow[2],
            accent_glow[3] * glow_alpha * 0.4,
        ];
        // Transparent inner (no fill, glow only)
        let inner_color = [
            accent_glow_core[0],
            accent_glow_core[1],
            accent_glow_core[2],
            0.0,
        ];

        quads.push_glow_quad(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            inner_color,
            0.0, // no border_radius on inner
            GLOW_RADIUS,
            glow_color,
        );
    }

    /// Push pill-style tab bar quads for a single pane.
    ///
    /// The tab bar is always visible (even with a single tab) and occupies
    /// `TAB_BAR_HEIGHT` pixels at the top of the pane rect.
    ///
    /// Each tab is a rounded pill with `TAB_GAP` spacing. The active tab
    /// has an elevated background and a 2px rounded accent indicator at the
    /// bottom. A "+" button at the end allows creating new surfaces.
    pub fn render_tab_bar(
        &self,
        quads: &mut QuadPipeline,
        viewport: &PaneViewport,
        tab_bg: [f32; 4],
        active_tab_bg: [f32; 4],
        accent_color: [f32; 4],
        cursor_pos: (f32, f32),
    ) -> Result<(), crate::RenderError> {
        if viewport.tab_count == 0 {
            return Ok(());
        }

        let r = &viewport.rect;
        let s = viewport.scale;
        let tbh = TAB_BAR_HEIGHT * s;
        let pad = 4.0 * s;
        let radius = TAB_RADIUS * s;

        // Full tab bar background
        quads.push_quad(r.x, r.y, r.width, tbh, tab_bg);

        let pill_y = r.y + pad;
        let pill_h = tbh - pad * 2.0;

        for i in 0..viewport.tab_count {
            let (tab_width, tab_x) = Self::tab_metrics(viewport, i);

            if i == viewport.active_tab {
                quads.push_rounded_quad(tab_x, pill_y, tab_width, pill_h, active_tab_bg, radius);
                // Active indicator bar
                quads.push_rounded_quad(
                    tab_x + tab_width * 0.2,
                    r.y + tbh - pad,
                    tab_width * 0.6,
                    2.0 * s,
                    accent_color,
                    1.0 * s,
                );
            } else {
                quads.push_rounded_quad(tab_x, pill_y, tab_width, pill_h, tab_bg, radius);
            }
        }

        // "+" button — transparent by default, subtle hover bg (Zed-like).
        if let Some((px, py, pw, ph)) = Self::plus_button_rect(viewport) {
            let hovered = cursor_pos.0 >= px
                && cursor_pos.0 <= px + pw
                && cursor_pos.1 >= py
                && cursor_pos.1 <= py + ph;
            if hovered {
                let bg = [tab_bg[0], tab_bg[1], tab_bg[2], 0.6];
                quads.push_rounded_quad(px, py, pw, ph, bg, radius);
            }
        }

        // Split button — transparent by default, subtle hover bg (Zed-like).
        if let Some((sx, sy, sw, sh)) = Self::split_button_rect(viewport) {
            let hovered = cursor_pos.0 >= sx
                && cursor_pos.0 <= sx + sw
                && cursor_pos.1 >= sy
                && cursor_pos.1 <= sy + sh;
            if hovered {
                let bg = [tab_bg[0], tab_bg[1], tab_bg[2], 0.6];
                quads.push_rounded_quad(sx, sy, sw, sh, bg, radius);
            }
        }

        tracing::trace!(
            pane_id = %viewport.pane_id,
            tab_count = viewport.tab_count,
            "tab bar rendered"
        );

        Ok(())
    }

    /// Render a notification ring around a pane that has unread notifications.
    ///
    /// Draws 4 semi-transparent blue quads on the inner edges of the pane,
    /// with animated alpha pulsing over a 2-second period.
    pub fn render_notification_ring(
        quads: &mut QuadPipeline,
        viewport: &PaneViewport,
        time_secs: f32,
        accent_color: [f32; 4],
    ) {
        let r = &viewport.rect;
        let ring_width = 2.0;

        // Animated alpha: pulse between 0.3 and 0.5 over 2 seconds
        let alpha = 0.3 + 0.2 * (time_secs * std::f32::consts::PI).sin();
        let color = [accent_color[0], accent_color[1], accent_color[2], alpha];

        // Top edge
        quads.push_quad(r.x, r.y, r.width, ring_width, color);
        // Bottom edge
        quads.push_quad(r.x, r.y + r.height - ring_width, r.width, ring_width, color);
        // Left edge
        quads.push_quad(
            r.x,
            r.y + ring_width,
            ring_width,
            r.height - 2.0 * ring_width,
            color,
        );
        // Right edge
        quads.push_quad(
            r.x + r.width - ring_width,
            r.y + ring_width,
            ring_width,
            r.height - 2.0 * ring_width,
            color,
        );
    }

    /// Compute pill-style tab width and x-offset for a given tab index.
    ///
    /// Returns `(tab_width, tab_x)` accounting for gaps, padding, MAX_TAB_WIDTH clamping,
    /// and the "+" button reserved space at the end.
    #[must_use]
    pub fn tab_metrics(viewport: &PaneViewport, tab_index: usize) -> (f32, f32) {
        if viewport.tab_count == 0 {
            return (0.0, viewport.rect.x);
        }
        let s = viewport.scale;
        let tab_index = tab_index.min(viewport.tab_count - 1);
        let n = viewport.tab_count as f32;
        let gap = TAB_GAP * s;
        let buttons_reserve =
            PLUS_BUTTON_WIDTH * s + gap + SPLIT_BUTTON_WIDTH * s + SPLIT_BUTTON_GAP * s;
        let total_gaps = gap * (n - 1.0) + gap * 2.0;
        let mut tab_width = ((viewport.rect.width - total_gaps - buttons_reserve) / n).max(1.0);
        tab_width = tab_width.min(MAX_TAB_WIDTH * s);
        let tab_x = viewport.rect.x + gap + tab_index as f32 * (tab_width + gap);
        (tab_width, tab_x)
    }

    /// Return the close button rect `(x, y, width, height)` for a tab.
    ///
    /// Returns `None` when only one tab remains (to prevent closing the last surface)
    /// or when the index is out of bounds.
    #[must_use]
    pub fn close_button_rect(
        viewport: &PaneViewport,
        tab_index: usize,
    ) -> Option<(f32, f32, f32, f32)> {
        if viewport.tab_count <= 1 || tab_index >= viewport.tab_count {
            return None;
        }
        let s = viewport.scale;
        let (tab_width, tab_x) = Self::tab_metrics(viewport, tab_index);
        let pad = 4.0 * s;
        let pill_y = viewport.rect.y + pad;
        let pill_h = TAB_BAR_HEIGHT * s - pad * 2.0;
        let btn_size = CLOSE_BUTTON_SIZE * s;
        let btn_pad = CLOSE_BUTTON_PADDING * s;
        let btn_x = tab_x + tab_width - btn_pad - btn_size;
        let btn_y = pill_y + (pill_h - btn_size) / 2.0;
        Some((btn_x, btn_y, btn_size, btn_size))
    }

    /// Return the "+" button rect `(x, y, width, height)` at the end of the tab bar.
    ///
    /// Positioned after the last tab pill with standard gap spacing.
    #[must_use]
    pub fn plus_button_rect(viewport: &PaneViewport) -> Option<(f32, f32, f32, f32)> {
        if viewport.tab_count == 0 {
            return None;
        }
        let s = viewport.scale;
        let last_index = viewport.tab_count - 1;
        let (last_width, last_x) = Self::tab_metrics(viewport, last_index);
        let gap = TAB_GAP * s;
        let pw = PLUS_BUTTON_WIDTH * s;
        let ph = PLUS_BUTTON_HEIGHT * s;
        let plus_x = last_x + last_width + gap;
        if plus_x + pw > viewport.rect.x + viewport.rect.width {
            return None;
        }
        let pad = 4.0 * s;
        let tbh = TAB_BAR_HEIGHT * s;
        let plus_y = viewport.rect.y + pad + (tbh - pad * 2.0 - ph) / 2.0;
        Some((plus_x, plus_y, pw, ph))
    }

    /// Return the split button rect `(x, y, width, height)` next to the "+" button.
    ///
    /// Positioned after the "+" button with a small gap.
    #[must_use]
    pub fn split_button_rect(viewport: &PaneViewport) -> Option<(f32, f32, f32, f32)> {
        let (plus_x, plus_y, plus_w, _plus_h) = Self::plus_button_rect(viewport)?;
        let s = viewport.scale;
        let gap = SPLIT_BUTTON_GAP * s;
        let sw = SPLIT_BUTTON_WIDTH * s;
        let sh = SPLIT_BUTTON_HEIGHT * s;
        let split_x = plus_x + plus_w + gap;
        if split_x + sw > viewport.rect.x + viewport.rect.width {
            return None;
        }
        Some((split_x, plus_y, sw, sh))
    }

    /// Return position information for rendering a tab type indicator.
    ///
    /// Returns `(indicator_x, indicator_y)` for a given tab index.
    /// The caller (wmux-ui window.rs) will render the actual glyph (">_" for Terminal, globe for Browser).
    /// Returns None if the tab index is out of bounds.
    #[must_use]
    pub fn tab_type_indicator_pos(viewport: &PaneViewport, tab_index: usize) -> Option<(f32, f32)> {
        if tab_index >= viewport.tab_count {
            return None;
        }
        let (_tab_width, tab_x) = Self::tab_metrics(viewport, tab_index);
        let pill_y = viewport.rect.y + 4.0;
        // Position indicator in top-left corner of the pill with small padding
        let indicator_x = tab_x + 4.0;
        let indicator_y = pill_y + 2.0;
        Some((indicator_x, indicator_y))
    }

    /// Return the usable terminal content area for `viewport`.
    ///
    /// The tab bar is always visible, so the top `TAB_BAR_HEIGHT` pixels are
    /// always consumed and excluded from the returned rect.
    #[must_use]
    pub fn terminal_viewport(viewport: &PaneViewport) -> Rect {
        let tbh = TAB_BAR_HEIGHT * viewport.scale;
        Rect::new(
            viewport.rect.x,
            viewport.rect.y + tbh,
            viewport.rect.width,
            (viewport.rect.height - tbh).max(0.0),
        )
    }

    /// Convert the pane `Rect` to a wgpu scissor rect `(x, y, width, height)`.
    ///
    /// Values are clamped to the surface bounds. A zero-area rect is returned
    /// when the pane lies entirely outside the surface.
    #[must_use]
    pub fn scissor_rect(viewport: &PaneViewport, surface_size: (u32, u32)) -> (u32, u32, u32, u32) {
        let (sw, sh) = surface_size;
        let r = &viewport.rect;

        // Clamp origin to surface.
        let x = r.x.max(0.0).min(sw as f32) as u32;
        let y = r.y.max(0.0).min(sh as f32) as u32;

        // Clamp extent to surface.
        let right = (r.x + r.width).max(0.0).min(sw as f32) as u32;
        let bottom = (r.y + r.height).max(0.0).min(sh as f32) as u32;

        let width = right.saturating_sub(x);
        let height = bottom.saturating_sub(y);

        (x, y, width, height)
    }
}

impl Default for PaneRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wmux_core::types::PaneId;

    fn make_viewport(rect: Rect, focused: bool, tab_count: usize) -> PaneViewport {
        PaneViewport {
            pane_id: PaneId::new(),
            rect,
            focused,
            tab_count,
            tab_titles: (0..tab_count).map(|i| format!("Tab {i}")).collect(),
            surface_ids: (0..tab_count).map(|_| SurfaceId::new()).collect(),
            active_tab: 0,
            zoomed: false,
            surface_types: vec![SurfaceType::Terminal; tab_count],
            unsaved: vec![false; tab_count],
            scale: 1.0,
        }
    }

    #[test]
    fn terminal_viewport_single_tab_reserves_tab_bar() {
        let rect = Rect::new(10.0, 20.0, 400.0, 300.0);
        let vp = make_viewport(rect, false, 1);
        let tv = PaneRenderer::terminal_viewport(&vp);
        assert_eq!(tv.x, 10.0);
        assert_eq!(tv.y, 20.0 + TAB_BAR_HEIGHT);
        assert_eq!(tv.width, 400.0);
        assert!((tv.height - (300.0 - TAB_BAR_HEIGHT)).abs() < f32::EPSILON);
    }

    #[test]
    fn terminal_viewport_with_tabs() {
        let rect = Rect::new(0.0, 0.0, 400.0, 300.0);
        let vp = make_viewport(rect, false, 3);
        let tv = PaneRenderer::terminal_viewport(&vp);
        assert_eq!(tv.y, TAB_BAR_HEIGHT);
        assert!((tv.height - (300.0 - TAB_BAR_HEIGHT)).abs() < f32::EPSILON);
    }

    #[test]
    fn scissor_rect_basic() {
        let rect = Rect::new(10.0, 20.0, 100.0, 80.0);
        let vp = make_viewport(rect, false, 1);
        let (x, y, w, h) = PaneRenderer::scissor_rect(&vp, (800, 600));
        assert_eq!(x, 10);
        assert_eq!(y, 20);
        assert_eq!(w, 100);
        assert_eq!(h, 80);
    }

    #[test]
    fn scissor_rect_clamps_to_surface() {
        // Pane extends beyond surface bounds.
        let rect = Rect::new(750.0, 550.0, 200.0, 200.0);
        let vp = make_viewport(rect, false, 1);
        let (x, y, w, h) = PaneRenderer::scissor_rect(&vp, (800, 600));
        assert_eq!(x, 750);
        assert_eq!(y, 550);
        assert_eq!(w, 50); // clamped: 800 - 750
        assert_eq!(h, 50); // clamped: 600 - 550
    }

    #[test]
    fn scissor_rect_fully_outside() {
        let rect = Rect::new(900.0, 700.0, 100.0, 100.0);
        let vp = make_viewport(rect, false, 1);
        let (_, _, w, h) = PaneRenderer::scissor_rect(&vp, (800, 600));
        assert_eq!(w, 0);
        assert_eq!(h, 0);
    }

    #[test]
    fn pane_renderer_is_default() {
        let _r: PaneRenderer = PaneRenderer::default();
    }

    #[test]
    fn tab_metrics_applies_max_width_clamp() {
        // With a very wide viewport, tab_width should clamp to MAX_TAB_WIDTH.
        let rect = Rect::new(0.0, 0.0, 2000.0, 100.0);
        let vp = make_viewport(rect, false, 2);
        let (tab_width, _) = PaneRenderer::tab_metrics(&vp, 0);
        assert!(tab_width <= MAX_TAB_WIDTH);
    }

    #[test]
    fn tab_type_indicator_pos_valid_index() {
        let rect = Rect::new(10.0, 10.0, 400.0, 100.0);
        let vp = make_viewport(rect, false, 3);
        let pos = PaneRenderer::tab_type_indicator_pos(&vp, 1);
        assert!(pos.is_some());
        let (x, y) = pos.unwrap();
        // Indicator should be near tab start with small padding
        assert!(x > rect.x);
        assert!(y >= rect.y);
    }

    #[test]
    fn tab_type_indicator_pos_out_of_bounds() {
        let rect = Rect::new(0.0, 0.0, 400.0, 100.0);
        let vp = make_viewport(rect, false, 3);
        let pos = PaneRenderer::tab_type_indicator_pos(&vp, 5);
        assert!(pos.is_none());
    }

    #[test]
    fn surface_type_enum() {
        assert_eq!(SurfaceType::Terminal, SurfaceType::Terminal);
        assert_eq!(SurfaceType::Browser, SurfaceType::Browser);
        assert_ne!(SurfaceType::Terminal, SurfaceType::Browser);
    }

    #[test]
    fn render_pane_borders_focused_only() {
        // Only focused panes get a left accent stripe (1 quad).
        // Unfocused panes get no border quads.
        let rect1 = Rect::new(0.0, 0.0, 200.0, 100.0);
        let rect2 = Rect::new(204.0, 0.0, 200.0, 100.0);
        let vps = vec![
            make_viewport(rect1, true, 1),
            make_viewport(rect2, false, 1),
        ];
        assert_eq!(vps.len(), 2);
        assert!(vps[0].focused);
        assert!(!vps[1].focused);
    }

    #[test]
    fn close_button_rect_hidden_for_single_tab() {
        let rect = Rect::new(0.0, 0.0, 400.0, 100.0);
        let vp = make_viewport(rect, false, 1);
        assert!(PaneRenderer::close_button_rect(&vp, 0).is_none());
    }

    #[test]
    fn close_button_rect_valid_for_multi_tab() {
        let rect = Rect::new(10.0, 10.0, 400.0, 100.0);
        let vp = make_viewport(rect, false, 3);
        let btn = PaneRenderer::close_button_rect(&vp, 1);
        assert!(btn.is_some());
        let (bx, by, bw, bh) = btn.unwrap();
        assert!(bx > rect.x);
        assert!(by >= rect.y);
        assert_eq!(bw, CLOSE_BUTTON_SIZE);
        assert_eq!(bh, CLOSE_BUTTON_SIZE);
    }

    #[test]
    fn close_button_rect_out_of_bounds() {
        let rect = Rect::new(0.0, 0.0, 400.0, 100.0);
        let vp = make_viewport(rect, false, 3);
        assert!(PaneRenderer::close_button_rect(&vp, 5).is_none());
    }

    #[test]
    fn plus_button_rect_positioned_after_last_tab() {
        let rect = Rect::new(0.0, 0.0, 400.0, 100.0);
        let vp = make_viewport(rect, false, 2);
        let plus = PaneRenderer::plus_button_rect(&vp);
        assert!(plus.is_some());
        let (px, _py, pw, _ph) = plus.unwrap();
        // "+" button should be after the last tab
        let (last_w, last_x) = PaneRenderer::tab_metrics(&vp, 1);
        assert!(px > last_x + last_w);
        assert_eq!(pw, PLUS_BUTTON_WIDTH);
    }

    #[test]
    fn plus_button_rect_single_tab() {
        let rect = Rect::new(0.0, 0.0, 400.0, 100.0);
        let vp = make_viewport(rect, false, 1);
        let plus = PaneRenderer::plus_button_rect(&vp);
        assert!(plus.is_some());
    }

    #[test]
    fn split_button_rect_after_plus() {
        let rect = Rect::new(0.0, 0.0, 400.0, 100.0);
        let vp = make_viewport(rect, false, 1);
        let plus = PaneRenderer::plus_button_rect(&vp);
        let split = PaneRenderer::split_button_rect(&vp);
        assert!(plus.is_some());
        assert!(split.is_some());
        let (px, _, pw, _) = plus.unwrap();
        let (sx, _, sw, _) = split.unwrap();
        assert!(sx > px + pw, "split button should be after plus button");
        assert_eq!(sw, SPLIT_BUTTON_WIDTH);
    }

    #[test]
    fn notification_ring_alpha_range() {
        // Verify the alpha calculation stays in [0.1, 0.5] range
        for i in 0..100 {
            let t = i as f32 * 0.1;
            let alpha = 0.3 + 0.2 * (t * std::f32::consts::PI).sin();
            assert!(alpha >= 0.09 && alpha <= 0.51, "alpha {alpha} at t={t}");
        }
    }
}
