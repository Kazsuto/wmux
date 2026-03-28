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

const FONT_SIZE: f32 = 20.0;
const LINE_HEIGHT: f32 = 26.0;
const BLINK_INTERVAL_MS: u128 = 500;

/// Thickness (in pixels) for the cursor bar (vertical beam) and underline shapes.
const CURSOR_LINE_THICKNESS: f32 = 2.0;

/// Brightness multiplier for SGR 2 (faint/dim) text — 80% of original intensity.
/// Standard terminals use ~67% (2/3), but ultra-dark backgrounds (#131313) need
/// a gentler reduction to maintain readability.
const DIM_INTENSITY_FACTOR: f32 = 0.80;

/// Cell dimensions derived from font metrics.
///
/// `cell_width` is measured by shaping the reference character 'M' in a
/// monospace font. `cell_height` equals the configured line height.
#[derive(Debug, Clone, Copy)]
pub struct TerminalMetrics {
    pub cell_width: f32,
    pub cell_height: f32,
    /// Effective font size (in physical pixels) used to compute these metrics.
    /// Includes DPI scaling: `config_font_size * scale_factor`.
    pub font_size: f32,
}

impl TerminalMetrics {
    /// Compute cell dimensions from the current font system.
    ///
    /// When `font_family` is provided, uses that font; otherwise falls back to
    /// the system monospace font. When `font_size` is provided, overrides the
    /// default `FONT_SIZE` / `LINE_HEIGHT`.
    pub fn new(
        font_system: &mut glyphon::FontSystem,
        font_family: Option<&str>,
        font_size: Option<f32>,
    ) -> Self {
        let size = font_size.unwrap_or(FONT_SIZE);
        let line_h = font_size.map_or(LINE_HEIGHT, |s| (s * 1.3).ceil());
        let family = font_family.map_or(Family::Monospace, Family::Name);

        let mut buf = Buffer::new(font_system, Metrics::new(size, line_h));
        buf.set_size(font_system, Some(1000.0), Some(line_h));
        buf.set_text(
            font_system,
            "M",
            &Attrs::new().family(family),
            Shaping::Basic,
            None,
        );
        buf.shape_until_scroll(font_system, false);

        let cell_width = buf
            .layout_runs()
            .next()
            .map_or(size * 0.6, |run| run.line_w);

        Self {
            cell_width,
            cell_height: line_h,
            font_size: size,
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
    /// Reusable dirty row indices buffer (avoids per-frame allocation).
    dirty_buf: Vec<u16>,
    /// Theme ANSI palette (16 colors) for terminal color rendering.
    ansi_palette: [(u8, u8, u8); 16],
    /// Theme cursor color.
    cursor_color: [f32; 4],
    /// Theme foreground color for glyphon default text.
    foreground_color: glyphon::Color,
    /// Cursor transparency from theme (0.0 = invisible, 1.0 = opaque).
    cursor_alpha: f32,
    /// User-configured font family for terminal text (e.g., "JetBrainsMono Nerd Font").
    /// When set, used instead of the system monospace default.
    terminal_font_family: Option<String>,
}

impl TerminalRenderer {
    pub fn new(
        font_system: &mut glyphon::FontSystem,
        cols: u16,
        rows: u16,
        font_family: Option<&str>,
        font_size: Option<f32>,
    ) -> Self {
        let metrics = TerminalMetrics::new(font_system, font_family, font_size);
        let row_buffers = build_row_buffers(font_system, &metrics, cols, rows);
        Self {
            row_buffers,
            metrics,
            cursor_visible: true,
            last_blink: Instant::now(),
            cols,
            rows,
            last_viewport_offset: usize::MAX, // sentinel: forces full re-render on first frame
            cell_buf: Vec::with_capacity(cols as usize),
            dirty_buf: Vec::with_capacity(rows as usize),
            ansi_palette: [(0, 0, 0); 16],
            cursor_color: [1.0, 1.0, 1.0, 0.85],
            foreground_color: crate::DEFAULT_TEXT_COLOR,
            cursor_alpha: 0.85,
            terminal_font_family: font_family.map(String::from),
        }
    }

    /// Set the theme palette for ANSI color rendering.
    pub fn set_palette(
        &mut self,
        ansi: [(u8, u8, u8); 16],
        cursor: (u8, u8, u8),
        foreground: (u8, u8, u8),
        cursor_alpha: f32,
    ) {
        self.ansi_palette = ansi;
        self.cursor_alpha = cursor_alpha;
        self.cursor_color = [
            cursor.0 as f32 / 255.0,
            cursor.1 as f32 / 255.0,
            cursor.2 as f32 / 255.0,
            cursor_alpha,
        ];
        self.foreground_color = GlyphonColor::rgb(foreground.0, foreground.1, foreground.2);
    }

    /// Advance the cursor blink timer, toggling visibility at `BLINK_INTERVAL_MS`.
    #[inline]
    fn advance_blink(&mut self) {
        if self.last_blink.elapsed().as_millis() >= BLINK_INTERVAL_MS {
            self.cursor_visible = !self.cursor_visible;
            self.last_blink = Instant::now();
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
        self.advance_blink();

        let vp_offset = scrollback.viewport_offset();
        let sb_len = scrollback.len();
        let rows = self.rows as usize;
        let cols = self.cols as usize;
        let cw = self.metrics.cell_width;
        let ch = self.metrics.cell_height;

        // When the viewport moves, all rows need re-rendering.
        let scroll_changed = vp_offset != self.last_viewport_offset;
        self.last_viewport_offset = vp_offset;

        // Swap out dirty_buf to avoid holding an immutable borrow on `self`
        // while calling &mut self methods in the loop body.
        let mut dirty_buf = std::mem::take(&mut self.dirty_buf);
        dirty_buf.clear();
        if scroll_changed {
            grid.reset_dirty();
            dirty_buf.extend(0..self.rows);
        } else {
            grid.take_dirty_rows_into(&mut dirty_buf);
        }

        for &row_idx in &dirty_buf {
            let r = row_idx as usize;
            let y = r as f32 * ch;

            if !self.resolve_row_cells(r, row_idx, grid, scrollback, vp_offset, sb_len, rows, cols)
            {
                continue;
            }

            // Push non-default background quads.
            push_background_quads(
                &self.cell_buf,
                quad_pipeline,
                cw,
                ch,
                0.0,
                y,
                &self.ansi_palette,
            );

            // Re-shape the row's glyphon buffer.
            if r < self.row_buffers.len() {
                update_row_buffer(
                    &mut self.row_buffers[r],
                    font_system,
                    &self.cell_buf,
                    cols as f32 * cw,
                    ch,
                    &self.ansi_palette,
                    self.terminal_font_family.as_deref(),
                );
            }
        }
        self.dirty_buf = dirty_buf;
    }

    /// Upload shaped row glyphs to the GPU atlas. Call after `update`, before the render pass.
    ///
    /// - `pane_origin`: `(x, y)` pixel offset of the pane within the surface.
    ///   Text areas are positioned relative to this origin so that each pane's
    ///   text lands at the correct surface coordinates.
    /// - `pane_rect`: the usable terminal content rect (after subtracting any
    ///   tab bar). Used to set tight `TextBounds` so glyphs are clipped to the
    ///   pane and do not bleed into adjacent panes.
    #[expect(
        clippy::too_many_arguments,
        reason = "wgpu prepare needs device, queue, glyphon, surface dims, pane origin, and pane rect"
    )]
    pub fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        glyphon: &mut GlyphonRenderer,
        surface_width: u32,
        surface_height: u32,
        pane_origin: (f32, f32),
        pane_rect: wmux_core::rect::Rect,
    ) -> Result<(), RenderError> {
        glyphon.prepare_text_areas(
            device,
            queue,
            self.text_areas(pane_origin, pane_rect, surface_width, surface_height),
        )
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
    /// - `grid`: cloned grid (dirty flags consumed by the caller beforehand).
    /// - `viewport_offset`: scrollback viewport position.
    /// - `scrollback_len`: total scrollback row count.
    /// - `scrollback_visible_rows`: only the rows visible in the current viewport
    ///   (index 0 = topmost visible scrollback row).
    /// - `pane_origin`: `(x, y)` pixel offset of the pane's top-left corner
    ///   within the surface. Used to position quads and text areas correctly
    ///   when rendering multiple panes into a shared render pass.
    #[expect(
        clippy::too_many_arguments,
        reason = "snapshot update needs grid, dirty rows, viewport offset, scrollback, font system, quads, and pane origin"
    )]
    pub fn update_from_snapshot(
        &mut self,
        grid: &Grid,
        dirty_rows: &[u16],
        viewport_offset: usize,
        scrollback_visible_rows: &[wmux_core::cell::Row],
        font_system: &mut glyphon::FontSystem,
        quad_pipeline: &mut QuadPipeline,
        pane_origin: (f32, f32),
    ) {
        self.advance_blink();

        let rows = self.rows as usize;
        let cols = self.cols as usize;
        let cw = self.metrics.cell_width;
        let ch = self.metrics.cell_height;
        let (x_off, y_off) = pane_origin;

        let scroll_changed = viewport_offset != self.last_viewport_offset;
        self.last_viewport_offset = viewport_offset;

        // If viewport moved, re-render all rows; otherwise use provided dirty list.
        // Swap out dirty_buf to avoid borrow conflicts with &mut self in the loop.
        let mut dirty_buf = std::mem::take(&mut self.dirty_buf);
        dirty_buf.clear();
        let dirty: &[u16] = if scroll_changed {
            dirty_buf.extend(0..self.rows);
            &dirty_buf
        } else {
            dirty_rows
        };

        for &row_idx in dirty {
            let r = row_idx as usize;
            // Apply pane origin offset to position quads correctly in the surface.
            let y = y_off + r as f32 * ch;

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

            push_background_quads(
                &self.cell_buf,
                quad_pipeline,
                cw,
                ch,
                x_off,
                y,
                &self.ansi_palette,
            );

            if r < self.row_buffers.len() {
                update_row_buffer(
                    &mut self.row_buffers[r],
                    font_system,
                    &self.cell_buf,
                    cols as f32 * cw,
                    ch,
                    &self.ansi_palette,
                    self.terminal_font_family.as_deref(),
                );
            }
        }
        self.dirty_buf = dirty_buf;
    }

    /// Push the cursor quad into the pipeline (call AFTER selection highlights for correct z-order).
    pub fn push_cursor(
        &self,
        cursor: &CursorState,
        quad_pipeline: &mut QuadPipeline,
        pane_origin: (f32, f32),
    ) {
        if cursor.visible && self.cursor_visible {
            push_cursor_quad(
                cursor,
                self.metrics.cell_width,
                self.metrics.cell_height,
                pane_origin.0,
                pane_origin.1,
                quad_pipeline,
                self.cursor_color,
            );
        }
    }

    /// Return the current column count.
    pub fn cols(&self) -> u16 {
        self.cols
    }

    /// Return the current row count.
    pub fn rows(&self) -> u16 {
        self.rows
    }

    /// Build `TextArea` descriptors for all visible rows without uploading to GPU.
    ///
    /// Used by the multi-pane render loop: each pane's renderer produces its
    /// text areas, they are collected into one slice, and a single
    /// `GlyphonRenderer::prepare_text_areas` call uploads everything at once.
    ///
    /// Returns an iterator to avoid per-frame `Vec` allocation.
    pub fn text_areas(
        &self,
        pane_origin: (f32, f32),
        pane_rect: wmux_core::rect::Rect,
        surface_width: u32,
        surface_height: u32,
    ) -> impl Iterator<Item = TextArea<'_>> + '_ {
        let ch = self.metrics.cell_height;
        let (x_off, y_off) = pane_origin;

        let bounds_left = pane_rect.x.max(0.0) as i32;
        let bounds_top = pane_rect.y.max(0.0) as i32;
        let bounds_right = (pane_rect.x + pane_rect.width)
            .min(surface_width as f32)
            .max(0.0) as i32;
        let bounds_bottom = (pane_rect.y + pane_rect.height)
            .min(surface_height as f32)
            .max(0.0) as i32;

        self.row_buffers
            .iter()
            .enumerate()
            .map(move |(r, buf)| TextArea {
                buffer: buf,
                left: x_off,
                top: y_off + r as f32 * ch,
                // 1.0 because font_size already includes DPI scaling (physical pixels).
                scale: 1.0,
                bounds: TextBounds {
                    left: bounds_left,
                    top: bounds_top,
                    right: bounds_right,
                    bottom: bounds_bottom,
                },
                default_color: self.foreground_color,
                custom_glyphs: &[],
            })
    }

    /// Resize the terminal — rebuilds all row buffers for the new dimensions.
    pub fn resize(&mut self, cols: u16, rows: u16, font_system: &mut glyphon::FontSystem) {
        self.cols = cols;
        self.rows = rows;
        self.row_buffers = build_row_buffers(font_system, &self.metrics, cols, rows);
        // Resize reusable buffers.
        self.cell_buf = Vec::with_capacity(cols as usize);
        self.dirty_buf = Vec::with_capacity(rows as usize);
    }

    /// Populate `cell_buf` with cells for the given visible row.
    ///
    /// Returns `true` if cells were resolved, `false` if the row is out of bounds.
    /// Clears `cell_buf` before populating and truncates to `cols` to prevent
    /// stale wider scrollback rows from painting outside the visible area.
    #[expect(
        clippy::too_many_arguments,
        reason = "row resolution needs grid, scrollback, viewport offset, scrollback length, and grid dimensions"
    )]
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
        let row = grid.row_slice(grid_row);
        let len = cols.min(row.len());
        self.cell_buf.extend_from_slice(&row[..len]);
        true
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Push non-default background color quads and underline/strikethrough
/// decorations for a row of cells.
///
/// `x_off` and `y` are the surface-space coordinates of the row's top-left
/// corner (already including the pane origin offset).
fn push_background_quads(
    cells: &[wmux_core::cell::Cell],
    quad_pipeline: &mut QuadPipeline,
    cw: f32,
    ch: f32,
    x_off: f32,
    y: f32,
    palette: &[(u8, u8, u8); 16],
) {
    let line_thickness = 1.0_f32.max(ch / 16.0);

    for (col, cell) in cells.iter().enumerate() {
        let bg = if cell.flags.contains(CellFlags::INVERSE) {
            cell.fg
        } else {
            cell.bg
        };
        if bg != Color::Named(0) {
            quad_pipeline.push_quad(
                x_off + col as f32 * cw,
                y,
                cw,
                ch,
                color_to_rgba(bg, palette),
            );
        }

        let fg = if cell.flags.contains(CellFlags::INVERSE) {
            cell.bg
        } else {
            cell.fg
        };
        let fg_color = apply_dim(color_to_rgba(fg, palette), cell.flags);

        // Underline: thin line near the bottom of the cell.
        if cell.flags.contains(CellFlags::UNDERLINE) {
            quad_pipeline.push_quad(
                x_off + col as f32 * cw,
                y + ch - line_thickness - 1.0,
                cw,
                line_thickness,
                fg_color,
            );
        }

        // Strikethrough: thin line through the middle of the cell.
        if cell.flags.contains(CellFlags::STRIKETHROUGH) {
            quad_pipeline.push_quad(
                x_off + col as f32 * cw,
                y + (ch - line_thickness) / 2.0,
                cw,
                line_thickness,
                fg_color,
            );
        }
    }
}

