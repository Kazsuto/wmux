use std::sync::mpsc;

use webview2_com::{
    CreateCoreWebView2ControllerCompletedHandler,
    Microsoft::Web::WebView2::Win32::{
        ICoreWebView2, ICoreWebView2Environment, ICoreWebView2Settings,
    },
};
use windows::core::w;
use windows::Win32::Foundation::{E_POINTER, HWND};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassExW, CS_HREDRAW, CS_VREDRAW,
    WNDCLASSEXW, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
};

use crate::com::recv_with_pump;
use crate::BrowserError;

use super::BrowserPanel;

impl BrowserPanel {
    /// Apply security settings to a freshly created WebView2.
    ///
    /// Disables context menus, status bar, script dialogs, host object injection,
    /// and web messaging. In release builds, also disables DevTools.
    fn apply_security_settings(webview: &ICoreWebView2) -> Result<(), BrowserError> {
        // SAFETY: Settings() returns the ICoreWebView2Settings for this webview.
        // The webview COM pointer is valid (just created).
        let settings: ICoreWebView2Settings = unsafe { webview.Settings() }
            .map_err(|e| BrowserError::General(format!("Settings: {e}")))?;

        // SAFETY: All Set* methods take a plain BOOL value. The settings
        // COM object is valid for the duration of these calls.
        unsafe {
            settings
                .SetAreDefaultContextMenusEnabled(false)
                .map_err(|e| {
                    BrowserError::General(format!("SetAreDefaultContextMenusEnabled: {e}"))
                })?;
            settings
                .SetIsStatusBarEnabled(false)
                .map_err(|e| BrowserError::General(format!("SetIsStatusBarEnabled: {e}")))?;
            settings
                .SetAreDefaultScriptDialogsEnabled(false)
                .map_err(|e| {
                    BrowserError::General(format!("SetAreDefaultScriptDialogsEnabled: {e}"))
                })?;
            // Disable host object injection — wmux does not use AddHostObjectToScript.
            settings
                .SetAreHostObjectsAllowed(false)
                .map_err(|e| BrowserError::General(format!("SetAreHostObjectsAllowed: {e}")))?;
            // Disable web messaging (chrome.webview.postMessage) — not used by wmux.
            settings
                .SetIsWebMessageEnabled(false)
                .map_err(|e| BrowserError::General(format!("SetIsWebMessageEnabled: {e}")))?;
        }

        // Disable DevTools in release builds to prevent page inspection
        // and CSP bypass via IPC.
        #[cfg(not(debug_assertions))]
        unsafe {
            settings
                .SetAreDevToolsEnabled(false)
                .map_err(|e| BrowserError::General(format!("SetAreDevToolsEnabled: {e}")))?;
        }

        tracing::debug!("WebView2 security settings applied");
        Ok(())
    }

    /// Attach the panel to a parent window by creating a dedicated owned popup
    /// HWND and then creating the WebView2 controller inside it.
    ///
    /// The popup HWND is a separate top-level window (WS_POPUP) owned by
    /// `parent_hwnd`. DWM composites it as an independent visual, immune to
    /// DXGI flip swap chain occlusion on the parent. This is a blocking
    /// COM operation — call from the UI/STA thread.
    pub fn attach(
        &mut self,
        environment: &ICoreWebView2Environment,
        parent_hwnd: HWND,
    ) -> Result<(), BrowserError> {
        // Create an owned popup HWND to host the WebView2 controller.
        // Using WS_POPUP (not WS_CHILD) ensures DWM composites the WebView2
        // as a separate top-level visual, unaffected by the wgpu swap chain.
        let host = create_host_hwnd(parent_hwnd).map_err(|e| {
            BrowserError::General(format!("failed to create WebView2 host HWND: {e}"))
        })?;
        // Store the host HWND immediately so Drop cleans it up on error paths.
        self.host_hwnd = Some(host);
        self.parent_hwnd = parent_hwnd;

        let (tx, rx) = mpsc::sync_channel(1);

        // Clone the COM interface so it can be moved into the 'static closure.
        let environment = environment.clone();

        CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
            Box::new(move |handler| {
                // SAFETY: CreateCoreWebView2Controller takes a valid HWND and a
                // COM callback handler. Both are valid for the duration of the call.
                // `environment` is a cloned COM reference with its own ref-count.
                // `host` is a valid child HWND just created above.
                unsafe { environment.CreateCoreWebView2Controller(host, &handler) }
                    .map_err(webview2_com::Error::WindowsError)
            }),
            Box::new(move |error_code, controller| {
                error_code?;
                let _ = tx.send(controller.ok_or_else(|| windows::core::Error::from(E_POINTER)));
                Ok(())
            }),
        )
        .map_err(|e| {
            BrowserError::EnvironmentCreationFailed(format!("CreateCoreWebView2Controller: {e}"))
        })?;

