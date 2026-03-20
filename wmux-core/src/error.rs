use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("grid position out of bounds: row={row}, col={col}")]
    OutOfBounds { row: usize, col: usize },

    #[error("invalid scroll region: top={top}, bottom={bottom}")]
    InvalidScrollRegion { top: usize, bottom: usize },

    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

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
        _assert_send::<CoreError>();
        _assert_sync::<CoreError>();
    }

    #[test]
    fn core_error_messages() {
        let oob = CoreError::OutOfBounds { row: 5, col: 100 };
        let msg = oob.to_string();
        assert!(msg.contains("5"), "should contain row: {msg}");
        assert!(msg.contains("100"), "should contain col: {msg}");

        let scroll = CoreError::InvalidScrollRegion { top: 0, bottom: 24 };
        let msg = scroll.to_string();
        assert!(msg.contains("0"), "should contain top: {msg}");
        assert!(msg.contains("24"), "should contain bottom: {msg}");

        let config = CoreError::InvalidConfig("bad value".into());
        assert!(config.to_string().contains("bad value"));

        let io_err = CoreError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file gone",
        ));
        assert!(
            io_err.to_string().contains("file gone"),
            "Io should display inner error message"
        );
    }
}
