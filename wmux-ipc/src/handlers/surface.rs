use std::future::Future;
use std::pin::Pin;

use serde_json::Value;
use uuid::Uuid;
use wmux_core::{AppStateHandle, PaneId, PanelKind, SplitDirection, SurfaceId, WorkspaceId};

use crate::auth::ConnectionCtx;
use crate::handler::{Handler, RpcError};

/// Maximum size (bytes) for `send_text` payload to avoid overwhelming the PTY.
const MAX_SEND_TEXT_SIZE: usize = 64 * 1024;

/// Handles all `surface.*` JSON-RPC methods.
///
/// Includes `send_text`, `send_key`, and `read_text` — these were previously
/// under a separate `input.*` domain but belong to `surface.*` per the cmux
/// protocol specification.
pub struct SurfaceHandler {
    app_state: AppStateHandle,
}

impl SurfaceHandler {
    /// Create a new SurfaceHandler with a cloned app state handle.
    pub fn new(app_state: AppStateHandle) -> Self {
        Self { app_state }
    }
}

impl Handler for SurfaceHandler {
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
                "split" => {
                    let direction_str = params
                        .get("direction")
                        .and_then(|v| v.as_str())
                        .unwrap_or("right");
                    let direction = match direction_str {
                        "right" => SplitDirection::Horizontal,
                        "bottom" => SplitDirection::Vertical,
                        other => {
                            return Err(RpcError::invalid_params(format!(
                                "unknown direction '{other}': use 'right' or 'bottom'"
                            )));
                        }
                    };

                    let pane_id = self
                        .app_state
                        .get_focused_pane_id()
                        .await
                        .ok_or_else(|| RpcError::internal_error("no focused pane"))?;

                    match self.app_state.split_pane(pane_id, direction).await {
                        Ok(new_pane_id) => {
                            tracing::info!(
                                pane_id = %pane_id,
                                new_pane_id = %new_pane_id,
                                direction = direction_str,
                                "surface split via IPC"
                            );
                            Ok(serde_json::json!({ "surface_id": new_pane_id.to_string() }))
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "surface.split failed");
                            Err(RpcError::internal_error("failed to split surface"))
                        }
                    }
                }

                "list" => {
                    let workspace_id = params
                        .get("workspace_id")
                        .and_then(|v| v.as_str())
                        .map(|s| {
                            Uuid::parse_str(s).map(WorkspaceId::from_uuid).map_err(|_| {
                                RpcError::invalid_params(format!("invalid workspace_id: {s}"))
                            })
                        })
                        .transpose()?;

                    let surfaces = self.app_state.list_surfaces(workspace_id).await;
                    let items: Vec<Value> = surfaces
                        .into_iter()
                        .map(|s| {
                            let kind_str = match s.kind {
                                PanelKind::Terminal => "terminal",
                                PanelKind::Browser => "browser",
                            };
                            serde_json::json!({
                                "surface_id": s.surface_id.to_string(),
                                "pane_id": s.pane_id.to_string(),
                                "title": s.title,
                                "kind": kind_str,
                                "active": s.active,
                            })
                        })
                        .collect();
                    tracing::debug!(count = items.len(), "surface.list responded");
                    Ok(Value::Array(items))
                }

                "focus" => {
                    let surface_id_str = params
                        .get("surface_id")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| RpcError::invalid_params("missing 'surface_id'"))?;
                    let uuid = Uuid::parse_str(surface_id_str).map_err(|_| {
                        RpcError::invalid_params(format!("invalid surface_id: {surface_id_str}"))
                    })?;
                    let surface_id = SurfaceId::from_uuid(uuid);

                    let pane_id = self
                        .app_state
                        .find_pane_for_surface(surface_id)
                        .await
                        .ok_or_else(|| {
                            RpcError::invalid_params(format!("surface not found: {surface_id_str}"))
                        })?;

                    self.app_state.focus_pane(pane_id);
                    self.app_state.focus_surface(pane_id, surface_id);
                    tracing::info!(
                        surface_id = %surface_id,
                        pane_id = %pane_id,
                        "surface focused via IPC"
                    );
                    Ok(serde_json::json!({ "ok": true }))
                }

                "close" => {
                    let surface_id_str = params
                        .get("surface_id")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| RpcError::invalid_params("missing 'surface_id'"))?;
                    let uuid = Uuid::parse_str(surface_id_str).map_err(|_| {
                        RpcError::invalid_params(format!("invalid surface_id: {surface_id_str}"))
                    })?;
                    let surface_id = SurfaceId::from_uuid(uuid);

                    let pane_id = self
                        .app_state
                        .find_pane_for_surface(surface_id)
                        .await
                        .ok_or_else(|| {
                            RpcError::invalid_params(format!("surface not found: {surface_id_str}"))
                        })?;

                    self.app_state.close_surface(pane_id, surface_id);
                    tracing::info!(
                        surface_id = %surface_id,
                        pane_id = %pane_id,
                        "surface closed via IPC"
                    );
                    Ok(serde_json::json!({ "ok": true }))
                }

                // Input methods — cmux protocol puts these under surface.*
                "send_text" => handle_send_text(self.app_state.clone(), params).await,
                "send_key" => handle_send_key(self.app_state.clone(), params).await,
                "read_text" => handle_read_text(self.app_state.clone(), params).await,

                _ => Err(RpcError::method_not_found(&format!("surface.{method}"))),
            }
        })
    }
}

