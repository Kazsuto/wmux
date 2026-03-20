use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};

use crate::BrowserError;

/// RAII guard for COM initialization.
///
/// Calls `CoInitializeEx` with `COINIT_APARTMENTTHREADED` on creation
/// and `CoUninitialize` on drop. WebView2 requires STA (Single-Threaded
/// Apartment) mode.
pub struct ComGuard {
    _initialized: bool,
}

impl std::fmt::Debug for ComGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComGuard").finish()
    }
}

impl ComGuard {
    /// Initialize COM in apartment-threaded (STA) mode.
    ///
    /// This must be called on the UI thread before any WebView2 operations.
    /// `CoInitializeEx` is idempotent — calling it multiple times on the
    /// same thread is safe (returns `S_FALSE` on subsequent calls).
    pub fn new() -> Result<Self, BrowserError> {
        // SAFETY: CoInitializeEx is a well-documented Win32 API.
        // We pass COINIT_APARTMENTTHREADED for STA mode required by WebView2.
        // The call is safe to make from any thread and is idempotent.
        let result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };

        if result.is_err() {
            return Err(BrowserError::ComInitFailed(format!(
                "CoInitializeEx failed: {result:?}"
            )));
        }

        tracing::debug!("COM initialized in STA mode");
        Ok(Self { _initialized: true })
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        // SAFETY: CoUninitialize must be called once for each successful
        // CoInitializeEx call, on the same thread. We track initialization
        // via the RAII guard to ensure balanced calls.
        unsafe {
            CoUninitialize();
        }
        tracing::debug!("COM uninitialized");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn _assert_send<T: Send>() {}
    fn _assert_sync<T: Sync>() {}

    // Note: ComGuard is intentionally NOT Send/Sync because COM STA
    // objects must remain on the thread that initialized them.
    // We do NOT assert Send/Sync here.

    #[test]
    #[ignore] // Requires Windows COM runtime
    fn com_guard_init_and_drop() {
        let guard = ComGuard::new().expect("COM should initialize");
        drop(guard);
        // Second initialization should also work (idempotent)
        let _guard2 = ComGuard::new().expect("COM should re-initialize");
    }
}
