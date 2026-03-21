use wmux_ipc::protocol::RpcResponse;

/// Format an RPC response for display.
///
/// In JSON mode, returns the raw pretty-printed JSON.
/// In human mode, formats the result or error in a readable form.
pub fn format_response(response: &RpcResponse, json_mode: bool) -> String {
    if json_mode {
        serde_json::to_string_pretty(response)
            .unwrap_or_else(|e| format!("error serializing response: {e}"))
    } else {
        format_human(response)
    }
}

fn format_human(response: &RpcResponse) -> String {
    if response.ok {
        match &response.result {
            None => String::from("ok"),
            Some(value) => format_value(value),
        }
    } else {
        match &response.error {
            Some(err) => format!("error [{}]: {}", err.code, err.message),
            None => String::from("error: unknown error"),
        }
    }
}

fn format_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Null => String::from("null"),
        serde_json::Value::Array(arr) => {
            arr.iter().map(format_value).collect::<Vec<_>>().join("\n")
        }
        serde_json::Value::Object(_) => {
            serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wmux_ipc::protocol::{RpcErrorCode, RpcResponse};

    #[test]
    fn json_mode_outputs_pretty_json() {
        let response = RpcResponse::success("1", serde_json::json!({"value": "pong"}));
        let output = format_response(&response, true);
        assert!(output.contains("pong"));
        assert!(output.contains('\n'));
    }

    #[test]
    fn human_mode_string_result() {
        let response = RpcResponse::success("1", serde_json::json!("pong"));
        let output = format_response(&response, false);
        assert_eq!(output, "pong");
    }

    #[test]
    fn human_mode_error_result() {
        let response = RpcResponse::error("1", RpcErrorCode::MethodNotFound, "unknown method: foo");
        let output = format_response(&response, false);
        assert!(output.starts_with("error [method_not_found]:"));
        assert!(output.contains("unknown method: foo"));
    }

    #[test]
    fn human_mode_ok_no_result() {
        let response = RpcResponse {
            id: "1".to_owned(),
            ok: true,
            result: None,
            error: None,
        };
        let output = format_response(&response, false);
        assert_eq!(output, "ok");
    }
}
