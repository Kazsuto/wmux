use std::future::Future;
use std::pin::Pin;

use serde_json::Value;

use crate::auth::ConnectionCtx;

/// Error returned by handler implementations.
#[derive(Debug, Clone)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<Value>,
}

impl RpcError {
    /// JSON-RPC v2 parse error (-32700).
    pub fn parse_error(msg: impl Into<String>) -> Self {
        Self {
            code: -32700,
            message: msg.into(),
            data: None,
        }
    }

    /// JSON-RPC v2 invalid request (-32600).
    pub fn invalid_request(msg: impl Into<String>) -> Self {
        Self {
            code: -32600,
            message: msg.into(),
            data: None,
        }
    }

    /// JSON-RPC v2 method not found (-32601).
    pub fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("Method not found: {method}"),
            data: None,
        }
    }

    /// JSON-RPC v2 invalid params (-32602).
    pub fn invalid_params(msg: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: msg.into(),
            data: None,
        }
    }

    /// JSON-RPC v2 internal error (-32603).
    pub fn internal_error(msg: impl Into<String>) -> Self {
        Self {
            code: -32603,
            message: msg.into(),
            data: None,
        }
    }
}

/// Handler trait for JSON-RPC domain handlers.
///
/// One implementation per domain (system, workspace, surface, etc.).
/// Handlers are shared across connections and must be `Send + Sync`.
///
/// Uses `Pin<Box<dyn Future>>` for object safety — required to store
/// `Arc<dyn Handler>` in a `HashMap`.
pub trait Handler: Send + Sync {
    /// Handle a JSON-RPC method call.
    ///
    /// `method` is the action part after the dot (e.g., `"ping"` for `"system.ping"`).
    /// `params` is the request params (`Value::Null` if absent).
    /// `ctx` is the per-connection authentication context.
    fn handle(
        &self,
        method: &str,
        params: Value,
        ctx: &ConnectionCtx,
    ) -> Pin<Box<dyn Future<Output = Result<Value, RpcError>> + Send + '_>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn _assert_send<T: Send + Sync>() {}

    #[test]
    fn rpc_error_codes_match_jsonrpc_spec() {
        assert_eq!(RpcError::parse_error("").code, -32700);
        assert_eq!(RpcError::invalid_request("").code, -32600);
        assert_eq!(RpcError::method_not_found("foo").code, -32601);
        assert_eq!(RpcError::invalid_params("").code, -32602);
        assert_eq!(RpcError::internal_error("").code, -32603);
    }

    #[test]
    fn rpc_error_method_not_found_includes_name() {
        let err = RpcError::method_not_found("workspace.list");
        assert!(err.message.contains("workspace.list"));
    }
}
