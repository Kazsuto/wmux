use std::future::Future;
use std::pin::Pin;

use serde_json::Value;

use crate::auth::ConnectionCtx;
use crate::handler::{Handler, RpcError};

/// All browser.* method names supported by this handler.
///
/// Most are stubs pending the COM thread bridge (see task L3_07 notes).
const BROWSER_METHODS: &[&str] = &[
    "browser.open",
    "browser.navigate",
    "browser.back",
    "browser.forward",
    "browser.reload",
    "browser.url",
    "browser.eval",
    "browser.click",
    "browser.dblclick",
    "browser.hover",
    "browser.focus",
    "browser.fill",
    "browser.type",
    "browser.press",
    "browser.select",
    "browser.check",
    "browser.uncheck",
    "browser.scroll",
    "browser.snapshot",
    "browser.screenshot",
    "browser.get",
    "browser.is",
    "browser.find",
    "browser.highlight",
    "browser.wait",
    "browser.console",
    "browser.errors",
    "browser.cookies",
    "browser.storage",
    "browser.state",
    "browser.tab",
    "browser.addinitscript",
    "browser.open-split",
    "browser.identify",
];

/// Handles all `browser.*` JSON-RPC methods.
///
/// This is a stub handler: `identify` works immediately, but all other methods
/// return an "not yet wired" error because the actual BrowserManager lives on
/// the COM STA thread and is not `Send + Sync`. A browser command channel will
/// be added to `AppState` in a later task to bridge the two threads.
pub struct BrowserHandler {
    #[expect(
        dead_code,
        reason = "used when browser command channel is wired to AppState"
    )]
    app_state: wmux_core::AppStateHandle,
}

impl BrowserHandler {
    /// Create a new `BrowserHandler` backed by the given `AppStateHandle`.
    pub fn new(app_state: wmux_core::AppStateHandle) -> Self {
        Self { app_state }
    }
}

impl Handler for BrowserHandler {
    fn handle(
        &self,
        method: &str,
        _params: Value,
        _ctx: &ConnectionCtx,
    ) -> Pin<Box<dyn Future<Output = Result<Value, RpcError>> + Send + '_>> {
        let result = match method {
            "identify" => {
                tracing::debug!("browser.identify called");
                Ok(serde_json::json!({
                    "handler": "browser",
                    "status": "stub",
                    "methods": BROWSER_METHODS,
                }))
            }

            "open" | "navigate" | "back" | "forward" | "reload" | "url" | "eval" | "click"
            | "dblclick" | "hover" | "focus" | "fill" | "type" | "press" | "select" | "check"
            | "uncheck" | "scroll" | "snapshot" | "screenshot" | "get" | "is" | "find"
            | "highlight" | "wait" | "console" | "errors" | "cookies" | "storage" | "state"
            | "tab" | "addinitscript" | "open-split" => {
                tracing::debug!(method = %method, "browser stub method called — not yet wired to COM thread");
                Err(RpcError::internal_error(format!(
                    "browser.{method} not yet wired to COM thread"
                )))
            }

            _ => {
                tracing::debug!(method = %method, "browser handler received unknown method");
                Err(RpcError::method_not_found(&format!("browser.{method}")))
            }
        };

        Box::pin(async move { result })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::SecurityMode;

    fn make_ctx() -> ConnectionCtx {
        let mut ctx = ConnectionCtx::new(SecurityMode::AllowAll);
        ctx.authenticate("test-session".to_owned());
        ctx
    }

    fn make_handler() -> BrowserHandler {
        let (event_tx, _event_rx) = tokio::sync::mpsc::channel(8);
        let (app_state, _actor_handle) = wmux_core::AppStateHandle::spawn(event_tx);
        BrowserHandler::new(app_state)
    }

    #[tokio::test]
    async fn identify_returns_capabilities() {
        let handler = make_handler();
        let result = handler
            .handle("identify", Value::Null, &make_ctx())
            .await
            .unwrap();

        assert_eq!(result["handler"], "browser");
        assert_eq!(result["status"], "stub");

        let methods = result["methods"].as_array().unwrap();
        assert!(!methods.is_empty());
        let method_strs: Vec<&str> = methods.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(method_strs.contains(&"browser.open"));
        assert!(method_strs.contains(&"browser.navigate"));
        assert!(method_strs.contains(&"browser.identify"));
    }

    #[tokio::test]
    async fn unknown_method_returns_method_not_found() {
        let handler = make_handler();
        let err = handler
            .handle("nonexistent", Value::Null, &make_ctx())
            .await
            .unwrap_err();

        assert_eq!(err.code, -32601);
        assert!(err.message.contains("browser.nonexistent"));
    }

    #[tokio::test]
    async fn stub_methods_return_internal_error() {
        let handler = make_handler();
        for method in &["open", "navigate", "eval", "screenshot", "open-split"] {
            let err = handler
                .handle(method, Value::Null, &make_ctx())
                .await
                .unwrap_err();

            assert_eq!(
                err.code, -32603,
                "expected internal_error (-32603) for browser.{method}"
            );
            assert!(
                err.message.contains("not yet wired to COM thread"),
                "expected COM thread message for browser.{method}"
            );
        }
    }
}
