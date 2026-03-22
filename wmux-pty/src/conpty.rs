//! Safe RAII wrapper around Windows ConPTY (Console Pseudo Terminal).
//!
//! Replaces `portable-pty` with direct ConPTY API access via the `windows` crate,
//! enabling `PSEUDOCONSOLE_RESIZE_QUIRK` and proper 24H2+ shutdown via
//! `ReleasePseudoConsole`.

use std::fs::File;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};

use windows::Win32::Foundation::{
    SetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE,
};
use windows::Win32::Security::SECURITY_ATTRIBUTES;
use windows::Win32::System::Console::{
    ClosePseudoConsole, CreatePseudoConsole, ResizePseudoConsole, COORD, HPCON,
};
use windows::Win32::System::Pipes::CreatePipe;

use crate::error::PtyError;

/// `PSEUDOCONSOLE_RESIZE_QUIRK` (0x2) — disables automatic reflow output on
/// resize. Without this flag, `ResizePseudoConsole` emits VT reflow sequences
/// into the output pipe that corrupt the terminal grid. wmux implements its
/// own reflow in the scrollback buffer.
///
/// This flag is used by Windows Terminal / OpenConsole but is not officially
/// documented in the Windows SDK headers.
const PSEUDOCONSOLE_RESIZE_QUIRK: u32 = 0x2;

/// Safe RAII wrapper around a Windows ConPTY pseudo-console.
///
/// Owns the `HPCON` handle. On drop, performs the correct shutdown sequence:
/// - **Windows 11 24H2+:** `ReleasePseudoConsole` → `ClosePseudoConsole`
/// - **Older Windows:** `ClosePseudoConsole` (which blocks until conhost exits)
///
/// The I/O pipe handles (`input_write`, `output_read`) are returned from
/// [`create_conpty`] and owned separately by the caller.
pub struct ConPtyHandle {
    /// `Some` while the handle is live; `None` after `shutdown()` or `close()`.
    hpc: Option<HPCON>,
}

// SAFETY: HPCON is a kernel object handle (isize). Kernel handles are safe to
// send between threads — the kernel serialises access internally.
unsafe impl Send for ConPtyHandle {}

impl ConPtyHandle {
    /// Wrap an existing HPCON in a safe handle.
    fn new(hpc: HPCON) -> Self {
        Self { hpc: Some(hpc) }
    }

    /// Return the raw HPCON value for use in `ResizePseudoConsole` calls.
    ///
    /// The returned value is `Copy` and can be sent to other tasks. It remains
    /// valid until this `ConPtyHandle` is dropped or [`shutdown`] is called.
    ///
    /// # Thread safety
    ///
    /// `ResizePseudoConsole` is not documented as thread-safe. Callers MUST
    /// ensure that only one task calls resize at a time (the canonical pattern
    /// is the dedicated `spawn_resize_handler` task in actor.rs).
    #[inline]
    pub(crate) fn hpcon(&self) -> HPCON {
        self.hpc.expect("ConPtyHandle used after shutdown")
    }

    /// Resize the pseudo-console to the given dimensions.
    ///
    /// `cols` is clamped to `[2, i16::MAX]` to prevent ConPTY bug #19922.
    /// `rows` is clamped to `[1, i16::MAX]`.
    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), PtyError> {
        let Some(hpc) = self.hpc else {
            return Err(PtyError::ConPtyCreationFailed(
                "resize after shutdown".to_string(),
            ));
        };
        do_resize(hpc, cols, rows)
    }

    /// Shut down the pseudo-console cleanly.
    ///
    /// On Windows 11 24H2+ (build 26100), calls `ReleasePseudoConsole` first
    /// to allow ConPTY to self-terminate, then `ClosePseudoConsole`.
    /// On older Windows, only `ClosePseudoConsole` is called (which blocks).
    ///
    /// **Must be called from a context that can block** (`spawn_blocking`).
    /// Safe to call multiple times (no-op after the first).
    pub fn shutdown(&mut self) {
        if let Some(hpc) = self.hpc.take() {
            try_release_pseudo_console(hpc);
            // SAFETY: HPCON is valid (taken from Option, first and only close).
            unsafe { ClosePseudoConsole(hpc) };
        }
    }
}

impl Drop for ConPtyHandle {
    fn drop(&mut self) {
        // Reuse shutdown() — safe because it's idempotent (Option::take).
        self.shutdown();
    }
}

