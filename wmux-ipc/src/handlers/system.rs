use std::future::Future;
use std::pin::Pin;

use serde_json::Value;

use crate::auth::ConnectionCtx;
use crate::handler::{Handler, RpcError};

/// Supported methods exposed in the capabilities list.
///
/// Includes all functional methods across all registered handlers.
/// Stub methods (e.g. most browser.*) are excluded.
const CAPABILITIES: &[&str] = &[
    // system
    "system.ping",
    "system.capabilities",
    "system.identify",
    // workspace
    "workspace.list",
    "workspace.create",
    "workspace.current",
    "workspace.select",
    "workspace.close",
    "workspace.rename",
    // surface
    "surface.split",
    "surface.list",
    "surface.focus",
    "surface.close",
    "surface.send_text",
    "surface.send_key",
    "surface.read_text",
    // sidebar
    "sidebar.set_status",
    "sidebar.clear_status",
    "sidebar.list_status",
    "sidebar.set_progress",
    "sidebar.clear_progress",
    "sidebar.log",
    "sidebar.list_log",
    "sidebar.clear_log",
    "sidebar.state",
    // browser (only identify is functional)
    "browser.identify",
];

/// Handles all `system.*` JSON-RPC methods.
pub struct SystemHandler {
    version: String,
}

impl SystemHandler {
    /// Create a new SystemHandler with the given application version string.
    pub fn new() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_owned(),
        }
    }

    /// Create a SystemHandler with a custom version string (useful for tests).
    #[cfg(test)]
    pub fn with_version(version: impl Into<String>) -> Self {
        Self {
            version: version.into(),
        }
    }
}

impl Default for SystemHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl Handler for SystemHandler {
    fn handle(
        &self,
        method: &str,
        _params: Value,
        _ctx: &ConnectionCtx,
    ) -> Pin<Box<dyn Future<Output = Result<Value, RpcError>> + Send + '_>> {
        let result = match method {
            "ping" => Ok(serde_json::json!({ "pong": true })),
            "capabilities" => {
                let methods: Vec<Value> = CAPABILITIES
                    .iter()
                    .map(|&m| Value::String(m.to_owned()))
                    .collect();
                Ok(serde_json::json!({
                    "methods": methods,
                    "version": self.version,
                }))
            }
            "identify" => Ok(serde_json::json!({
                "app": "wmux",
                "version": self.version,
                "platform": "windows",
                "protocol_version": 1,
            })),
            _ => Err(RpcError::method_not_found(&format!("system.{method}"))),
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

    #[tokio::test]
    async fn ping_returns_pong() {
        let handler = SystemHandler::with_version("0.1.0");
        let result = handler
            .handle("ping", Value::Null, &make_ctx())
            .await
            .unwrap();
        assert_eq!(result["pong"], true);
    }

    #[tokio::test]
    async fn capabilities_returns_method_list() {
        let handler = SystemHandler::with_version("0.1.0");
        let result = handler
            .handle("capabilities", Value::Null, &make_ctx())
            .await
            .unwrap();
        let methods = result["methods"].as_array().unwrap();
        assert!(!methods.is_empty());
        let method_strs: Vec<&str> = methods.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(method_strs.contains(&"system.ping"));
        assert!(method_strs.contains(&"system.capabilities"));
        assert!(method_strs.contains(&"system.identify"));
        assert_eq!(result["version"], "0.1.0");
    }

    #[tokio::test]
    async fn identify_returns_app_info() {
        let handler = SystemHandler::with_version("0.1.0");
        let result = handler
            .handle("identify", Value::Null, &make_ctx())
            .await
            .unwrap();
        assert_eq!(result["app"], "wmux");
        assert_eq!(result["version"], "0.1.0");
        assert_eq!(result["platform"], "windows");
        assert_eq!(result["protocol_version"], 1);
    }

    #[tokio::test]
    async fn unknown_action_returns_method_not_found() {
        let handler = SystemHandler::with_version("0.1.0");
        let err = handler
            .handle("unknown_action", Value::Null, &make_ctx())
            .await
            .unwrap_err();
        assert_eq!(err.code, -32601);
        assert!(err.message.contains("system.unknown_action"));
    }

    #[tokio::test]
    async fn params_ignored_for_ping() {
        let handler = SystemHandler::with_version("0.1.0");
        let large_params = serde_json::json!({ "ignored": "data", "extra": [1, 2, 3] });
        let result = handler
            .handle("ping", large_params, &make_ctx())
            .await
            .unwrap();
        assert_eq!(result["pong"], true);
    }
}
