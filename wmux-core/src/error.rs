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
}
