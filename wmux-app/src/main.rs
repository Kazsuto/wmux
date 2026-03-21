mod updater;

use anyhow::{Context, Result};
use tracing_subscriber::EnvFilter;
use wmux_ui::App;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    tracing::info!("wmux starting...");

    // Create tokio runtime for PTY I/O and async tasks.
    // winit owns the main thread — tokio runs on background threads.
    let rt = tokio::runtime::Runtime::new().context("failed to create tokio runtime")?;
    let _guard = rt.enter();

    App::run(rt.handle().clone()).context("application terminated with error")?;

    Ok(())
}
