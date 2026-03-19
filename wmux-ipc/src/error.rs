use thiserror::Error;

#[derive(Debug, Error)]
pub enum IpcError {
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
        _assert_send::<IpcError>();
        _assert_sync::<IpcError>();
    }
}
