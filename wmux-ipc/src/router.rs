use crate::protocol::{RpcErrorCode, RpcRequest, RpcResponse};

/// Method router for JSON-RPC v2 requests.
///
/// Currently supports `system.ping` as a built-in method.
/// Will be expanded by the Handler trait in a future task.
#[derive(Debug)]
pub struct Router {
    // Will be expanded with Handler trait registration.
}

impl Router {
    /// Create a new router with built-in methods only.
    pub fn new() -> Self {
        Self {}
    }

    /// Dispatch an RPC request to the appropriate handler.
    pub async fn dispatch(&self, request: &RpcRequest) -> RpcResponse {
        match request.method.as_str() {
            "system.ping" => RpcResponse::success(&request.id, serde_json::json!({"status": "ok"})),
            _ => RpcResponse::error(
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(method: &str) -> RpcRequest {
        RpcRequest {
            id: "test-1".to_owned(),
            method: method.to_owned(),
            params: None,
        }
    }

    #[tokio::test]
    async fn system_ping_returns_success() {
        let router = Router::new();
        let response = router.dispatch(&make_request("system.ping")).await;

        assert!(response.ok);
        assert_eq!(response.id, "test-1");
        let result = response.result.unwrap();
        assert_eq!(result["status"], "ok");
    }

    #[tokio::test]
    async fn unknown_method_returns_not_found() {
        let router = Router::new();
        let response = router.dispatch(&make_request("workspace.destroy")).await;

        assert!(!response.ok);
        assert_eq!(response.id, "test-1");
        let error = response.error.unwrap();
        assert_eq!(error.code, "method_not_found");
        assert!(error.message.contains("workspace.destroy"));
    }
}
