mod dom;
mod inspect;
mod navigation;

use crate::BrowserError;

// Re-export all public functions to preserve the `automation::function_name` API.
pub use dom::{
    check, click, dblclick, fill, focus_element, hover, press_key, scroll_into_view, scroll_page,
    select_option, type_text, uncheck,
};
pub use inspect::{
    find_elements, get_attribute, highlight, is_state, read_console, read_errors, screenshot,
    setup_console_capture, snapshot,
};
pub use navigation::{
    add_init_script, back, current_url, eval, focus_webview, forward, is_webview_focused, navigate,
    reload, wait_for,
};

/// Serialize a value to a JSON string for embedding in JavaScript.
///
/// Maps serialization errors to `BrowserError::JavaScriptError`.
fn json_str(s: &str) -> Result<String, BrowserError> {
    serde_json::to_string(s).map_err(|e| BrowserError::JavaScriptError(e.to_string()))
}

/// Current navigation state of the WebView2 panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NavigationState {
    /// A navigation is in progress.
    Loading,
    /// Navigation completed successfully.
    Complete,
    /// Navigation failed.
    Failed,
}

/// Condition to wait for during `wait_for`.
#[derive(Debug, Clone)]
pub enum WaitCondition {
    /// Wait for a CSS selector to appear in the DOM.
    Selector(String),
    /// Wait for specific text to be present in the document.
    Text(String),
    /// Wait for the URL to match a pattern (substring match).
    UrlPattern(String),
    /// Wait for navigation to reach the Complete state.
    LoadState,
    /// Wait until a JavaScript expression evaluates to truthy.
    ///
    /// **Internal-only** — the JS expression is interpolated into `Boolean(...)`.
    /// If exposed via IPC, the same auth restrictions as `browser.eval` apply.
    JsCondition(String),
}

impl std::fmt::Display for WaitCondition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WaitCondition::Selector(s) => write!(f, "Selector({s})"),
            WaitCondition::Text(t) => write!(f, "Text({t})"),
            WaitCondition::UrlPattern(p) => write!(f, "UrlPattern({p})"),
            WaitCondition::LoadState => write!(f, "LoadState"),
            WaitCondition::JsCondition(js) => write!(f, "JsCondition({js})"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn _assert_send<T: Send>() {}
    fn _assert_sync<T: Sync>() {}

    #[test]
    fn navigation_state_equality() {
        assert_eq!(NavigationState::Loading, NavigationState::Loading);
        assert_eq!(NavigationState::Complete, NavigationState::Complete);
        assert_eq!(NavigationState::Failed, NavigationState::Failed);
        assert_ne!(NavigationState::Loading, NavigationState::Complete);
        assert_ne!(NavigationState::Complete, NavigationState::Failed);
    }

    #[test]
    fn navigation_state_debug() {
        assert_eq!(format!("{:?}", NavigationState::Loading), "Loading");
        assert_eq!(format!("{:?}", NavigationState::Complete), "Complete");
        assert_eq!(format!("{:?}", NavigationState::Failed), "Failed");
    }

    #[test]
    fn wait_condition_display() {
        assert_eq!(
            WaitCondition::Selector("#id".into()).to_string(),
            "Selector(#id)"
        );
        assert_eq!(
            WaitCondition::Text("hello".into()).to_string(),
            "Text(hello)"
        );
        assert_eq!(
            WaitCondition::UrlPattern("example.com".into()).to_string(),
            "UrlPattern(example.com)"
        );
        assert_eq!(WaitCondition::LoadState.to_string(), "LoadState");
        assert_eq!(
            WaitCondition::JsCondition("window.ready".into()).to_string(),
            "JsCondition(window.ready)"
        );
    }

    #[test]
    fn wait_condition_send_sync() {
        _assert_send::<WaitCondition>();
        _assert_sync::<WaitCondition>();
    }

    #[test]
    fn navigation_state_send_sync() {
        _assert_send::<NavigationState>();
        _assert_sync::<NavigationState>();
    }
}
