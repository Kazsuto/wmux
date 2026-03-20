use std::path::{Path, PathBuf};
use std::sync::mpsc;

use webview2_com::CreateCoreWebView2EnvironmentCompletedHandler;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    CreateCoreWebView2EnvironmentWithOptions, GetAvailableCoreWebView2BrowserVersionString,
    ICoreWebView2Environment,
};
use windows::core::PWSTR;
use windows::Win32::System::Com::CoTaskMemFree;

use crate::com::ComGuard;
use crate::BrowserError;

/// Manages the WebView2 browser lifecycle.
///
/// Holds the COM guard and provides methods for runtime detection
/// and environment creation. All COM operations happen on the
/// creating thread (STA requirement).
pub struct BrowserManager {
    _com_guard: ComGuard,
    user_data_dir: PathBuf,
}

impl BrowserManager {
    /// Create a new `BrowserManager`, initializing COM and setting up user data dir.
    pub fn new() -> Result<Self, BrowserError> {
        let com_guard = ComGuard::new()?;
        let user_data_dir = Self::ensure_user_data_dir()?;

        tracing::info!(
            user_data_dir = %user_data_dir.display(),
            "BrowserManager initialized"
        );

        Ok(Self {
            _com_guard: com_guard,
            user_data_dir,
        })
    }

    /// Check if the WebView2 runtime is installed on this system.
    pub fn is_runtime_available() -> bool {
        Self::get_runtime_version_string().is_ok()
    }

    /// Get the WebView2 runtime version string, if available.
    pub fn runtime_version() -> Result<String, BrowserError> {
        Self::get_runtime_version_string()
    }

    /// Query the WebView2 runtime version, freeing the COM-allocated PWSTR.
    fn get_runtime_version_string() -> Result<String, BrowserError> {
        let mut version = PWSTR::null();
        // SAFETY: GetAvailableCoreWebView2BrowserVersionString is a well-documented
        // WebView2 API. We pass null for browser folder (use default Edge install)
        // and a valid mut pointer for version output.
        unsafe { GetAvailableCoreWebView2BrowserVersionString(None, &mut version) }.map_err(
            |e| {
                tracing::warn!(error = %e, "WebView2 runtime not detected");
                BrowserError::RuntimeNotInstalled
            },
        )?;

        if version.is_null() {
            return Err(BrowserError::RuntimeNotInstalled);
        }

        // SAFETY: PWSTR from the API is valid and null-terminated.
        let version_str = unsafe {
            let s = version
                .to_string()
                .map_err(|_| BrowserError::RuntimeNotInstalled);
            // SAFETY: The PWSTR was allocated by the WebView2 API via CoTaskMemAlloc.
            // We must free it with CoTaskMemFree to avoid a memory leak.
            CoTaskMemFree(Some(version.as_ptr().cast()));
            s
        }?;

        tracing::debug!(version = %version_str, "WebView2 runtime detected");
        Ok(version_str)
    }

    /// Get the user data directory path.
    pub fn user_data_dir(&self) -> &Path {
        &self.user_data_dir
    }

    /// Create the WebView2 environment with the configured user data directory.
    ///
    /// This is an expensive operation (~200-500ms for first instance).
    /// The returned environment should be cached for creating multiple controllers.
    pub fn create_environment(&self) -> Result<ICoreWebView2Environment, BrowserError> {
        let (tx, rx) = mpsc::sync_channel(1);

        let user_data_folder = self.user_data_dir.to_string_lossy().to_string();

        CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
            Box::new(move |handler| {
                let user_data_wide: Vec<u16> = user_data_folder
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect();
                let user_data_pcwstr = windows::core::PCWSTR::from_raw(user_data_wide.as_ptr());

                // SAFETY: CreateCoreWebView2EnvironmentWithOptions is a standard
                // WebView2 API. We pass valid null-terminated wide string for user
                // data folder, and a valid callback handler.
                unsafe {
                    CreateCoreWebView2EnvironmentWithOptions(None, user_data_pcwstr, None, &handler)
                }
                .map_err(webview2_com::Error::WindowsError)
            }),
            Box::new(move |error_code, environment| {
                error_code?;
                tx.send(environment.ok_or_else(|| {
                    windows::core::Error::from(windows::Win32::Foundation::E_POINTER)
                }))
                .expect("send over mpsc channel");
                Ok(())
            }),
        )
        .map_err(|e| {
            BrowserError::EnvironmentCreationFailed(format!(
                "WebView2 environment creation failed: {e}"
            ))
        })?;

        let env = rx
            .recv()
            .map_err(|e| {
                BrowserError::EnvironmentCreationFailed(format!(
                    "failed to receive environment: {e}"
                ))
            })?
            .map_err(|e| {
                BrowserError::EnvironmentCreationFailed(format!("environment creation error: {e}"))
            })?;

        tracing::info!("WebView2 environment created");
        Ok(env)
    }

    /// Ensure the user data directory exists at `%APPDATA%\wmux\webview2-data`.
    fn ensure_user_data_dir() -> Result<PathBuf, BrowserError> {
        let config_dir = dirs::config_dir().ok_or_else(|| {
            BrowserError::UserDataDirFailed("could not determine %APPDATA% directory".into())
        })?;

        let user_data_dir = config_dir.join("wmux").join("webview2-data");

        if !user_data_dir.exists() {
            std::fs::create_dir_all(&user_data_dir).map_err(|e| {
                BrowserError::UserDataDirFailed(format!(
                    "failed to create {}: {e}",
                    user_data_dir.display()
                ))
            })?;
            tracing::debug!(
                path = %user_data_dir.display(),
                "created WebView2 user data directory"
            );
        }

        Ok(user_data_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_detection_does_not_panic() {
        // This test works on any system — just checks it doesn't panic
        let _ = BrowserManager::is_runtime_available();
    }

    #[test]
    fn runtime_version_returns_result() {
        let result = BrowserManager::runtime_version();
        match result {
            Ok(version) => assert!(!version.is_empty()),
            Err(BrowserError::RuntimeNotInstalled) => {} // expected on some systems
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    #[test]
    #[ignore] // Requires COM runtime and WebView2
    fn create_browser_manager() {
        let manager = BrowserManager::new().expect("BrowserManager should initialize");
        assert!(manager.user_data_dir().ends_with("webview2-data"));
    }

    #[test]
    #[ignore] // Requires COM runtime and WebView2
    fn create_environment() {
        let manager = BrowserManager::new().expect("BrowserManager should initialize");
        let _env = manager
            .create_environment()
            .expect("environment should be created");
    }
}
