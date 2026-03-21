use std::sync::mpsc;

use webview2_com::{
    CreateCoreWebView2ControllerCompletedHandler,
    Microsoft::Web::WebView2::Win32::{
        ICoreWebView2, ICoreWebView2Controller, ICoreWebView2Environment,
        COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC,
    },
};
use windows::Win32::Foundation::{E_POINTER, HWND, RECT};
use wmux_core::types::SurfaceId;

use crate::{automation, BrowserError};

/// A hosted WebView2 browser panel.
///
/// Wraps an `ICoreWebView2Controller` and its associated `ICoreWebView2`
/// view. The controller is attached to a child `HWND` (never the wgpu
/// surface) and managed via the standard WebView2 COM lifecycle.
pub struct BrowserPanel {
    surface_id: SurfaceId,
    controller: Option<ICoreWebView2Controller>,
    webview: Option<ICoreWebView2>,
    has_focus: bool,
}

impl std::fmt::Debug for BrowserPanel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowserPanel")
            .field("surface_id", &self.surface_id)
            .field("has_controller", &self.controller.is_some())
            .field("has_webview", &self.webview.is_some())
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

        tracing::info!(surface_id = %self.surface_id, "BrowserPanel attached to HWND");

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

    /// Give keyboard focus to the WebView2 controller.
    ///
    /// Should be called when the user clicks inside the browser panel area.
    /// Sets the internal focus flag so the caller can check `has_focus()`.
    pub fn focus(&mut self) -> Result<(), BrowserError> {
        let controller = self
            .controller
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;

        tracing::debug!(surface_id = %self.surface_id, "focusing browser panel");

        // SAFETY: MoveFocus takes a COREWEBVIEW2_MOVE_FOCUS_REASON value by value.
        // COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC is a valid enum variant.
        unsafe { controller.MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC) }
            .map_err(|e| BrowserError::General(format!("MoveFocus: {e}")))?;

        self.has_focus = true;
        Ok(())
    }

    /// Open the DevTools window for this panel (equivalent to pressing F12).
    pub fn toggle_devtools(&self) -> Result<(), BrowserError> {
        let webview = self
            .webview
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;

        tracing::debug!(surface_id = %self.surface_id, "opening DevTools");

        // SAFETY: OpenDevToolsWindow is a simple COM call with no pointer arguments.
        unsafe { webview.OpenDevToolsWindow() }
            .map_err(|e| BrowserError::General(format!("OpenDevToolsWindow: {e}")))
    }

    /// Notify the panel that focus has left the browser (e.g. user clicked terminal).
    pub fn blur(&mut self) {
        self.has_focus = false;
    }

    // ── DOM interaction ───────────────────────────────────────────────────────

    /// Click the element matching `selector`.
    pub fn click(&self, selector: &str) -> Result<(), BrowserError> {
        let webview = self
            .webview
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;
        automation::click(webview, selector)
    }

    /// Double-click the element matching `selector`.
    pub fn dblclick(&self, selector: &str) -> Result<(), BrowserError> {
        let webview = self
            .webview
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;
        automation::dblclick(webview, selector)
    }

    /// Hover over the element matching `selector`.
    pub fn hover(&self, selector: &str) -> Result<(), BrowserError> {
        let webview = self
            .webview
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;
        automation::hover(webview, selector)
    }

    /// Focus the element matching `selector`.
    pub fn focus_element(&self, selector: &str) -> Result<(), BrowserError> {
        let webview = self
            .webview
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;
        automation::focus_element(webview, selector)
    }

    /// Check the checkbox/radio matching `selector`.
    pub fn check(&self, selector: &str) -> Result<(), BrowserError> {
        let webview = self
            .webview
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;
        automation::check(webview, selector)
    }

    /// Uncheck the checkbox/radio matching `selector`.
    pub fn uncheck(&self, selector: &str) -> Result<(), BrowserError> {
        let webview = self
            .webview
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;
        automation::uncheck(webview, selector)
    }

    /// Scroll the element matching `selector` into view.
    pub fn scroll_into_view(&self, selector: &str) -> Result<(), BrowserError> {
        let webview = self
            .webview
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;
        automation::scroll_into_view(webview, selector)
    }

    /// Clear and fill the input matching `selector` with `value`.
    pub fn fill(&self, selector: &str, value: &str) -> Result<(), BrowserError> {
        let webview = self
            .webview
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;
        automation::fill(webview, selector, value)
    }

    /// Type `text` character-by-character into the element matching `selector`.
    pub fn type_text(&self, selector: &str, text: &str) -> Result<(), BrowserError> {
        let webview = self
            .webview
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;
        automation::type_text(webview, selector, text)
    }

    /// Dispatch a keyboard event for `key` on the active element.
    pub fn press_key(&self, key: &str) -> Result<(), BrowserError> {
        let webview = self
            .webview
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;
        automation::press_key(webview, key)
    }

    /// Set the value of a `<select>` element matching `selector`.
    pub fn select_option(&self, selector: &str, value: &str) -> Result<(), BrowserError> {
        let webview = self
            .webview
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;
        automation::select_option(webview, selector, value)
    }

    /// Scroll the page to absolute coordinates `(x, y)`.
    pub fn scroll_page(&self, x: i32, y: i32) -> Result<(), BrowserError> {
        let webview = self
            .webview
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;
        automation::scroll_page(webview, x, y)
    }

    /// Return an accessibility snapshot of the DOM as a JSON tree.
    pub fn snapshot(&self) -> Result<serde_json::Value, BrowserError> {
        let webview = self
            .webview
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;
        automation::snapshot(webview)
    }

    /// Capture a screenshot (not yet implemented — returns an error).
    pub fn screenshot(&self) -> Result<String, BrowserError> {
        let controller = self
            .controller
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;
        let webview = self
            .webview
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;
        automation::screenshot(controller, webview)
    }

    /// Get an attribute or property from the element matching `selector`.
    pub fn get_attribute(
        &self,
        selector: &str,
        attribute: &str,
    ) -> Result<serde_json::Value, BrowserError> {
        let webview = self
            .webview
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;
        automation::get_attribute(webview, selector, attribute)
    }

    /// Check element state: "checked", "disabled", "visible", "editable", "selected", "focused".
    pub fn is_state(&self, selector: &str, state: &str) -> Result<bool, BrowserError> {
        let webview = self
            .webview
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;
        automation::is_state(webview, selector, state)
    }

    /// Return an array of element descriptors matching `selector`.
    pub fn find_elements(&self, selector: &str) -> Result<serde_json::Value, BrowserError> {
        let webview = self
            .webview
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;
        automation::find_elements(webview, selector)
    }

    /// Inject a temporary red outline on the element matching `selector`.
    pub fn highlight(&self, selector: &str) -> Result<(), BrowserError> {
        let webview = self
            .webview
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;
        automation::highlight(webview, selector)
    }

    /// Inject an init script that captures console output and window errors.
    pub fn setup_console_capture(&self) -> Result<(), BrowserError> {
        let webview = self
            .webview
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;
        automation::setup_console_capture(webview)
    }

    /// Read and clear captured console messages.
    pub fn read_console(&self) -> Result<serde_json::Value, BrowserError> {
        let webview = self
            .webview
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;
        automation::read_console(webview)
    }

    /// Read captured window errors.
    pub fn read_errors(&self) -> Result<serde_json::Value, BrowserError> {
        let webview = self
            .webview
            .as_ref()
            .ok_or(BrowserError::ControllerNotAvailable)?;
        automation::read_errors(webview)
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
        assert!(debug_str.contains("has_focus: false"));
    }

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
    fn navigate_without_webview_returns_error() {
        let panel = BrowserPanel::new(SurfaceId::new());
        let result = panel.navigate("https://example.com");
        assert!(matches!(result, Err(BrowserError::ControllerNotAvailable)));
    }

    #[test]
    fn eval_without_webview_returns_error() {
        let panel = BrowserPanel::new(SurfaceId::new());
        let result = panel.eval("1 + 1");
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
    fn toggle_devtools_without_webview_returns_error() {
        let panel = BrowserPanel::new(SurfaceId::new());
        assert!(matches!(
            panel.toggle_devtools(),
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

    // Note: BrowserPanel wraps ICoreWebView2Controller / ICoreWebView2 which are
    // COM STA objects. They contain raw pointers (NonNull<c_void>) and are
    // therefore not Send + Sync by design. Usage must remain on the UI/STA thread.

    #[test]
    #[ignore] // Requires COM runtime and a real HWND with WebView2
    fn attach_and_navigate() {
        // This test requires a real COM environment, HWND, and WebView2 runtime.
        // Run with: cargo test -p wmux-browser -- --ignored
    }

    #[test]
    fn click_without_webview_returns_error() {
        let panel = BrowserPanel::new(SurfaceId::new());
        assert!(matches!(
            panel.click("button"),
            Err(BrowserError::ControllerNotAvailable)
        ));
    }

    #[test]
    fn fill_without_webview_returns_error() {
        let panel = BrowserPanel::new(SurfaceId::new());
        assert!(matches!(
            panel.fill("input", "value"),
            Err(BrowserError::ControllerNotAvailable)
        ));
    }

    #[test]
    fn snapshot_without_webview_returns_error() {
        let panel = BrowserPanel::new(SurfaceId::new());
        assert!(matches!(
            panel.snapshot(),
            Err(BrowserError::ControllerNotAvailable)
        ));
    }

    #[test]
    fn screenshot_without_controller_returns_error() {
        let panel = BrowserPanel::new(SurfaceId::new());
        assert!(matches!(
            panel.screenshot(),
            Err(BrowserError::ControllerNotAvailable)
        ));
    }

    #[test]
    fn find_elements_without_webview_returns_error() {
        let panel = BrowserPanel::new(SurfaceId::new());
        assert!(matches!(
            panel.find_elements("div"),
            Err(BrowserError::ControllerNotAvailable)
        ));
    }

    #[test]
    fn read_console_without_webview_returns_error() {
        let panel = BrowserPanel::new(SurfaceId::new());
        assert!(matches!(
            panel.read_console(),
            Err(BrowserError::ControllerNotAvailable)
        ));
    }

    #[test]
    fn is_state_without_webview_returns_error() {
        let panel = BrowserPanel::new(SurfaceId::new());
        assert!(matches!(
            panel.is_state("input", "checked"),
            Err(BrowserError::ControllerNotAvailable)
        ));
    }
}
