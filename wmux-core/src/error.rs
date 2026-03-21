use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("grid position out of bounds: row={row}, col={col}")]
    OutOfBounds { row: usize, col: usize },

    #[error("invalid scroll region: top={top}, bottom={bottom}")]
    InvalidScrollRegion { top: usize, bottom: usize },

    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("pane not found: {pane_id}")]
    PaneNotFound { pane_id: String },

    #[error("surface not found: {surface_id}")]
    SurfaceNotFound { surface_id: String },

    #[error("workspace not found: {workspace_id}")]
    WorkspaceNotFound { workspace_id: String },

    #[error("cannot close the last workspace")]
    CannotCloseLastWorkspace,

    #[error("cannot split pane: {0}")]
    CannotSplit(String),

    #[error("cannot close pane: {0}")]
    CannotClose(String),

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

        let pane_not_found = CoreError::PaneNotFound {
            pane_id: "abc-123".into(),
        };
        let msg = pane_not_found.to_string();
        assert!(msg.contains("abc-123"), "should contain pane_id: {msg}");
        assert!(
            msg.contains("pane not found"),
            "should contain 'pane not found': {msg}"
        );

        let cannot_split = CoreError::CannotSplit("already at minimum size".into());
        let msg = cannot_split.to_string();
        assert!(
            msg.contains("already at minimum size"),
            "should contain reason: {msg}"
        );
        assert!(
            msg.contains("cannot split pane"),
            "should contain prefix: {msg}"
        );
    }
}
