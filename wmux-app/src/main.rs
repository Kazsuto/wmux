mod updater;

use anyhow::{Context, Result};
use tracing_subscriber::EnvFilter;
use wmux_core::AppStateHandle;
use wmux_ipc::{pipe_name, IpcServer, Router, SecurityMode};
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

    // Spawn AppState actor — owns all terminal/pane state.
    let (app_event_tx, app_event_rx) = tokio::sync::mpsc::channel(64);
    let app_state = AppStateHandle::spawn(app_event_tx);

    // Start IPC server for CLI and AI agent access.
    let ipc_pipe = pipe_name();
    let router = std::sync::Arc::new(Router::new());
    // WmuxOnly mode: only child processes of wmux can connect (most secure default).
    // Auth secret is not needed for WmuxOnly — PID ancestry check is used instead.
    let (ipc_server, _ipc_handle) = IpcServer::new(ipc_pipe.clone(), SecurityMode::WmuxOnly, None);
    rt.spawn(async move {
        if let Err(e) = ipc_server.run(router).await {
            tracing::error!(error = %e, "IPC server failed");
        }
    });
    // SAFETY: No other thread is reading WMUX_SOCKET_PATH at this point
    // during startup initialization. This is set before any child processes
    // are spawned that would inherit the environment.
    unsafe {
        std::env::set_var("WMUX_SOCKET_PATH", &ipc_pipe);
    }
    tracing::info!(pipe = %ipc_pipe, "IPC server started");

    App::run(rt.handle().clone(), app_state, app_event_rx)
        .context("application terminated with error")?;

    Ok(())
}
