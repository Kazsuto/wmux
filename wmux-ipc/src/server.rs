use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::ServerOptions;
use tokio::sync::{mpsc, Semaphore};
use tokio::time::{timeout, Duration};

use crate::auth::{
    check_pid_ancestry, generate_nonce, generate_session_token, get_client_pid,
    is_unauthenticated_method, verify_hmac, ConnectionCtx, SecurityMode,
};
use crate::error::IpcError;
use crate::protocol::{
    AuthLoginRequest, AuthLoginResponse, RpcErrorCode, RpcRequest, RpcResponse, MAX_REQUEST_SIZE,
};
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

impl std::fmt::Debug for IpcServerHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IpcServerHandle")
            .field("pipe_name", &self.pipe_name)
            .finish_non_exhaustive()
    }
}

impl IpcServerHandle {
    /// Signal the server to shut down gracefully.
    pub async fn shutdown(&self) {
        let _ = self.shutdown_tx.send(()).await;
    }

    /// Returns the Named Pipe path the server is listening on.
    #[inline]
    pub fn pipe_name(&self) -> &str {
        &self.pipe_name
    }
}

/// Named Pipe IPC server for JSON-RPC v2 requests.
pub struct IpcServer {
    pipe_name: String,
    shutdown_rx: mpsc::Receiver<()>,
    security_mode: SecurityMode,
    /// Hex-encoded auth secret used in challenge-response mode. None for other modes.
    /// Wrapped in `Arc` to avoid cloning the secret string per connection.
    auth_secret: Option<Arc<String>>,
}

impl std::fmt::Debug for IpcServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IpcServer")
            .field("pipe_name", &self.pipe_name)
            .field("security_mode", &self.security_mode)
            .finish_non_exhaustive()
    }
}