/// Result of creating a ConPTY pseudo-console.
pub struct ConPtyPair {
    /// The ConPTY handle (owns the HPCON).
    pub conpty: ConPtyHandle,
    /// Read end of the output pipe — PTY stdout. Wrap in `File` for `Read`.
    pub output_read: File,
    /// Write end of the input pipe — PTY stdin. Wrap in `File` for `Write`.
    pub input_write: File,
}

/// Create a new ConPTY pseudo-console with the given dimensions.
///
/// Creates two anonymous pipes (input and output), then creates the
/// pseudo-console with `PSEUDOCONSOLE_RESIZE_QUIRK` to prevent reflow
/// output on resize.
///
/// Returns the ConPTY handle and the pipe ends that the caller uses for I/O.
pub fn create_conpty(cols: u16, rows: u16) -> Result<ConPtyPair, PtyError> {
    let cols = cols.max(2);
    let rows = rows.max(1);

    // Create input pipe (wmux writes → ConPTY reads).
    let (input_read, input_write) = create_pipe()?;
    // Create output pipe (ConPTY writes → wmux reads).
    let (output_read, output_write) = create_pipe()?;

    let size = COORD {
        X: cols as i16,
        Y: rows as i16,
    };

    // SAFETY: All handles are valid. PSEUDOCONSOLE_RESIZE_QUIRK disables
    // automatic reflow output on resize. In windows 0.62, CreatePseudoConsole
    // returns Result<HPCON> directly.
    let hpc = unsafe {
        CreatePseudoConsole(
            size,
            HANDLE(input_read.as_raw_handle()),
            HANDLE(output_write.as_raw_handle()),
            PSEUDOCONSOLE_RESIZE_QUIRK,
        )
    }
    .map_err(|e| PtyError::ConPtyCreationFailed(format!("CreatePseudoConsole failed: {e}")))?;

    if hpc.is_invalid() {
        return Err(PtyError::ConPtyCreationFailed(
            "CreatePseudoConsole returned invalid handle".to_string(),
        ));
    }

    // Close the pipe ends now owned by ConPTY (it duplicated them internally).
    drop(input_read);
    drop(output_write);

    // Remove HANDLE_FLAG_INHERIT from the pipe handles we keep. This prevents
    // other child processes (WebView2, etc.) from inheriting them, which would
    // leak pipe handles and prevent EOF detection.
    // SAFETY: Both handles are valid (not yet dropped).
    if let Err(e) = unsafe {
        SetHandleInformation(
            HANDLE(output_read.as_raw_handle()),
            HANDLE_FLAG_INHERIT.0,
            windows::Win32::Foundation::HANDLE_FLAGS(0),
        )
    } {
        tracing::warn!(error = %e, "SetHandleInformation failed on output_read pipe");
    }
    if let Err(e) = unsafe {
        SetHandleInformation(
            HANDLE(input_write.as_raw_handle()),
            HANDLE_FLAG_INHERIT.0,
            windows::Win32::Foundation::HANDLE_FLAGS(0),
        )
    } {
        tracing::warn!(error = %e, "SetHandleInformation failed on input_write pipe");
    }

    tracing::debug!(cols, rows, "ConPTY created with PSEUDOCONSOLE_RESIZE_QUIRK");

    Ok(ConPtyPair {
        conpty: ConPtyHandle::new(hpc),
        output_read: File::from(output_read),
        input_write: File::from(input_write),
    })
}

/// Create an anonymous pipe, returning `(read_handle, write_handle)` as `OwnedHandle`.
fn create_pipe() -> Result<(OwnedHandle, OwnedHandle), PtyError> {
    let mut read_handle = INVALID_HANDLE_VALUE;
    let mut write_handle = INVALID_HANDLE_VALUE;

    let sa = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: true.into(),
    };

    // SAFETY: Pointers to valid stack-allocated HANDLEs. sa is valid.
    unsafe { CreatePipe(&mut read_handle, &mut write_handle, Some(&sa), 0) }
        .map_err(|e| PtyError::SpawnFailed(e.into()))?;

    // SAFETY: CreatePipe succeeded — both handles are valid and owned by us.
    let read_owned = unsafe { OwnedHandle::from_raw_handle(read_handle.0) };
    let write_owned = unsafe { OwnedHandle::from_raw_handle(write_handle.0) };

    Ok((read_owned, write_owned))
}

