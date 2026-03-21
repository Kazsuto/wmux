use std::future::Future;
use std::pin::Pin;

use serde_json::Value;
use uuid::Uuid;
use wmux_core::{AppStateHandle, WorkspaceId};

use crate::auth::ConnectionCtx;
use crate::handler::{Handler, RpcError};

/// Maximum workspace name length to prevent abuse.
const MAX_WORKSPACE_NAME_LEN: usize = 255;

/// Handles all `workspace.*` JSON-RPC methods.
pub struct WorkspaceHandler {
    app_state: AppStateHandle,
}

impl WorkspaceHandler {
    /// Create a new WorkspaceHandler with a cloned app state handle.
    pub fn new(app_state: AppStateHandle) -> Self {
        Self { app_state }
    }
}

impl Handler for WorkspaceHandler {
    fn handle(
        &self,
        method: &str,
        params: Value,
        _ctx: &ConnectionCtx,
    ) -> Pin<Box<dyn Future<Output = Result<Value, RpcError>> + Send + '_>> {
        // Convert method to owned String so the future does not capture the
        // short-lived `&str` reference — required for `Send + '_` bound.
        let method = method.to_owned();
        Box::pin(async move {
            match method.as_str() {
                "list" => {
                    let workspaces = self.app_state.list_workspaces().await;
                    let items: Vec<Value> = workspaces
                        .into_iter()
                        .map(|w| {
                            serde_json::json!({
                                "id": w.id.to_string(),
                                "name": w.name,
                                "active": w.active,
                                "pane_count": w.pane_count,
                            })
                        })
                        .collect();
                    tracing::debug!(count = items.len(), "workspace.list responded");
                    Ok(Value::Array(items))
                }

                "create" => {
                    let name = params
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("workspace")
                        .to_owned();
                    if name.len() > MAX_WORKSPACE_NAME_LEN {
                        return Err(RpcError::invalid_params(format!(
                            "name too long: {} bytes (max {MAX_WORKSPACE_NAME_LEN})",
                            name.len()
                        )));
                    }
                    match self.app_state.create_workspace(name).await {
                        Some(id) => {
                            tracing::info!(workspace_id = %id, "workspace created via IPC");
                            Ok(serde_json::json!({ "workspace_id": id.to_string() }))
                        }
                        None => Err(RpcError::internal_error("actor did not respond")),
                    }
                }

                "current" => match self.app_state.get_current_workspace().await {
                    Some(w) => Ok(serde_json::json!({
                        "id": w.id.to_string(),
                        "name": w.name,
                        "active": w.active,
                        "pane_count": w.pane_count,
                    })),
                    None => Err(RpcError::internal_error("no active workspace")),
                },

                "select" => {
                    if let Some(id_str) = params.get("workspace_id").and_then(|v| v.as_str()) {
                        let uuid = Uuid::parse_str(id_str).map_err(|_| {
                            RpcError::invalid_params(format!("invalid workspace_id: {id_str}"))
                        })?;
                        let id = WorkspaceId::from_uuid(uuid);
                        let found = self.app_state.select_workspace_by_id(id).await;
                        if found {
                            tracing::info!(workspace_id = %id, "workspace selected by id");
                            Ok(serde_json::json!({ "ok": true }))
                        } else {
                            Err(RpcError::invalid_params(format!(
                                "workspace not found: {id_str}"
                            )))
                        }
                    } else if let Some(index_val) = params.get("index").and_then(|v| v.as_u64()) {
                        if index_val == 0 {
                            return Err(RpcError::invalid_params("index is 1-based, must be >= 1"));
                        }
                        let zero_based = (index_val - 1) as usize;
                        let workspaces = self.app_state.list_workspaces().await;
                        if zero_based >= workspaces.len() {
                            return Err(RpcError::invalid_params(format!(
                                "index {index_val} out of range (1-{})",
                                workspaces.len()
                            )));
                        }
                        self.app_state.switch_workspace(zero_based);
                        tracing::info!(index = zero_based, "workspace selected by index");
                        Ok(serde_json::json!({ "ok": true }))
                    } else {
                        Err(RpcError::invalid_params(
                            "params must contain 'workspace_id' or 'index'",
                        ))
                    }
                }

                "close" => {
                    let id_str = params
                        .get("workspace_id")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| RpcError::invalid_params("missing 'workspace_id'"))?;
                    let uuid = Uuid::parse_str(id_str).map_err(|_| {
                        RpcError::invalid_params(format!("invalid workspace_id: {id_str}"))
                    })?;
                    let id = WorkspaceId::from_uuid(uuid);
                    self.app_state.close_workspace(id);
                    tracing::info!(workspace_id = %id, "workspace closed via IPC");
                    Ok(serde_json::json!({ "ok": true }))
                }

                "rename" => {
                    let id_str = params
                        .get("workspace_id")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| RpcError::invalid_params("missing 'workspace_id'"))?;
                    let name = params
                        .get("name")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| RpcError::invalid_params("missing 'name'"))?
                        .to_owned();
                    if name.len() > MAX_WORKSPACE_NAME_LEN {
                        return Err(RpcError::invalid_params(format!(
                            "name too long: {} bytes (max {MAX_WORKSPACE_NAME_LEN})",
                            name.len()
                        )));
                    }
                    let uuid = Uuid::parse_str(id_str).map_err(|_| {
                        RpcError::invalid_params(format!("invalid workspace_id: {id_str}"))
                    })?;
                    let id = WorkspaceId::from_uuid(uuid);
                    self.app_state.rename_workspace(id, name);
                    tracing::info!(workspace_id = %id, "workspace renamed via IPC");
                    Ok(serde_json::json!({ "ok": true }))
                }

