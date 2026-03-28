use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2;

use crate::BrowserError;

use super::json_str;
use super::navigation::eval;

// -- DOM interaction ---------------------------------------------------------

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

// -- Form input --------------------------------------------------------------

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

// -- Scroll ------------------------------------------------------------------

/// Scroll the page to absolute coordinates `(x, y)`.
pub fn scroll_page(webview: &ICoreWebView2, x: i32, y: i32) -> Result<(), BrowserError> {
    let js = format!("window.scrollTo({x},{y});true");
    tracing::debug!(x = x, y = y, "scroll_page");
    eval(webview, &js)?;
    Ok(())
}

#[cfg(test)]
mod tests {
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
}
