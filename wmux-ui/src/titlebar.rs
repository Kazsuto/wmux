//! Custom title bar component — replaces the native Windows title bar.
//!
//! Uses Win32 `SetWindowSubclass` to intercept `WM_NCCALCSIZE` (extend client
//! area) and `WM_NCHITTEST` (enable drag/snap/resize). The title bar content
//! is rendered via wgpu quads + glyphon text, matching the theme system.

use glyphon::{Buffer, CustomGlyph, Family, Metrics, Shaping};
use wmux_config::UiChrome;
use wmux_render::QuadPipeline;

use crate::f32_to_glyphon_color;
use crate::typography;

/// Height of the custom title bar in logical pixels.
pub const TITLE_BAR_HEIGHT: f32 = 36.0;

/// Width of each window chrome button in logical pixels (Windows standard).
const BUTTON_WIDTH: f32 = 46.0;

/// Title bar text — uses Body token (same as tab labels).
const FONT_SIZE: f32 = typography::BODY_FONT_SIZE;
const LINE_HEIGHT: f32 = typography::BODY_LINE_HEIGHT;

/// Icon render size for chrome buttons in logical pixels.
const ICON_SIZE: f32 = 10.0;

/// Which chrome button is hovered or clicked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromeButton {
    Minimize,
    Maximize,
    Close,
}

/// Title bar component state.
pub struct TitleBarState {
    title_buffer: Buffer,
    /// Last title text for dirty tracking (locale change).
    #[expect(dead_code, reason = "reserved for locale hot-reload support")]
    last_title: String,
    /// Currently hovered button, if any.
    pub hovered_button: Option<ChromeButton>,
    /// Whether the window is currently maximized.
    pub is_maximized: bool,
    /// Whether custom chrome (subclassing) was successfully installed.
    pub custom_chrome_active: bool,
}

impl TitleBarState {
    /// Create a new title bar with a pre-allocated text buffer.
    ///
    /// `width` must be in **logical pixels** (physical / scale_factor) so that
    /// `Align::Center` computes the correct center when `TextArea.scale` is applied.
    pub fn new(
        font_system: &mut glyphon::FontSystem,
        width: f32,
        locale: &wmux_config::Locale,
    ) -> Self {
        let mut buf = Buffer::new(font_system, Metrics::new(FONT_SIZE, LINE_HEIGHT));
        buf.set_size(font_system, Some(width), Some(TITLE_BAR_HEIGHT));
        let title = locale.t("titlebar.title").to_string();
        buf.set_text(
            font_system,
            &title,
            &glyphon::Attrs::new()
                .family(Family::Name("Segoe UI"))
                .weight(glyphon::Weight::SEMIBOLD),
            Shaping::Advanced,
            Some(glyphon::cosmic_text::Align::Center),
        );
        buf.shape_until_scroll(font_system, false);
        Self {
            title_buffer: buf,
            last_title: title,
            hovered_button: None,
            is_maximized: false,
            custom_chrome_active: false,
        }
    }

    /// Update the maximized state by querying Win32.
    pub fn update_maximized(&mut self, hwnd: windows::Win32::Foundation::HWND) {
        #[cfg(target_os = "windows")]
        {
            // SAFETY: IsZoomed is safe for any valid HWND from winit.
            self.is_maximized =
                unsafe { windows::Win32::UI::WindowsAndMessaging::IsZoomed(hwnd) }.as_bool();
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = hwnd;
        }
    }

    /// Resize the text buffer when the window width changes.
    ///
    /// `width` must be in **logical pixels** (physical / scale_factor).
    pub fn resize(&mut self, font_system: &mut glyphon::FontSystem, width: f32) {
        self.title_buffer
            .set_size(font_system, Some(width), Some(TITLE_BAR_HEIGHT));
    }

    /// Compute the physical-pixel rect (x, y, w, h) for a given chrome button.
    fn button_rect(
        button: ChromeButton,
        surface_width: f32,
        scale_factor: f32,
    ) -> (f32, f32, f32, f32) {
        let btn_w = BUTTON_WIDTH * scale_factor;
        let tb_h = TITLE_BAR_HEIGHT * scale_factor;
        // Buttons are right-aligned: Close is rightmost, then Maximize, then Minimize.
        let index = match button {
            ChromeButton::Minimize => 2.0,
            ChromeButton::Maximize => 1.0,
            ChromeButton::Close => 0.0,
        };
        let x = surface_width - btn_w * (index + 1.0);
        (x, 0.0, btn_w, tb_h)
    }