// ─── Pane Resolution ──────────────────────────────────────────────────────────

async fn resolve_pane(app_state: &AppStateHandle, params: &Value) -> Result<PaneId, RpcError> {
    if let Some(sid) = params.get("surface_id").and_then(|v| v.as_str()) {
        let uuid =
            Uuid::parse_str(sid).map_err(|_| RpcError::invalid_params("invalid surface_id"))?;
        let surface_id = SurfaceId::from_uuid(uuid);
        app_state
            .find_pane_for_surface(surface_id)
            .await
            .ok_or_else(|| RpcError::invalid_params("surface not found"))
    } else {
        app_state
            .get_focused_pane_id()
            .await
            .ok_or_else(|| RpcError::internal_error("no focused pane"))
    }
}

// ─── Input Method Handlers ──────────────────────────────────────────────────

async fn handle_send_text(app_state: AppStateHandle, params: Value) -> Result<Value, RpcError> {
    let text = params
        .get("text")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_params("missing required param: text"))?
        .to_owned();

    if text.len() > MAX_SEND_TEXT_SIZE {
        return Err(RpcError::invalid_params(format!(
            "text too large: {} bytes (max {})",
            text.len(),
            MAX_SEND_TEXT_SIZE
        )));
    }

    let pane_id = resolve_pane(&app_state, &params).await?;

    tracing::debug!(pane_id = %pane_id, len = text.len(), "send_text");
    app_state.send_input(pane_id, text.into_bytes());

    Ok(serde_json::json!({ "ok": true }))
}

async fn handle_send_key(app_state: AppStateHandle, params: Value) -> Result<Value, RpcError> {
    let key = params
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_params("missing required param: key"))?;

    let bytes = key_to_vt_bytes(key)?;

    let pane_id = resolve_pane(&app_state, &params).await?;

    tracing::debug!(pane_id = %pane_id, key = key, "send_key");
    app_state.send_input(pane_id, bytes);

    Ok(serde_json::json!({ "ok": true }))
}

async fn handle_read_text(app_state: AppStateHandle, params: Value) -> Result<Value, RpcError> {
    let pane_id = resolve_pane(&app_state, &params).await?;

    let start = params
        .get("start")
        .and_then(|v| v.as_i64())
        .map(|v| v as i32);
    let end = params.get("end").and_then(|v| v.as_i64()).map(|v| v as i32);

    tracing::debug!(pane_id = %pane_id, ?start, ?end, "read_text");

    let text = app_state
        .read_text(pane_id, start, end)
        .await
        .unwrap_or_default();

    Ok(serde_json::json!({ "text": text }))
}

// ─── Key Mapping ─────────────────────────────────────────────────────────────

