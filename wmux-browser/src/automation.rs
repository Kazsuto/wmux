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

use crate::BrowserError;

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

/// Navigate the WebView2 to the given URL.
///
/// The URL must be a valid absolute URL (e.g. `https://example.com`).
pub fn navigate(webview: &ICoreWebView2, url: &str) -> Result<(), BrowserError> {
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

    let result_str = rx
        .recv()
        .map_err(|e| BrowserError::JavaScriptError(format!("recv JS result: {e}")))?;

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

    rx.recv()
        .map_err(|e| BrowserError::JavaScriptError(format!("recv init script result: {e}")))?;

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
/// Polls every 100 ms using `std::thread::sleep` (call from a blocking
/// context — e.g., inside `tokio::task::spawn_blocking`).
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

// ── DOM interaction ──────────────────────────────────────────────────────────

/// Click the element matching `selector`.
pub fn click(webview: &ICoreWebView2, selector: &str) -> Result<(), BrowserError> {
    let sel_json = json_str(selector)?;
    let js = format!(
        "(function(){{var el=document.querySelector({sel_json});\
         if(!el)throw new Error('Element not found: '+{sel_json});\
         el.click();return true;}})()"
    );
    tracing::debug!(selector = %selector, "click");
    eval(webview, &js)?;
    Ok(())
}

/// Double-click the element matching `selector`.
pub fn dblclick(webview: &ICoreWebView2, selector: &str) -> Result<(), BrowserError> {
    let sel_json = json_str(selector)?;
    let js = format!(
        "(function(){{var el=document.querySelector({sel_json});\
         if(!el)throw new Error('Element not found: '+{sel_json});\
         el.dispatchEvent(new MouseEvent('dblclick',{{bubbles:true,cancelable:true}}));\
         return true;}})()"
    );
    tracing::debug!(selector = %selector, "dblclick");
    eval(webview, &js)?;
    Ok(())
}

/// Hover over the element matching `selector` (dispatches mouseover + mouseenter).
pub fn hover(webview: &ICoreWebView2, selector: &str) -> Result<(), BrowserError> {
    let sel_json = json_str(selector)?;
    let js = format!(
        "(function(){{var el=document.querySelector({sel_json});\
         if(!el)throw new Error('Element not found: '+{sel_json});\
         el.dispatchEvent(new MouseEvent('mouseover',{{bubbles:true,cancelable:true}}));\
         el.dispatchEvent(new MouseEvent('mouseenter',{{bubbles:false,cancelable:false}}));\
         return true;}})()"
    );
    tracing::debug!(selector = %selector, "hover");
    eval(webview, &js)?;
    Ok(())
}

/// Focus the element matching `selector`.
pub fn focus_element(webview: &ICoreWebView2, selector: &str) -> Result<(), BrowserError> {
    let sel_json = json_str(selector)?;
    let js = format!(
        "(function(){{var el=document.querySelector({sel_json});\
         if(!el)throw new Error('Element not found: '+{sel_json});\
         el.focus();return true;}})()"
    );
    tracing::debug!(selector = %selector, "focus_element");
    eval(webview, &js)?;
    Ok(())
}

/// Check the checkbox/radio matching `selector`.
pub fn check(webview: &ICoreWebView2, selector: &str) -> Result<(), BrowserError> {
    let sel_json = json_str(selector)?;
    let js = format!(
        "(function(){{var el=document.querySelector({sel_json});\
         if(!el)throw new Error('Element not found: '+{sel_json});\
         el.checked=true;\
         el.dispatchEvent(new Event('change',{{bubbles:true}}));\
         el.dispatchEvent(new Event('input',{{bubbles:true}}));\
         return true;}})()"
    );
    tracing::debug!(selector = %selector, "check");
    eval(webview, &js)?;
    Ok(())
}

/// Uncheck the checkbox/radio matching `selector`.
pub fn uncheck(webview: &ICoreWebView2, selector: &str) -> Result<(), BrowserError> {
    let sel_json = json_str(selector)?;
    let js = format!(
        "(function(){{var el=document.querySelector({sel_json});\
         if(!el)throw new Error('Element not found: '+{sel_json});\
         el.checked=false;\
         el.dispatchEvent(new Event('change',{{bubbles:true}}));\
         el.dispatchEvent(new Event('input',{{bubbles:true}}));\
         return true;}})()"
    );
    tracing::debug!(selector = %selector, "uncheck");
    eval(webview, &js)?;
    Ok(())
}

/// Scroll the element matching `selector` into view.
pub fn scroll_into_view(webview: &ICoreWebView2, selector: &str) -> Result<(), BrowserError> {
    let sel_json = json_str(selector)?;
    let js = format!(
        "(function(){{var el=document.querySelector({sel_json});\
         if(!el)throw new Error('Element not found: '+{sel_json});\
         el.scrollIntoView({{behavior:'smooth',block:'center'}});\
         return true;}})()"
    );
    tracing::debug!(selector = %selector, "scroll_into_view");
    eval(webview, &js)?;
    Ok(())
}

// ── Form input ───────────────────────────────────────────────────────────────

/// Clear and fill the input/textarea matching `selector` with `value`.
pub fn fill(webview: &ICoreWebView2, selector: &str, value: &str) -> Result<(), BrowserError> {
    let sel_json = json_str(selector)?;
    let val_json = json_str(value)?;
    let js = format!(
        "(function(){{var el=document.querySelector({sel_json});\
         if(!el)throw new Error('Element not found: '+{sel_json});\
         el.focus();\
         el.value={val_json};\
         el.dispatchEvent(new Event('input',{{bubbles:true}}));\
         el.dispatchEvent(new Event('change',{{bubbles:true}}));\
         return true;}})()"
    );
    tracing::debug!(selector = %selector, "fill");
    eval(webview, &js)?;
    Ok(())
}

/// Type `text` character-by-character into the element matching `selector`.
pub fn type_text(webview: &ICoreWebView2, selector: &str, text: &str) -> Result<(), BrowserError> {
    let sel_json = json_str(selector)?;
    let text_json = json_str(text)?;
    let js = format!(
        "(function(){{
         var el=document.querySelector({sel_json});
         if(!el)throw new Error('Element not found: '+{sel_json});
         el.focus();
         var text={text_json};
         for(var i=0;i<text.length;i++){{
           var ch=text[i];
           el.dispatchEvent(new KeyboardEvent('keydown',{{key:ch,bubbles:true}}));
           el.dispatchEvent(new KeyboardEvent('keypress',{{key:ch,bubbles:true}}));
           el.value+=ch;
           el.dispatchEvent(new Event('input',{{bubbles:true}}));
           el.dispatchEvent(new KeyboardEvent('keyup',{{key:ch,bubbles:true}}));
         }}
         el.dispatchEvent(new Event('change',{{bubbles:true}}));
         return true;}})()"
    );
    tracing::debug!(selector = %selector, "type_text");
    eval(webview, &js)?;
    Ok(())
}

