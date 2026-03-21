use std::future::Future;
use std::pin::Pin;

use serde_json::Value;
use wmux_core::AppStateHandle;

use crate::auth::ConnectionCtx;
use crate::handler::{Handler, RpcError};

/// Maximum string lengths for IPC input validation.
const MAX_KEY_LEN: usize = 255;
const MAX_VALUE_LEN: usize = 4096;
const MAX_LOG_MSG_LEN: usize = 8192;
const MAX_SOURCE_LEN: usize = 255;
const MAX_LABEL_LEN: usize = 255;

/// Validate a string parameter does not exceed the given max length.
fn validate_len(field: &str, value: &str, max: usize) -> Result<(), RpcError> {
    if value.len() > max {
        Err(RpcError::invalid_params(format!(
            "'{field}' exceeds max length {max}"
        )))
    } else {
        Ok(())
    }
}

/// Handles all `sidebar.*` JSON-RPC methods for sidebar metadata management.
pub struct SidebarHandler {
    app_state: AppStateHandle,
}

impl SidebarHandler {
    /// Create a new SidebarHandler with a cloned app state handle.
    pub fn new(app_state: AppStateHandle) -> Self {
        Self { app_state }
    }
}

impl Handler for SidebarHandler {
    fn handle(
        &self,
        method: &str,
        params: Value,
        _ctx: &ConnectionCtx,
    ) -> Pin<Box<dyn Future<Output = Result<Value, RpcError>> + Send + '_>> {
        let method = method.to_owned();
        Box::pin(async move {
            match method.as_str() {
                "set_status" => {
                    let key = params
                        .get("key")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| RpcError::invalid_params("missing 'key'"))?
                        .to_owned();
                    validate_len("key", &key, MAX_KEY_LEN)?;
                    let value = params
                        .get("value")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| RpcError::invalid_params("missing 'value'"))?
                        .to_owned();
                    validate_len("value", &value, MAX_VALUE_LEN)?;
                    let icon = params
                        .get("icon")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    if let Some(ref s) = icon {
                        validate_len("icon", s, MAX_KEY_LEN)?;
                    }
                    let color = params
                        .get("color")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    if let Some(ref s) = color {
                        validate_len("color", s, MAX_KEY_LEN)?;
                    }
                    let pid = params
                        .get("pid")
                        .and_then(|v| v.as_u64())
                        .and_then(|p| u32::try_from(p).ok());

                    self.app_state
                        .sidebar_set_status(key, value, icon, color, pid);
                    tracing::debug!("sidebar.set_status via IPC");
                    Ok(serde_json::json!({ "ok": true }))
                }

                "clear_status" => {
                    let key = params
                        .get("key")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| RpcError::invalid_params("missing 'key'"))?
                        .to_owned();
                    self.app_state.sidebar_clear_status(key);
                    tracing::debug!("sidebar.clear_status via IPC");
                    Ok(serde_json::json!({ "ok": true }))
                }

                "list_status" => {
                    let statuses = self.app_state.sidebar_list_status().await;
                    let list: Vec<Value> = statuses
                        .iter()
                        .map(|e| {
                            serde_json::json!({
                                "key": e.key,
                                "value": e.value,
                                "icon": e.icon,
                                "color": e.color,
                                "pid": e.pid,
                            })
                        })
                        .collect();
                    Ok(Value::Array(list))
                }

                "set_progress" => {
                    let value = params
                        .get("value")
                        .and_then(|v| v.as_f64())
                        .ok_or_else(|| RpcError::invalid_params("missing 'value' (float)"))?
                        as f32;
                    let label = params
                        .get("label")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    if let Some(ref s) = label {
                        validate_len("label", s, MAX_LABEL_LEN)?;
                    }
                    self.app_state.sidebar_set_progress(value, label);
                    tracing::debug!("sidebar.set_progress via IPC");
                    Ok(serde_json::json!({ "ok": true }))
                }

