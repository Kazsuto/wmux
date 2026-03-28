use thiserror::Error;

#[derive(Debug, Error)]
pub enum BrowserError {
    #[error("WebView2 runtime not installed")]
    RuntimeNotInstalled,

    #[error("COM initialization failed: {0}")]
    ComInitFailed(String),

    #[error("WebView2 environment creation failed: {0}")]
    EnvironmentCreationFailed(String),

    #[error("user data directory setup failed: {0}")]
    UserDataDirFailed(String),

    #[error("navigation failed: {0}")]
    NavigationFailed(String),

    #[error("JavaScript eval error: {0}")]
    JavaScriptError(String),

    #[error("wait timed out: {0}")]
    Timeout(String),

    #[error("WebView2 controller not available")]
    ControllerNotAvailable,

    #[error("invalid URL scheme: {0} (only http:// and https:// are allowed)")]
    InvalidUrlScheme(String),

    #[error("{0}")]
    General(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn _assert_send<T: Send>() {}
    fn _assert_sync<T: Sync>() {}

    #[test]
    fn error_is_send_and_sync() {
        _assert_send::<BrowserError>();
        _assert_sync::<BrowserError>();
    }

    #[test]
    fn error_messages() {
        let err = BrowserError::RuntimeNotInstalled;
        assert_eq!(err.to_string(), "WebView2 runtime not installed");

        let err = BrowserError::ComInitFailed("STA failed".into());
        assert_eq!(err.to_string(), "COM initialization failed: STA failed");

        let err = BrowserError::EnvironmentCreationFailed("timeout".into());
        assert_eq!(
            err.to_string(),
            "WebView2 environment creation failed: timeout"
        );

        let err = BrowserError::UserDataDirFailed("permission denied".into());
        assert_eq!(
            err.to_string(),
            "user data directory setup failed: permission denied"
        );
    }
}