fn key_to_vt_bytes(key: &str) -> Result<Vec<u8>, RpcError> {
    // Ctrl+<letter> shortcuts resolved first.
    if let Some(letter) = key.strip_prefix("Ctrl+") {
        return ctrl_byte(letter)
            .map(|b| vec![b])
            .ok_or_else(|| RpcError::invalid_params(format!("unknown key: {key}")));
    }

    let seq: &[u8] = match key {
        "Enter" => b"\r",
        "Tab" => b"\t",
        "Escape" => b"\x1b",
        "Backspace" => b"\x7f",
        "Delete" => b"\x1b[3~",
        "Up" => b"\x1b[A",
        "Down" => b"\x1b[B",
        "Left" => b"\x1b[D",
        "Right" => b"\x1b[C",
        "Home" => b"\x1b[H",
        "End" => b"\x1b[F",
        "PageUp" => b"\x1b[5~",
        "PageDown" => b"\x1b[6~",
        "Space" => b" ",
        "F1" => b"\x1bOP",
        "F2" => b"\x1bOQ",
        "F3" => b"\x1bOR",
        "F4" => b"\x1bOS",
        "F5" => b"\x1b[15~",
        "F6" => b"\x1b[17~",
        "F7" => b"\x1b[18~",
        "F8" => b"\x1b[19~",
        "F9" => b"\x1b[20~",
        "F10" => b"\x1b[21~",
        "F11" => b"\x1b[23~",
        "F12" => b"\x1b[24~",
        _ => return Err(RpcError::invalid_params(format!("unknown key: {key}"))),
    };

    Ok(seq.to_vec())
}

