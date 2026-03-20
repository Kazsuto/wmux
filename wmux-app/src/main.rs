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

    App::run().context("application terminated with error")?;

    Ok(())
}