/// Dispatch a keyboard event for `key` on the currently focused element.
pub fn press_key(webview: &ICoreWebView2, key: &str) -> Result<(), BrowserError> {
    let key_json = json_str(key)?;
    let js = format!(
        "(function(){{
         var el=document.activeElement||document.body;
         el.dispatchEvent(new KeyboardEvent('keydown',{{key:{key_json},bubbles:true}}));
         el.dispatchEvent(new KeyboardEvent('keypress',{{key:{key_json},bubbles:true}}));
         el.dispatchEvent(new KeyboardEvent('keyup',{{key:{key_json},bubbles:true}}));
         return true;}})()"
    );
    tracing::debug!(key = %key, "press_key");
    eval(webview, &js)?;
    Ok(())
}

/// Set the value of a `<select>` element matching `selector` and dispatch change.
pub fn select_option(
    webview: &ICoreWebView2,
    selector: &str,
    value: &str,
) -> Result<(), BrowserError> {
    let sel_json = json_str(selector)?;
    let val_json = json_str(value)?;
    let js = format!(
        "(function(){{var el=document.querySelector({sel_json});\
         if(!el)throw new Error('Element not found: '+{sel_json});\
         el.value={val_json};\
         el.dispatchEvent(new Event('change',{{bubbles:true}}));\
         return true;}})()"
    );
    tracing::debug!(selector = %selector, value = %value, "select_option");
    eval(webview, &js)?;
    Ok(())
}

// ── Scroll ───────────────────────────────────────────────────────────────────

/// Scroll the page to absolute coordinates `(x, y)`.
pub fn scroll_page(webview: &ICoreWebView2, x: i32, y: i32) -> Result<(), BrowserError> {
    let js = format!("window.scrollTo({x},{y});true");
    tracing::debug!(x = x, y = y, "scroll_page");
    eval(webview, &js)?;
    Ok(())
}

// ── Inspection ───────────────────────────────────────────────────────────────