        let controller = recv_with_pump(&rx)?.map_err(|e| {
            BrowserError::EnvironmentCreationFailed(format!("controller error: {e}"))
        })?;

        // SAFETY: CoreWebView2() returns the ICoreWebView2 associated with the
        // controller. The controller is valid (just created above).
        let webview = unsafe { controller.CoreWebView2() }
            .map_err(|_| BrowserError::ControllerNotAvailable)?;

        // Apply security hardening — disable context menus, status bar, and
        // script dialogs (alert/confirm/prompt) to prevent page-level UI from
        // interfering with the terminal multiplexer experience.
        Self::apply_security_settings(&webview)?;

        tracing::info!(surface_id = %self.surface_id, "BrowserPanel attached to HWND");

        self.controller = Some(controller);
        self.webview = Some(webview);
        Ok(())
    }
}

impl Drop for BrowserPanel {
    fn drop(&mut self) {
        if let Some(controller) = self.controller.take() {
            // SAFETY: Close() releases the WebView2 controller and its
            // associated browser process + child HWND.  Must be called to
            // prevent resource leaks when a panel is removed.
            if let Err(e) = unsafe { controller.Close() } {
                tracing::warn!(
                    surface_id = %self.surface_id,
                    error = %e,
                    "failed to close WebView2 controller — possible zombie msedgewebview2.exe"
                );
            } else {
                tracing::debug!(surface_id = %self.surface_id, "BrowserPanel controller closed");
            }
        }
        if let Some(host) = self.host_hwnd.take() {
            // SAFETY: DestroyWindow destroys a valid HWND we own. Must be
            // called after Close() so WebView2 releases its child windows first.
            if let Err(e) = unsafe { DestroyWindow(host) } {
                tracing::warn!(
                    surface_id = %self.surface_id,
                    error = %e,
                    "failed to destroy host HWND"
                );
            } else {
                tracing::debug!(surface_id = %self.surface_id, "host HWND destroyed");
            }
        }
    }
}

/// Default window procedure forwarding for the host popup HWND.
///
/// All messages are passed through to `DefWindowProcW` — the host HWND
/// exists solely as a container for WebView2's own child windows.
unsafe extern "system" fn host_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

/// Create an owned popup HWND to host a WebView2 controller.
///
/// Uses `WS_POPUP` (not `WS_CHILD`) so the host is a separate top-level
/// window in DWM's visual tree. This avoids the known issue where DXGI
/// flip-model swap chains occlude child HWNDs on the same parent.
///
/// `WS_EX_TOOLWINDOW` prevents a taskbar entry. `WS_EX_NOACTIVATE` prevents
/// stealing focus from the main window on click.
///
/// The popup is **owned** by `parent` (via the `hwndParent` parameter), so
/// DWM keeps it above the owner and hides/shows it when the owner is
/// minimized/restored.
fn create_host_hwnd(parent: HWND) -> Result<HWND, windows::core::Error> {
    // SAFETY: GetModuleHandleW(None) returns the current executable's HINSTANCE.
    let hinstance = unsafe { GetModuleHandleW(None)? };

    let class_name = w!("WmuxWebView2Host");
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(host_wndproc),
        hInstance: hinstance.into(),
        lpszClassName: class_name,
        style: CS_HREDRAW | CS_VREDRAW,
        ..Default::default()
    };
    // RegisterClassExW is idempotent — returns 0 if class already exists, which is fine.
    // SAFETY: wc is fully initialized with valid function pointer and class name.
    unsafe {
        RegisterClassExW(&wc);
    }

    // SAFETY: CreateWindowExW with WS_POPUP creates a top-level popup window
    // owned by `parent`. WS_EX_TOOLWINDOW prevents a taskbar button.
    // WS_EX_NOACTIVATE prevents focus theft on click (WebView2 manages its
    // own focus via MoveFocus). The popup is created hidden at (0,0,0,0) —
    // the caller sets position via `set_bounds`.
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class_name,
            w!(""),
            WS_POPUP | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
            0,
            0,
            0,
            0,
            Some(parent),
            None,
            Some(hinstance.into()),
            None,
        )?
    };

    tracing::debug!("WebView2 host popup HWND created");
    Ok(hwnd)
}

#[cfg(test)]
mod tests {
    use wmux_core::types::SurfaceId;

    use super::super::BrowserPanel;

    #[test]
    #[ignore] // Requires COM runtime and a real HWND with WebView2
    fn attach_and_navigate() {
        // This test requires a real COM environment, HWND, and WebView2 runtime.
        // Run with: cargo test -p wmux-browser -- --ignored
    }

    #[test]
    fn drop_without_attach_is_safe() {
        let _panel = BrowserPanel::new(SurfaceId::new());
        // Panel drops without panic — no controller or host to clean up.
    }
}
