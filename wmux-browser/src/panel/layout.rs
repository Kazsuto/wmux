use webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC;
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    SetWindowPos, ShowWindow, HWND_TOP, SWP_NOACTIVATE, SWP_NOCOPYBITS, SW_HIDE, SW_SHOW,
};

use crate::BrowserError;

use super::BrowserPanel;

impl BrowserPanel {
    /// Set the position and size of the browser panel within the parent window.
    ///
    /// Coordinates `(x, y, width, height)` must be in **physical pixels**
    /// (matching winit's `PhysicalSize`). The layout system provides physical
    /// pixel rects; no DPI scaling is applied here to avoid double-scaling.
    /// Since the host is a popup (top-level) window, we convert client
    /// coordinates to screen coordinates before calling `SetWindowPos`.
    pub fn set_bounds(&self, x: i32, y: i32, width: i32, height: i32) -> Result<(), BrowserError> {
        let controller = self.require_controller()?;

        // Query DPI for diagnostic logging — coordinates are already physical
        // pixels from the layout system, so no scaling is applied.
        // SAFETY: GetDpiForWindow is a standard Win32 API, parent_hwnd is valid.
        let dpi = unsafe { GetDpiForWindow(self.parent_hwnd) };

        // Convert parent-client coords → screen coords for the popup window.
        let mut pt = windows::Win32::Foundation::POINT { x, y };
        // SAFETY: ClientToScreen converts a POINT from client to screen space.
        // parent_hwnd is the valid main window we received in attach().
        if !unsafe { ClientToScreen(self.parent_hwnd, &mut pt) }.as_bool() {
            return Err(BrowserError::General(
                "ClientToScreen failed (invalid parent HWND?)".into(),
            ));
        }

        tracing::debug!(
            client_x = x,
            client_y = y,
            screen_x = pt.x,
            screen_y = pt.y,
            width = width,
            height = height,
            dpi = dpi,
            "setting panel bounds (physical pixels)"
        );

        // Reposition the popup HWND at the computed screen coordinates.
        if let Some(host) = self.host_hwnd {
            // SAFETY: SetWindowPos repositions a valid HWND. The host is an
            // owned popup window we created. HWND_TOP keeps it above the owner.
            unsafe {
                SetWindowPos(
                    host,
                    Some(HWND_TOP),
                    pt.x,
                    pt.y,
                    width,
                    height,
                    SWP_NOACTIVATE | SWP_NOCOPYBITS,
                )
            }
            .map_err(|e| BrowserError::General(format!("SetWindowPos host: {e}")))?;
        }

        // Controller bounds are relative to the host HWND → always (0, 0, w, h).
        let rect = RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        };

        // SAFETY: SetBounds takes a RECT by value. `rect` is fully initialized
        // on the stack and valid for this COM call.
        unsafe { controller.SetBounds(rect) }
            .map_err(|e| BrowserError::General(format!("SetBounds: {e}")))?;

        // Notify WebView2 that the host moved — required for correct hit-testing
        // and input routing when the parent window is dragged.
        // SAFETY: controller is a valid STA-bound COM object on the UI thread.
        unsafe { controller.NotifyParentWindowPositionChanged() }
            .map_err(|e| BrowserError::General(format!("NotifyParentWindowPositionChanged: {e}")))
    }

    /// Show or hide the browser panel (host HWND + WebView2 controller).
    pub fn set_visible(&self, visible: bool) -> Result<(), BrowserError> {
        let controller = self.require_controller()?;

        tracing::debug!(visible = visible, "setting panel visibility");

        // Show/hide the popup host HWND.
        if let Some(host) = self.host_hwnd {
            let cmd = if visible { SW_SHOW } else { SW_HIDE };
            // SAFETY: ShowWindow toggles visibility of a valid HWND we own.
            unsafe {
                let _ = ShowWindow(host, cmd);
            }
        }

        // SAFETY: SetIsVisible takes a plain bool — no pointer concerns.
        unsafe { controller.SetIsVisible(visible) }
            .map_err(|e| BrowserError::General(format!("SetIsVisible: {e}")))
    }

    /// Give keyboard focus to the WebView2 controller.
    ///
    /// Should be called when the user clicks inside the browser panel area.
    /// Sets the internal focus flag so the caller can check `has_focus()`.
    pub fn focus(&mut self) -> Result<(), BrowserError> {
        let controller = self.require_controller()?;

        tracing::debug!(surface_id = %self.surface_id, "focusing browser panel");

        // SAFETY: MoveFocus takes a COREWEBVIEW2_MOVE_FOCUS_REASON value by value.
        // COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC is a valid enum variant.
        unsafe { controller.MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC) }
            .map_err(|e| BrowserError::General(format!("MoveFocus: {e}")))?;

        self.has_focus = true;
        Ok(())
    }

    /// Open the DevTools window for this panel (equivalent to pressing F12).
    ///
    /// Only available in debug builds. In release builds, DevTools are disabled
    /// via `SetAreDevToolsEnabled(false)` at creation time.
    #[cfg(debug_assertions)]
    pub fn open_devtools(&self) -> Result<(), BrowserError> {
        tracing::debug!(surface_id = %self.surface_id, "opening DevTools");

        // SAFETY: OpenDevToolsWindow is a simple COM call with no pointer arguments.
        unsafe { self.require_webview()?.OpenDevToolsWindow() }
            .map_err(|e| BrowserError::General(format!("OpenDevToolsWindow: {e}")))
    }

    /// Notify the panel that focus has left the browser (e.g. user clicked terminal).
    pub fn blur(&mut self) {
        self.has_focus = false;
    }
}

#[cfg(test)]
mod tests {
    use wmux_core::types::SurfaceId;

    use crate::BrowserError;

    use super::super::BrowserPanel;

    #[test]
    fn set_bounds_without_controller_returns_error() {
        let panel = BrowserPanel::new(SurfaceId::new());
        let result = panel.set_bounds(0, 0, 800, 600);
        assert!(matches!(result, Err(BrowserError::ControllerNotAvailable)));
    }

    #[test]
    fn set_visible_without_controller_returns_error() {
        let panel = BrowserPanel::new(SurfaceId::new());
        let result = panel.set_visible(true);
        assert!(matches!(result, Err(BrowserError::ControllerNotAvailable)));
    }

    #[test]
    fn focus_without_controller_returns_error() {
        let mut panel = BrowserPanel::new(SurfaceId::new());
        assert!(matches!(
            panel.focus(),
            Err(BrowserError::ControllerNotAvailable)
        ));
    }

    #[test]
    fn open_devtools_without_webview_returns_error() {
        let panel = BrowserPanel::new(SurfaceId::new());
        assert!(matches!(
            panel.open_devtools(),
            Err(BrowserError::ControllerNotAvailable)
        ));
    }

    #[test]
    fn blur_clears_focus_flag() {
        let mut panel = BrowserPanel::new(SurfaceId::new());
        // Directly manipulate focus state to test blur without COM
        panel.has_focus = true;
        assert!(panel.has_focus());
        panel.blur();
        assert!(!panel.has_focus());
    }
}
