use std::future::Future;
use std::pin::Pin;

use serde_json::Value;
use tokio::sync::{mpsc, oneshot};

use crate::auth::ConnectionCtx;
use crate::handler::{Handler, RpcError};

/// All browser.* method names supported by this handler.
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
/// Commands are forwarded to the UI thread via a `BrowserCommand` channel.
/// The UI thread processes them on the STA thread (required for WebView2 COM)
/// and sends results back via a oneshot reply channel.
pub struct BrowserHandler {
    browser_cmd_tx: mpsc::Sender<wmux_core::BrowserCommand>,
}

impl BrowserHandler {
    /// Create a new `BrowserHandler` backed by a browser command channel.
    pub fn new(browser_cmd_tx: mpsc::Sender<wmux_core::BrowserCommand>) -> Self {
        Self { browser_cmd_tx }
    }
}

impl Handler for BrowserHandler {
    fn handle(
        &self,
        method: &str,
        params: Value,
        _ctx: &ConnectionCtx,
    ) -> Pin<Box<dyn Future<Output = Result<Value, RpcError>> + Send + '_>> {
        // identify is handled immediately — no need to round-trip to UI thread.
        if method == "identify" {
            tracing::debug!("browser.identify called");
            return Box::pin(async {
                Ok(serde_json::json!({
                    "handler": "browser",
                    "status": "active",
                    "methods": BROWSER_METHODS,
                }))
            });
        }

        let method_owned = method.to_owned();
        let tx = self.browser_cmd_tx.clone();

        Box::pin(async move {
            let (reply_tx, reply_rx) = oneshot::channel();

            let cmd = wmux_core::BrowserCommand {
                method: method_owned,
                params,
                reply: reply_tx,
            };

            tx.send(cmd)
                .await
                .map_err(|_| RpcError::internal_error("browser command channel closed"))?;

            let result = reply_rx
                .await
                .map_err(|_| RpcError::internal_error("browser command reply dropped"))?;

            result.map_err(RpcError::internal_error)
        })
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
        let (tx, _rx) = mpsc::channel(8);
        BrowserHandler::new(tx)
    }

    #[tokio::test]
    async fn identify_returns_capabilities() {
        let handler = make_handler();
        let result = handler
            .handle("identify", Value::Null, &make_ctx())
            .await
            .unwrap();

        assert_eq!(result["handler"], "browser");
        assert_eq!(result["status"], "active");

        let methods = result["methods"].as_array().unwrap();
        assert!(!methods.is_empty());
        let method_strs: Vec<&str> = methods.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(method_strs.contains(&"browser.open"));
        assert!(method_strs.contains(&"browser.navigate"));
        assert!(method_strs.contains(&"browser.identify"));
    }

    #[tokio::test]
    async fn unknown_method_forwards_to_channel() {
        // Handler now forwards all methods to the channel.
        // With a dropped receiver, the channel send will fail.
        let (tx, _rx) = mpsc::channel(1);
        let handler = BrowserHandler::new(tx);

        // Drop the receiver so the channel is closed.
        drop(_rx);

        let err = handler
            .handle("nonexistent", Value::Null, &make_ctx())
            .await
            .unwrap_err();

        assert_eq!(err.code, -32603);
    }
}
