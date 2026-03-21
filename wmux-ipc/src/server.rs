use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::ServerOptions;
use tokio::sync::{mpsc, Semaphore};
use tokio::time::{timeout, Duration};

use crate::error::IpcError;
use crate::protocol::{RpcErrorCode, RpcRequest, RpcResponse, MAX_REQUEST_SIZE};
use crate::router::Router;

/// Maximum number of simultaneous IPC connections.
const MAX_CONCURRENT_CONNECTIONS: usize = 64;

/// Returns the Named Pipe path for the IPC server.
///
/// Checks `WMUX_SOCKET_PATH` env var first, then falls back to
/// `\\.\pipe\wmux-debug` (debug builds) or `\\.\pipe\wmux` (release builds).
pub fn pipe_name() -> String {
    if let Ok(path) = std::env::var("WMUX_SOCKET_PATH") {
        if !path.is_empty() {
            // Validate that the path is in the Named Pipe namespace.
            if path.starts_with(r"\\.\pipe\") {
                return path;
            }
            tracing::warn!(
                path = %path,
                "WMUX_SOCKET_PATH is not a valid pipe path, ignoring"
            );
        }
    }

    if cfg!(debug_assertions) {
        r"\\.\pipe\wmux-debug".to_owned()
    } else {
        r"\\.\pipe\wmux".to_owned()
    }
}

/// Handle returned by [`IpcServer::new`] to control the running server.
pub struct IpcServerHandle {
    shutdown_tx: mpsc::Sender<()>,
    pipe_name: String,
}

impl IpcServerHandle {
    /// Signal the server to shut down gracefully.
    pub async fn shutdown(&self) {
        let _ = self.shutdown_tx.send(()).await;
    }

    /// Returns the Named Pipe path the server is listening on.
    pub fn pipe_name(&self) -> &str {
        &self.pipe_name
    }
}

/// Named Pipe IPC server for JSON-RPC v2 requests.
pub struct IpcServer {
    pipe_name: String,
    shutdown_rx: mpsc::Receiver<()>,
}

impl IpcServer {
    /// Create a new IPC server and its control handle.
    pub fn new(pipe_name: String) -> (Self, IpcServerHandle) {
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);
        let handle = IpcServerHandle {
            shutdown_tx,
            pipe_name: pipe_name.clone(),
        };
        let server = Self {
            pipe_name,
            shutdown_rx,
        };
        (server, handle)
    }

    /// Run the IPC server, accepting connections until shutdown is signaled.
    pub async fn run(mut self, router: Arc<Router>) -> Result<(), IpcError> {
        // Try to create the first pipe instance.
        let active_pipe_name = match ServerOptions::new()
            .first_pipe_instance(true)
            .create(&self.pipe_name)
        {
            Ok(first) => {
                // Successfully created — start the accept loop with this pipe.
                self.accept_loop(first, router).await?;
                return Ok(());
            }
            Err(e) if e.raw_os_error() == Some(231) => {
                // ERROR_PIPE_BUSY (231) — fall back to PID-suffixed name.
                let fallback = format!(r"\\.\pipe\wmux-{}", std::process::id());
                tracing::warn!(
                    original = %self.pipe_name,
                    fallback = %fallback,
                    "pipe busy, using fallback name"
                );
                fallback
            }
            Err(e) => return Err(IpcError::Io(e)),
        };

        // Create with fallback name.
        self.pipe_name = active_pipe_name;
        let first = ServerOptions::new()
            .first_pipe_instance(true)
            .create(&self.pipe_name)?;
        self.accept_loop(first, router).await
    }

    async fn accept_loop(
        &mut self,
        mut server: tokio::net::windows::named_pipe::NamedPipeServer,
        router: Arc<Router>,
    ) -> Result<(), IpcError> {
        tracing::info!(pipe = %self.pipe_name, "IPC server listening");
        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_CONNECTIONS));

        loop {
            tokio::select! {
                result = server.connect() => {
                    match result {
                        Ok(()) => {
                            let conn_router = Arc::clone(&router);
                            let permit = Arc::clone(&semaphore);
                            tokio::spawn(async move {
                                // Acquire semaphore permit to limit concurrency.
                                let _permit = match permit.acquire().await {
                                    Ok(p) => p,
                                    Err(_) => return, // semaphore closed
                                };
                                handle_connection(server, conn_router).await;
                            });
                            // Create the next pipe instance for the next connection.
                            server = ServerOptions::new().create(&self.pipe_name)?;
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "failed to accept connection");
                        }
                    }
                }
                _ = self.shutdown_rx.recv() => {
                    tracing::info!("IPC server shutting down");
                    break;
                }
            }
        }

        Ok(())
    }
}

