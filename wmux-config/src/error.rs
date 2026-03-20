use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("{0}")]
    General(String),

    #[error("parse error on line {line}: {message}")]
    ParseError { line: usize, message: String },

    #[error("invalid value for key '{key}': expected {expected}, got '{got}'")]
    InvalidValue {
        key: String,
        expected: String,
        got: String,
    },

    #[error("config directory not found")]
    ConfigDirNotFound,

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
        _assert_send::<ConfigError>();
        _assert_sync::<ConfigError>();
    }
}
