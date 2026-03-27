use std::sync::mpsc;

use webview2_com::{
    CreateCoreWebView2ControllerCompletedHandler,
    Microsoft::Web::WebView2::Win32::{
        ICoreWebView2, ICoreWebView2Controller, ICoreWebView2Environment,
        COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC,
    },
};
use windows::core::w;
use windows::Win32::Foundation::{E_POINTER, HWND, RECT};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassExW, SetWindowPos, ShowWindow,
    CS_HREDRAW, CS_VREDRAW, HWND_TOP, SWP_NOACTIVATE, SWP_NOCOPYBITS, SW_HIDE, SW_SHOW,
    WNDCLASSEXW, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
};
use wmux_core::types::SurfaceId;

use crate::{automation, BrowserError};

/// A hosted WebView2 browser panel.
///
/// Wraps an `ICoreWebView2Controller` and its associated `ICoreWebView2`
/// view. The controller lives in a dedicated owned popup `HWND` (separate
/// from the wgpu surface) so DWM composites WebView2 above the DirectX
/// swap chain as an independent top-level visual.
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

    /// Attach the panel to a parent window by creating a dedicated owned popup
    /// HWND and then creating the WebView2 controller inside it.
    ///
    /// The popup HWND is a separate top-level window (WS_POPUP) owned by
    /// `parent_hwnd`. DWM composites it as an independent visual, immune to
    /// DXGI flip swap chain occlusion on the parent. This is a blocking
    /// COM operation — call from the UI/STA thread.
    pub fn attach(
        &mut self,
        environment: &ICoreWebView2Environment,
        parent_hwnd: HWND,
    ) -> Result<(), BrowserError> {
        // Create an owned popup HWND to host the WebView2 controller.
        // Using WS_POPUP (not WS_CHILD) ensures DWM composites the WebView2
        // as a separate top-level visual, unaffected by the wgpu swap chain.
        let host = create_host_hwnd(parent_hwnd).map_err(|e| {
            BrowserError::General(format!("failed to create WebView2 host HWND: {e}"))
        })?;
        // Store the host HWND immediately so Drop cleans it up on error paths.
        self.host_hwnd = Some(host);
        self.parent_hwnd = parent_hwnd;

        let (tx, rx) = mpsc::sync_channel(1);

        // Clone the COM interface so it can be moved into the 'static closure.
        let environment = environment.clone();

        CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
            Box::new(move |handler| {
                // SAFETY: CreateCoreWebView2Controller takes a valid HWND and a
                // COM callback handler. Both are valid for the duration of the call.
                // `environment` is a cloned COM reference with its own ref-count.
                // `host` is a valid child HWND just created above.
                unsafe { environment.CreateCoreWebView2Controller(host, &handler) }
                    .map_err(webview2_com::Error::WindowsError)
            }),
            Box::new(move |error_code, controller| {
                error_code?;
                let _ = tx.send(controller.ok_or_else(|| windows::core::Error::from(E_POINTER)));
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

    /// Set the position and size of the browser panel within the parent window.
    ///
    /// Coordinates `(x, y)` are in the parent's **client** coordinate space.
    /// Since the host is a popup (top-level) window, we convert to screen
    /// coordinates before calling `SetWindowPos`.
    pub fn set_bounds(&self, x: i32, y: i32, width: i32, height: i32) -> Result<(), BrowserError> {
        let controller = self.require_controller()?;

        // Convert parent-client coords → screen coords for the popup window.
        let mut pt = windows::Win32::Foundation::POINT { x, y };
        // SAFETY: ClientToScreen converts a POINT from client to screen space.
        // parent_hwnd is the valid main window we received in attach().
        let _ = unsafe { ClientToScreen(self.parent_hwnd, &mut pt) };

        tracing::debug!(
            client_x = x,
            client_y = y,
            screen_x = pt.x,
            screen_y = pt.y,
            width = width,
            height = height,
            "setting panel bounds"
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

    /// Navigate the panel to a URL.
    pub fn navigate(&self, url: &str) -> Result<(), BrowserError> {
        automation::navigate(self.require_webview()?, url)
    }

    /// Navigate back in browser history.
    pub fn back(&self) -> Result<(), BrowserError> {
        automation::back(self.require_webview()?)
    }

    /// Navigate forward in browser history.
    pub fn forward(&self) -> Result<(), BrowserError> {
        automation::forward(self.require_webview()?)
    }

    /// Reload the current page.
    pub fn reload(&self) -> Result<(), BrowserError> {
        automation::reload(self.require_webview()?)
    }

    /// Return the current URL.
    pub fn current_url(&self) -> Result<String, BrowserError> {
        automation::current_url(self.require_webview()?)
    }

    /// Evaluate a JavaScript expression and return the JSON result.
    pub fn eval(&self, js: &str) -> Result<serde_json::Value, BrowserError> {
        automation::eval(self.require_webview()?, js)
    }

    /// Inject a script that runs on every document creation.
    pub fn add_init_script(&self, js: &str) -> Result<(), BrowserError> {
        automation::add_init_script(self.require_webview()?, js)
    }

    /// Focus the WebView2 controller.
    pub fn focus_webview(&self) -> Result<(), BrowserError> {
        automation::focus_webview(self.require_controller()?)
    }

    /// Return whether the WebView2 controller is visible (proxy for focus).
    pub fn is_webview_focused(&self) -> Result<bool, BrowserError> {
        automation::is_webview_focused(self.require_controller()?)
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

    // ── DOM interaction ───────────────────────────────────────────────────────

    /// Click the element matching `selector`.
    pub fn click(&self, selector: &str) -> Result<(), BrowserError> {
        automation::click(self.require_webview()?, selector)
    }

    /// Double-click the element matching `selector`.
    pub fn dblclick(&self, selector: &str) -> Result<(), BrowserError> {
        automation::dblclick(self.require_webview()?, selector)
    }

    /// Hover over the element matching `selector`.
    pub fn hover(&self, selector: &str) -> Result<(), BrowserError> {
        automation::hover(self.require_webview()?, selector)
    }

    /// Focus the element matching `selector`.
    pub fn focus_element(&self, selector: &str) -> Result<(), BrowserError> {
        automation::focus_element(self.require_webview()?, selector)
    }

    /// Check the checkbox/radio matching `selector`.
    pub fn check(&self, selector: &str) -> Result<(), BrowserError> {
        automation::check(self.require_webview()?, selector)
    }

    /// Uncheck the checkbox/radio matching `selector`.
    pub fn uncheck(&self, selector: &str) -> Result<(), BrowserError> {
        automation::uncheck(self.require_webview()?, selector)
    }

    /// Scroll the element matching `selector` into view.
    pub fn scroll_into_view(&self, selector: &str) -> Result<(), BrowserError> {
        automation::scroll_into_view(self.require_webview()?, selector)
    }

    /// Clear and fill the input matching `selector` with `value`.
    pub fn fill(&self, selector: &str, value: &str) -> Result<(), BrowserError> {
        automation::fill(self.require_webview()?, selector, value)
    }

    /// Type `text` character-by-character into the element matching `selector`.
    pub fn type_text(&self, selector: &str, text: &str) -> Result<(), BrowserError> {
        automation::type_text(self.require_webview()?, selector, text)
    }

    /// Dispatch a keyboard event for `key` on the active element.
    pub fn press_key(&self, key: &str) -> Result<(), BrowserError> {
        automation::press_key(self.require_webview()?, key)
    }

    /// Set the value of a `<select>` element matching `selector`.
    pub fn select_option(&self, selector: &str, value: &str) -> Result<(), BrowserError> {
        automation::select_option(self.require_webview()?, selector, value)
    }

    /// Scroll the page to absolute coordinates `(x, y)`.
    pub fn scroll_page(&self, x: i32, y: i32) -> Result<(), BrowserError> {
        automation::scroll_page(self.require_webview()?, x, y)
    }

    /// Return an accessibility snapshot of the DOM as a JSON tree.
    pub fn snapshot(&self) -> Result<serde_json::Value, BrowserError> {
        automation::snapshot(self.require_webview()?)
    }

    /// Capture a screenshot (not yet implemented — returns an error).
    pub fn screenshot(&self) -> Result<String, BrowserError> {
        automation::screenshot(self.require_controller()?, self.require_webview()?)
    }

    /// Get an attribute or property from the element matching `selector`.
    pub fn get_attribute(
        &self,
        selector: &str,
        attribute: &str,
    ) -> Result<serde_json::Value, BrowserError> {
        automation::get_attribute(self.require_webview()?, selector, attribute)
    }

    /// Check element state: "checked", "disabled", "visible", "editable", "selected", "focused".
    pub fn is_state(&self, selector: &str, state: &str) -> Result<bool, BrowserError> {
        automation::is_state(self.require_webview()?, selector, state)
    }

    /// Return an array of element descriptors matching `selector`.
    pub fn find_elements(&self, selector: &str) -> Result<serde_json::Value, BrowserError> {
        automation::find_elements(self.require_webview()?, selector)
    }

    /// Inject a temporary red outline on the element matching `selector`.
    pub fn highlight(&self, selector: &str) -> Result<(), BrowserError> {
        automation::highlight(self.require_webview()?, selector)
    }

    /// Inject an init script that captures console output and window errors.
    pub fn setup_console_capture(&self) -> Result<(), BrowserError> {
        automation::setup_console_capture(self.require_webview()?)
    }

    /// Read and clear captured console messages.
    pub fn read_console(&self) -> Result<serde_json::Value, BrowserError> {
        automation::read_console(self.require_webview()?)
    }

    /// Read captured window errors.
    pub fn read_errors(&self) -> Result<serde_json::Value, BrowserError> {
        automation::read_errors(self.require_webview()?)
    }
}

impl Drop for BrowserPanel {
    fn drop(&mut self) {
        if let Some(controller) = self.controller.take() {
            // SAFETY: Close() releases the WebView2 controller and its
            // associated browser process + child HWND.  Must be called to
            // prevent resource leaks when a panel is removed.
            let _ = unsafe { controller.Close() };
            tracing::debug!(surface_id = %self.surface_id, "BrowserPanel controller closed");
        }
        if let Some(host) = self.host_hwnd.take() {
            // SAFETY: DestroyWindow destroys a valid HWND we own. Must be
            // called after Close() so WebView2 releases its child windows first.
            let _ = unsafe { DestroyWindow(host) };
            tracing::debug!(surface_id = %self.surface_id, "host HWND destroyed");
        }
    }
}

/// Default window procedure forwarding for the host popup HWND.
///
/// All messages are passed through to `DefWindowProcW` — the host HWND
/// exists solely as a container for WebView2's own child windows.
unsafe extern "system" fn host_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

/// Create an owned popup HWND to host a WebView2 controller.
///
/// Uses `WS_POPUP` (not `WS_CHILD`) so the host is a separate top-level
/// window in DWM's visual tree. This avoids the known issue where DXGI
/// flip-model swap chains occlude child HWNDs on the same parent.
///
/// `WS_EX_TOOLWINDOW` prevents a taskbar entry. `WS_EX_NOACTIVATE` prevents
/// stealing focus from the main window on click.
///
/// The popup is **owned** by `parent` (via the `hwndParent` parameter), so
/// DWM keeps it above the owner and hides/shows it when the owner is
/// minimized/restored.
fn create_host_hwnd(parent: HWND) -> Result<HWND, windows::core::Error> {
    // SAFETY: GetModuleHandleW(None) returns the current executable's HINSTANCE.
    let hinstance = unsafe { GetModuleHandleW(None)? };

    let class_name = w!("WmuxWebView2Host");
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(host_wndproc),
        hInstance: hinstance.into(),
        lpszClassName: class_name,
        style: CS_HREDRAW | CS_VREDRAW,
        ..Default::default()
    };
    // RegisterClassExW is idempotent — returns 0 if class already exists, which is fine.
    // SAFETY: wc is fully initialized with valid function pointer and class name.
    unsafe {
        RegisterClassExW(&wc);
    }

    // SAFETY: CreateWindowExW with WS_POPUP creates a top-level popup window
    // owned by `parent`. WS_EX_TOOLWINDOW prevents a taskbar button.
    // WS_EX_NOACTIVATE prevents focus theft on click (WebView2 manages its
    // own focus via MoveFocus). The popup is created hidden at (0,0,0,0) —
    // the caller sets position via `set_bounds`.
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class_name,
            w!(""),
            WS_POPUP | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
            0,
            0,
            0,
            0,
            Some(parent),
            None,
            Some(hinstance.into()),
            None,
        )?
    };

    tracing::debug!("WebView2 host popup HWND created");
    Ok(hwnd)
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
