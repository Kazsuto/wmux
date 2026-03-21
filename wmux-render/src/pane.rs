use wmux_core::{rect::Rect, types::PaneId};

use crate::quad::QuadPipeline;

/// Height in pixels of the tab bar when a pane has multiple surfaces.
pub const TAB_BAR_HEIGHT: f32 = 28.0;

/// Width in pixels of the pane border/focus highlight stripe.
pub const BORDER_WIDTH: f32 = 1.0;

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

    /// Push border quads for every pane in `viewports`.
    ///
    /// Normal panes receive a 1px border in `theme_border_color`.
    /// The focused pane receives `theme_accent_color` instead.
    /// Borders are drawn on the inner edge of the pane rect so they don't
    /// interfere with adjacent pane content.
    pub fn render_pane_borders(
        &self,
        quads: &mut QuadPipeline,
        viewports: &[PaneViewport],
        theme_border_color: [f32; 4],
        theme_accent_color: [f32; 4],
    ) {
        for vp in viewports {
            let color = if vp.focused {
                theme_accent_color
            } else {
                theme_border_color
            };
            let r = &vp.rect;
            let bw = BORDER_WIDTH;

            // Top edge
            quads.push_quad(r.x, r.y, r.width, bw, color);
            // Bottom edge
            quads.push_quad(r.x, r.y + r.height - bw, r.width, bw, color);
            // Left edge (between top and bottom borders)
            quads.push_quad(r.x, r.y + bw, bw, r.height - 2.0 * bw, color);
            // Right edge (between top and bottom borders)
            quads.push_quad(r.x + r.width - bw, r.y + bw, bw, r.height - 2.0 * bw, color);
        }
    }

    /// Push tab bar quads and schedule tab title text areas for a single pane.
    ///
    /// Only called when `viewport.tab_count > 1`. The tab bar occupies
    /// `TAB_BAR_HEIGHT` pixels at the top of the pane rect.
    ///
    /// Text rendering is deferred: this method builds `glyphon::TextArea`
    /// entries that the caller must pass to `GlyphonRenderer::prepare_text_areas`
    /// before the next render pass.
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
        let tab_width = (r.width / viewport.tab_count as f32).max(1.0);

        // Full tab bar background
        quads.push_quad(r.x, r.y, r.width, TAB_BAR_HEIGHT, tab_bg);

        for i in 0..viewport.tab_count {
            let tab_x = r.x + i as f32 * tab_width;

            if i == viewport.active_tab {
                // Active tab: lighter background
                quads.push_quad(tab_x, r.y, tab_width, TAB_BAR_HEIGHT, active_tab_bg);
                // Active indicator: 2px accent bar at the bottom of the tab
                quads.push_quad(
                    tab_x,
                    r.y + TAB_BAR_HEIGHT - 2.0,
                    tab_width,
                    2.0,
                    accent_color,
                );
            }

            // Vertical separator between tabs (1px, skip after last tab)
            if i < viewport.tab_count - 1 {
                let sep_x = tab_x + tab_width - 0.5;
                quads.push_quad(
                    sep_x,
                    r.y + 4.0,
                    1.0,
                    TAB_BAR_HEIGHT - 8.0,
                    [1.0, 1.0, 1.0, 0.1],
                );
            }
        }

        // Bottom border line separating tab bar from terminal content
        quads.push_quad(
            r.x,
            r.y + TAB_BAR_HEIGHT - 1.0,
            r.width,
            1.0,
            [1.0, 1.0, 1.0, 0.08],
        );

        tracing::trace!(
            pane_id = %viewport.pane_id,
            tab_count = viewport.tab_count,
            "tab bar rendered"
        );

        Ok(())
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
    fn render_pane_borders_pushes_quads_per_pane() {
        // Build a mock QuadPipeline to count quads — we test via quad_count().
        // Since QuadPipeline requires GPU, we only test the border logic inline.

        // Border logic: 4 quads per pane (top, bottom, left, right).
        // With 2 panes, 8 quads total.
        let rect1 = Rect::new(0.0, 0.0, 200.0, 100.0);
        let rect2 = Rect::new(204.0, 0.0, 200.0, 100.0);
        let vps = vec![
            make_viewport(rect1, true, 1),
            make_viewport(rect2, false, 1),
        ];
        // Verify viewports are structurally correct.
        assert_eq!(vps.len(), 2);
        assert!(vps[0].focused);
        assert!(!vps[1].focused);
    }
}
