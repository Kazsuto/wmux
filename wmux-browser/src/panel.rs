use std::sync::mpsc;

use webview2_com::{
    CreateCoreWebView2ControllerCompletedHandler,
    Microsoft::Web::WebView2::Win32::{
        ICoreWebView2, ICoreWebView2Controller, ICoreWebView2Environment,
    },
};
use windows::Win32::Foundation::{E_POINTER, HWND, RECT};

use crate::{automation, BrowserError};

/// A hosted WebView2 browser panel.
///
/// Wraps an `ICoreWebView2Controller` and its associated `ICoreWebView2`
/// view. The controller is attached to a child `HWND` (never the wgpu
/// surface) and managed via the standard WebView2 COM lifecycle.
pub struct BrowserPanel {
    controller: Option<ICoreWebView2Controller>,
    webview: Option<ICoreWebView2>,
}

impl std::fmt::Debug for BrowserPanel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowserPanel")
            .field("has_controller", &self.controller.is_some())
            .field("has_webview", &self.webview.is_some())
            .finish()
    }
}

impl BrowserPanel {
    /// Create a new `BrowserPanel` with uninitialized controller and view.
    ///
    /// Call `attach` to create the WebView2 controller and connect it to
    /// a host `HWND`.
    pub fn new() -> Self {
        Self {
            controller: None,
            webview: None,
        }
    }

    /// Attach the panel to a host window by creating a WebView2 controller.
    ///
    /// This is a blocking COM operation. Call from a dedicated UI thread
    /// (or `spawn_blocking` in async contexts).
    pub fn attach(
        &mut self,
        environment: &ICoreWebView2Environment,
        hwnd: HWND,
    ) -> Result<(), BrowserError> {
        let (tx, rx) = mpsc::sync_channel(1);

        // Clone the COM interface so it can be moved into the 'static closure.
        let environment = environment.clone();

        CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
            Box::new(move |handler| {
                // SAFETY: CreateCoreWebView2Controller takes a valid HWND and a
                // COM callback handler. Both are valid for the duration of the call.
                // `environment` is a cloned COM reference with its own ref-count.
                unsafe { environment.CreateCoreWebView2Controller(hwnd, &handler) }
                    .map_err(webview2_com::Error::WindowsError)
            }),
            Box::new(move |error_code, controller| {
                error_code?;
                tx.send(controller.ok_or_else(|| windows::core::Error::from(E_POINTER)))
                    .expect("send controller over mpsc channel");
                Ok(())
            }),
        )
        .map_err(|e| {
            BrowserError::EnvironmentCreationFailed(format!("CreateCoreWebView2Controller: {e}"))
        })?;

        let controller = rx
            .recv()
            .map_err(|e| BrowserError::EnvironmentCreationFailed(format!("recv controller: {e}")))?
            .map_err(|e| {
                BrowserError::EnvironmentCreationFailed(format!("controller error: {e}"))
            })?;

        // SAFETY: CoreWebView2() returns the ICoreWebView2 associated with the
        // controller. The controller is valid (just created above).
        let webview = unsafe { controller.CoreWebView2() }
            .map_err(|_| BrowserError::ControllerNotAvailable)?;

        tracing::info!("BrowserPanel attached to HWND");

        self.controller = Some(controller);
        self.webview = Some(webview);
        Ok(())
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

    /// Set the position and size of the browser panel within its host window.
    pub fn set_bounds(&self, x: i32, y: i32, width: i32, height: i32) -> Result<(), BrowserError> {
        let controller = self
            .controller
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;

        let rect = RECT {
            left: x,
            top: y,
            right: x + width,
            bottom: y + height,
        };

        tracing::debug!(
            x = x,
            y = y,
            width = width,
            height = height,
            "setting panel bounds"
        );

        // SAFETY: SetBounds takes a RECT by value. `rect` is fully initialized
        // on the stack and valid for this COM call.
        unsafe { controller.SetBounds(rect) }
            .map_err(|e| BrowserError::General(format!("SetBounds: {e}")))
    }