                _ => Err(RpcError::method_not_found(&format!("workspace.{method}"))),
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

    fn make_handler() -> WorkspaceHandler {
        let (event_tx, _event_rx) = tokio::sync::mpsc::channel(16);
        let handle = wmux_core::AppStateHandle::spawn(event_tx);
        WorkspaceHandler::new(handle)
    }

    #[tokio::test]
    async fn list_returns_array() {
        let handler = make_handler();
        let result = handler
            .handle("list", Value::Null, &make_ctx())
            .await
            .unwrap();
        assert!(result.is_array());
    }

    #[tokio::test]
    async fn create_returns_workspace_id() {
        let handler = make_handler();
        let params = serde_json::json!({ "name": "test-ws" });
        let result = handler.handle("create", params, &make_ctx()).await.unwrap();
        assert!(result["workspace_id"].is_string());
        let id_str = result["workspace_id"].as_str().unwrap();
        assert!(Uuid::parse_str(id_str).is_ok());
    }

    #[tokio::test]
    async fn current_returns_workspace_object() {
        let handler = make_handler();
        // The actor starts with a default workspace, so current() should succeed.
        let result = handler
            .handle("current", Value::Null, &make_ctx())
            .await
            .unwrap();
        assert!(result["id"].is_string());
        assert!(result["name"].is_string());
    }

    #[tokio::test]
    async fn select_by_invalid_uuid_returns_error() {
        let handler = make_handler();
        let params = serde_json::json!({ "workspace_id": "not-a-uuid" });
        let err = handler
            .handle("select", params, &make_ctx())
            .await
            .unwrap_err();
        assert_eq!(err.code, -32602);
    }

    #[tokio::test]
    async fn select_by_index_zero_returns_error() {
        let handler = make_handler();
        let params = serde_json::json!({ "index": 0u64 });
        let err = handler
            .handle("select", params, &make_ctx())
            .await
            .unwrap_err();
        assert_eq!(err.code, -32602);
    }

    #[tokio::test]
    async fn close_missing_workspace_id_returns_error() {
        let handler = make_handler();
        let err = handler
            .handle("close", Value::Null, &make_ctx())
            .await
            .unwrap_err();
        assert_eq!(err.code, -32602);
    }

    #[tokio::test]
    async fn rename_missing_params_returns_error() {
        let handler = make_handler();
        let params = serde_json::json!({ "workspace_id": Uuid::new_v4().to_string() });
        let err = handler
            .handle("rename", params, &make_ctx())
            .await
            .unwrap_err();
        assert_eq!(err.code, -32602);
    }

    #[tokio::test]
    async fn unknown_method_returns_method_not_found() {
        let handler = make_handler();
        let err = handler
            .handle("bogus", Value::Null, &make_ctx())
            .await
            .unwrap_err();
        assert_eq!(err.code, -32601);
        assert!(err.message.contains("workspace.bogus"));
    }
}
