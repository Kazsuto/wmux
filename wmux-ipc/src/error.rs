use thiserror::Error;

#[derive(Debug, Error)]
pub enum IpcError {
    #[error("{0}")]
    General(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("protocol error: {0}")]
    Protocol(String),

    #[error("pipe busy: {0}")]
    PipeBusy(String),

    #[error("connection timed out")]
    Timeout,

    #[error("request too large: {size} bytes (max {max})")]
    RequestTooLarge { size: usize, max: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn _assert_send<T: Send>() {}
    fn _assert_sync<T: Sync>() {}

    #[test]
    fn error_is_send_and_sync() {
        _assert_send::<IpcError>();
        _assert_sync::<IpcError>();
    }

    #[test]
    fn protocol_error_display() {
        let err = IpcError::Protocol("invalid JSON".to_owned());
        assert_eq!(err.to_string(), "protocol error: invalid JSON");
    }

    #[test]
    fn pipe_busy_error_display() {
        let err = IpcError::PipeBusy(r"\\.\pipe\wmux".to_owned());
        assert_eq!(err.to_string(), r"pipe busy: \\.\pipe\wmux");
    }

    #[test]
    fn timeout_error_display() {
        let err = IpcError::Timeout;
        assert_eq!(err.to_string(), "connection timed out");
    }

    #[test]
    fn request_too_large_error_display() {
        let err = IpcError::RequestTooLarge {
            size: 2_000_000,
            max: 1_048_576,
        };
        assert_eq!(
            err.to_string(),
            "request too large: 2000000 bytes (max 1048576)"
        );
    }
}
