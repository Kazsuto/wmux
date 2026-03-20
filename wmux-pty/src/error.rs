use thiserror::Error;

#[derive(Debug, Error)]
pub enum PtyError {
    #[error("pty spawn failed")]
    SpawnFailed(#[source] Box<dyn std::error::Error + Send + Sync>),

    #[error("shell not found: {0}")]
    ShellNotFound(String),

    #[error("pty resize failed")]
    ResizeFailed(#[source] Box<dyn std::error::Error + Send + Sync>),

    #[error("clone reader failed")]
    CloneReaderFailed(#[source] Box<dyn std::error::Error + Send + Sync>),

    #[error("pty actor channel closed")]
    ChannelClosed,

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
        _assert_send::<PtyError>();
        _assert_sync::<PtyError>();
    }
}
