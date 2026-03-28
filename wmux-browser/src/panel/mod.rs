mod attach;
mod delegation;
mod layout;

use webview2_com::Microsoft::Web::WebView2::Win32::{ICoreWebView2, ICoreWebView2Controller};
use windows::Win32::Foundation::HWND;
use wmux_core::types::SurfaceId;

use crate::BrowserError;

/// A hosted WebView2 browser panel.
///
/// Wraps an `ICoreWebView2Controller` and its associated `ICoreWebView2`
/// view. The controller lives in a dedicated owned popup `HWND` (separate
/// from the wgpu surface) so DWM composites WebView2 above the DirectX
/// swap chain as an independent top-level visual.
///
/// ## ADR: WS_POPUP instead of WS_CHILD
///
/// The documented rule (`webview2-browser.md`) recommends sibling `WS_CHILD`
/// HWNDs. We deliberately use `WS_POPUP` instead because DXGI flip-model
/// swap chains (required by wgpu) occlude all `WS_CHILD` siblings on the
/// same parent — the WebView2 child window becomes invisible behind the
/// swap chain. A `WS_POPUP` owned by the parent bypasses this: DWM
/// composites it as a separate top-level visual. Trade-offs:
/// - Requires `ClientToScreen` conversion for positioning (vs. direct client coords)
/// - Not auto-clipped by parent (managed via `SetWindowPos` bounds)
/// - `WS_EX_NOACTIVATE` prevents focus theft; `WS_EX_TOOLWINDOW` hides from taskbar
pub struct BrowserPanel {
    surface_id: SurfaceId,
    controller: Option<ICoreWebView2Controller>,
    webview: Option<ICoreWebView2>,
    /// Owned popup HWND that hosts the WebView2 controller.
    /// Uses WS_POPUP (not WS_CHILD) so DWM composites it as a separate
    /// top-level visual, immune to DXGI flip swap chain occlusion.
    host_hwnd: Option<HWND>,
    /// Parent (owner) HWND — needed to convert client coords to screen coords.
    parent_hwnd: HWND,
    has_focus: bool,
}

impl std::fmt::Debug for BrowserPanel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowserPanel")
            .field("surface_id", &self.surface_id)
            .field("has_controller", &self.controller.is_some())
            .field("has_webview", &self.webview.is_some())
            .field("has_host_hwnd", &self.host_hwnd.is_some())
            .field("has_focus", &self.has_focus)
            .finish()
    }
}

impl BrowserPanel {
    /// Create a new `BrowserPanel` with uninitialized controller and view.
    ///
    /// Call `attach` to create the WebView2 controller and connect it to
    /// a host `HWND`.
    pub fn new(surface_id: SurfaceId) -> Self {
        Self {
            surface_id,
            controller: None,
            webview: None,
            host_hwnd: None,
            parent_hwnd: HWND::default(),
            has_focus: false,
        }
    }

    /// Return the surface ID associated with this panel.
    #[inline]
    pub fn id(&self) -> SurfaceId {
        self.surface_id
    }

    /// Return whether this panel currently holds keyboard focus.
    #[inline]
    pub fn has_focus(&self) -> bool {
        self.has_focus
    }

    /// Return the webview, or `ControllerNotAvailable` if not attached.
    fn require_webview(&self) -> Result<&ICoreWebView2, BrowserError> {
        self.webview
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)
    }

    /// Return the controller, or `ControllerNotAvailable` if not attached.
    fn require_controller(&self) -> Result<&ICoreWebView2Controller, BrowserError> {
        self.controller
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)
    }

    /// Return a reference to the controller, if available.
    #[inline]
    pub fn controller(&self) -> Option<&ICoreWebView2Controller> {
        self.controller.as_ref()
    }

    /// Return a reference to the webview, if available.
    #[inline]
    pub fn webview(&self) -> Option<&ICoreWebView2> {
        self.webview.as_ref()
    }
}

impl Default for BrowserPanel {
    fn default() -> Self {
        Self::new(SurfaceId::new())
    }
}

#[cfg(test)]
mod tests {
    use wmux_core::types::SurfaceId;

    use super::*;

    #[test]
    fn browser_panel_construction_default() {
        let id = SurfaceId::new();
        let panel = BrowserPanel::new(id);
        assert_eq!(panel.id(), id);
        assert!(panel.controller().is_none());
        assert!(panel.webview().is_none());
        assert!(!panel.has_focus());
    }

    #[test]
    fn browser_panel_default_trait() {
        let panel = BrowserPanel::default();
        assert!(panel.controller().is_none());
        assert!(panel.webview().is_none());
    }

    #[test]
    fn browser_panel_debug() {
        let id = SurfaceId::new();
        let panel = BrowserPanel::new(id);
        let debug_str = format!("{panel:?}");
        assert!(debug_str.contains("BrowserPanel"));
        assert!(debug_str.contains("has_controller: false"));
        assert!(debug_str.contains("has_webview: false"));
        assert!(debug_str.contains("has_host_hwnd: false"));
        assert!(debug_str.contains("has_focus: false"));
    }
}
