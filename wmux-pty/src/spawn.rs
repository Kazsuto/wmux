//! Process spawning with ConPTY pseudo-console attribute.
//!
//! Creates a child process attached to a ConPTY pseudo-console using
//! `CreateProcessW` with `PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE`.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::Path;

use windows::Win32::Foundation::{HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Console::HPCON;
use windows::Win32::System::Threading::{
    CreateProcessW, DeleteProcThreadAttributeList, GetExitCodeProcess,
    InitializeProcThreadAttributeList, TerminateProcess, UpdateProcThreadAttribute,
    WaitForSingleObject, EXTENDED_STARTUPINFO_PRESENT, LPPROC_THREAD_ATTRIBUTE_LIST,
    PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE, STARTUPINFOEXW, STARTUPINFOW,
};

use crate::error::PtyError;

/// A child process spawned inside a ConPTY pseudo-console.
///
/// Owns the process and thread handles. Implements `wait()` for blocking
/// on process exit and `kill()` for forced termination.
pub struct ChildProcess {
    process: OwnedHandle,
    _thread: OwnedHandle,
    pid: u32,
}

// SAFETY: Process handles are kernel objects — safe to send between threads.
unsafe impl Send for ChildProcess {}
// SAFETY: All methods take &mut self or &self with no interior mutability.
unsafe impl Sync for ChildProcess {}

impl std::fmt::Debug for ChildProcess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChildProcess")
            .field("pid", &self.pid)
            .finish_non_exhaustive()
    }
}

impl ChildProcess {
    /// Wait for the child process to exit. Returns `true` if exit code was 0.
    ///
    /// **Blocks the calling thread.** Must be called from `spawn_blocking`.
    pub fn wait(&self) -> Result<bool, PtyError> {
        // SAFETY: process handle is valid (owned by self).
        let wait_result =
            unsafe { WaitForSingleObject(HANDLE(self.process.as_raw_handle()), u32::MAX) };
        if wait_result != WAIT_OBJECT_0 {
            return Err(PtyError::SpawnFailed(
                format!("WaitForSingleObject returned {wait_result:?}").into(),
            ));
        }

        let mut exit_code: u32 = 1;
        // SAFETY: process handle is valid, exit_code is a valid out-pointer.
        unsafe { GetExitCodeProcess(HANDLE(self.process.as_raw_handle()), &mut exit_code) }
            .map_err(|e| PtyError::SpawnFailed(e.into()))?;

        Ok(exit_code == 0)
    }

    /// Forcefully terminate the child process.
    pub fn kill(&self) -> Result<(), PtyError> {
        // SAFETY: process handle is valid.
        unsafe { TerminateProcess(HANDLE(self.process.as_raw_handle()), 1) }
            .map_err(|e| PtyError::SpawnFailed(e.into()))
    }

    /// Process ID of the child.
    #[inline]
    pub fn pid(&self) -> u32 {
        self.pid
    }
}