/// Map a single letter (A-Z, a-z) to its Ctrl byte (0x01-0x1A).
/// Special cases: C=0x03, D=0x04, L=0x0C, Z=0x1A (all fall out of the formula).
fn ctrl_byte(letter: &str) -> Option<u8> {
    let ch = letter.chars().next()?;
    let upper = ch.to_ascii_uppercase();
    if upper.is_ascii_alphabetic() {
        Some((upper as u8) - b'A' + 1)
    } else {
        None
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::SecurityMode;

    fn make_ctx() -> ConnectionCtx {
        let mut ctx = ConnectionCtx::new(SecurityMode::AllowAll);
        ctx.authenticate("test-session".to_owned());
        ctx
    }

    fn make_handler() -> SurfaceHandler {
        let (event_tx, _event_rx) = tokio::sync::mpsc::channel(16);
        let handle = wmux_core::AppStateHandle::spawn(event_tx);
        SurfaceHandler::new(handle)
    }

    // ── Surface methods ──────────────────────────────────────────────────

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
    async fn list_with_invalid_workspace_id_returns_error() {
        let handler = make_handler();
        let params = serde_json::json!({ "workspace_id": "bad-uuid" });
        let err = handler
            .handle("list", params, &make_ctx())
            .await
            .unwrap_err();
        assert_eq!(err.code, -32602);
    }

    #[tokio::test]
    async fn focus_missing_surface_id_returns_error() {
        let handler = make_handler();
        let err = handler
            .handle("focus", Value::Null, &make_ctx())
            .await
            .unwrap_err();
        assert_eq!(err.code, -32602);
    }

    #[tokio::test]
    async fn focus_invalid_uuid_returns_error() {
        let handler = make_handler();
        let params = serde_json::json!({ "surface_id": "not-a-uuid" });
        let err = handler
            .handle("focus", params, &make_ctx())
            .await
            .unwrap_err();
        assert_eq!(err.code, -32602);
    }

    #[tokio::test]
    async fn close_missing_surface_id_returns_error() {
        let handler = make_handler();
        let err = handler
            .handle("close", Value::Null, &make_ctx())
            .await
            .unwrap_err();
        assert_eq!(err.code, -32602);
    }

    #[tokio::test]
    async fn split_invalid_direction_returns_error() {
        let handler = make_handler();
        let params = serde_json::json!({ "direction": "left" });
        let err = handler
            .handle("split", params, &make_ctx())
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
        assert!(err.message.contains("surface.bogus"));
    }

    // ── Input methods (send_text, send_key, read_text) ───────────────────

    // key_to_vt_bytes

    #[test]
    fn enter_maps_to_cr() {
        assert_eq!(key_to_vt_bytes("Enter").unwrap(), b"\r");
    }

    #[test]
    fn escape_maps_to_esc_byte() {
        assert_eq!(key_to_vt_bytes("Escape").unwrap(), b"\x1b");
    }

    #[test]
    fn arrow_keys_map_to_csi_sequences() {
        assert_eq!(key_to_vt_bytes("Up").unwrap(), b"\x1b[A");
        assert_eq!(key_to_vt_bytes("Down").unwrap(), b"\x1b[B");
        assert_eq!(key_to_vt_bytes("Left").unwrap(), b"\x1b[D");
        assert_eq!(key_to_vt_bytes("Right").unwrap(), b"\x1b[C");
    }

    #[test]
    fn function_keys_map_correctly() {
        assert_eq!(key_to_vt_bytes("F1").unwrap(), b"\x1bOP");
        assert_eq!(key_to_vt_bytes("F5").unwrap(), b"\x1b[15~");
        assert_eq!(key_to_vt_bytes("F10").unwrap(), b"\x1b[21~");
        assert_eq!(key_to_vt_bytes("F12").unwrap(), b"\x1b[24~");
    }

    #[test]
    fn ctrl_c_maps_to_etx() {
        assert_eq!(key_to_vt_bytes("Ctrl+C").unwrap(), vec![0x03]);
    }

    #[test]
    fn ctrl_d_maps_to_eot() {
        assert_eq!(key_to_vt_bytes("Ctrl+D").unwrap(), vec![0x04]);
    }

    #[test]
    fn ctrl_l_maps_to_ff() {
        assert_eq!(key_to_vt_bytes("Ctrl+L").unwrap(), vec![0x0c]);
    }

    #[test]
    fn ctrl_z_maps_to_sub() {
        assert_eq!(key_to_vt_bytes("Ctrl+Z").unwrap(), vec![0x1a]);
    }

    #[test]
    fn ctrl_a_maps_to_soh() {
        assert_eq!(key_to_vt_bytes("Ctrl+A").unwrap(), vec![0x01]);
    }

    #[test]
    fn ctrl_lowercase_letter_works() {
        assert_eq!(key_to_vt_bytes("Ctrl+c").unwrap(), vec![0x03]);
    }

    #[test]
    fn unknown_key_returns_invalid_params() {
        let err = key_to_vt_bytes("FooBar").unwrap_err();
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("FooBar"));
    }

    #[test]
    fn ctrl_non_letter_returns_invalid_params() {
        let err = key_to_vt_bytes("Ctrl+1").unwrap_err();
        assert_eq!(err.code, -32602);
    }

    #[test]
    fn space_maps_to_space_byte() {
        assert_eq!(key_to_vt_bytes("Space").unwrap(), b" ");
    }

    #[test]
    fn page_up_down_map_correctly() {
        assert_eq!(key_to_vt_bytes("PageUp").unwrap(), b"\x1b[5~");
        assert_eq!(key_to_vt_bytes("PageDown").unwrap(), b"\x1b[6~");
    }

    #[test]
    fn home_end_map_correctly() {
        assert_eq!(key_to_vt_bytes("Home").unwrap(), b"\x1b[H");
        assert_eq!(key_to_vt_bytes("End").unwrap(), b"\x1b[F");
    }

    // Handler dispatch (send_text, send_key, read_text)

    #[tokio::test]
    async fn send_text_missing_text_param_returns_invalid_params() {
        let handler = make_handler();
        let err = handler
            .handle("send_text", serde_json::json!({}), &make_ctx())
            .await
            .unwrap_err();
        assert_eq!(err.code, -32602);
    }

    #[tokio::test]
    async fn send_key_missing_key_param_returns_invalid_params() {
        let handler = make_handler();
        let err = handler
            .handle("send_key", serde_json::json!({}), &make_ctx())
            .await
            .unwrap_err();
        assert_eq!(err.code, -32602);
    }

    #[tokio::test]
    async fn send_key_unknown_key_returns_invalid_params() {
        let handler = make_handler();
        let err = handler
            .handle(
                "send_key",
                serde_json::json!({ "key": "FooBar" }),
                &make_ctx(),
            )
            .await
            .unwrap_err();
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("FooBar"));
    }

    #[tokio::test]
    async fn send_text_too_large_returns_invalid_params() {
        let handler = make_handler();
        let big_text = "x".repeat(MAX_SEND_TEXT_SIZE + 1);
        let params = serde_json::json!({ "text": big_text });
        let err = handler
            .handle("send_text", params, &make_ctx())
            .await
            .unwrap_err();
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("too large"));
    }
}