fn build_row_buffers(
    font_system: &mut glyphon::FontSystem,
    metrics: &TerminalMetrics,
    cols: u16,
    rows: u16,
) -> Vec<Buffer> {
    let glyph_metrics = Metrics::new(metrics.font_size, metrics.cell_height);
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
    palette: &[(u8, u8, u8); 16],
    font_family: Option<&str>,
) {
    buf.set_size(font_system, Some(buf_width), Some(buf_height));

    let family = font_family.map_or(Family::Monospace, Family::Name);
    let default_attrs = Attrs::new().family(family);

    buf.set_rich_text(
        font_system,
        cells.iter().map(|cell| {
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
            let [r, g, b, _] = apply_dim(color_to_rgba(fg, palette), cell.flags);
            let mut attrs = Attrs::new().family(family).color(GlyphonColor::rgba(
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
        }),
        &default_attrs,
        Shaping::Advanced,
        None,
    );
    buf.shape_until_scroll(font_system, false);
}

/// Push a colored quad for the terminal cursor at its current position.
///
/// `x_off` and `y_off` are the surface-space pane origin offsets applied on
/// top of the cell grid coordinates.
fn push_cursor_quad(
    cursor: &CursorState,
    cw: f32,
    ch: f32,
    x_off: f32,
    y_off: f32,
    quad_pipeline: &mut QuadPipeline,
    cursor_color: [f32; 4],
) {
    let x = x_off + cursor.col as f32 * cw;
    let y = y_off + cursor.row as f32 * ch;
    match cursor.shape {
        CursorShape::Block => quad_pipeline.push_quad(x, y, cw, ch, cursor_color),
        CursorShape::Underline => quad_pipeline.push_quad(
            x,
            y + ch - CURSOR_LINE_THICKNESS,
            cw,
            CURSOR_LINE_THICKNESS,
            cursor_color,
        ),
        CursorShape::Bar => {
            quad_pipeline.push_quad(x, y, CURSOR_LINE_THICKNESS, ch, cursor_color);
        }
    }
}

/// Convert a terminal `Color` to a normalized RGBA `[f32; 4]`.
fn color_to_rgba(color: Color, palette: &[(u8, u8, u8); 16]) -> [f32; 4] {
    let (r, g, b) = match color {
        Color::Named(n) => named_color_rgb(n, palette),
        Color::Indexed(n) => indexed_color_rgb(n, palette),
        Color::Rgb(r, g, b) => (r, g, b),
    };
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}

/// 16 ANSI colors from the theme palette (indices 0–15).
fn named_color_rgb(n: u8, palette: &[(u8, u8, u8); 16]) -> (u8, u8, u8) {
    if (n as usize) < palette.len() {
        palette[n as usize]
    } else {
        indexed_color_rgb(n, palette)
    }
}

/// xterm-256 palette lookup (covers 0–255; indices 0–15 redirect to named table).
fn indexed_color_rgb(n: u8, palette: &[(u8, u8, u8); 16]) -> (u8, u8, u8) {
    if n < 16 {
        named_color_rgb(n, palette)
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

/// Apply DIM (SGR 2 / faint) attenuation to an RGBA color if the flag is set.
fn apply_dim(mut color: [f32; 4], flags: CellFlags) -> [f32; 4] {
    if flags.contains(CellFlags::DIM) {
        color[0] *= DIM_INTENSITY_FACTOR;
        color[1] *= DIM_INTENSITY_FACTOR;
        color[2] *= DIM_INTENSITY_FACTOR;
    }
    color
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use wmux_core::color::Color;

    /// Test palette (GitHub Dark colors) for color resolution tests.
    const TEST_PALETTE: [(u8, u8, u8); 16] = [
        (0x48, 0x4f, 0x58), // 0  black
        (0xff, 0x7b, 0x72), // 1  red
        (0x3f, 0xb9, 0x50), // 2  green
        (0xd2, 0x99, 0x22), // 3  yellow
        (0x58, 0xa6, 0xff), // 4  blue
        (0xbc, 0x8c, 0xff), // 5  magenta
        (0x56, 0xd4, 0xdd), // 6  cyan
        (0xb1, 0xba, 0xc4), // 7  white
        (0x6e, 0x76, 0x81), // 8  bright black
        (0xff, 0xa1, 0x98), // 9  bright red
        (0x56, 0xd3, 0x64), // 10 bright green
        (0xe3, 0xb3, 0x41), // 11 bright yellow
        (0x79, 0xc0, 0xff), // 12 bright blue
        (0xd2, 0xa8, 0xff), // 13 bright magenta
        (0xa5, 0xd6, 0xff), // 14 bright cyan
        (0xf0, 0xf6, 0xfc), // 15 bright white
    ];

    #[test]
    fn color_to_rgba_rgb_passthrough() {
        let [r, g, b, a] = color_to_rgba(Color::Rgb(255, 128, 0), &TEST_PALETTE);
        assert!((r - 1.0).abs() < f32::EPSILON);
        assert!((g - 128.0 / 255.0).abs() < 0.001);
        assert!((b - 0.0).abs() < f32::EPSILON);
        assert!((a - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn color_to_rgba_named_black() {
        // Named(0) uses TEST_PALETTE[0] = (0x48, 0x4f, 0x58)
        let [r, g, b, a] = color_to_rgba(Color::Named(0), &TEST_PALETTE);
        assert!((r - 0x48 as f32 / 255.0).abs() < 0.001);
        assert!((g - 0x4f as f32 / 255.0).abs() < 0.001);
        assert!((b - 0x58 as f32 / 255.0).abs() < 0.001);
        assert_eq!(a, 1.0);
    }

    #[test]
    fn color_to_rgba_named_white() {
        // Named(15) uses TEST_PALETTE[15] = (0xf0, 0xf6, 0xfc)
        let [r, g, b, a] = color_to_rgba(Color::Named(15), &TEST_PALETTE);
        assert!((r - 0xf0 as f32 / 255.0).abs() < 0.001);
        assert!((g - 0xf6 as f32 / 255.0).abs() < 0.001);
        assert!((b - 0xfc as f32 / 255.0).abs() < 0.001);
        assert!((a - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn color_to_rgba_indexed_cube() {
        // Index 16 = first cube entry = (0,0,0) black.
        let [r, g, b, _] = color_to_rgba(Color::Indexed(16), &TEST_PALETTE);
        assert_eq!(r, 0.0);
        assert_eq!(g, 0.0);
        assert_eq!(b, 0.0);

        // Index 231 = last cube entry = (255,255,255) white.
        let [r2, g2, b2, _] = color_to_rgba(Color::Indexed(231), &TEST_PALETTE);
        assert!((r2 - 1.0).abs() < f32::EPSILON);
        assert!((g2 - 1.0).abs() < f32::EPSILON);
        assert!((b2 - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn color_to_rgba_indexed_grayscale() {
        // Index 232 = darkest gray = rgb(8,8,8).
        let [r, g, b, _] = color_to_rgba(Color::Indexed(232), &TEST_PALETTE);
        assert!((r - 8.0 / 255.0).abs() < 0.001);
        assert_eq!(r, g);
        assert_eq!(g, b);

        // Index 255 = lightest gray = rgb(238,238,238).
        let [r2, g2, b2, _] = color_to_rgba(Color::Indexed(255), &TEST_PALETTE);
        assert!((r2 - 238.0 / 255.0).abs() < 0.001);
        assert_eq!(r2, g2);
        assert_eq!(g2, b2);
    }

    #[test]
    fn color_to_rgba_all_named_are_opaque() {
        for n in 0u8..=15 {
            let [_, _, _, a] = color_to_rgba(Color::Named(n), &TEST_PALETTE);
            assert!((a - 1.0).abs() < f32::EPSILON, "Named({n}) alpha != 1.0");
        }
    }

    #[test]
    fn xterm_cube_index_47_is_green() {
        // Index 47 = 16 + 36*0 + 6*2 + 5 = 16 + 17 = no...
        // Actually: 16 + 36*r + 6*g + b. For pure green: r=0, g=5, b=0 → 16+30=46.
        let [r, g, b, _] = color_to_rgba(Color::Indexed(46), &TEST_PALETTE);
        assert_eq!(r, 0.0);
        assert!(
            (g - 1.0).abs() < f32::EPSILON,
            "expected green=1.0, got {g}"
        );
        assert_eq!(b, 0.0);
    }

    #[test]
    fn apply_dim_reduces_brightness() {
        let color = [1.0, 0.8, 0.5, 1.0];
        let dimmed = apply_dim(color, CellFlags::DIM);
        assert!((dimmed[0] - 1.0 * DIM_INTENSITY_FACTOR).abs() < 0.001);
        assert!((dimmed[1] - 0.8 * DIM_INTENSITY_FACTOR).abs() < 0.001);
        assert!((dimmed[2] - 0.5 * DIM_INTENSITY_FACTOR).abs() < 0.001);
        assert!((dimmed[3] - 1.0).abs() < f32::EPSILON, "alpha unchanged");
    }

    #[test]
    fn apply_dim_noop_without_flag() {
        let color = [0.5, 0.6, 0.7, 1.0];
        let result = apply_dim(color, CellFlags::empty());
        assert_eq!(result, color);
    }

    #[test]
    fn apply_dim_with_bold_still_dims() {
        let color = [1.0, 1.0, 1.0, 1.0];
        let flags = CellFlags::BOLD | CellFlags::DIM;
        let dimmed = apply_dim(color, flags);
        assert!((dimmed[0] - DIM_INTENSITY_FACTOR).abs() < 0.001);
    }

    /// TerminalMetrics requires FontSystem (no GPU) — not ignored.
    #[test]
    fn terminal_metrics_dimensions_positive() {
        let mut font_system = glyphon::FontSystem::new();
        let metrics = TerminalMetrics::new(&mut font_system, None, None);
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
        let metrics = TerminalMetrics::new(&mut font_system, None, None);
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
        let renderer = TerminalRenderer::new(&mut font_system, 80, 24, None, None);
        assert_eq!(renderer.row_buffers.len(), 24);
        assert_eq!(renderer.cols, 80);
        assert_eq!(renderer.rows, 24);
    }

    #[test]
    fn resize_rebuilds_row_buffers() {
        let mut font_system = glyphon::FontSystem::new();
        let mut renderer = TerminalRenderer::new(&mut font_system, 80, 24, None, None);
        renderer.resize(120, 40, &mut font_system);
        assert_eq!(renderer.row_buffers.len(), 40);
        assert_eq!(renderer.cols, 120);
        assert_eq!(renderer.rows, 40);
    }
}
