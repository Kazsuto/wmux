use hmac::{Hmac, Mac};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::error::IpcError;

type HmacSha256 = Hmac<Sha256>;

/// Security mode for the IPC server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SecurityMode {
    /// No pipe created — IPC server disabled entirely.
    Off,
    /// Only wmux child processes are allowed (default).
    /// Authentication is automatic via PID ancestry check.
    #[default]
    WmuxOnly,
    /// Accept all local connections without authentication.
    AllowAll,
    /// HMAC-SHA256 challenge-response authentication required.
    Password,
}

/// Per-connection authentication context.
#[derive(Clone)]
pub struct ConnectionCtx {
    /// Whether the connection has been authenticated.
    pub authenticated: bool,
    /// The security mode active for this connection.
    pub mode: SecurityMode,
    /// Session token issued after successful authentication.
    pub session_token: Option<String>,
    /// PID of the connecting client process.
    pub client_pid: Option<u32>,
}

impl std::fmt::Debug for ConnectionCtx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionCtx")
            .field("authenticated", &self.authenticated)
            .field("mode", &self.mode)
            .field(
                "session_token",
                &self.session_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field("client_pid", &self.client_pid)
            .finish()
    }
}

impl ConnectionCtx {
    /// Create a new unauthenticated connection context.
    pub fn new(mode: SecurityMode) -> Self {
        Self {
            authenticated: false,
            mode,
            session_token: None,
            client_pid: None,
        }
    }

    /// Mark the connection as authenticated with a session token.
    pub fn authenticate(&mut self, token: String) {
        self.authenticated = true;
        self.session_token = Some(token);
    }
}

/// Path to the auth secret file: `%APPDATA%\wmux\auth_secret`.
fn auth_secret_path() -> Result<std::path::PathBuf, IpcError> {
    let app_data = dirs::data_dir()
        .ok_or_else(|| IpcError::General("could not determine %APPDATA% directory".to_owned()))?;
    Ok(app_data.join("wmux").join("auth_secret"))
}

/// Generate a 256-bit random secret, hex-encode it, and write it to
/// `%APPDATA%\wmux\auth_secret` with owner-only ACL.
///
/// If the file already exists, it will NOT be overwritten — the existing
/// secret is preserved. Returns the secret.
pub async fn generate_auth_secret() -> Result<String, IpcError> {
    let path = auth_secret_path()?;

    // Create the parent directory if needed.
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| IpcError::General(format!("failed to create auth dir: {e}")))?;
    }

    // Generate 256-bit random secret.
    let mut secret_bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut secret_bytes);
    let secret_hex = hex::encode(secret_bytes);

    // Write with create_new to avoid TOCTOU races — if file exists, bail out
    // and read the existing one instead.
    // NOTE: tokio::fs::OpenOptions doesn't support create_new, so we use
    // spawn_blocking for the atomic create-or-fail operation.
    let hex_clone = secret_hex.clone();
    let path_clone = path.clone();
    let write_result = tokio::task::spawn_blocking(move || {
        use std::io::Write;
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path_clone)
        {
            Ok(mut f) => {
                f.write_all(hex_clone.as_bytes())?;
                Ok(true) // created
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
            Err(e) => Err(e),
        }
    })
    .await
    .map_err(|e| IpcError::General(format!("spawn_blocking join error: {e}")))?
    .map_err(|e| IpcError::General(format!("failed to create auth secret file: {e}")))?;

    if write_result {
        // Apply owner-only ACL on Windows.
        restrict_file_permissions(&path)?;
        // NOTE: secret_hex intentionally not logged.
        tracing::info!("auth secret file created");
        Ok(secret_hex)
    } else {
        // File already exists — read the existing secret.
        load_auth_secret().await
    }
}

/// Read the auth secret from `%APPDATA%\wmux\auth_secret`.
pub async fn load_auth_secret() -> Result<String, IpcError> {
    let path = auth_secret_path()?;
    let contents = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| IpcError::General(format!("failed to read auth secret: {e}")))?;
    let secret = contents.trim().to_owned();
    if secret.is_empty() {
        return Err(IpcError::General("auth secret file is empty".to_owned()));
    }
    // NOTE: secret intentionally not logged.
    Ok(secret)
}

