use std::sync::mpsc;
use std::time::{Duration, Instant};

use webview2_com::{
    AddScriptToExecuteOnDocumentCreatedCompletedHandler, ExecuteScriptCompletedHandler,
    Microsoft::Web::WebView2::Win32::{
        ICoreWebView2, ICoreWebView2Controller, COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC,
    },
};
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::System::Com::CoTaskMemFree;

use crate::com::recv_with_pump;
use crate::BrowserError;

use super::{json_str, WaitCondition};

/// Navigate the WebView2 to the given URL.
///
/// Only `http://` and `https://` schemes are allowed. Other schemes
/// (`javascript:`, `file://`, `data:`) are rejected to prevent local file
/// exfiltration and script injection via IPC.
pub fn navigate(webview: &ICoreWebView2, url: &str) -> Result<(), BrowserError> {
    // Normalize and validate URL scheme — reject everything except http(s).
    let url = url.trim();
    let lower = url.to_ascii_lowercase();
    if !lower.starts_with("https://") && !lower.starts_with("http://") {
        return Err(BrowserError::InvalidUrlScheme(url.to_owned()));
    }

    let url_wide: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();
    let url_pcwstr = PCWSTR::from_raw(url_wide.as_ptr());

    tracing::debug!(url = %url, "navigating WebView2");

    // SAFETY: Navigate takes a null-terminated wide string. `url_wide` is
    // kept alive for the duration of this call. The ICoreWebView2 COM
    // pointer is valid while `webview` is alive.
    unsafe { webview.Navigate(url_pcwstr) }
        .map_err(|e| BrowserError::NavigationFailed(format!("Navigate({url}): {e}")))
}

/// Navigate back in the browser history.
pub fn back(webview: &ICoreWebView2) -> Result<(), BrowserError> {
    tracing::debug!("navigating back");
    // SAFETY: GoBack is a simple COM call with no pointer arguments.
    unsafe { webview.GoBack() }.map_err(|e| BrowserError::NavigationFailed(format!("GoBack: {e}")))
}

/// Navigate forward in the browser history.
pub fn forward(webview: &ICoreWebView2) -> Result<(), BrowserError> {
    tracing::debug!("navigating forward");
    // SAFETY: GoForward is a simple COM call with no pointer arguments.
    unsafe { webview.GoForward() }
        .map_err(|e| BrowserError::NavigationFailed(format!("GoForward: {e}")))
}

/// Reload the current page.
pub fn reload(webview: &ICoreWebView2) -> Result<(), BrowserError> {
    tracing::debug!("reloading WebView2");
    // SAFETY: Reload is a simple COM call with no pointer arguments.
    unsafe { webview.Reload() }.map_err(|e| BrowserError::NavigationFailed(format!("Reload: {e}")))
}

/// Return the current URL of the WebView2.
///
/// Frees the COM-allocated `PWSTR` before returning.
pub fn current_url(webview: &ICoreWebView2) -> Result<String, BrowserError> {
    let mut uri = PWSTR::null();

    // SAFETY: Source() writes a COM-allocated PWSTR into `uri`.
    // We must call CoTaskMemFree after consuming the string.
    unsafe { webview.Source(&mut uri) }
        .map_err(|e| BrowserError::NavigationFailed(format!("Source: {e}")))?;

    if uri.is_null() {
        return Ok(String::new());
    }

    // SAFETY: `uri` is a valid, null-terminated wide string allocated by
    // the WebView2 COM API. We free it immediately after converting.
    let url_string = unsafe {
        let s = uri
            .to_string()
            .map_err(|_| BrowserError::NavigationFailed("Source: invalid UTF-16".into()));
        CoTaskMemFree(Some(uri.as_ptr().cast()));
        s
    }?;

    Ok(url_string)
}

/// Evaluate a JavaScript expression in the WebView2 context.
///
/// Returns the result as a `serde_json::Value`. If the script returns
/// `undefined` or `null`, the corresponding JSON value is returned.
///
/// # Security
///
/// This function executes arbitrary JavaScript in the WebView2 context.
/// This is intentional — it powers IPC commands like `browser.eval`. Access
/// is gated by the IPC authentication layer (`SecurityMode`). Callers must
/// ensure IPC auth is not `AllowAll` in production configurations.
pub fn eval(webview: &ICoreWebView2, js: &str) -> Result<serde_json::Value, BrowserError> {
    let (tx, rx) = mpsc::sync_channel(1);

    // Clone the COM interface so it can be captured by the 'static closure.
    let webview = webview.clone();
    let js_owned = js.to_owned();

    ExecuteScriptCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| {
            let js_wide: Vec<u16> = js_owned.encode_utf16().chain(std::iter::once(0)).collect();
            let js_pcwstr = PCWSTR::from_raw(js_wide.as_ptr());
            // SAFETY: ExecuteScript takes a PCWSTR and a handler. `js_wide`
            // is kept alive for the duration of this call by the local Vec.
            // The handler is a valid COM object produced by webview2-com.
            unsafe { webview.ExecuteScript(js_pcwstr, &handler) }
                .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |error_code, result_pcwstr| {
            error_code?;
            let result = result_pcwstr;
            let _ = tx.send(result);
            Ok(())
        }),
    )
    .map_err(|e| BrowserError::JavaScriptError(format!("ExecuteScript handler: {e}")))?;

    let result_str = recv_with_pump(&rx)?;

    serde_json::from_str(&result_str)
        .map_err(|e| BrowserError::JavaScriptError(format!("JSON parse: {e} (raw: {result_str})")))
}

