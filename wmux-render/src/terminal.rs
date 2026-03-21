use std::time::Instant;

use glyphon::{
    Attrs, Buffer, Color as GlyphonColor, Family, Metrics, Shaping, TextArea, TextBounds,
};
use wmux_core::{
    cell::CellFlags,
    color::Color,
    cursor::{CursorShape, CursorState},
    grid::Grid,
    scrollback::Scrollback,
};

use crate::{quad::QuadPipeline, text::GlyphonRenderer, RenderError};

const FONT_SIZE: f32 = 14.0;
const LINE_HEIGHT: f32 = 18.0;
const BLINK_INTERVAL_MS: u128 = 500;

/// Cell dimensions derived from font metrics.
///
/// `cell_width` is measured by shaping the reference character 'M' in a
/// monospace font. `cell_height` equals the configured line height.
#[derive(Debug, Clone, Copy)]
pub struct TerminalMetrics {
    pub cell_width: f32,
    pub cell_height: f32,
}

impl TerminalMetrics {
    /// Compute cell dimensions from the current font system.
    pub fn new(font_system: &mut glyphon::FontSystem) -> Self {
        let mut buf = Buffer::new(font_system, Metrics::new(FONT_SIZE, LINE_HEIGHT));
        buf.set_size(font_system, Some(1000.0), Some(LINE_HEIGHT));
        buf.set_text(
            font_system,
            "M",
            &Attrs::new().family(Family::Monospace),
            Shaping::Advanced,
            None,
        );
        buf.shape_until_scroll(font_system, false);

        let cell_width = buf
            .layout_runs()
            .next()
            .map_or(FONT_SIZE * 0.6, |run| run.line_w);

        Self {
            cell_width,
            cell_height: LINE_HEIGHT,
        }
    }
}

/// GPU terminal renderer — connects `Grid`/`Scrollback` to the glyphon + `QuadPipeline`.
///
/// Each visible row gets its own glyphon `Buffer`. On each frame:
/// 1. Call `update()` to push background quads and re-shape only dirty rows.
/// 2. Call `prepare()` to upload glyph data to the GPU.
/// 3. Call `render()` inside the wgpu render pass.
pub struct TerminalRenderer {
    /// One glyphon Buffer per visible row for caching shaped glyphs.
    row_buffers: Vec<Buffer>,
    /// Computed cell dimensions.
    pub metrics: TerminalMetrics,
    /// Current cursor blink state.
    cursor_visible: bool,
    /// Timestamp of last blink toggle.
    last_blink: Instant,
    cols: u16,
    rows: u16,
    /// Track viewport offset changes to know when to refresh scrollback rows.
    last_viewport_offset: usize,
    /// Reusable cell buffer for row rendering (avoids per-row allocation).
    cell_buf: Vec<wmux_core::cell::Cell>,
}

impl TerminalRenderer {
    pub fn new(font_system: &mut glyphon::FontSystem, cols: u16, rows: u16) -> Self {
        let metrics = TerminalMetrics::new(font_system);
        let row_buffers = build_row_buffers(font_system, &metrics, cols, rows);
        Self {
            row_buffers,
            metrics,
            cursor_visible: true,
            last_blink: Instant::now(),
            cols,
            rows,
            last_viewport_offset: 0,
            cell_buf: Vec::with_capacity(cols as usize),
        }
    }

    /// Update dirty rows, background quads, and cursor. Call once per frame before `prepare`.
    ///
    /// Only rows flagged dirty in `grid` (or all rows when the scrollback viewport moves)
    /// trigger glyphon buffer re-shaping, keeping CPU work proportional to actual changes.
    pub fn update(
        &mut self,
        grid: &mut Grid,
        scrollback: &Scrollback,
        font_system: &mut glyphon::FontSystem,
        quad_pipeline: &mut QuadPipeline,
    ) {
        // Advance cursor blink.
        if self.last_blink.elapsed().as_millis() >= BLINK_INTERVAL_MS {
            self.cursor_visible = !self.cursor_visible;
            self.last_blink = Instant::now();
        }

        let vp_offset = scrollback.viewport_offset();
        let sb_len = scrollback.len();
        let rows = self.rows as usize;
        let cols = self.cols as usize;
        let cw = self.metrics.cell_width;
        let ch = self.metrics.cell_height;

        // When the viewport moves, all rows need re-rendering.
        let scroll_changed = vp_offset != self.last_viewport_offset;
        self.last_viewport_offset = vp_offset;

        let dirty: Vec<u16> = if scroll_changed {
            let _ = grid.take_dirty_rows(); // consume and discard to reset flags
            (0..self.rows).collect()
        } else {
            grid.take_dirty_rows()
        };

        for row_idx in dirty {
            let r = row_idx as usize;
            let y = r as f32 * ch;

            if !self.resolve_row_cells(r, row_idx, grid, scrollback, vp_offset, sb_len, rows, cols)
            {
                continue;
            }

            // Push non-default background quads.
            push_background_quads(&self.cell_buf, quad_pipeline, cw, ch, y);

            // Re-shape the row's glyphon buffer.
            if r < self.row_buffers.len() {
                update_row_buffer(
                    &mut self.row_buffers[r],
                    font_system,
                    &self.cell_buf,
                    cols as f32 * cw,
                    ch,
                );
            }
        }

        // Render cursor quad on top of everything.
        let cursor = grid.cursor();
        if cursor.visible && self.cursor_visible {
            push_cursor_quad(cursor, cw, ch, quad_pipeline);
        }
    }