/// Try to call `ReleasePseudoConsole` (available on Windows 11 24H2+, build 26100).
///
/// Loaded dynamically via `GetProcAddress` since the function may not exist
/// on older Windows versions. Returns `true` if the function was found and
/// called successfully.
fn try_release_pseudo_console(hpc: HPCON) -> bool {
    use windows::core::w;
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};

    // SAFETY: kernel32.dll is always loaded in every Windows process.
    let kernel32 = unsafe { GetModuleHandleW(w!("kernel32.dll")) };
    let Ok(kernel32) = kernel32 else {
        return false;
    };

    // SAFETY: Module handle is valid. Function name is a valid C string.
    let proc = unsafe { GetProcAddress(kernel32, windows::core::s!("ReleasePseudoConsole")) };
    let Some(proc) = proc else {
        tracing::trace!("ReleasePseudoConsole not available (pre-24H2 Windows)");
        return false;
    };

    // SAFETY: `proc` was obtained from `GetProcAddress(kernel32, "ReleasePseudoConsole")`
    // which is guaranteed to return the correct function pointer for
    // `ReleasePseudoConsole(HPCON) -> HRESULT` when non-null (checked above).
    // HPCON is repr(transparent) over isize, HRESULT is i32, and the calling
    // convention is "system" (stdcall on x86, C on x86-64) — matching the Win32 ABI.
    type ReleaseFn = unsafe extern "system" fn(isize) -> i32;
    let release: ReleaseFn = unsafe { std::mem::transmute(proc) };
    let hr = unsafe { release(hpc.0) };

    if hr >= 0 {
        tracing::debug!("ReleasePseudoConsole called (24H2+ shutdown path)");
        true
    } else {
        tracing::warn!(hresult = hr, "ReleasePseudoConsole failed");
        false
    }
}

/// Resize a ConPTY by raw HPCON value.
///
/// Used by the actor's resize handler which only holds a copy of the HPCON
/// (not the full `ConPtyHandle`).
///
/// # Thread safety
///
/// Must only be called from a single task (the resize handler). See
/// [`ConPtyHandle::hpcon`] for details.
pub(crate) fn resize_by_hpcon(hpc: HPCON, cols: u16, rows: u16) -> Result<(), PtyError> {
    do_resize(hpc, cols, rows)
}

/// Shared resize implementation: clamp dimensions and call `ResizePseudoConsole`.
///
/// `cols` is clamped to `[2, i16::MAX]` to prevent ConPTY bug #19922.
/// `rows` is clamped to `[1, i16::MAX]`.
fn do_resize(hpc: HPCON, cols: u16, rows: u16) -> Result<(), PtyError> {
    let cols = cols.clamp(2, i16::MAX as u16);
    let rows = rows.clamp(1, i16::MAX as u16);
    let size = COORD {
        X: cols as i16,
        Y: rows as i16,
    };
    // SAFETY: Caller guarantees HPCON is valid. COORD is a plain value type.
    unsafe { ResizePseudoConsole(hpc, size) }.map_err(|e| PtyError::ResizeFailed(e.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pseudoconsole_resize_quirk_is_0x2() {
        assert_eq!(PSEUDOCONSOLE_RESIZE_QUIRK, 0x2);
    }

    #[test]
    fn create_pipe_returns_valid_handles() {
        let (read_h, write_h) = create_pipe().expect("create_pipe failed");
        // Handles should be valid (non-null).
        assert!(!read_h.as_raw_handle().is_null());
        assert!(!write_h.as_raw_handle().is_null());
    }

    #[test]
    fn pipe_read_write_roundtrip() {
        let (read_h, write_h) = create_pipe().expect("create_pipe failed");
        let mut writer = File::from(write_h);
        let mut reader = File::from(read_h);

        use std::io::{Read, Write};
        writer.write_all(b"hello").unwrap();
        drop(writer); // Close write end to get EOF on read

        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).unwrap();
        assert_eq!(&buf, b"hello");
    }

    #[test]
    #[ignore] // Requires real ConPTY
    fn create_conpty_succeeds() {
        let pair = create_conpty(80, 24).expect("create_conpty failed");
        assert!(pair.conpty.hpc.is_some());
    }

    #[test]
    #[ignore] // Requires real ConPTY
    fn create_conpty_and_resize() {
        let pair = create_conpty(80, 24).expect("create_conpty failed");
        pair.conpty.resize(120, 40).expect("resize failed");
        // cols=1 should be clamped to 2
        pair.conpty.resize(1, 1).expect("small resize failed");
    }

    #[test]
    fn try_release_with_invalid_handle_does_not_panic() {
        // Should return false gracefully, not panic.
        let invalid = HPCON(0);
        // This tests the dynamic lookup path only.
        let _ = try_release_pseudo_console(invalid);
    }
}