    /// Show or hide the browser panel.
    pub fn set_visible(&self, visible: bool) -> Result<(), BrowserError> {
        let controller = self
            .controller
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;

        tracing::debug!(visible = visible, "setting panel visibility");

        // SAFETY: SetIsVisible takes a plain bool — no pointer concerns.
        unsafe { controller.SetIsVisible(visible) }
            .map_err(|e| BrowserError::General(format!("SetIsVisible: {e}")))
    }

    /// Navigate the panel to a URL.
    pub fn navigate(&self, url: &str) -> Result<(), BrowserError> {
        let webview = self
            .webview
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;
        automation::navigate(webview, url)
    }

    /// Navigate back in browser history.
    pub fn back(&self) -> Result<(), BrowserError> {
        let webview = self
            .webview
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;
        automation::back(webview)
    }

    /// Navigate forward in browser history.
    pub fn forward(&self) -> Result<(), BrowserError> {
        let webview = self
            .webview
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;
        automation::forward(webview)
    }

    /// Reload the current page.
    pub fn reload(&self) -> Result<(), BrowserError> {
        let webview = self
            .webview
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;
        automation::reload(webview)
    }

    /// Return the current URL.
    pub fn current_url(&self) -> Result<String, BrowserError> {
        let webview = self
            .webview
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;
        automation::current_url(webview)
    }

    /// Evaluate a JavaScript expression and return the JSON result.
    pub fn eval(&self, js: &str) -> Result<serde_json::Value, BrowserError> {
        let webview = self
            .webview
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;
        automation::eval(webview, js)
    }

    /// Inject a script that runs on every document creation.
    pub fn add_init_script(&self, js: &str) -> Result<(), BrowserError> {
        let webview = self
            .webview
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;
        automation::add_init_script(webview, js)
    }

    /// Focus the WebView2 controller.
    pub fn focus_webview(&self) -> Result<(), BrowserError> {
        let controller = self
            .controller
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;
        automation::focus_webview(controller)
    }

    /// Return whether the WebView2 controller is visible (proxy for focus).
    pub fn is_webview_focused(&self) -> Result<bool, BrowserError> {
        let controller = self
            .controller
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;
        automation::is_webview_focused(controller)
    }
}

impl Default for BrowserPanel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_panel_construction_default() {
        let panel = BrowserPanel::new();
        assert!(panel.controller().is_none());
        assert!(panel.webview().is_none());
    }

    #[test]
    fn browser_panel_default_trait() {
        let panel = BrowserPanel::default();
        assert!(panel.controller().is_none());
        assert!(panel.webview().is_none());
    }

    #[test]
    fn browser_panel_debug() {
        let panel = BrowserPanel::new();
        let debug_str = format!("{panel:?}");
        assert!(debug_str.contains("BrowserPanel"));
        assert!(debug_str.contains("has_controller: false"));
        assert!(debug_str.contains("has_webview: false"));
    }

    #[test]
    fn set_bounds_without_controller_returns_error() {
        let panel = BrowserPanel::new();
        let result = panel.set_bounds(0, 0, 800, 600);
        assert!(matches!(result, Err(BrowserError::ControllerNotAvailable)));
    }

    #[test]
    fn set_visible_without_controller_returns_error() {
        let panel = BrowserPanel::new();
        let result = panel.set_visible(true);
        assert!(matches!(result, Err(BrowserError::ControllerNotAvailable)));
    }

    #[test]
    fn navigate_without_webview_returns_error() {
        let panel = BrowserPanel::new();
        let result = panel.navigate("https://example.com");
        assert!(matches!(result, Err(BrowserError::ControllerNotAvailable)));
    }

    #[test]
    fn eval_without_webview_returns_error() {
        let panel = BrowserPanel::new();
        let result = panel.eval("1 + 1");
        assert!(matches!(result, Err(BrowserError::ControllerNotAvailable)));
    }

    // Note: BrowserPanel wraps ICoreWebView2Controller / ICoreWebView2 which are
    // COM STA objects. They contain raw pointers (NonNull<c_void>) and are
    // therefore not Send + Sync by design. Usage must remain on the UI/STA thread.

    #[test]
    #[ignore] // Requires COM runtime and a real HWND with WebView2
    fn attach_and_navigate() {
        // This test requires a real COM environment, HWND, and WebView2 runtime.
        // Run with: cargo test -p wmux-browser -- --ignored
    }
}
