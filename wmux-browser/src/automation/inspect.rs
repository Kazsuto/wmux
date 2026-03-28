use webview2_com::Microsoft::Web::WebView2::Win32::{ICoreWebView2, ICoreWebView2Controller};

use crate::BrowserError;

use super::json_str;
use super::navigation::{add_init_script, eval};

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

// -- Console / Error capture -------------------------------------------------

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

/// Read and clear captured window errors. Returns a JSON array.
pub fn read_errors(webview: &ICoreWebView2) -> Result<serde_json::Value, BrowserError> {
    let js = r#"
(function(){
  var errs=window.__wmux_errors||[];
  window.__wmux_errors=[];
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
    #[test]
    #[ignore] // Requires WebView2 runtime
    fn snapshot_requires_webview() {}
}