/// Handle a single client connection: read one request, dispatch, write one response.
async fn handle_connection(
    pipe: tokio::net::windows::named_pipe::NamedPipeServer,
    router: Arc<Router>,
) {
    use tracing::Instrument;
    handle_connection_inner(pipe, router)
        .instrument(tracing::info_span!("ipc_connection"))
        .await;
}

async fn handle_connection_inner(
    pipe: tokio::net::windows::named_pipe::NamedPipeServer,
    router: Arc<Router>,
) {
    let (reader, mut writer) = tokio::io::split(pipe);
    // Limit reads to MAX_REQUEST_SIZE + 1 to detect oversized requests
    // BEFORE they consume unbounded memory (defense against DoS).
    let limited_reader = reader.take(MAX_REQUEST_SIZE as u64 + 1);
    let mut buf_reader = BufReader::new(limited_reader);
    let mut buf = String::new();

    // Read one line with 30s timeout. The Take adapter ensures we never
    // read more than MAX_REQUEST_SIZE + 1 bytes into memory.
    let read_result = timeout(Duration::from_secs(30), buf_reader.read_line(&mut buf)).await;

    match read_result {
        Err(_elapsed) => {
            tracing::warn!("connection timed out after 30s");
            return;
        }
        Ok(Err(e)) => {
            tracing::warn!(error = %e, "failed to read from connection");
            return;
        }
        Ok(Ok(0)) => {
            // Client disconnected without sending data.
            return;
        }
        Ok(Ok(_)) => {}
    }

    // Check size limit (the Take adapter bounds memory, but we still reject oversized).
    if buf.len() > MAX_REQUEST_SIZE {
        tracing::warn!(
            size = buf.len(),
            max = MAX_REQUEST_SIZE,
            "request too large"
        );
        let response = RpcResponse::parse_error();
        if let Err(e) = write_response(&mut writer, &response).await {
            tracing::warn!(error = %e, "failed to write error response");
        }
        return;
    }

    // Parse the request.
    let request: RpcRequest = match serde_json::from_str(buf.trim()) {
        Ok(req) => req,
        Err(e) => {
            tracing::warn!(error = %e, "failed to parse JSON-RPC request");
            let response = RpcResponse::parse_error();
            if let Err(e) = write_response(&mut writer, &response).await {
                tracing::warn!(error = %e, "failed to write parse error response");
            }
            return;
        }
    };

    // Validate the request.
    if request.method.is_empty() {
        let response = RpcResponse::error(
            &request.id,
            RpcErrorCode::InvalidRequest,
            "method must not be empty",
        );
        if let Err(e) = write_response(&mut writer, &response).await {
            tracing::warn!(error = %e, "failed to write invalid request response");
        }
        return;
    }

    // Dispatch to router.
    let response = router.dispatch(&request).await;

    if let Err(e) = write_response(&mut writer, &response).await {
        tracing::warn!(error = %e, "failed to write response");
    }
}

/// Serialize and write a response followed by a newline.
async fn write_response<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    response: &RpcResponse,
) -> Result<(), std::io::Error> {
    let mut json = serde_json::to_string(response)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    json.push('\n');
    writer.write_all(json.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // All pipe_name tests are combined into one to avoid env-var race
    // conditions between parallel Rust tests (env vars are process-global).
    #[test]
    fn pipe_name_env_behavior() {
        // 1. Custom valid pipe path is respected.
        let custom = r"\\.\pipe\wmux-test-custom";
        // SAFETY: This test combines all env-var scenarios in a single #[test]
        // to avoid races. No other thread reads WMUX_SOCKET_PATH concurrently.
        unsafe { std::env::set_var("WMUX_SOCKET_PATH", custom) };
        assert_eq!(pipe_name(), custom);

        // 2. Invalid path (not in pipe namespace) is rejected — falls back to default.
        // SAFETY: Same single-test isolation guarantee as above.
        unsafe { std::env::set_var("WMUX_SOCKET_PATH", r"C:\bad\path") };
        let name = pipe_name();
        assert!(
            name.starts_with(r"\\.\pipe\wmux"),
            "should reject non-pipe path, got: {name}"
        );

        // 3. Empty string is ignored — falls back to default.
        // SAFETY: Same single-test isolation guarantee as above.
        unsafe { std::env::set_var("WMUX_SOCKET_PATH", "") };
        let name = pipe_name();
        assert!(
            name.starts_with(r"\\.\pipe\wmux"),
            "should ignore empty env var, got: {name}"
        );

        // 4. Absent env var — falls back to default.
        // SAFETY: Same single-test isolation guarantee as above.
        unsafe { std::env::remove_var("WMUX_SOCKET_PATH") };
        let name = pipe_name();
        assert!(
            name.starts_with(r"\\.\pipe\wmux"),
            "expected default pipe name, got: {name}"
        );
    }
}