    /// Push title bar background and button quads into the pipeline.
    pub fn render_quads(
        &self,
        quads: &mut QuadPipeline,
        ui_chrome: &UiChrome,
        surface_width: f32,
        scale_factor: f32,
    ) {
        if !self.custom_chrome_active {
            return;
        }

        let tb_h = TITLE_BAR_HEIGHT * scale_factor;

        // Full-width background
        quads.push_quad(0.0, 0.0, surface_width, tb_h, ui_chrome.surface_1);

        // Chrome buttons — only draw a background quad when hovered.
        for &button in &[
            ChromeButton::Minimize,
            ChromeButton::Maximize,
            ChromeButton::Close,
        ] {
            if self.hovered_button == Some(button) {
                let (bx, by, bw, bh) = Self::button_rect(button, surface_width, scale_factor);
                let color = if button == ChromeButton::Close {
                    ui_chrome.error
                } else {
                    ui_chrome.surface_2
                };
                quads.push_quad(bx, by, bw, bh, color);
            }
        }
    }

    /// Return a text area descriptor for the centered title text.
    pub fn text_area(
        &self,
        ui_chrome: &UiChrome,
        surface_width: f32,
        scale_factor: f32,
    ) -> Option<glyphon::TextArea<'_>> {
        if !self.custom_chrome_active {
            return None;
        }

        let tb_h = TITLE_BAR_HEIGHT * scale_factor;
        let top = (tb_h - LINE_HEIGHT * scale_factor) / 2.0;