                "clear_progress" => {
                    self.app_state.sidebar_clear_progress();
                    tracing::debug!("sidebar.clear_progress via IPC");
                    Ok(serde_json::json!({ "ok": true }))
                }

                "log" => {
                    let level = params
                        .get("level")
                        .and_then(|v| v.as_str())
                        .unwrap_or("info")
                        .to_owned();
                    let source = params
                        .get("source")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_owned();
                    validate_len("source", &source, MAX_SOURCE_LEN)?;
                    let message = params
                        .get("message")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| RpcError::invalid_params("missing 'message'"))?
                        .to_owned();
                    validate_len("message", &message, MAX_LOG_MSG_LEN)?;
                    self.app_state.sidebar_add_log(level, source, message);
                    tracing::debug!("sidebar.log via IPC");
                    Ok(serde_json::json!({ "ok": true }))
                }

                "list_log" => {
                    let limit = params
                        .get("limit")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(50)
                        .min(10_000) as usize;
                    let logs = self.app_state.sidebar_list_log(limit).await;
                    let entries: Vec<Value> = logs
                        .iter()
                        .map(|e| {
                            serde_json::json!({
                                "level": e.level.to_string(),
                                "source": e.source,
                                "message": e.message,
                            })
                        })
                        .collect();
                    Ok(Value::Array(entries))
                }

                "clear_log" => {
                    self.app_state.sidebar_clear_log();
                    tracing::debug!("sidebar.clear_log via IPC");
                    Ok(serde_json::json!({ "ok": true }))
                }

                "state" => {
                    let snapshot = self.app_state.sidebar_state().await;
                    Ok(serde_json::to_value(snapshot).unwrap_or(Value::Null))
                }

                _ => Err(RpcError::method_not_found(&format!("sidebar.{method}"))),
            }
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

    fn make_handler() -> SidebarHandler {
        let (event_tx, _event_rx) = tokio::sync::mpsc::channel(16);
        let (handle, _actor_handle) = wmux_core::AppStateHandle::spawn(event_tx);
        SidebarHandler::new(handle)
    }

    #[tokio::test]
    async fn unknown_method_returns_error() {
        let handler = make_handler();
        let err = handler
            .handle("bogus", Value::Null, &make_ctx())
            .await
            .unwrap_err();
        assert_eq!(err.code, -32601);
        assert!(err.message.contains("sidebar.bogus"));
    }

    #[tokio::test]
    async fn set_status_missing_key_returns_error() {
        let handler = make_handler();
        let params = serde_json::json!({ "value": "test" });
        let err = handler
            .handle("set_status", params, &make_ctx())
            .await
            .unwrap_err();
        assert_eq!(err.code, -32602);
    }

    #[tokio::test]
    async fn set_status_missing_value_returns_error() {
        let handler = make_handler();
        let params = serde_json::json!({ "key": "test" });
        let err = handler
            .handle("set_status", params, &make_ctx())
            .await
            .unwrap_err();
        assert_eq!(err.code, -32602);
    }

    #[tokio::test]
    async fn clear_status_missing_key_returns_error() {
        let handler = make_handler();
        let err = handler
            .handle("clear_status", Value::Null, &make_ctx())
            .await
            .unwrap_err();
        assert_eq!(err.code, -32602);
    }

    #[tokio::test]
    async fn log_missing_message_returns_error() {
        let handler = make_handler();
        let params = serde_json::json!({ "level": "info" });
        let err = handler
            .handle("log", params, &make_ctx())
            .await
            .unwrap_err();
        assert_eq!(err.code, -32602);
    }

    #[tokio::test]
    async fn set_progress_missing_value_returns_error() {
        let handler = make_handler();
        let err = handler
            .handle("set_progress", Value::Null, &make_ctx())
            .await
            .unwrap_err();
        assert_eq!(err.code, -32602);
    }
}