/// Return an accessibility snapshot of the DOM as a JSON tree (max depth 10).
pub fn snapshot(webview: &ICoreWebView2) -> Result<serde_json::Value, BrowserError> {
    let js = r#"
(function(){
  function nodeInfo(el,depth){
    if(depth>10)return null;
    var obj={
      tag:el.tagName?el.tagName.toLowerCase():'#text',
      role:el.getAttribute?el.getAttribute('role')||'':'',
      ariaLabel:el.getAttribute?el.getAttribute('aria-label')||'':'',
      text:(el.textContent||'').trim().substring(0,100)
    };
    var children=[];
    var childNodes=el.children||[];
    for(var i=0;i<childNodes.length;i++){
      var child=nodeInfo(childNodes[i],depth+1);
      if(child)children.push(child);
    }
    if(children.length>0)obj.children=children;
    return obj;
  }
  return JSON.stringify(nodeInfo(document.body||document.documentElement,0));
})()
"#;
    tracing::debug!("snapshot");
    let raw = eval(webview, js)?;
    let s = raw
        .as_str()
        .ok_or_else(|| BrowserError::JavaScriptError("snapshot: expected string".into()))?;
    serde_json::from_str(s)
        .map_err(|e| BrowserError::JavaScriptError(format!("snapshot JSON: {e}")))
}

/// Return a not-implemented error for screenshot (requires native CapturePreview).
pub fn screenshot(
    _controller: &ICoreWebView2Controller,
    _webview: &ICoreWebView2,
) -> Result<String, BrowserError> {
    Err(BrowserError::General(
        "screenshot requires WebView2 CapturePreview — not yet implemented".into(),
    ))
}

/// Get an attribute (or `.value`/`.checked`/`.textContent`) from the element.
pub fn get_attribute(
    webview: &ICoreWebView2,
    selector: &str,
    attribute: &str,
) -> Result<serde_json::Value, BrowserError> {
    let sel_json = json_str(selector)?;
    let attr_json = json_str(attribute)?;
    let js = format!(
        "(function(){{
         var el=document.querySelector({sel_json});
         if(!el)throw new Error('Element not found: '+{sel_json});
         var attr={attr_json};
         if(attr==='value')return JSON.stringify(el.value);
         if(attr==='checked')return JSON.stringify(el.checked);
         if(attr==='textContent')return JSON.stringify(el.textContent);
         return JSON.stringify(el.getAttribute(attr));
         }})()"
    );
    tracing::debug!(selector = %selector, attribute = %attribute, "get_attribute");
    let raw = eval(webview, &js)?;
    let s = raw
        .as_str()
        .ok_or_else(|| BrowserError::JavaScriptError("get_attribute: expected string".into()))?;
    serde_json::from_str(s)
        .map_err(|e| BrowserError::JavaScriptError(format!("get_attribute JSON: {e}")))
}

/// Check element state: "checked", "disabled", "visible", "editable", "selected", "focused".
pub fn is_state(
    webview: &ICoreWebView2,
    selector: &str,
    state: &str,
) -> Result<bool, BrowserError> {
    let sel_json = json_str(selector)?;
    let js = format!(
        "(function(){{
         var el=document.querySelector({sel_json});
         if(!el)return false;
         var s={state_json};
         if(s==='checked')return el.checked===true;
         if(s==='disabled')return el.disabled===true;
         if(s==='selected')return el.selected===true;
         if(s==='focused')return document.activeElement===el;
         if(s==='editable')return !el.disabled&&!el.readOnly;
         if(s==='visible'){{
           var style=window.getComputedStyle(el);
           return style.display!=='none'&&style.visibility!=='hidden'&&style.opacity!=='0';
         }}
         return false;
         }})()",
        state_json = json_str(state)?
    );
    tracing::debug!(selector = %selector, state = %state, "is_state");
    let result = eval(webview, &js)?;
    Ok(result.as_bool().unwrap_or(false))
}

/// Return an array of element descriptors for all elements matching `selector`.
pub fn find_elements(
    webview: &ICoreWebView2,
    selector: &str,
) -> Result<serde_json::Value, BrowserError> {
    let sel_json = json_str(selector)?;
    let js = format!(
        "(function(){{
         var els=document.querySelectorAll({sel_json});
         var result=[];
         for(var i=0;i<els.length;i++){{
           var el=els[i];
           result.push({{
             tag:el.tagName.toLowerCase(),
             id:el.id||'',
             className:el.className||'',
             text:(el.textContent||'').trim().substring(0,100)
           }});
         }}
         return JSON.stringify(result);
         }})()"
    );
    tracing::debug!(selector = %selector, "find_elements");
    let raw = eval(webview, &js)?;
    let s = raw
        .as_str()
        .ok_or_else(|| BrowserError::JavaScriptError("find_elements: expected string".into()))?;
    serde_json::from_str(s)
        .map_err(|e| BrowserError::JavaScriptError(format!("find_elements JSON: {e}")))
}