impl IpcServer {
    /// Create a new IPC server and its control handle.
    ///
    /// `security_mode` controls connection authentication policy.
    /// For challenge-response mode, `auth_secret` must be `Some(secret_hex)`.
    pub fn new(
        pipe_name: String,
        security_mode: SecurityMode,
        auth_secret: Option<String>,
    ) -> (Self, IpcServerHandle) {
        let auth_secret = auth_secret.map(Arc::new);
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);
        let handle = IpcServerHandle {
            shutdown_tx,
            pipe_name: pipe_name.clone(),
        };
        let server = Self {
            pipe_name,
            shutdown_rx,
            security_mode,
            auth_secret,
        };
        (server, handle)
    }

    /// Run the IPC server, accepting connections until shutdown is signaled.
    ///
    /// Returns immediately with `Ok(())` if `security_mode` is `Off`.
    pub async fn run(mut self, router: Arc<Router>) -> Result<(), IpcError> {
        if self.security_mode == SecurityMode::Off {
            tracing::info!("IPC server disabled (security_mode = off)");
            return Ok(());
        }

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
        tracing::info!(pipe = %self.pipe_name, mode = ?self.security_mode, "IPC server listening");
        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_CONNECTIONS));

        loop {
            tokio::select! {
                result = server.connect() => {
                    match result {
                        Ok(()) => {
                            // Acquire semaphore permit BEFORE spawning to bound
                            // the number of in-flight tasks under connection flood.
                            let permit = match semaphore.clone().acquire_owned().await {
                                Ok(p) => p,
                                Err(_) => break, // semaphore closed — shutting down
                            };
                            let conn_router = Arc::clone(&router);
                            let mode = self.security_mode;
                            let secret = self.auth_secret.as_ref().map(Arc::clone);
                            tokio::spawn(async move {
                                let _permit = permit;
                                handle_connection(server, conn_router, mode, secret).await;
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

/// Handle a single client connection with security mode enforcement.
async fn handle_connection(
    pipe: tokio::net::windows::named_pipe::NamedPipeServer,
    router: Arc<Router>,
    mode: SecurityMode,
    auth_secret: Option<Arc<String>>,
) {
    use tracing::Instrument;
    handle_connection_inner(pipe, router, mode, auth_secret)
        .instrument(tracing::info_span!("ipc_connection"))
        .await;
}

async fn handle_connection_inner(
    pipe: tokio::net::windows::named_pipe::NamedPipeServer,
    router: Arc<Router>,
    mode: SecurityMode,
    auth_secret: Option<Arc<String>>,
) {
    let mut ctx = ConnectionCtx::new(mode);

    match mode {
        SecurityMode::Off => {
            // Should not reach here — server returns early for Off mode.
            tracing::warn!("handle_connection_inner called with SecurityMode::Off");
            return;
        }
        SecurityMode::AllowAll => {
            // Auto-authenticate all connections.
            let token = generate_session_token();
            ctx.authenticate(token);
            if let Ok(pid) = get_client_pid(&pipe) {
                ctx.client_pid = Some(pid);
                tracing::debug!(
                    client_pid = pid,
                    "connection auto-authenticated (allow_all)"
                );
            }
        }
        SecurityMode::WmuxOnly => {
            // Authenticate only if the client is a descendant of the wmux process.
            match get_client_pid(&pipe) {
                Ok(pid) => {
                    ctx.client_pid = Some(pid);
                    // check_pid_ancestry walks the process tree via
                    // CreateToolhelp32Snapshot — run off the async runtime.
                    let ancestry_result =
                        tokio::task::spawn_blocking(move || check_pid_ancestry(pid)).await;
                    match ancestry_result {
                        Ok(Ok(true)) => {
                            let token = generate_session_token();
                            ctx.authenticate(token);
                            tracing::debug!(
                                client_pid = pid,
                                "connection authenticated (wmux_only)"
                            );
                        }
                        Ok(Ok(false)) => {
                            tracing::warn!(
                                client_pid = pid,
                                "rejecting connection: not a wmux child process"
                            );
                            return;
                        }
                        Ok(Err(e)) => {
                            tracing::warn!(error = %e, "PID ancestry check failed, rejecting");
                            return;
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "PID ancestry task panicked, rejecting");
                            return;
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "failed to get client PID, rejecting connection");
                    return;
                }
            }
        }
        SecurityMode::Password => {
            // Auth happens via auth.login exchange in the request loop.
            if let Ok(pid) = get_client_pid(&pipe) {
                ctx.client_pid = Some(pid);
            }
        }
    }

    run_connection_loop(pipe, router, ctx, auth_secret).await;
}

/// Process the request/response loop on an established connection.
///
/// Supports multi-request sessions needed for the challenge-response auth
/// handshake. The loop terminates when the client disconnects or an
/// unrecoverable I/O error occurs.
async fn run_connection_loop(
    pipe: tokio::net::windows::named_pipe::NamedPipeServer,
    router: Arc<Router>,
    mut ctx: ConnectionCtx,
    auth_secret: Option<Arc<String>>,
) {
    let (reader, mut writer) = tokio::io::split(pipe);
    let mut buf_reader = BufReader::new(reader);
    // Nonce issued during challenge-response auth (single-use per connection).
    let mut pending_nonce: Option<[u8; 32]> = None;
    // Reuse the read buffer across iterations to avoid per-request allocations.
    let mut buf = String::new();

    loop {
        buf.clear();

        let read_result = timeout(
            Duration::from_secs(30),
            bounded_read_line(&mut buf_reader, &mut buf),
        )
        .await;

        match read_result {
            Err(_elapsed) => {
                tracing::warn!("connection timed out after 30s");
                return;
            }
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "failed to read from connection");
                // Size-limit violations arrive as InvalidData errors.
                let response = RpcResponse::parse_error();
                let _ = write_response(&mut writer, &response).await;
                return;
            }
            Ok(Ok(0)) => {
                // Client disconnected.
                return;
            }
            Ok(Ok(_)) => {}
        }

        // Belt-and-suspenders: reject if somehow the buffer exceeds the limit.
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

        // Parse the JSON-RPC request.
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

        tracing::debug!(method = %request.method, "dispatching request");

        // Auth gate: unauthenticated connections may only call allowed methods.
        if !ctx.authenticated && !is_unauthenticated_method(&request.method) {
            let response = RpcResponse::error(
                &request.id,
                RpcErrorCode::Unauthorized,
                "authentication required",
            );
            if let Err(e) = write_response(&mut writer, &response).await {
                tracing::warn!(error = %e, "failed to write unauthorized response");
            }
            // Keep connection open so client can authenticate.
            continue;
        }

        // Handle auth.login internally (not routed through handler registry).
        if request.method == "auth.login" {
            let response = handle_auth_login(
                &request,
                &mut ctx,
                &mut pending_nonce,
                auth_secret.as_ref().map(|s| s.as_str()),
            );
            if let Err(e) = write_response(&mut writer, &response).await {
                tracing::warn!(error = %e, "failed to write auth.login response");
            }
            continue;
        }

        // Dispatch authenticated request to the router.
        let response = router.dispatch(&request, &ctx).await;
        if let Err(e) = write_response(&mut writer, &response).await {
            tracing::warn!(error = %e, "failed to write response");
            return;
        }
    }
}

/// Handle an `auth.login` request using HMAC challenge-response.
///
/// Two-step flow:
/// 1. Client sends `auth.login` with no `nonce_response` → server replies with a nonce.
/// 2. Client sends `auth.login` with `nonce_response: "<hmac-hex>"` → server verifies and
///    replies with `session_token` on success, or an error on failure.
///
/// CRITICAL: auth_secret and nonce_response values are never logged.
fn handle_auth_login(
    request: &RpcRequest,
    ctx: &mut ConnectionCtx,
    pending_nonce: &mut Option<[u8; 32]>,
    auth_secret: Option<&str>,
) -> RpcResponse {
    // If already authenticated, return the current session token.
    if ctx.authenticated {
        if let Some(ref token) = ctx.session_token {
            let payload = AuthLoginResponse {
                nonce: None,
                session_token: Some(token.clone()),
            };
            return RpcResponse::success(
                &request.id,
                serde_json::to_value(payload).expect("AuthLoginResponse serializes to valid JSON"),
            );
        }
    }

    let params: AuthLoginRequest = match &request.params {
        Some(v) => serde_json::from_value(v.clone()).unwrap_or(AuthLoginRequest {
            nonce_response: None,
        }),
        None => AuthLoginRequest {
            nonce_response: None,
        },
    };

    match params.nonce_response {
        None => {
            // Step 1: issue a fresh nonce.
            let nonce = generate_nonce();
            let nonce_hex = hex::encode(nonce);
            *pending_nonce = Some(nonce);
            let payload = AuthLoginResponse {
                nonce: Some(nonce_hex),
                session_token: None,
            };
            RpcResponse::success(
                &request.id,
                serde_json::to_value(payload).expect("AuthLoginResponse serializes to valid JSON"),
            )
        }
        Some(response_hex) => {
            // Step 2: verify HMAC response.
            let Some(nonce) = pending_nonce.take() else {
                return RpcResponse::error(
                    &request.id,
                    RpcErrorCode::InvalidRequest,
                    "no pending nonce — call auth.login without nonce_response first",
                );
            };

            let Some(secret) = auth_secret else {
                tracing::error!(
                    "auth.login invoked in challenge-response mode but no secret configured"
                );
                return RpcResponse::error(
                    &request.id,
                    RpcErrorCode::InternalError,
                    "server authentication not configured",
                );
            };

            // CRITICAL: response_hex and secret are intentionally not logged.
            if verify_hmac(secret, &nonce, &response_hex) {
                let token = generate_session_token();
                ctx.authenticate(token.clone());
                tracing::info!("client authenticated successfully");
                let payload = AuthLoginResponse {
                    nonce: None,
                    session_token: Some(token),
                };
                RpcResponse::success(
                    &request.id,
                    serde_json::to_value(payload)
                        .expect("AuthLoginResponse serializes to valid JSON"),
                )
            } else {
                tracing::warn!("auth.login: HMAC verification failed");
                RpcResponse::error(
                    &request.id,
                    RpcErrorCode::Unauthorized,
                    "authentication failed: invalid HMAC response",
                )
            }
        }
    }
}

/// Read a newline-delimited line into `buf`, aborting with an error if the
/// line exceeds `MAX_REQUEST_SIZE` bytes.  This prevents a malicious client
/// from exhausting server memory with a single gigantic line.
async fn bounded_read_line<R: tokio::io::AsyncBufRead + Unpin>(
    reader: &mut R,
    buf: &mut String,
) -> std::io::Result<usize> {
    let max = MAX_REQUEST_SIZE;
    let mut total = 0usize;
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            return Ok(total); // EOF
        }

        // Scan for a newline in the buffered data.
        if let Some(pos) = available.iter().position(|&b| b == b'\n') {
            let line_end = pos + 1;
            if total + line_end > max {
                reader.consume(line_end);
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "request exceeds maximum size",
                ));
            }
            // Reject invalid UTF-8 rather than silently transforming it
            // (matches std::io::BufRead::read_line behaviour).
            let chunk = std::str::from_utf8(&available[..line_end]).map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid UTF-8 in request")
            })?;
            buf.push_str(chunk);
            reader.consume(line_end);
            total += line_end;
            return Ok(total);
        }

        // No newline yet — append and check size.
        let len = available.len();
        if total + len > max {
            reader.consume(len);
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "request exceeds maximum size",
            ));
        }
        let chunk = std::str::from_utf8(available).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid UTF-8 in request")
        })?;
        buf.push_str(chunk);
        reader.consume(len);
        total += len;
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

    #[test]
    fn auth_login_challenge_response_flow() {
        use crate::auth::SecurityMode;
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        type HmacSha256 = Hmac<Sha256>;

        let secret_bytes = [0xCCu8; 32];
        let secret_hex = hex::encode(secret_bytes);
        let mut ctx = ConnectionCtx::new(SecurityMode::Password);
        let mut pending_nonce: Option<[u8; 32]> = None;
        let auth_secret = Some(secret_hex.clone());

        // Step 1: request a nonce.
        let req1 = RpcRequest {
            id: "1".to_owned(),
            method: "auth.login".to_owned(),
            params: None,
        };
        let resp1 = handle_auth_login(&req1, &mut ctx, &mut pending_nonce, auth_secret.as_deref());
        assert!(resp1.ok, "nonce request should succeed");
        let nonce_hex = resp1.result.as_ref().unwrap()["nonce"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(!nonce_hex.is_empty());

        // Step 2: compute a valid HMAC and send it.
        let nonce_bytes = hex::decode(&nonce_hex).unwrap();
        let mut mac = HmacSha256::new_from_slice(&secret_bytes).unwrap();
        mac.update(&nonce_bytes);
        let hmac_hex = hex::encode(mac.finalize().into_bytes());

        let req2 = RpcRequest {
            id: "2".to_owned(),
            method: "auth.login".to_owned(),
            params: Some(serde_json::json!({ "nonce_response": hmac_hex })),
        };
        let resp2 = handle_auth_login(&req2, &mut ctx, &mut pending_nonce, auth_secret.as_deref());
        assert!(resp2.ok, "valid HMAC should authenticate");
        assert!(ctx.authenticated);
        let token = resp2.result.as_ref().unwrap()["session_token"]
            .as_str()
            .unwrap();
        assert!(!token.is_empty());
    }

    #[test]
    fn auth_login_invalid_hmac_rejected() {
        use crate::auth::SecurityMode;

        let secret_hex = hex::encode([0xDDu8; 32]);
        let mut ctx = ConnectionCtx::new(SecurityMode::Password);
        let mut pending_nonce: Option<[u8; 32]> = None;
        let auth_secret = Some(secret_hex);

        // Get a nonce first.
        let req1 = RpcRequest {
            id: "1".to_owned(),
            method: "auth.login".to_owned(),
            params: None,
        };
        handle_auth_login(&req1, &mut ctx, &mut pending_nonce, auth_secret.as_deref());

        // Send a wrong HMAC.
        let req2 = RpcRequest {
            id: "2".to_owned(),
            method: "auth.login".to_owned(),
            params: Some(serde_json::json!({ "nonce_response": "deadbeef" })),
        };
        let resp2 = handle_auth_login(&req2, &mut ctx, &mut pending_nonce, auth_secret.as_deref());
        assert!(!resp2.ok);
        assert!(!ctx.authenticated);
        assert_eq!(resp2.error.unwrap().code, "unauthorized");
    }

    #[test]
    fn auth_login_response_without_prior_nonce_rejected() {
        use crate::auth::SecurityMode;

        let mut ctx = ConnectionCtx::new(SecurityMode::Password);
        let mut pending_nonce: Option<[u8; 32]> = None;
        let auth_secret = Some("aabbcc".to_owned());

        let req = RpcRequest {
            id: "1".to_owned(),
            method: "auth.login".to_owned(),
            params: Some(serde_json::json!({ "nonce_response": "deadbeef" })),
        };
        let resp = handle_auth_login(&req, &mut ctx, &mut pending_nonce, auth_secret.as_deref());
        assert!(!resp.ok);
        assert_eq!(resp.error.unwrap().code, "invalid_request");
    }
}
