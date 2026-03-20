use thiserror::Error;

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("failed to create wgpu surface")]
    CreateSurface(#[from] wgpu::CreateSurfaceError),

    #[error("failed to request GPU adapter")]
    RequestAdapter(#[from] wgpu::RequestAdapterError),

    #[error("failed to request GPU device")]
    RequestDevice(#[from] wgpu::RequestDeviceError),

    #[error("text preparation failed")]
    TextPrepare(#[from] glyphon::PrepareError),

    #[error("text render failed")]
    GlyphonRender(#[from] glyphon::RenderError),

    #[error("GPU surface reported no supported texture formats")]
    NoSupportedFormats,

    #[error("GPU surface reported no supported alpha modes")]
    NoSupportedAlphaModes,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn _assert_send<T: Send>() {}
    fn _assert_sync<T: Sync>() {}

    #[test]
    fn error_is_send_and_sync() {
        _assert_send::<RenderError>();
        _assert_sync::<RenderError>();
    }
}
