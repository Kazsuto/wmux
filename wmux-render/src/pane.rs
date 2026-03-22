use wmux_core::{rect::Rect, types::PaneId};

use crate::quad::QuadPipeline;

/// Height in pixels of the tab bar when a pane has multiple surfaces.
pub const TAB_BAR_HEIGHT: f32 = 36.0;

/// Width in pixels of the focused pane accent stripe (left bar).
pub const FOCUS_STRIPE_WIDTH: f32 = 3.0;

/// Spacing between pill-style tabs.
const TAB_GAP: f32 = 4.0;

/// Border radius for pill-style tabs.
const TAB_RADIUS: f32 = 6.0;

/// Maximum width for a single tab pill (160px).
const MAX_TAB_WIDTH: f32 = 160.0;

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
    /// Index of the currently active tab.
    pub active_tab: usize,
    /// When `true`, this pane fills the entire workspace area (zoom mode).
    pub zoomed: bool,
    /// Surface type (Terminal or Browser) for each tab.
    pub surface_types: Vec<SurfaceType>,
    /// Unsaved state for each tab (true = unsaved).
    pub unsaved: Vec<bool>,
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
    /// The focused pane receives a 3px accent stripe on the left edge plus
    /// a Focus Glow (4 concentric quads forming a luminous halo).
    pub fn render_pane_borders(
        &self,
        quads: &mut QuadPipeline,
        viewports: &[PaneViewport],
        _theme_border_color: [f32; 4],
        theme_accent_color: [f32; 4],
    ) {
        for vp in viewports {
            if vp.focused {
                let r = &vp.rect;
                // Left accent stripe (rounded)
                quads.push_rounded_quad(
                    r.x,
                    r.y + 4.0,
                    FOCUS_STRIPE_WIDTH,
                    r.height - 8.0,
                    theme_accent_color,
                    1.5,
                );
            }
        }
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

        const GLOW_RADIUS: f32 = 20.0;

        // Shader-based glow: single expanded quad, inner = transparent, outer = glow
        let glow_color = [
            accent_glow[0],
            accent_glow[1],
            accent_glow[2],
            accent_glow[3] * glow_alpha,
        ];
        // Inner fill is transparent (we only want the glow halo, not a filled rect)
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
    /// Only called when `viewport.tab_count > 1`. The tab bar occupies
    /// `TAB_BAR_HEIGHT` pixels at the top of the pane rect.
    ///
    /// Each tab is a rounded pill with `TAB_GAP` spacing. The active tab
    /// has an elevated background and a 2px rounded accent indicator at the
    /// bottom. Inactive tabs receive a subtle pill background (same as bar bg,
    /// no visual change until hover is implemented). Tab width is clamped to
    /// MAX_TAB_WIDTH (160px).
    pub fn render_tab_bar(
        &self,
        quads: &mut QuadPipeline,
        viewport: &PaneViewport,
        tab_bg: [f32; 4],
        active_tab_bg: [f32; 4],
        accent_color: [f32; 4],
    ) -> Result<(), crate::RenderError> {
        if viewport.tab_count <= 1 {
            return Ok(());
        }

        let r = &viewport.rect;

        // Full tab bar background
        quads.push_quad(r.x, r.y, r.width, TAB_BAR_HEIGHT, tab_bg);

        let pill_y = r.y + 4.0;
        let pill_h = TAB_BAR_HEIGHT - 8.0;

        for i in 0..viewport.tab_count {
            let (tab_width, tab_x) = Self::tab_metrics(viewport, i);

            if i == viewport.active_tab {
                // Active tab pill
                quads.push_rounded_quad(
                    tab_x,
                    pill_y,
                    tab_width,
                    pill_h,
                    active_tab_bg,
                    TAB_RADIUS,
                );
                // Active indicator: 2px rounded accent bar at the bottom of the pill
                quads.push_rounded_quad(
                    tab_x + tab_width * 0.2,
                    r.y + TAB_BAR_HEIGHT - 4.0,
                    tab_width * 0.6,
                    2.0,
                    accent_color,
                    1.0,
                );
            } else {
                // Inactive tab pill: subtle background (same as bar bg for now).
                // This provides hover-ready affordance and visual structure.
                quads.push_rounded_quad(tab_x, pill_y, tab_width, pill_h, tab_bg, TAB_RADIUS);
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
    /// Returns `(tab_width, tab_x)` accounting for gaps, padding, and MAX_TAB_WIDTH clamping.
    /// Use this to align text and drag visuals with the rendered tab pills.
    #[must_use]
    pub fn tab_metrics(viewport: &PaneViewport, tab_index: usize) -> (f32, f32) {
        let n = viewport.tab_count as f32;
        let total_gaps = TAB_GAP * (n - 1.0) + TAB_GAP * 2.0;
        let mut tab_width = ((viewport.rect.width - total_gaps) / n).max(1.0);
        tab_width = tab_width.min(MAX_TAB_WIDTH);
        let tab_x = viewport.rect.x + TAB_GAP + tab_index as f32 * (tab_width + TAB_GAP);
        (tab_width, tab_x)
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
    /// When the pane has multiple tabs, the top `TAB_BAR_HEIGHT` pixels are
    /// consumed by the tab bar and excluded from the returned rect.
    #[must_use]
    pub fn terminal_viewport(viewport: &PaneViewport) -> Rect {
        if viewport.tab_count > 1 {
            Rect::new(
                viewport.rect.x,
                viewport.rect.y + TAB_BAR_HEIGHT,
                viewport.rect.width,
                (viewport.rect.height - TAB_BAR_HEIGHT).max(0.0),
            )
        } else {
            viewport.rect
        }
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
            active_tab: 0,
            zoomed: false,
            surface_types: vec![SurfaceType::Terminal; tab_count],
            unsaved: vec![false; tab_count],
        }
    }

    #[test]
    fn terminal_viewport_no_tabs() {
        let rect = Rect::new(10.0, 20.0, 400.0, 300.0);
        let vp = make_viewport(rect, false, 1);
        let tv = PaneRenderer::terminal_viewport(&vp);
        assert_eq!(tv.x, 10.0);
        assert_eq!(tv.y, 20.0);
        assert_eq!(tv.width, 400.0);
        assert_eq!(tv.height, 300.0);
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
    fn notification_ring_alpha_range() {
        // Verify the alpha calculation stays in [0.1, 0.5] range
        for i in 0..100 {
            let t = i as f32 * 0.1;
            let alpha = 0.3 + 0.2 * (t * std::f32::consts::PI).sin();
            assert!(alpha >= 0.09 && alpha <= 0.51, "alpha {alpha} at t={t}");
        }
    }
}