        Some(glyphon::TextArea {
            buffer: &self.title_buffer,
            left: 0.0,
            top,
            scale: scale_factor,
            bounds: glyphon::TextBounds {
                left: 0,
                top: 0,
                right: surface_width as i32,
                bottom: tb_h as i32,
            },
            default_color: f32_to_glyphon_color(ui_chrome.text_secondary),
            custom_glyphs: &[],
        })
    }

    /// Compute updated `CustomGlyph` descriptors for the 3 chrome button icons.
    ///
    /// Returns an array of 3 glyphs: \[minimize, maximize/restore, close\].
    /// The caller stores them in `UiState` and borrows them for the icon TextArea.
    ///
    /// **Important**: positions and sizes are in **logical pixels** (pre-scale).
    /// glyphon multiplies `CustomGlyph.left/top/width/height` by `TextArea.scale`,
    /// so values here must NOT be pre-multiplied by `scale_factor`.
    pub fn chrome_button_glyphs(
        &self,
        surface_width: f32,
        scale_factor: f32,
        ui_chrome: &UiChrome,
    ) -> [CustomGlyph; 3] {
        use wmux_render::svg_icons::*;

        // Logical (unscaled) button geometry — glyphon applies TextArea.scale.
        let logical_width = surface_width / scale_factor;
        let hover_close = self.hovered_button == Some(ChromeButton::Close);
        let close_icon_color = f32_to_glyphon_color(ui_chrome.text_inverse);

        let make_glyph = |button: ChromeButton, icon_id: u16| -> CustomGlyph {
            // Compute button rect in logical pixels.
            let index = match button {
                ChromeButton::Minimize => 2.0,
                ChromeButton::Maximize => 1.0,
                ChromeButton::Close => 0.0,
            };
            let bx = logical_width - BUTTON_WIDTH * (index + 1.0);
            let bh = TITLE_BAR_HEIGHT;
            let icon_x = bx + (BUTTON_WIDTH - ICON_SIZE) / 2.0;
            let icon_y = (bh - ICON_SIZE) / 2.0;
            // Close button icon uses text_inverse on red hover background.
            let color = if button == ChromeButton::Close && hover_close {
                Some(close_icon_color)
            } else {
                None // Uses TextArea's default_color (text_secondary)
            };
            CustomGlyph {
                id: icon_id,
                left: icon_x,
                top: icon_y,
                width: ICON_SIZE,
                height: ICON_SIZE,
                color,
                snap_to_physical_pixel: true,
                metadata: 0,
            }
        };

        let max_icon = if self.is_maximized {
            ICON_CHROME_RESTORE
        } else {
            ICON_CHROME_MAXIMIZE
        };

        [
            make_glyph(ChromeButton::Minimize, ICON_CHROME_MINIMIZE),
            make_glyph(ChromeButton::Maximize, max_icon),
            make_glyph(ChromeButton::Close, ICON_CHROME_CLOSE),
        ]
    }

    /// Test if a pixel coordinate hits a chrome button.
    pub fn hit_test_button(
        &self,
        px: f32,
        py: f32,
        surface_width: f32,
        scale_factor: f32,
    ) -> Option<ChromeButton> {
        if !self.custom_chrome_active {
            return None;
        }

        let tb_h = TITLE_BAR_HEIGHT * scale_factor;
        if py >= tb_h {
            return None;
        }

        for &button in &[
            ChromeButton::Close,
            ChromeButton::Maximize,
            ChromeButton::Minimize,
        ] {
            let (bx, by, bw, bh) = Self::button_rect(button, surface_width, scale_factor);
            if px >= bx && px < bx + bw && py >= by && py < by + bh {
                return Some(button);
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Win32 subclassing — extends client area over the native title bar
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
mod platform {
    use std::sync::atomic::{AtomicU32, Ordering};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows::Win32::Graphics::Dwm::DwmExtendFrameIntoClientArea;
    use windows::Win32::UI::Controls::MARGINS;
    use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClientRect, GetSystemMetrics, GetWindowRect, IsZoomed, SetWindowPos, SWP_FRAMECHANGED,
        SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SYSTEM_METRICS_INDEX,
    };

    use super::{BUTTON_WIDTH, TITLE_BAR_HEIGHT};

    /// Subclass ID — unique per-callback identifier.
    const SUBCLASS_ID: usize = 0x574D_5842; // "WMXB"

    /// Resize edge detection zone in physical pixels.
    const RESIZE_BORDER: i32 = 8;

    /// WM_NCCALCSIZE message.
    const WM_NCCALCSIZE: u32 = 0x0083;
    /// WM_NCHITTEST message.
    const WM_NCHITTEST: u32 = 0x0084;
    /// WM_NCDESTROY message — sent when the non-client area is destroyed.
    const WM_NCDESTROY: u32 = 0x0082;

    // WM_NCHITTEST return values.
    const HTCLIENT: isize = 1;
    const HTCAPTION: isize = 2;
    const HTLEFT: isize = 10;
    const HTRIGHT: isize = 11;
    const HTTOP: isize = 12;
    const HTTOPLEFT: isize = 13;
    const HTTOPRIGHT: isize = 14;
    const HTBOTTOM: isize = 15;
    const HTBOTTOMLEFT: isize = 16;
    const HTBOTTOMRIGHT: isize = 17;

    // System metrics indices.
    const SM_CXSIZEFRAME: SYSTEM_METRICS_INDEX = SYSTEM_METRICS_INDEX(32);
    const SM_CYSIZEFRAME: SYSTEM_METRICS_INDEX = SYSTEM_METRICS_INDEX(33);
    const SM_CXPADDEDBORDER: SYSTEM_METRICS_INDEX = SYSTEM_METRICS_INDEX(92);

    // Scaled metrics — updated when DPI changes, read by subclass proc.
    static TITLE_BAR_HEIGHT_PX: AtomicU32 = AtomicU32::new(36);
    static BUTTON_WIDTH_PX: AtomicU32 = AtomicU32::new(46);

    /// `NCCALCSIZE_PARAMS` layout for `WM_NCCALCSIZE` (wParam=TRUE).
    #[repr(C)]
    struct NcCalcSizeParams {
        rgrc: [RECT; 3],
        _lppos: *const std::ffi::c_void, // WINDOWPOS pointer — not used
    }

    /// Update the physical-pixel metrics used by the subclass proc.
    pub fn update_metrics(scale_factor: f32) {
        TITLE_BAR_HEIGHT_PX.store(
            (TITLE_BAR_HEIGHT * scale_factor).round() as u32,
            Ordering::Relaxed,
        );
        BUTTON_WIDTH_PX.store(
            (BUTTON_WIDTH * scale_factor).round() as u32,
            Ordering::Relaxed,
        );
    }

    /// Install custom window chrome by subclassing the HWND.
    ///
    /// Returns `true` on success. On failure, the native title bar is preserved.
    ///
    /// # Safety
    /// `hwnd` must be a valid window handle from winit.
    pub unsafe fn install_custom_chrome(hwnd: HWND) -> bool {
        // Extend the DWM frame into the client area by 1px at the top.
        // This preserves the DWM drop shadow effect on the window.
        let margins = MARGINS {
            cxLeftWidth: 0,
            cxRightWidth: 0,
            cyTopHeight: 1,
            cyBottomHeight: 0,
        };
        if let Err(e) = DwmExtendFrameIntoClientArea(hwnd, &margins) {
            tracing::warn!(error = %e, "DwmExtendFrameIntoClientArea failed");
            return false;
        }

        // Install the subclass callback that handles WM_NCCALCSIZE and WM_NCHITTEST.
        let ok = SetWindowSubclass(hwnd, Some(chrome_subclass_proc), SUBCLASS_ID, 0);
        if !ok.as_bool() {
            tracing::warn!("SetWindowSubclass failed");
            return false;
        }

        // Force Windows to recalculate the frame. Without this, WM_NCCALCSIZE
        // is not sent until the next resize — the native title bar stays visible.
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
        );

        tracing::info!("custom title bar chrome installed");
        true
    }

    /// Subclass callback — intercepts NC messages for custom title bar behavior.
    ///
    /// All unhandled messages are forwarded to the next handler via `DefSubclassProc`,
    /// which chains back to winit's own WndProc.
    unsafe extern "system" fn chrome_subclass_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _uid: usize,
        _data: usize,
    ) -> LRESULT {
        match msg {
            WM_NCCALCSIZE => handle_nccalcsize(hwnd, wparam, lparam),
            WM_NCHITTEST => handle_nchittest(hwnd, lparam),
            WM_NCDESTROY => {
                // Clean up the subclass before the window is destroyed.
                let _ = RemoveWindowSubclass(hwnd, Some(chrome_subclass_proc), SUBCLASS_ID);
                DefSubclassProc(hwnd, msg, wparam, lparam)
            }
            _ => DefSubclassProc(hwnd, msg, wparam, lparam),
        }
    }

    /// Extend the client area to cover the entire window (removes native title bar).
    ///
    /// When maximized, insets by the frame thickness to prevent overlap with the taskbar.
    unsafe fn handle_nccalcsize(hwnd: HWND, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if wparam.0 == 0 {
            // wParam=FALSE: just validate — nothing to adjust.
            return LRESULT(0);
        }

        // wParam=TRUE: rgrc[0] = proposed window rect. By returning 0 without
        // modifying it, we claim the entire window as client area.

        // When maximized, Windows extends the window beyond the monitor work area
        // by the frame thickness. Inset the rect to prevent taskbar overlap.
        if IsZoomed(hwnd).as_bool() {
            // SAFETY: When WM_NCCALCSIZE wParam=TRUE, Windows guarantees lParam
            // points to a valid NCCALCSIZE_PARAMS struct with the same layout as
            // our NcCalcSizeParams. The pointer is valid for the duration of the message.
            let params = lparam.0 as *mut NcCalcSizeParams;
            let frame_x = GetSystemMetrics(SM_CXSIZEFRAME) + GetSystemMetrics(SM_CXPADDEDBORDER);
            let frame_y = GetSystemMetrics(SM_CYSIZEFRAME) + GetSystemMetrics(SM_CXPADDEDBORDER);
            (*params).rgrc[0].left += frame_x;
            (*params).rgrc[0].top += frame_y;
            (*params).rgrc[0].right -= frame_x;
            (*params).rgrc[0].bottom -= frame_y;
        }

        LRESULT(0)
    }

    /// Hit-test the custom title bar for drag, resize, and button zones.
    ///
    /// Uses `GetWindowRect` for resize edge detection (needs full window extent)
    /// and `GetClientRect` for button/title bar zones (matches wgpu client coords).
    /// On Win10/11, `GetWindowRect` includes invisible DWM borders (~7px) that
    /// `GetClientRect` does not — using the wrong one causes coordinate mismatches.
    unsafe fn handle_nchittest(hwnd: HWND, lparam: LPARAM) -> LRESULT {
        // Extract screen coordinates from lParam (signed 16-bit values).
        let screen_x = (lparam.0 & 0xFFFF) as i16 as i32;
        let screen_y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

        // GetWindowRect includes invisible DWM shadow borders.
        // GetClientRect gives the actual visible client area (after WM_NCCALCSIZE).
        let mut win_rect = RECT::default();
        if GetWindowRect(hwnd, &mut win_rect).is_err() {
            return LRESULT(HTCLIENT);
        }
        let mut client_rect = RECT::default();
        if GetClientRect(hwnd, &mut client_rect).is_err() {
            return LRESULT(HTCLIENT);
        }

        // Window-relative coords (for resize edges — includes invisible borders).
        let wx = screen_x - win_rect.left;
        let wy = screen_y - win_rect.top;
        let win_w = win_rect.right - win_rect.left;
        let win_h = win_rect.bottom - win_rect.top;

        // Client-relative coords (for title bar and buttons — matches wgpu surface).
        // Offset = difference between window rect and client rect (invisible borders).
        let border_left = (win_w - client_rect.right) / 2;
        let border_top = win_h - client_rect.bottom - border_left;
        let cx = screen_x - win_rect.left - border_left;
        let cy = screen_y - win_rect.top - border_top;
        let client_w = client_rect.right;

        let tb_h = TITLE_BAR_HEIGHT_PX.load(Ordering::Relaxed) as i32;
        let btn_w = BUTTON_WIDTH_PX.load(Ordering::Relaxed) as i32;
        let is_max = IsZoomed(hwnd).as_bool();

        // Resize edges — use window-relative coords (includes invisible borders).
        if !is_max {
            if wy < RESIZE_BORDER {
                if wx < RESIZE_BORDER * 2 {
                    return LRESULT(HTTOPLEFT);
                }
                if wx > win_w - RESIZE_BORDER * 2 {
                    return LRESULT(HTTOPRIGHT);
                }
                return LRESULT(HTTOP);
            }
            if wy > win_h - RESIZE_BORDER {
                if wx < RESIZE_BORDER * 2 {
                    return LRESULT(HTBOTTOMLEFT);
                }
                if wx > win_w - RESIZE_BORDER * 2 {
                    return LRESULT(HTBOTTOMRIGHT);
                }
                return LRESULT(HTBOTTOM);
            }
            if wx < RESIZE_BORDER {
                return LRESULT(HTLEFT);
            }
            if wx > win_w - RESIZE_BORDER {
                return LRESULT(HTRIGHT);
            }
        }

        // Title bar zone — use client-relative coords (matches wgpu/winit).
        if cy >= 0 && cy < tb_h {
            // Button area (right side: 3 buttons × btn_w).
            // Return HTCLIENT so wgpu click handler processes the button.
            let buttons_start = client_w - btn_w * 3;
            if cx >= buttons_start {
                return LRESULT(HTCLIENT);
            }
            // Drag zone — enables window move and snap.
            return LRESULT(HTCAPTION);
        }

        // Below title bar — standard client area.
        LRESULT(HTCLIENT)
    }
}

