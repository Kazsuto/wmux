use anyhow::{Context, Result};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::ClientOptions;
use tokio::time::{timeout, Duration};
use wmux_ipc::protocol::{RpcRequest, RpcResponse};

/// Timeout for pipe connection and response.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// One-shot Named Pipe IPC client.
///
/// Connects to the wmux IPC server, sends a single JSON-RPC request,
/// reads the response, and disconnects.
pub struct IpcClient {
    pipe_name: String,
}

impl IpcClient {
    /// Create a new client targeting the given pipe path.
    pub fn new(pipe_name: String) -> Self {
        Self { pipe_name }
    }

    /// Discover the pipe name from environment or use the default.
    ///
    /// Order:
    /// 1. `WMUX_SOCKET_PATH` environment variable (if set and valid)
    /// 2. `\\.\pipe\wmux-debug` (debug builds) or `\\.\pipe\wmux` (release builds)
    #[allow(dead_code)]
    pub fn discover() -> String {
        wmux_ipc::pipe_name()
    }

    /// Send a JSON-RPC request and return the response.
    ///
    /// Opens the pipe, writes the request as a newline-delimited JSON message,
    /// reads one newline-delimited JSON response, then closes the connection.
    pub async fn request(&self, method: &str, params: Option<Value>) -> Result<RpcResponse> {
        let request = RpcRequest {
            id: next_request_id(),
            method: method.to_owned(),
            params,
        };

        let mut json =
            serde_json::to_string(&request).context("failed to serialize JSON-RPC request")?;
        json.push('\n');

        timeout(REQUEST_TIMEOUT, self.send_and_receive(json))
            .await
            .context("request timed out after 30 seconds")?
    }

    async fn send_and_receive(&self, payload: String) -> Result<RpcResponse> {
        let pipe = ClientOptions::new()
            .open(&self.pipe_name)
            .with_context(|| {
                format!(
                    "failed to connect to wmux at {} — is wmux-app running?",
                    self.pipe_name
                )
            })?;

        let (read_half, mut write_half) = tokio::io::split(pipe);

        write_half
            .write_all(payload.as_bytes())
            .await
            .context("failed to send request to wmux")?;

        let mut reader = BufReader::new(read_half);
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .await
            .context("failed to read response from wmux")?;

        let response: RpcResponse = serde_json::from_str(line.trim_end())
            .context("failed to parse JSON-RPC response from wmux")?;

        Ok(response)
    }
}

/// Monotonically increasing request ID generator.
fn next_request_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    COUNTER.fetch_add(1, Ordering::Relaxed).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_ids_are_unique() {
        let id1 = next_request_id();
        let id2 = next_request_id();
        assert_ne!(id1, id2);
    }

    #[test]
    fn discover_returns_valid_pipe_path() {
        // Without WMUX_SOCKET_PATH set this should return the default debug path.
        let path = IpcClient::discover();
        assert!(path.starts_with(r"\\.\pipe\"));
    }
}
