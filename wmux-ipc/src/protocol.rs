use serde::{Deserialize, Serialize};

/// Maximum request size in bytes (1 MB).
pub const MAX_REQUEST_SIZE: usize = 1_048_576;

/// A JSON-RPC v2 request from a client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    /// Request identifier (echoed in response).
    pub id: String,
    /// Method name in `domain.action` format (e.g., `workspace.list`).
    pub method: String,
    /// Optional parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// A JSON-RPC v2 response to a client.
/// Uses cmux format with `ok` boolean field (NOT standard JSON-RPC result/error).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    /// Request identifier (echoed from request).
    pub id: String,
    /// Whether the request succeeded.
    pub ok: bool,
    /// Result payload (present when ok=true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error detail (present when ok=false).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcErrorDetail>,
}

/// Error detail in an RPC response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcErrorDetail {
    pub code: String,
    pub message: String,
}

/// Standard error codes for RPC responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpcErrorCode {
    ParseError,
    InvalidRequest,
    MethodNotFound,
    InvalidParams,
    InternalError,
    Unauthorized,
}

impl RpcErrorCode {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ParseError => "parse_error",
            Self::InvalidRequest => "invalid_request",
            Self::MethodNotFound => "method_not_found",
            Self::InvalidParams => "invalid_params",
            Self::InternalError => "internal_error",
            Self::Unauthorized => "unauthorized",
        }
    }
}

/// Parameters for the initial `auth.login` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthLoginRequest {
    /// HMAC-SHA256 response to a previously issued nonce.
    /// Absent on the first call; the server then issues a fresh nonce.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nonce_response: Option<String>,
}

/// Response payload for `auth.login`.
///
/// `Debug` is manually implemented to redact `session_token`.
#[derive(Clone, Serialize, Deserialize)]
pub struct AuthLoginResponse {
    /// Nonce to sign (present on the first call when no nonce_response was sent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
    /// Session token (present when authentication succeeded).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_token: Option<String>,
}

impl std::fmt::Debug for AuthLoginResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthLoginResponse")
            .field("nonce", &self.nonce)
            .field(
                "session_token",
                &self.session_token.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

impl RpcResponse {
    /// Create a success response with the given result payload.
    pub fn success(id: &str, result: serde_json::Value) -> Self {
        Self {
            id: id.to_owned(),
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response with the given error code and message.
    pub fn error(id: &str, code: RpcErrorCode, message: &str) -> Self {
        Self {
            id: id.to_owned(),
            ok: false,
            result: None,
            error: Some(RpcErrorDetail {
                code: code.code().to_owned(),
                message: message.to_owned(),
            }),
        }
    }

    /// Create a parse error response for malformed input.
    /// Uses empty id since the request could not be parsed.
    pub fn parse_error() -> Self {
        Self::error("", RpcErrorCode::ParseError, "failed to parse request")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rpc_request_serde_roundtrip() {
        let request = RpcRequest {
            id: "42".to_owned(),
            method: "workspace.list".to_owned(),
            params: Some(serde_json::json!({"filter": "active"})),
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: RpcRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, "42");
        assert_eq!(deserialized.method, "workspace.list");
        assert_eq!(
            deserialized.params,
            Some(serde_json::json!({"filter": "active"}))
        );
    }

    #[test]
    fn rpc_response_success_serialization() {
        let response = RpcResponse::success("1", serde_json::json!({"status": "ok"}));
        let json = serde_json::to_string(&response).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["id"], "1");
        assert_eq!(parsed["ok"], true);
        assert_eq!(parsed["result"]["status"], "ok");
        // error field should be omitted
        assert!(parsed.get("error").is_none());
    }

    #[test]
    fn rpc_response_error_serialization() {
        let response = RpcResponse::error("1", RpcErrorCode::MethodNotFound, "unknown method: foo");
        let json = serde_json::to_string(&response).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["id"], "1");
        assert_eq!(parsed["ok"], false);
        assert_eq!(parsed["error"]["code"], "method_not_found");
        assert_eq!(parsed["error"]["message"], "unknown method: foo");
        // result field should be omitted
        assert!(parsed.get("result").is_none());
    }

    #[test]
    fn rpc_response_parse_error_has_empty_id() {
        let response = RpcResponse::parse_error();

        assert_eq!(response.id, "");
        assert!(!response.ok);
        assert!(response.error.is_some());
        let error = response.error.unwrap();
        assert_eq!(error.code, "parse_error");
    }

    #[test]
    fn rpc_request_no_params_omits_field() {
        let request = RpcRequest {
            id: "5".to_owned(),
            method: "system.ping".to_owned(),
            params: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(parsed.get("params").is_none());
    }

    #[test]
    fn rpc_response_success_omits_error_field() {
        let response = RpcResponse::success("2", serde_json::json!({}));
        let json = serde_json::to_string(&response).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(parsed.get("error").is_none());
    }
}
