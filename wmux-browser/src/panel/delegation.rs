use crate::{automation, BrowserError};

use super::BrowserPanel;

impl BrowserPanel {
    // -- Navigation ----------------------------------------------------------

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

    // -- DOM interaction -----------------------------------------------------

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

    // -- Form input ----------------------------------------------------------

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

    // -- Inspection ----------------------------------------------------------

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

    // -- Console capture -----------------------------------------------------

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

#[cfg(test)]
mod tests {
    use wmux_core::types::SurfaceId;

    use crate::BrowserError;

    use super::super::BrowserPanel;

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

    // Note: BrowserPanel wraps ICoreWebView2Controller / ICoreWebView2 which are
    // COM STA objects. They contain raw pointers (NonNull<c_void>) and are
    // therefore not Send + Sync by design. Usage must remain on the UI/STA thread.
}