#[cfg(target_os = "windows")]
pub use platform::{install_custom_chrome, update_metrics};

#[cfg(not(target_os = "windows"))]
pub unsafe fn install_custom_chrome(_hwnd: windows::Win32::Foundation::HWND) -> bool {
    false
}

#[cfg(not(target_os = "windows"))]
pub fn update_metrics(_scale_factor: f32) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_bar_height_is_36() {
        assert_eq!(TITLE_BAR_HEIGHT, 36.0);
    }

    #[test]
    fn button_rect_close_is_rightmost() {
        let (x, _, w, _) = TitleBarState::button_rect(ChromeButton::Close, 1200.0, 1.0);
        // Close is rightmost: x = 1200 - 46 * 1 = 1154
        assert!((x - 1154.0).abs() < 0.01);
        assert!((w - 46.0).abs() < 0.01);
    }

    #[test]
    fn button_rect_minimize_is_leftmost() {
        let (x, _, _, _) = TitleBarState::button_rect(ChromeButton::Minimize, 1200.0, 1.0);
        // Minimize: x = 1200 - 46 * 3 = 1062
        assert!((x - 1062.0).abs() < 0.01);
    }

    #[test]
    fn button_rect_scales_with_dpi() {
        let (x1, _, w1, h1) = TitleBarState::button_rect(ChromeButton::Close, 2400.0, 2.0);
        // At 2x: btn_w = 92, tb_h = 72
        assert!((w1 - 92.0).abs() < 0.01);
        assert!((h1 - 72.0).abs() < 0.01);
        // x = 2400 - 92 * 1 = 2308
        assert!((x1 - 2308.0).abs() < 0.01);
    }

    #[test]
    fn chrome_button_eq() {
        assert_eq!(ChromeButton::Close, ChromeButton::Close);
        assert_ne!(ChromeButton::Close, ChromeButton::Minimize);
    }
}
