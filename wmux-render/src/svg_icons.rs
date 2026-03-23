/// SVG icon rasterization for glyphon's CustomGlyph system.
///
/// This module provides a callback for `prepare_with_custom()` that rasterizes
/// SVG data into pixel buffers on demand. Glyphon caches the results in its
/// LRU atlas, so each unique (id, width, height) combination is only rasterized
/// once.
///
/// # Adding a new SVG icon
///
/// 1. Place the `.svg` file in `wmux-render/assets/icons/`.
/// 2. Add a constant ID below (must be unique `u16`).
/// 3. Add the `include_bytes!` + match arm in `rasterize_svg_icon`.
use glyphon::{ContentType, RasterizeCustomGlyphRequest, RasterizedCustomGlyph};

// Custom glyph IDs — each SVG icon gets a unique ID.
// Source: Codicons (https://microsoft.github.io/vscode-codicons/dist/codicon.html)
pub const ICON_CLOSE: u16 = 1;
pub const ICON_ADD: u16 = 2;
pub const ICON_TERMINAL: u16 = 3;
pub const ICON_GLOBE: u16 = 4;
pub const ICON_SPLIT_H: u16 = 5;
pub const ICON_SPLIT_V: u16 = 6;
pub const ICON_SEARCH: u16 = 7;
pub const ICON_FOLDER: u16 = 8;
pub const ICON_SETTINGS: u16 = 9;
pub const ICON_CHEVRON_RIGHT: u16 = 10;
pub const ICON_CHEVRON_DOWN: u16 = 11;
pub const ICON_ARROW_RIGHT: u16 = 12;
pub const ICON_ARROW_LEFT: u16 = 13;
pub const ICON_ARROW_UP: u16 = 14;
pub const ICON_ARROW_DOWN: u16 = 15;
pub const ICON_INFO: u16 = 16;
pub const ICON_WARNING: u16 = 17;
pub const ICON_ERROR: u16 = 18;

/// Rasterize a custom glyph by ID. Returns `None` for unknown IDs,
/// which tells glyphon to skip that glyph (no crash, no error).
///
/// This function is called by glyphon's `prepare_with_custom()` only when
/// a `CustomGlyph` with a matching `id` appears in a `TextArea`.
/// Results are cached by glyphon's LRU atlas — subsequent frames reuse
/// the cached rasterization.
pub fn rasterize_svg_icon(request: RasterizeCustomGlyphRequest) -> Option<RasterizedCustomGlyph> {
    // Map custom glyph IDs to embedded SVG data.
    // When SVG icons are added to assets/icons/, add match arms here:
    //   ICON_SPLIT => include_bytes!("../assets/icons/split.svg"),
    //   ICON_TERMINAL => include_bytes!("../assets/icons/terminal.svg"),
    let svg_data: &[u8] = icon_id_to_svg(request.id)?;
    rasterize_svg_bytes(svg_data, request.width, request.height)
}

/// Map a custom glyph ID to its embedded SVG data.
fn icon_id_to_svg(id: u16) -> Option<&'static [u8]> {
    match id {
        ICON_CLOSE => Some(include_bytes!("../assets/icons/close.svg")),
        ICON_ADD => Some(include_bytes!("../assets/icons/add.svg")),
        ICON_TERMINAL => Some(include_bytes!("../assets/icons/terminal.svg")),
        ICON_GLOBE => Some(include_bytes!("../assets/icons/globe.svg")),
        ICON_SPLIT_H => Some(include_bytes!("../assets/icons/split-horizontal.svg")),
        ICON_SPLIT_V => Some(include_bytes!("../assets/icons/split-vertical.svg")),
        ICON_SEARCH => Some(include_bytes!("../assets/icons/search.svg")),
        ICON_FOLDER => Some(include_bytes!("../assets/icons/folder.svg")),
        ICON_SETTINGS => Some(include_bytes!("../assets/icons/settings-gear.svg")),
        ICON_CHEVRON_RIGHT => Some(include_bytes!("../assets/icons/chevron-right.svg")),
        ICON_CHEVRON_DOWN => Some(include_bytes!("../assets/icons/chevron-down.svg")),
        ICON_ARROW_RIGHT => Some(include_bytes!("../assets/icons/arrow-right.svg")),
        ICON_ARROW_LEFT => Some(include_bytes!("../assets/icons/arrow-left.svg")),
        ICON_ARROW_UP => Some(include_bytes!("../assets/icons/arrow-up.svg")),
        ICON_ARROW_DOWN => Some(include_bytes!("../assets/icons/arrow-down.svg")),
        ICON_INFO => Some(include_bytes!("../assets/icons/info.svg")),
        ICON_WARNING => Some(include_bytes!("../assets/icons/warning.svg")),
        ICON_ERROR => Some(include_bytes!("../assets/icons/error.svg")),
        _ => None,
    }
}

/// Rasterize SVG bytes into an alpha mask at the requested physical size.
///
/// The SVG is rendered as white-on-transparent, then the alpha channel is
/// extracted as a `ContentType::Mask`. This allows glyphon's shader to
/// colorize the icon with the TextArea's `default_color` — just like text.
fn rasterize_svg_bytes(svg_data: &[u8], width: u16, height: u16) -> Option<RasterizedCustomGlyph> {
    if width == 0 || height == 0 {
        return None;
    }

    // Override fill="currentColor" (black) to white so the alpha channel
    // captures the icon shape correctly.
    let svg_str = std::str::from_utf8(svg_data).ok()?;
    let white_svg = svg_str.replace("fill=\"currentColor\"", "fill=\"white\"");

    let tree = usvg::Tree::from_data(white_svg.as_bytes(), &usvg::Options::default()).ok()?;
    let mut pixmap = tiny_skia::Pixmap::new(width as u32, height as u32)?;

    let svg_size = tree.size();
    let sx = width as f32 / svg_size.width();
    let sy = height as f32 / svg_size.height();
    let transform = tiny_skia::Transform::from_scale(sx, sy);

    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // Extract alpha channel only — each pixel becomes a single byte.
    // White pixels (the icon paths) have alpha=255, transparent areas have alpha=0.
    let alpha: Vec<u8> = pixmap.data().chunks_exact(4).map(|rgba| rgba[3]).collect();

    Some(RasterizedCustomGlyph {
        data: alpha,
        content_type: ContentType::Mask,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_id_returns_none() {
        let request = RasterizeCustomGlyphRequest {
            id: 9999,
            width: 24,
            height: 24,
            x_bin: glyphon::SubpixelBin::Zero,
            y_bin: glyphon::SubpixelBin::Zero,
            scale: 1.0,
        };
        assert!(rasterize_svg_icon(request).is_none());
    }

    #[test]
    fn rasterize_zero_size_returns_none() {
        assert!(rasterize_svg_bytes(b"<svg/>", 0, 0).is_none());
    }

    #[test]
    fn rasterize_valid_svg_returns_mask() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16">
            <rect width="16" height="16" fill="currentColor"/>
        </svg>"#;
        let result = rasterize_svg_bytes(svg, 16, 16);
        assert!(result.is_some());
        let glyph = result.unwrap();
        assert_eq!(glyph.content_type, ContentType::Mask);
        assert_eq!(glyph.data.len(), 16 * 16); // 1 byte per pixel (alpha only)
                                               // All pixels should be opaque (fill covers entire viewBox)
        assert!(glyph.data.iter().all(|&a| a == 255));
    }
}