/// Spawn a process inside a ConPTY pseudo-console.
///
/// Creates a process with `EXTENDED_STARTUPINFO_PRESENT` and the
/// `PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE` attribute pointing to the
/// given `HPCON`.
///
/// # Arguments
///
/// - `hpc`: The ConPTY handle to attach the process to.
/// - `exe`: Path to the executable.
/// - `args`: Command-line arguments (the exe path is NOT prepended automatically).
/// - `env`: Additional environment variables to set (merged with inherited env).
/// - `cwd`: Working directory for the child process.
pub fn spawn_command(
    hpc: HPCON,
    exe: &Path,
    args: &[String],
    env: &HashMap<String, String>,
    cwd: &Path,
) -> Result<ChildProcess, PtyError> {
    // Build command line: "exe_path" arg1 arg2 ...
    let mut cmdline = quote_arg(&exe.to_string_lossy());
    for arg in args {
        cmdline.push(' ');
        cmdline.push_str(&quote_arg(arg));
    }
    let mut cmdline_wide = to_wide_null(&cmdline);

    // Build environment block (UTF-16, double-null terminated).
    let env_block = build_env_block(env);

    // Build STARTUPINFOEXW with pseudo-console attribute.
    let attr_list = create_attribute_list(hpc)?;

    let si = STARTUPINFOEXW {
        StartupInfo: STARTUPINFOW {
            cb: std::mem::size_of::<STARTUPINFOEXW>() as u32,
            ..Default::default()
        },
        lpAttributeList: attr_list.ptr,
    };

    let mut pi = PROCESS_INFORMATION::default();
    let cwd_wide = to_wide_null(&cwd.to_string_lossy());

    // SAFETY:
    // - cmdline_wide is a valid mutable wide string (CreateProcessW may modify it).
    // - env_block is a valid null-terminated UTF-16 environment block.
    // - si contains a valid attribute list with the pseudo-console attribute.
    // - pi is an out-parameter.
    // - cwd_wide is a valid null-terminated wide string.
    let result = unsafe {
        CreateProcessW(
            None,
            Some(windows::core::PWSTR(cmdline_wide.as_mut_ptr())),
            None,
            None,
            false,
            EXTENDED_STARTUPINFO_PRESENT
                | windows::Win32::System::Threading::CREATE_UNICODE_ENVIRONMENT,
            Some(env_block.as_ptr().cast()),
            windows::core::PCWSTR(cwd_wide.as_ptr()),
            &si.StartupInfo,
            &mut pi,
        )
    };

    // Drop the attribute list before checking the result (cleanup).
    drop(attr_list);

    result.map_err(|e| PtyError::SpawnFailed(e.into()))?;

    tracing::info!(
        pid = pi.dwProcessId,
        exe = %exe.display(),
        "child process spawned in ConPTY"
    );

    // SAFETY: CreateProcessW succeeded — both handles are valid and owned by us.
    let process = unsafe { OwnedHandle::from_raw_handle(pi.hProcess.0) };
    let thread = unsafe { OwnedHandle::from_raw_handle(pi.hThread.0) };

    Ok(ChildProcess {
        process,
        _thread: thread,
        pid: pi.dwProcessId,
    })
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// RAII wrapper for a `PROC_THREAD_ATTRIBUTE_LIST` allocation.
struct AttributeList {
    ptr: LPPROC_THREAD_ATTRIBUTE_LIST,
    _buf: Vec<u8>,
}

impl Drop for AttributeList {
    fn drop(&mut self) {
        if !self.ptr.0.is_null() {
            // SAFETY: ptr was initialized by InitializeProcThreadAttributeList.
            unsafe { DeleteProcThreadAttributeList(self.ptr) };
        }
    }
}

/// Allocate and initialize a `PROC_THREAD_ATTRIBUTE_LIST` with the
/// pseudo-console attribute.
fn create_attribute_list(hpc: HPCON) -> Result<AttributeList, PtyError> {
    let mut size: usize = 0;

    // First call: get required size.
    // SAFETY: NULL list, 1 attribute, out-parameter for size.
    let _ = unsafe { InitializeProcThreadAttributeList(None, 1, None, &mut size) };

    if size == 0 {
        return Err(PtyError::SpawnFailed(
            "InitializeProcThreadAttributeList returned size 0".into(),
        ));
    }

    let mut buf = vec![0u8; size];
    let ptr = LPPROC_THREAD_ATTRIBUTE_LIST(buf.as_mut_ptr().cast());

    // Second call: initialize the list.
    // SAFETY: buf is large enough (size from first call), ptr points into buf.
    unsafe { InitializeProcThreadAttributeList(Some(ptr), 1, None, &mut size) }
        .map_err(|e| PtyError::SpawnFailed(e.into()))?;

    // SAFETY: ptr is initialized. The HPCON value is passed directly as
    // lpValue (not a pointer to it) — this matches the Microsoft sample
    // (EchoCon.cpp) and Windows Terminal source. UpdateProcThreadAttribute
    // stores lpValue as-is; CreateProcessW later interprets it as the HPCON.
    unsafe {
        UpdateProcThreadAttribute(
            ptr,
            0,
            PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
            Some(hpc.0 as *const std::ffi::c_void),
            std::mem::size_of::<HPCON>(),
            None,
            None,
        )
    }
    .map_err(|e| PtyError::SpawnFailed(e.into()))?;

    Ok(AttributeList { ptr, _buf: buf })
}

/// Build a UTF-16 environment block from the current process environment
/// plus additional variables.
///
/// Format: `KEY=VALUE\0KEY=VALUE\0\0` (each entry null-separated, block
/// double-null terminated).
fn build_env_block(extra: &HashMap<String, String>) -> Vec<u16> {
    let mut block = Vec::new();
    // Emit inherited env vars, skipping any overridden by `extra`.
    for (k, v) in std::env::vars() {
        if !extra.contains_key(&k) {
            let entry = format!("{k}={v}");
            block.extend(entry.encode_utf16());
            block.push(0);
        }
    }
    // Emit extra vars (overrides + additions).
    for (k, v) in extra {
        let entry = format!("{k}={v}");
        block.extend(entry.encode_utf16());
        block.push(0);
    }
    block.push(0); // double-null terminator
    block
}

/// Quote a command-line argument for Windows using the `CommandLineToArgvW`
/// escaping rules (MSDN).
///
/// Rules:
/// - 2n backslashes followed by `"` → n backslashes + literal `"`
/// - 2n+1 backslashes followed by `"` → n backslashes + end of argument
/// - n backslashes NOT followed by `"` → n backslashes (literal)
///
/// We reverse these rules to produce a correctly escaped argument.
fn quote_arg(s: &str) -> String {
    if s.is_empty() {
        return "\"\"".to_string();
    }
    if !s.contains([' ', '\t', '"', '\\']) {
        return s.to_string();
    }

    let mut result = String::with_capacity(s.len() + 2);
    result.push('"');

    let mut backslash_count: usize = 0;
    for c in s.chars() {
        match c {
            '\\' => backslash_count += 1,
            '"' => {
                // Output 2n backslashes (doubled originals) then escaped quote.
                for _ in 0..backslash_count * 2 {
                    result.push('\\');
                }
                backslash_count = 0;
                result.push('\\');
                result.push('"');
            }
            _ => {
                // Flush accumulated backslashes literally (no doubling needed).
                for _ in 0..backslash_count {
                    result.push('\\');
                }
                backslash_count = 0;
                result.push(c);
            }
        }
    }

    // Output 2n trailing backslashes (doubled) before the closing quote
    // to prevent them from escaping it.
    for _ in 0..backslash_count * 2 {
        result.push('\\');
    }

    result.push('"');
    result
}

/// Convert a string to a null-terminated UTF-16 wide string.
fn to_wide_null(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_arg_no_special_chars() {
        assert_eq!(quote_arg("hello"), "hello");
    }

    #[test]
    fn quote_arg_with_spaces() {
        assert_eq!(quote_arg("hello world"), "\"hello world\"");
    }

    #[test]
    fn quote_arg_with_quotes() {
        assert_eq!(quote_arg("say \"hi\""), "\"say \\\"hi\\\"\"");
    }

    #[test]
    fn quote_arg_trailing_backslash() {
        // C:\dir\ should become "C:\dir\\" — doubled backslashes before closing quote
        assert_eq!(quote_arg("C:\\dir\\"), "\"C:\\dir\\\\\"");
    }

    #[test]
    fn quote_arg_backslash_before_quote() {
        // a\"b should become "a\\\"b" — doubled backslash + escaped quote
        assert_eq!(quote_arg("a\\\"b"), "\"a\\\\\\\"b\"");
    }

    #[test]
    fn quote_arg_path_with_spaces_trailing_backslash() {
        // C:\Program Files\ → "C:\Program Files\\"
        assert_eq!(
            quote_arg("C:\\Program Files\\"),
            "\"C:\\Program Files\\\\\""
        );
    }

    #[test]
    fn quote_arg_empty() {
        assert_eq!(quote_arg(""), "\"\"");
    }

    #[test]
    fn build_env_block_contains_extra_vars() {
        let mut extra = HashMap::new();
        extra.insert("WMUX_TEST".to_string(), "hello".to_string());
        let block = build_env_block(&extra);

        // Convert block back to string to search.
        let s: String = block
            .split(|&c| c == 0)
            .filter(|s| !s.is_empty())
            .map(String::from_utf16_lossy)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(s.contains("WMUX_TEST=hello"));
    }

    #[test]
    fn to_wide_null_terminates() {
        let w = to_wide_null("AB");
        assert_eq!(w, vec![b'A' as u16, b'B' as u16, 0]);
    }
}