/// Set owner-only read/write permissions on the auth secret file via Windows ACL.
///
/// On failure, logs a warning and continues — the file is still usable,
/// just potentially readable by other local users.
fn restrict_file_permissions(path: &std::path::Path) -> Result<(), IpcError> {
    // Encode the path as a wide string for Win32.
    let wide_path: Vec<u16> = path
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0u16))
        .collect();

    // SAFETY: wide_path is a valid null-terminated UTF-16 string; its lifetime
    // exceeds the call to set_owner_only_dacl since it is defined in this scope.
    let restricted = unsafe { set_owner_only_dacl(windows::core::PCWSTR(wide_path.as_ptr())) };

    match restricted {
        Ok(()) => {
            tracing::debug!("auth secret file permissions restricted to owner");
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed to restrict auth secret file permissions");
        }
    }

    // Return Ok even on ACL failure — the secret was written and a warning
    // is sufficient so startup continues.
    Ok(())
}

/// Apply a DACL on `path` that grants full control only to the current user.
///
/// # Safety
/// Caller must ensure `path_wide` is a valid null-terminated UTF-16 path string
/// whose backing buffer outlives this function call.
unsafe fn set_owner_only_dacl(path_wide: windows::core::PCWSTR) -> Result<(), String> {
    use windows::Win32::Foundation::{CloseHandle, HANDLE, HLOCAL};
    use windows::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        SetNamedSecurityInfoW, SDDL_REVISION_1, SE_FILE_OBJECT,
    };
    use windows::Win32::Security::{
        GetSecurityDescriptorDacl, GetTokenInformation, TokenUser, ACL, DACL_SECURITY_INFORMATION,
        OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, TOKEN_QUERY, TOKEN_USER,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    // Open the current process token to obtain the user SID.
    let mut token: HANDLE = HANDLE::default();
    // SAFETY: GetCurrentProcess returns a valid pseudo-handle; OpenProcessToken
    // duplicates a handle into `token` which we must close.
    OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
        .map_err(|e| format!("OpenProcessToken: {e}"))?;

    struct TokenGuard(HANDLE);
    impl Drop for TokenGuard {
        fn drop(&mut self) {
            // SAFETY: We own this token handle and close it exactly once.
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
    let _token_guard = TokenGuard(token);

    // Retrieve the token user info to obtain the user SID.
    let mut needed: u32 = 0;
    // First call: obtain required buffer size (will fail with ERROR_INSUFFICIENT_BUFFER).
    let _ = GetTokenInformation(token, TokenUser, None, 0, &mut needed);

    let mut buf = vec![0u8; needed as usize];
    GetTokenInformation(
        token,
        TokenUser,
        Some(buf.as_mut_ptr().cast()),
        needed,
        &mut needed,
    )
    .map_err(|e| format!("GetTokenInformation: {e}"))?;

    // SAFETY: buf is correctly sized and aligned for TOKEN_USER per the Win32 API contract.
    let token_user = &*(buf.as_ptr() as *const TOKEN_USER);
    let user_sid = token_user.User.Sid;

    // Convert the user SID to a string for use in an SDDL expression.
    let mut sid_str_ptr: windows::core::PWSTR = windows::core::PWSTR::null();
    ConvertSidToStringSidW(user_sid, &mut sid_str_ptr)
        .map_err(|e| format!("ConvertSidToStringSidW: {e}"))?;

    struct SidStrGuard(windows::core::PWSTR);
    impl Drop for SidStrGuard {
        fn drop(&mut self) {
            // SAFETY: sid_str_ptr was heap-allocated by ConvertSidToStringSidW
            // and must be freed with LocalFree.
            unsafe {
                let _ = windows::Win32::Foundation::LocalFree(Some(HLOCAL(self.0.as_ptr().cast())));
            }
        }
    }
    let _sid_guard = SidStrGuard(sid_str_ptr);

    // SAFETY: ConvertSidToStringSidW succeeded; the pointer is a valid wide string.
    let sid_str = sid_str_ptr
        .to_string()
        .map_err(|e| format!("SID to string: {e}"))?;

    // Build SDDL: protected DACL granting full access only to this user.
    // "D:P(A;;FA;;;SID)" — D=DACL, P=protected, A=allow, FA=full access.
    let sddl = format!("D:P(A;;FA;;;{sid_str})");
    let sddl_wide: Vec<u16> = sddl.encode_utf16().chain(std::iter::once(0u16)).collect();

    let mut sd: PSECURITY_DESCRIPTOR = PSECURITY_DESCRIPTOR::default();
    let mut sd_size: u32 = 0;
    ConvertStringSecurityDescriptorToSecurityDescriptorW(
        windows::core::PCWSTR(sddl_wide.as_ptr()),
        SDDL_REVISION_1,
        &mut sd,
        Some(&mut sd_size),
    )
    .map_err(|e| format!("ConvertStringSecurityDescriptorToSecurityDescriptorW: {e}"))?;

    struct SdGuard(PSECURITY_DESCRIPTOR);
    impl Drop for SdGuard {
        fn drop(&mut self) {
            // SAFETY: sd was heap-allocated by
            // ConvertStringSecurityDescriptorToSecurityDescriptorW and must be
            // freed with LocalFree.
            unsafe {
                let _ = windows::Win32::Foundation::LocalFree(Some(HLOCAL(self.0 .0)));
            }
        }
    }
    let _sd_guard = SdGuard(sd);

    // Extract the DACL pointer from the security descriptor.
    let mut dacl_present = windows::core::BOOL::default();
    let mut dacl: *mut ACL = std::ptr::null_mut();
    let mut dacl_defaulted = windows::core::BOOL::default();

    // SAFETY: sd was successfully allocated above; all pointer params are valid.
    GetSecurityDescriptorDacl(sd, &mut dacl_present, &mut dacl, &mut dacl_defaulted)
        .map_err(|e| format!("GetSecurityDescriptorDacl: {e}"))?;

    // Apply the restricted DACL and owner SID to the file.
    SetNamedSecurityInfoW(
        path_wide,
        SE_FILE_OBJECT,
        DACL_SECURITY_INFORMATION | OWNER_SECURITY_INFORMATION,
        Some(user_sid),
        None,
        Some(dacl),
        None,
    )
    .ok()
    .map_err(|e| format!("SetNamedSecurityInfoW: {e}"))?;

    Ok(())
}

/// Generate a fresh 32-byte cryptographic nonce.
pub fn generate_nonce() -> [u8; 32] {
    let mut nonce = [0u8; 32];
    rand::rng().fill_bytes(&mut nonce);
    nonce
}

/// Verify an HMAC-SHA256 response.
///
/// `secret` is the hex-encoded auth secret.
/// `nonce` is the raw nonce bytes sent to the client.
/// `response` is the hex-encoded HMAC computed by the client.
///
/// CRITICAL: Never log `secret` or `response`.
pub fn verify_hmac(secret: &str, nonce: &[u8], response: &str) -> bool {
    let key = match hex::decode(secret) {
        Ok(k) => k,
        Err(_) => return false,
    };
    let expected = match hex::decode(response) {
        Ok(b) => b,
        Err(_) => return false,
    };

    let mut mac = match HmacSha256::new_from_slice(&key) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(nonce);

    // Use constant-time comparison via the `verify_slice` method.
    mac.verify_slice(&expected).is_ok()
}

/// Generate a session token: 128-bit random, hex-encoded.
///
/// CRITICAL: Never log the returned value.
pub fn generate_session_token() -> String {
    let mut bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Check whether a client PID is a descendant of the current wmux process.
///
/// Walks the process tree upward using `CreateToolhelp32Snapshot` to find
/// all processes and their parent PIDs, then checks if any ancestor of
/// `client_pid` is the current wmux process.
pub fn check_pid_ancestry(client_pid: u32) -> Result<bool, IpcError> {
    let wmux_pid = std::process::id();
    is_ancestor_of(wmux_pid, client_pid)
}

/// Returns true if `ancestor_pid` is an ancestor (or equal) of `target_pid`.
fn is_ancestor_of(ancestor_pid: u32, target_pid: u32) -> Result<bool, IpcError> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32First, Process32Next, PROCESSENTRY32, TH32CS_SNAPPROCESS,
    };

    // SAFETY: CreateToolhelp32Snapshot is safe to call with TH32CS_SNAPPROCESS and
    // process_id=0 to snapshot all processes.
    let snapshot = unsafe {
        CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
            .map_err(|e| IpcError::General(format!("CreateToolhelp32Snapshot: {e}")))?
    };

    struct SnapshotGuard(windows::Win32::Foundation::HANDLE);
    impl Drop for SnapshotGuard {
        fn drop(&mut self) {
            // SAFETY: We own this snapshot handle and close it exactly once.
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
    let _guard = SnapshotGuard(snapshot);

    // Build a parent-PID map: pid -> parent_pid.
    let mut parent_map: std::collections::HashMap<u32, u32> =
        std::collections::HashMap::with_capacity(256);

    let mut entry = PROCESSENTRY32 {
        dwSize: std::mem::size_of::<PROCESSENTRY32>() as u32,
        ..Default::default()
    };

    // SAFETY: snapshot is valid, entry is properly sized.
    let first_result = unsafe { Process32First(snapshot, &mut entry) };
    if first_result.is_ok() {
        parent_map.insert(entry.th32ProcessID, entry.th32ParentProcessID);

        // SAFETY: snapshot is valid, entry is properly sized.
        while unsafe { Process32Next(snapshot, &mut entry) }.is_ok() {
            parent_map.insert(entry.th32ProcessID, entry.th32ParentProcessID);
        }
    }

    // Walk up the ancestor chain from target_pid.
    let mut current = target_pid;
    // Guard against cycles (PIDs can be reused) with a step limit.
    for _ in 0..64 {
        if current == ancestor_pid {
            return Ok(true);
        }
        match parent_map.get(&current) {
            Some(&parent) if parent != 0 && parent != current => {
                current = parent;
            }
            _ => break,
        }
    }

    Ok(false)
}

/// Get the PID of the process connected to the named pipe server end.
pub fn get_client_pid(
    pipe: &tokio::net::windows::named_pipe::NamedPipeServer,
) -> Result<u32, IpcError> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Pipes::GetNamedPipeClientProcessId;

    let raw = pipe.as_raw_handle();
    let handle = HANDLE(raw as *mut _);
    let mut pid: u32 = 0;

    // SAFETY: `handle` comes from a valid NamedPipeServer; we pass a valid
    // mutable reference for the output PID. The handle is not closed here.
    unsafe {
        GetNamedPipeClientProcessId(handle, &mut pid)
            .map_err(|e| IpcError::General(format!("GetNamedPipeClientProcessId: {e}")))?;
    }

    Ok(pid)
}

/// Returns true if the method is allowed without authentication.
///
/// Only `system.ping` and `auth.login` are unauthenticated.
pub fn is_unauthenticated_method(method: &str) -> bool {
    matches!(method, "system.ping" | "auth.login")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_hmac_correct() {
        // Generate a test secret and nonce, compute HMAC, verify it.
        let secret_bytes = [0xABu8; 32];
        let secret_hex = hex::encode(secret_bytes);
        let nonce = b"test_nonce_value_here_123456789!";

        let mut mac = HmacSha256::new_from_slice(&secret_bytes).unwrap();
        mac.update(nonce);
        let result = mac.finalize().into_bytes();
        let response_hex = hex::encode(result);

        assert!(verify_hmac(&secret_hex, nonce, &response_hex));
    }

    #[test]
    fn verify_hmac_wrong_response() {
        let secret_bytes = [0xABu8; 32];
        let secret_hex = hex::encode(secret_bytes);
        let nonce = b"test_nonce_value_here_123456789!";

        assert!(!verify_hmac(&secret_hex, nonce, "deadbeef"));
    }

    #[test]
    fn verify_hmac_wrong_nonce() {
        let secret_bytes = [0xABu8; 32];
        let secret_hex = hex::encode(secret_bytes);
        let nonce1 = b"nonce_one_here_padded_to_32bytes";
        let nonce2 = b"nonce_two_here_padded_to_32bytes";

        let mut mac = HmacSha256::new_from_slice(&secret_bytes).unwrap();
        mac.update(nonce1);
        let result = mac.finalize().into_bytes();
        let response_hex = hex::encode(result);

        // Response computed for nonce1 should not verify against nonce2.
        assert!(!verify_hmac(&secret_hex, nonce2, &response_hex));
    }

    #[test]
    fn generate_nonce_is_32_bytes() {
        let nonce: [u8; 32] = generate_nonce();
        assert_eq!(nonce.len(), 32);
    }

    #[test]
    fn is_unauthenticated_method_allows_only_ping_and_login() {
        assert!(is_unauthenticated_method("system.ping"));
        assert!(is_unauthenticated_method("auth.login"));
        assert!(!is_unauthenticated_method("workspace.list"));
        assert!(!is_unauthenticated_method("surface.send_text"));
        assert!(!is_unauthenticated_method(""));
    }

    #[test]
    fn security_mode_default_is_wmux_only() {
        assert_eq!(SecurityMode::default(), SecurityMode::WmuxOnly);
    }

    #[test]
    fn connection_ctx_starts_unauthenticated() {
        let ctx = ConnectionCtx::new(SecurityMode::WmuxOnly);
        assert!(!ctx.authenticated);
        assert!(ctx.session_token.is_none());
        assert!(ctx.client_pid.is_none());
    }

    #[test]
    fn connection_ctx_authenticate_sets_fields() {
        let mut ctx = ConnectionCtx::new(SecurityMode::Password);
        ctx.authenticate("test-token".to_owned());
        assert!(ctx.authenticated);
        assert_eq!(ctx.session_token.as_deref(), Some("test-token"));
    }

    #[test]
    fn check_pid_ancestry_current_process_is_own_ancestor() {
        // The current process must be its own ancestor (target == ancestor).
        let pid = std::process::id();
        let result = check_pid_ancestry(pid).unwrap();
        assert!(result, "process should be ancestor of itself");
    }
}
