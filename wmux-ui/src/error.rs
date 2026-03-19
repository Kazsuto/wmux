use thiserror::Error;

#[derive(Debug, Error)]
pub enum UiError {
    #[error(transparent)]
    Render(#[from] wmux_render::RenderError),

    #[error("event loop error")]
    EventLoop(#[from] winit::error::EventLoopError),

    #[error("failed to acquire surface texture")]
    Surface(#[from] wgpu::SurfaceError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn _assert_send<T: Send>() {}
    fn _assert_sync<T: Sync>() {}

    #[test]
    fn error_is_send_and_sync() {
        _assert_send::<UiError>();
        _assert_sync::<UiError>();
    }
}