/// Inject a JavaScript script that runs on every document creation.
pub fn add_init_script(webview: &ICoreWebView2, js: &str) -> Result<(), BrowserError> {
    let (tx, rx) = mpsc::sync_channel(1);

    // Clone the COM interface so it can be captured by the 'static closure.
    let webview = webview.clone();
    let js_owned = js.to_owned();

    AddScriptToExecuteOnDocumentCreatedCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| {
            let js_wide: Vec<u16> = js_owned.encode_utf16().chain(std::iter::once(0)).collect();
            let js_pcwstr = PCWSTR::from_raw(js_wide.as_ptr());
            // SAFETY: AddScriptToExecuteOnDocumentCreated takes a PCWSTR and a
            // handler. `js_wide` is kept alive for this call by the local Vec.
            // Handler is a valid COM object.
            unsafe { webview.AddScriptToExecuteOnDocumentCreated(js_pcwstr, &handler) }
                .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |error_code, _id_pcwstr| {
            error_code?;
            let _ = tx.send(());
            Ok(())
        }),
    )
    .map_err(|e| {
        BrowserError::JavaScriptError(format!("AddScriptToExecuteOnDocumentCreated: {e}"))
    })?;

    recv_with_pump(&rx)?;

    tracing::debug!("init script added");
    Ok(())
}

/// Focus the WebView2 controller so it receives keyboard input.
pub fn focus_webview(controller: &ICoreWebView2Controller) -> Result<(), BrowserError> {
    tracing::debug!("focusing WebView2 controller");
    // SAFETY: MoveFocus with PROGRAMMATIC reason is a well-documented COM call
    // that transfers keyboard focus to the WebView2 host window.
    unsafe { controller.MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC) }
        .map_err(|e| BrowserError::General(format!("MoveFocus: {e}")))
}

/// Return whether the WebView2 controller is currently visible.
///
/// This is a proxy for "is focused" from the layout perspective; true focus
/// tracking would require subscribing to `GotFocus`/`LostFocus` events.
pub fn is_webview_focused(controller: &ICoreWebView2Controller) -> Result<bool, BrowserError> {
    let mut visible = windows::core::BOOL::default();
    // SAFETY: IsVisible writes into a caller-owned BOOL. `visible` is
    // a stack-allocated value that remains valid for the call.
    unsafe { controller.IsVisible(&mut visible) }
        .map_err(|e| BrowserError::General(format!("IsVisible: {e}")))?;
    Ok(visible.as_bool())
}

/// Wait until `condition` is satisfied or `timeout_ms` milliseconds elapse.
///
/// Polls every 100 ms using `std::thread::sleep`. **Must** be called from a
/// blocking context (e.g. `tokio::task::spawn_blocking`) — never from an
/// async task, as it would block the tokio runtime.
pub fn wait_for(
    webview: &ICoreWebView2,
    condition: &WaitCondition,
    timeout_ms: u64,
) -> Result<(), BrowserError> {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let poll_interval = Duration::from_millis(100);

    tracing::debug!(condition = %condition, timeout_ms = timeout_ms, "waiting for condition");

    loop {
        if check_condition(webview, condition)? {
            tracing::debug!(condition = %condition, "condition satisfied");
            return Ok(());
        }

        if Instant::now() >= deadline {
            return Err(BrowserError::Timeout(format!(
                "condition not met within {timeout_ms}ms: {condition}"
            )));
        }

        std::thread::sleep(poll_interval);
    }
}

/// Check whether a `WaitCondition` is currently satisfied.
fn check_condition(
    webview: &ICoreWebView2,
    condition: &WaitCondition,
) -> Result<bool, BrowserError> {
    match condition {
        WaitCondition::LoadState => {
            // Evaluate `document.readyState === 'complete'`
            let result = eval(webview, "document.readyState === 'complete'")?;
            Ok(result.as_bool().unwrap_or(false))
        }
        WaitCondition::Selector(selector) => {
            let js = format!("document.querySelector({}) !== null", json_str(selector)?);
            let result = eval(webview, &js)?;
            Ok(result.as_bool().unwrap_or(false))
        }
        WaitCondition::Text(text) => {
            let js = format!(
                "document.body && document.body.innerText.includes({})",
                json_str(text)?
            );
            let result = eval(webview, &js)?;
            Ok(result.as_bool().unwrap_or(false))
        }
        WaitCondition::UrlPattern(pattern) => {
            let url = current_url(webview)?;
            Ok(url.contains(pattern.as_str()))
        }
        WaitCondition::JsCondition(js) => {
            let wrapped = format!("Boolean({js})");
            let result = eval(webview, &wrapped)?;
            Ok(result.as_bool().unwrap_or(false))
        }
    }
}