    /// Upload shaped row glyphs to the GPU atlas. Call after `update`, before the render pass.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        glyphon: &mut GlyphonRenderer,
        surface_width: u32,
        surface_height: u32,
    ) -> Result<(), RenderError> {
        let ch = self.metrics.cell_height;
        let text_areas: Vec<TextArea<'_>> = self
            .row_buffers
            .iter()
            .enumerate()
            .map(|(r, buf)| TextArea {
                buffer: buf,
                left: 0.0,
                top: r as f32 * ch,
                scale: 1.0,
                bounds: TextBounds {
                    left: 0,
                    top: 0,
                    right: surface_width as i32,
                    bottom: surface_height as i32,
                },
                default_color: GlyphonColor::rgb(204, 204, 204),
                custom_glyphs: &[],
            })
            .collect();
        glyphon.prepare_text_areas(device, queue, text_areas)
    }

    /// Render the prepared terminal text into the active render pass.
    ///
    /// Must be called after `prepare` and `quad_pipeline.render()`.
    pub fn render<'pass>(
        &'pass self,
        render_pass: &mut wgpu::RenderPass<'pass>,
        glyphon: &'pass GlyphonRenderer,
    ) -> Result<(), RenderError> {
        glyphon.render(render_pass)
    }

    /// Update dirty rows from a render snapshot (actor pattern).
    ///
    /// Same logic as [`update`] but reads from a cloned `Grid` and
    /// pre-extracted scrollback rows instead of live references.
    ///
    /// - `grid`: cloned grid (dirty flags will be consumed via `take_dirty_rows`).
    /// - `viewport_offset`: scrollback viewport position.
    /// - `scrollback_len`: total scrollback row count.
    /// - `scrollback_visible_rows`: only the rows visible in the current viewport
    ///   (index 0 = topmost visible scrollback row).
    #[allow(clippy::too_many_arguments)]
    pub fn update_from_snapshot(
        &mut self,
        grid: &Grid,
        dirty_rows: &[u16],
        viewport_offset: usize,
        scrollback_visible_rows: &[wmux_core::cell::Row],
        font_system: &mut glyphon::FontSystem,
        quad_pipeline: &mut QuadPipeline,
    ) {
        // Advance cursor blink.
        if self.last_blink.elapsed().as_millis() >= BLINK_INTERVAL_MS {
            self.cursor_visible = !self.cursor_visible;
            self.last_blink = Instant::now();
        }

        let rows = self.rows as usize;
        let cols = self.cols as usize;
        let cw = self.metrics.cell_width;
        let ch = self.metrics.cell_height;

        let scroll_changed = viewport_offset != self.last_viewport_offset;
        self.last_viewport_offset = viewport_offset;

        // If viewport moved, re-render all rows; otherwise use provided dirty list.
        let all_rows: Vec<u16>;
        let dirty: &[u16] = if scroll_changed {
            all_rows = (0..self.rows).collect();
            &all_rows
        } else {
            dirty_rows
        };

        for &row_idx in dirty {
            let r = row_idx as usize;
            let y = r as f32 * ch;

            self.cell_buf.clear();

            let found = if viewport_offset > 0 {
                let sb_rows_shown = viewport_offset.min(rows);
                if r < sb_rows_shown {
                    if r < scrollback_visible_rows.len() {
                        self.cell_buf.extend_from_slice(&scrollback_visible_rows[r]);
                        true
                    } else {
                        false
                    }
                } else {
                    let grid_row = (r - sb_rows_shown) as u16;
                    self.collect_grid_row(grid, grid_row, cols)
                }
            } else {
                self.collect_grid_row(grid, row_idx, cols)
            };

            if !found {
                continue;
            }

            self.cell_buf.truncate(cols);

            push_background_quads(&self.cell_buf, quad_pipeline, cw, ch, y);

            if r < self.row_buffers.len() {
                update_row_buffer(
                    &mut self.row_buffers[r],
                    font_system,
                    &self.cell_buf,
                    cols as f32 * cw,
                    ch,
                );
            }
        }

        let cursor = grid.cursor();
        if cursor.visible && self.cursor_visible {
            push_cursor_quad(cursor, cw, ch, quad_pipeline);
        }
    }

    /// Resize the terminal — rebuilds all row buffers for the new dimensions.
    pub fn resize(&mut self, cols: u16, rows: u16, font_system: &mut glyphon::FontSystem) {
        self.cols = cols;
        self.rows = rows;
        self.row_buffers = build_row_buffers(font_system, &self.metrics, cols, rows);
        // Resize reusable buffer to match new column count.
        self.cell_buf = Vec::with_capacity(cols as usize);
    }

    /// Populate `cell_buf` with cells for the given visible row.
    ///
    /// Returns `true` if cells were resolved, `false` if the row is out of bounds.
    /// Clears `cell_buf` before populating and truncates to `cols` to prevent
    /// stale wider scrollback rows from painting outside the visible area.
    #[allow(clippy::too_many_arguments)]
    fn resolve_row_cells(
        &mut self,
        r: usize,
        row_idx: u16,
        grid: &Grid,
        scrollback: &Scrollback,
        vp_offset: usize,
        sb_len: usize,
        rows: usize,
        cols: usize,
    ) -> bool {
        self.cell_buf.clear();

        let found = if vp_offset > 0 {
            let sb_rows_shown = vp_offset.min(rows);
            if r < sb_rows_shown {
                let sb_idx = sb_len.saturating_sub(vp_offset) + r;
                if let Some(row) = scrollback.get_row(sb_idx) {
                    self.cell_buf.extend_from_slice(row);
                    true
                } else {
                    false
                }
            } else {
                let grid_row = (r - sb_rows_shown) as u16;
                self.collect_grid_row(grid, grid_row, cols)
            }
        } else {
            self.collect_grid_row(grid, row_idx, cols)
        };

        // Truncate to current column count — scrollback rows from a wider
        // layout may exceed the current terminal width.
        self.cell_buf.truncate(cols);

        found
    }

    /// Copy cells from one grid row into `cell_buf`.
    fn collect_grid_row(&mut self, grid: &Grid, grid_row: u16, cols: usize) -> bool {
        if grid_row >= grid.rows() {
            return false;
        }
        for c in 0..cols.min(grid.cols() as usize) {
            self.cell_buf.push(grid.cell(c as u16, grid_row).clone());
        }
        true
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Push non-default background color quads for a row of cells.
fn push_background_quads(
    cells: &[wmux_core::cell::Cell],
    quad_pipeline: &mut QuadPipeline,
    cw: f32,
    ch: f32,
    y: f32,
) {
    for (col, cell) in cells.iter().enumerate() {
        let bg = if cell.flags.contains(CellFlags::INVERSE) {
            cell.fg
        } else {
            cell.bg
        };
        if bg != Color::Named(0) {
            quad_pipeline.push_quad(col as f32 * cw, y, cw, ch, color_to_rgba(bg));
        }
    }
}

fn build_row_buffers(
    font_system: &mut glyphon::FontSystem,
    metrics: &TerminalMetrics,
    cols: u16,
    rows: u16,
) -> Vec<Buffer> {
    let glyph_metrics = Metrics::new(FONT_SIZE, metrics.cell_height);
    let buf_width = cols as f32 * metrics.cell_width;
    (0..rows as usize)
        .map(|_| {
            let mut buf = Buffer::new(font_system, glyph_metrics);
            buf.set_size(font_system, Some(buf_width), Some(metrics.cell_height));
            buf
        })
        .collect()
}

/// Re-shape a single row buffer from cell data.
fn update_row_buffer(
    buf: &mut Buffer,
    font_system: &mut glyphon::FontSystem,
    cells: &[wmux_core::cell::Cell],
    buf_width: f32,
    buf_height: f32,
) {
    buf.set_size(font_system, Some(buf_width), Some(buf_height));

    let default_attrs = Attrs::new().family(Family::Monospace);

    // Build one span per cell so each character gets its own color/weight.
    // Borrows graphemes directly from cell_buf — no owned String allocation needed.
    let spans: Vec<(&str, Attrs<'_>)> = cells
        .iter()
        .map(|cell| {
            let text: &str =
                if cell.grapheme.is_empty() || cell.flags.contains(CellFlags::WIDE_SPACER) {
                    " "
                } else {
                    cell.grapheme.as_str()
                };

            let fg = if cell.flags.contains(CellFlags::INVERSE) {
                cell.bg
            } else {
                cell.fg
            };
            let [r, g, b, _] = color_to_rgba(fg);
            let mut attrs = Attrs::new().family(Family::Monospace);
            attrs = attrs.color(GlyphonColor::rgba(
                (r * 255.0) as u8,
                (g * 255.0) as u8,
                (b * 255.0) as u8,
                255,
            ));
            if cell.flags.contains(CellFlags::BOLD) {
                attrs = attrs.weight(glyphon::Weight::BOLD);
            }
            if cell.flags.contains(CellFlags::ITALIC) {
                attrs = attrs.style(glyphon::Style::Italic);
            }

            (text, attrs)
        })
        .collect();

    buf.set_rich_text(font_system, spans, &default_attrs, Shaping::Advanced, None);
    buf.shape_until_scroll(font_system, false);
}

/// Push a colored quad for the terminal cursor at its current position.
fn push_cursor_quad(cursor: &CursorState, cw: f32, ch: f32, quad_pipeline: &mut QuadPipeline) {
    let x = cursor.col as f32 * cw;
    let y = cursor.row as f32 * ch;
    // White cursor with slight transparency so underlying text shows through on Block.
    let color = [1.0_f32, 1.0, 1.0, 0.85];
    match cursor.shape {
        CursorShape::Block => quad_pipeline.push_quad(x, y, cw, ch, color),
        CursorShape::Underline => quad_pipeline.push_quad(x, y + ch - 2.0, cw, 2.0, color),
        CursorShape::Bar => quad_pipeline.push_quad(x, y, 2.0, ch, color),
    }
}

/// Convert a terminal `Color` to a normalized RGBA `[f32; 4]`.
fn color_to_rgba(color: Color) -> [f32; 4] {
    let (r, g, b) = match color {
        Color::Named(n) => named_color_rgb(n),
        Color::Indexed(n) => indexed_color_rgb(n),
        Color::Rgb(r, g, b) => (r, g, b),
    };
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}

/// Standard 16 ANSI colors (indices 0–15).
fn named_color_rgb(n: u8) -> (u8, u8, u8) {
    const TABLE: [(u8, u8, u8); 16] = [
        (0, 0, 0),       // 0  black
        (128, 0, 0),     // 1  red
        (0, 128, 0),     // 2  green
        (128, 128, 0),   // 3  yellow
        (0, 0, 128),     // 4  blue
        (128, 0, 128),   // 5  magenta
        (0, 128, 128),   // 6  cyan
        (192, 192, 192), // 7  white
        (128, 128, 128), // 8  bright black
        (255, 0, 0),     // 9  bright red
        (0, 255, 0),     // 10 bright green
        (255, 255, 0),   // 11 bright yellow
        (0, 0, 255),     // 12 bright blue
        (255, 0, 255),   // 13 bright magenta
        (0, 255, 255),   // 14 bright cyan
        (255, 255, 255), // 15 bright white
    ];
    if (n as usize) < TABLE.len() {
        TABLE[n as usize]
    } else {
        indexed_color_rgb(n)
    }
}

/// xterm-256 palette lookup (covers 0–255; indices 0–15 redirect to named table).
fn indexed_color_rgb(n: u8) -> (u8, u8, u8) {
    if n < 16 {
        named_color_rgb(n)
    } else if n < 232 {
        // 6×6×6 color cube: indices 16–231.
        let i = n - 16;
        let bi = i % 6;
        let gi = (i / 6) % 6;
        let ri = i / 36;
        let scale = |v: u8| if v == 0 { 0u8 } else { 55u8 + v * 40 };
        (scale(ri), scale(gi), scale(bi))
    } else {
        // Grayscale ramp: indices 232–255.
        let v = 8u8 + (n - 232) * 10;
        (v, v, v)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use wmux_core::color::Color;

    #[test]
    fn color_to_rgba_rgb_passthrough() {
        let [r, g, b, a] = color_to_rgba(Color::Rgb(255, 128, 0));
        assert!((r - 1.0).abs() < f32::EPSILON);
        assert!((g - 128.0 / 255.0).abs() < 0.001);
        assert!((b - 0.0).abs() < f32::EPSILON);
        assert!((a - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn color_to_rgba_named_black() {
        let [r, g, b, a] = color_to_rgba(Color::Named(0));
        assert_eq!(r, 0.0);
        assert_eq!(g, 0.0);
        assert_eq!(b, 0.0);
        assert_eq!(a, 1.0);
    }

    #[test]
    fn color_to_rgba_named_white() {
        let [r, g, b, a] = color_to_rgba(Color::Named(15));
        assert!((r - 1.0).abs() < f32::EPSILON);
        assert!((g - 1.0).abs() < f32::EPSILON);
        assert!((b - 1.0).abs() < f32::EPSILON);
        assert!((a - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn color_to_rgba_indexed_cube() {
        // Index 16 = first cube entry = (0,0,0) black.
        let [r, g, b, _] = color_to_rgba(Color::Indexed(16));
        assert_eq!(r, 0.0);
        assert_eq!(g, 0.0);
        assert_eq!(b, 0.0);

        // Index 231 = last cube entry = (255,255,255) white.
        let [r2, g2, b2, _] = color_to_rgba(Color::Indexed(231));
        assert!((r2 - 1.0).abs() < f32::EPSILON);
        assert!((g2 - 1.0).abs() < f32::EPSILON);
        assert!((b2 - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn color_to_rgba_indexed_grayscale() {
        // Index 232 = darkest gray = rgb(8,8,8).
        let [r, g, b, _] = color_to_rgba(Color::Indexed(232));
        assert!((r - 8.0 / 255.0).abs() < 0.001);
        assert_eq!(r, g);
        assert_eq!(g, b);

        // Index 255 = lightest gray = rgb(238,238,238).
        let [r2, g2, b2, _] = color_to_rgba(Color::Indexed(255));
        assert!((r2 - 238.0 / 255.0).abs() < 0.001);
        assert_eq!(r2, g2);
        assert_eq!(g2, b2);
    }

    #[test]
    fn color_to_rgba_all_named_are_opaque() {
        for n in 0u8..=15 {
            let [_, _, _, a] = color_to_rgba(Color::Named(n));
            assert!((a - 1.0).abs() < f32::EPSILON, "Named({n}) alpha != 1.0");
        }
    }

    #[test]
    fn xterm_cube_index_47_is_green() {
        // Index 47 = 16 + 36*0 + 6*2 + 5 = 16 + 17 = no...
        // Actually: 16 + 36*r + 6*g + b. For pure green: r=0, g=5, b=0 → 16+30=46.
        let [r, g, b, _] = color_to_rgba(Color::Indexed(46));
        assert_eq!(r, 0.0);
        assert!(
            (g - 1.0).abs() < f32::EPSILON,
            "expected green=1.0, got {g}"
        );
        assert_eq!(b, 0.0);
    }

    /// TerminalMetrics requires FontSystem (no GPU) — not ignored.
    #[test]
    fn terminal_metrics_dimensions_positive() {
        let mut font_system = glyphon::FontSystem::new();
        let metrics = TerminalMetrics::new(&mut font_system);
        assert!(
            metrics.cell_width > 0.0,
            "cell_width must be positive, got {}",
            metrics.cell_width
        );
        assert!(
            metrics.cell_height > 0.0,
            "cell_height must be positive, got {}",
            metrics.cell_height
        );
        // Sanity: reasonable bounds for 14px monospace font.
        assert!(metrics.cell_width < 50.0, "cell_width suspiciously large");
        assert!(metrics.cell_height < 50.0, "cell_height suspiciously large");
    }

    #[test]
    fn terminal_metrics_cell_height_equals_line_height() {
        let mut font_system = glyphon::FontSystem::new();
        let metrics = TerminalMetrics::new(&mut font_system);
        assert!(
            (metrics.cell_height - LINE_HEIGHT).abs() < f32::EPSILON,
            "cell_height should equal LINE_HEIGHT={LINE_HEIGHT}, got {}",
            metrics.cell_height
        );
    }

    /// TerminalRenderer construction requires FontSystem, not GPU.
    #[test]
    fn terminal_renderer_builds_correct_row_count() {
        let mut font_system = glyphon::FontSystem::new();
        let renderer = TerminalRenderer::new(&mut font_system, 80, 24);
        assert_eq!(renderer.row_buffers.len(), 24);
        assert_eq!(renderer.cols, 80);
        assert_eq!(renderer.rows, 24);
    }

    #[test]
    fn resize_rebuilds_row_buffers() {
        let mut font_system = glyphon::FontSystem::new();
        let mut renderer = TerminalRenderer::new(&mut font_system, 80, 24);
        renderer.resize(120, 40, &mut font_system);
        assert_eq!(renderer.row_buffers.len(), 40);
        assert_eq!(renderer.cols, 120);
        assert_eq!(renderer.rows, 40);
    }
}
