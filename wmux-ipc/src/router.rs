use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;

use crate::auth::ConnectionCtx;
use crate::handler::{Handler, RpcError};
use crate::protocol::{RpcErrorCode, RpcRequest, RpcResponse};

/// Method router for JSON-RPC v2 requests.
///
/// Dispatches incoming requests to registered domain handlers by splitting the
/// method name on the first `"."`. The prefix selects a handler; the suffix is
/// passed to the handler as the action.
///
/// Example: `"system.ping"` → handler for `"system"`, action `"ping"`.
pub struct Router {
    handlers: HashMap<String, Arc<dyn Handler>>,
}

impl Router {
    /// Create a new router with built-in handlers pre-registered.
    pub fn new() -> Self {
        let mut router = Self {
            handlers: HashMap::new(),
        };
        router.register(
            "system",
            Arc::new(crate::handlers::system::SystemHandler::new()),
        );
        router
    }

    /// Register a handler for the given domain prefix.
    ///
    /// Overwrites any previously registered handler for the same domain.
    pub fn register(&mut self, domain: &str, handler: Arc<dyn Handler>) {
        self.handlers.insert(domain.to_owned(), handler);
    }

    /// Dispatch a JSON-RPC request to the appropriate domain handler.
    ///
    /// Splits the method name on the first `"."`:
    /// - If a handler is registered for the domain, it receives the action suffix.
    /// - If no handler is found, returns a `method_not_found` error.
    /// - If the method has no `"."`, the whole method is treated as the domain
    ///   with an empty action string, which handlers should return `method_not_found`.
    pub async fn dispatch(&self, request: &RpcRequest, ctx: &ConnectionCtx) -> RpcResponse {
        // Split on first dot to extract domain and action.
        let (domain, action) = match request.method.find('.') {
            Some(pos) => (&request.method[..pos], &request.method[pos + 1..]),
            None => (request.method.as_str(), ""),
        };

        let params = request.params.clone().unwrap_or(Value::Null);

        match self.handlers.get(domain) {
            Some(handler) => match handler.handle(action, params, ctx).await {
                Ok(result) => RpcResponse::success(&request.id, result),
                Err(err) => rpc_error_to_response(&request.id, err),
            },
            None => RpcResponse::error(
                &request.id,
                RpcErrorCode::MethodNotFound,
                &format!("unknown method: {}", request.method),
            ),
        }
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert an `RpcError` (with numeric code) to an `RpcResponse`.
///
/// Maps numeric JSON-RPC codes to `RpcErrorCode` variants for the protocol layer.
fn rpc_error_to_response(id: &str, err: RpcError) -> RpcResponse {
    let code = match err.code {
        -32700 => RpcErrorCode::ParseError,
        -32600 => RpcErrorCode::InvalidRequest,
        -32601 => RpcErrorCode::MethodNotFound,
        -32602 => RpcErrorCode::InvalidParams,
        _ => RpcErrorCode::InternalError,
    };
    RpcResponse::error(id, code, &err.message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::SecurityMode;

    fn make_request(method: &str) -> RpcRequest {
        RpcRequest {
            id: "test-1".to_owned(),
            method: method.to_owned(),
            params: None,
        }
    }

    fn make_ctx() -> ConnectionCtx {
        let mut ctx = ConnectionCtx::new(SecurityMode::AllowAll);
        ctx.authenticate("test-session".to_owned());
        ctx
    }

    #[tokio::test]
    async fn system_ping_returns_pong() {
        let router = Router::new();
        let response = router
            .dispatch(&make_request("system.ping"), &make_ctx())
            .await;

        assert!(response.ok);
        assert_eq!(response.id, "test-1");
        let result = response.result.unwrap();
        assert_eq!(result["pong"], true);
    }

    #[tokio::test]
    async fn system_identify_returns_app_info() {
        let router = Router::new();
        let response = router
            .dispatch(&make_request("system.identify"), &make_ctx())
            .await;

        assert!(response.ok);
        let result = response.result.unwrap();
        assert_eq!(result["app"], "wmux");
        assert_eq!(result["platform"], "windows");
    }

    #[tokio::test]
    async fn system_capabilities_returns_method_list() {
        let router = Router::new();
        let response = router
            .dispatch(&make_request("system.capabilities"), &make_ctx())
            .await;

        assert!(response.ok);
        let result = response.result.unwrap();
        let methods = result["methods"].as_array().unwrap();
        assert!(!methods.is_empty());
    }

    #[tokio::test]
    async fn unknown_domain_returns_method_not_found() {
        let router = Router::new();
        let response = router
            .dispatch(&make_request("workspace.destroy"), &make_ctx())
            .await;

        assert!(!response.ok);
        assert_eq!(response.id, "test-1");
        let error = response.error.unwrap();
        assert_eq!(error.code, "method_not_found");
        assert!(error.message.contains("workspace.destroy"));
    }

    #[tokio::test]
    async fn method_without_dot_returns_method_not_found() {
        let router = Router::new();
        let response = router.dispatch(&make_request("nodot"), &make_ctx()).await;

        assert!(!response.ok);
        let error = response.error.unwrap();
        assert_eq!(error.code, "method_not_found");
    }

    #[tokio::test]
    async fn unknown_action_in_known_domain_returns_method_not_found() {
        let router = Router::new();
        let response = router
            .dispatch(&make_request("system.nonexistent"), &make_ctx())
            .await;

        assert!(!response.ok);
        let error = response.error.unwrap();
        assert_eq!(error.code, "method_not_found");
    }

    #[tokio::test]
    async fn dispatch_with_null_params_does_not_panic() {
        let router = Router::new();
        let mut request = make_request("system.ping");
        request.params = None;
        let response = router.dispatch(&request, &make_ctx()).await;
        assert!(response.ok);
    }

    #[tokio::test]
    async fn dispatch_with_large_params_does_not_crash() {
        let router = Router::new();
        let large: Vec<i64> = (0..10_000).collect();
        let mut request = make_request("system.ping");
        request.params = Some(serde_json::json!({ "data": large }));
        let response = router.dispatch(&request, &make_ctx()).await;
        assert!(response.ok);
    }

    #[tokio::test]
    async fn register_custom_handler() {
        use crate::handler::RpcError;
        use std::future::Future;
        use std::pin::Pin;

        struct EchoHandler;

        impl Handler for EchoHandler {
            fn handle(
                &self,
                method: &str,
                params: Value,
                _ctx: &ConnectionCtx,
            ) -> Pin<Box<dyn Future<Output = Result<Value, RpcError>> + Send + '_>> {
                let result = if method == "echo" {
                    Ok(params)
                } else {
                    Err(RpcError::method_not_found(&format!("echo.{method}")))
                };
                Box::pin(async move { result })
            }
        }

        let mut router = Router::new();
        router.register("echo", Arc::new(EchoHandler));

        let mut request = make_request("echo.echo");
        request.params = Some(serde_json::json!({ "msg": "hello" }));
        let response = router.dispatch(&request, &make_ctx()).await;

        assert!(response.ok);
        assert_eq!(response.result.unwrap()["msg"], "hello");
    }
}
