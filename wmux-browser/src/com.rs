use std::marker::PhantomData;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
};

use crate::BrowserError;

/// RAII guard for COM initialization.
///
/// Calls `CoInitializeEx` with `COINIT_APARTMENTTHREADED` on creation
/// and `CoUninitialize` on drop. WebView2 requires STA (Single-Threaded
/// Apartment) mode.
pub struct ComGuard {
    /// Prevents `ComGuard` from being `Send` or `Sync`. COM STA objects must
    /// remain on the thread that called `CoInitializeEx`. `*mut ()` is
    /// `!Send + !Sync`, which the compiler propagates to `ComGuard`.
    _not_send: PhantomData<*mut ()>,
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
        // SAFETY: Initializing COM in STA mode. No prior MTA init on this
        // thread (would return RPC_E_CHANGED_MODE, caught below). Drop
        // guarantees balanced CoUninitialize via the RAII guard.
        let result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };

        if result.is_err() {
            return Err(BrowserError::ComInitFailed(format!(
                "CoInitializeEx failed: {result:?}"
            )));
        }

        tracing::debug!("COM initialized in STA mode");
        Ok(Self {
            _not_send: PhantomData,
        })
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

/// Default timeout for `recv_with_pump` — 30 seconds.
///
/// WebView2 environment creation is typically 200-500 ms; controller creation
/// takes a similar amount. 30 s covers pathological cases (first launch,
/// Edge runtime update) while still bailing out on truly hung COM callbacks.
const RECV_PUMP_TIMEOUT: Duration = Duration::from_secs(30);

/// Receive a value from `rx` while pumping the Windows message loop.
///
/// WebView2 COM callbacks dispatch on the STA thread. A blocking `rx.recv()`
/// would stall the message pump and prevent callbacks from firing. This helper
/// alternates between draining pending messages and checking the channel,
/// keeping the STA alive.
///
/// Returns `BrowserError::Timeout` if the value is not received within
/// [`RECV_PUMP_TIMEOUT`] (30 s).
pub fn recv_with_pump<T>(rx: &mpsc::Receiver<T>) -> Result<T, BrowserError> {
    let deadline = Instant::now() + RECV_PUMP_TIMEOUT;

    loop {
        // Check channel FIRST — if data is already available (common case
        // after wait_for_async_operation), return immediately without pumping
        // extra messages that could cause side effects.
        match rx.try_recv() {
            Ok(val) => return Ok(val),
            Err(mpsc::TryRecvError::Disconnected) => {
                return Err(BrowserError::General(
                    "COM callback channel disconnected before sending result".into(),
                ));
            }
            Err(mpsc::TryRecvError::Empty) => {
                if Instant::now() >= deadline {
                    return Err(BrowserError::Timeout(format!(
                        "recv_with_pump: no response within {}s",
                        RECV_PUMP_TIMEOUT.as_secs()
                    )));
                }
            }
        }

        // Channel empty — pump messages so COM callbacks can dispatch.
        // SAFETY: PeekMessageW/TranslateMessage/DispatchMessageW are standard
        // Win32 message pump APIs. Called on the STA thread that owns the
        // COM objects. `msg` is stack-allocated and valid for each iteration.
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        // Yield to avoid busy-spin while waiting for COM callback.
        std::thread::sleep(Duration::from_millis(1));
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