/// Inject a temporary red outline on the element matching `selector` for 2 s.
pub fn highlight(webview: &ICoreWebView2, selector: &str) -> Result<(), BrowserError> {
    let sel_json = json_str(selector)?;
    let js = format!(
        "(function(){{
         var el=document.querySelector({sel_json});
         if(!el)throw new Error('Element not found: '+{sel_json});
         var prev=el.style.outline;
         el.style.outline='2px solid red';
         setTimeout(function(){{el.style.outline=prev;}},2000);
         return true;
         }})()"
    );
    tracing::debug!(selector = %selector, "highlight");
    eval(webview, &js)?;
    Ok(())
}

// ── Console / Error capture ───────────────────────────────────────────────────

/// Inject an init script that captures console output and window errors.
pub fn setup_console_capture(webview: &ICoreWebView2) -> Result<(), BrowserError> {
    let js = r#"
(function(){
  if(window.__wmux_console)return true;
  window.__wmux_console=[];
  window.__wmux_errors=[];
  ['log','warn','error'].forEach(function(level){
    var orig=console[level].bind(console);
    console[level]=function(){
      var args=Array.prototype.slice.call(arguments);
      window.__wmux_console.push({level:level,args:args.map(String),ts:Date.now()});
      orig.apply(console,arguments);
    };
  });
  window.onerror=function(msg,src,line,col,err){
    window.__wmux_errors.push({message:String(msg),source:src,line:line,col:col,ts:Date.now()});
    return false;
  };
  return true;
})()
"#;
    tracing::debug!("setup_console_capture");
    add_init_script(webview, js)
}

/// Read and clear captured console messages. Returns a JSON array.
pub fn read_console(webview: &ICoreWebView2) -> Result<serde_json::Value, BrowserError> {
    let js = r#"
(function(){
  var msgs=window.__wmux_console||[];
  window.__wmux_console=[];
  return JSON.stringify(msgs);
})()
"#;
    tracing::debug!("read_console");
    let raw = eval(webview, js)?;
    let s = raw
        .as_str()
        .ok_or_else(|| BrowserError::JavaScriptError("read_console: expected string".into()))?;
    serde_json::from_str(s)
        .map_err(|e| BrowserError::JavaScriptError(format!("read_console JSON: {e}")))
}

/// Read captured window errors. Returns a JSON array.
pub fn read_errors(webview: &ICoreWebView2) -> Result<serde_json::Value, BrowserError> {
    let js = r#"
(function(){
  var errs=window.__wmux_errors||[];
  return JSON.stringify(errs);
})()
"#;
    tracing::debug!("read_errors");
    let raw = eval(webview, js)?;
    let s = raw
        .as_str()
        .ok_or_else(|| BrowserError::JavaScriptError("read_errors: expected string".into()))?;
    serde_json::from_str(s)
        .map_err(|e| BrowserError::JavaScriptError(format!("read_errors JSON: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn _assert_send<T: Send>() {}
    fn _assert_sync<T: Sync>() {}

    fn make_ensure_element_js(selector: &str) -> String {
        let sel_json = serde_json::to_string(selector).unwrap();
        format!(
            "(function(){{var el=document.querySelector({sel_json});\
             if(!el)throw new Error('Element not found: '+{sel_json});\
             return el;}})()"
        )
    }

    #[test]
    fn ensure_element_js_embeds_selector_safely() {
        let js = make_ensure_element_js("button.ok");
        assert!(js.contains("\"button.ok\""));
        assert!(js.contains("Element not found"));
    }

    #[test]
    fn ensure_element_js_escapes_quotes() {
        let js = make_ensure_element_js(r#"[data-id="x"]"#);
        assert!(js.contains(r#"[data-id=\"x\"]"#));
    }

    #[test]
    #[ignore] // Requires WebView2 runtime
    fn click_requires_webview() {}

    #[test]
    #[ignore] // Requires WebView2 runtime
    fn fill_requires_webview() {}

    #[test]
    #[ignore] // Requires WebView2 runtime
    fn snapshot_requires_webview() {}

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
