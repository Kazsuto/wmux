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

    // Compute pipe name and set environment variable BEFORE creating the tokio
    // runtime. std::env::set_var is not thread-safe — it must be called while
    // the process is still single-threaded.
    let ipc_pipe = pipe_name();
    // SAFETY: Single-threaded context — no other threads exist yet.
    // The tokio runtime (and its worker threads) is created below.
    unsafe {
        std::env::set_var("WMUX_SOCKET_PATH", &ipc_pipe);
    }

    // Create tokio runtime for PTY I/O and async tasks.
    // winit owns the main thread — tokio runs on background threads.
    let rt = tokio::runtime::Runtime::new().context("failed to create tokio runtime")?;
    let _guard = rt.enter();

    // Spawn AppState actor — owns all terminal/pane state.
    let (app_event_tx, app_event_rx) = tokio::sync::mpsc::channel(64);
    let (app_state, actor_handle) = AppStateHandle::spawn(app_event_tx);

    // Browser command channel: IPC handler → UI thread (bounded, 32 pending commands).
    let (browser_cmd_tx, browser_cmd_rx) = tokio::sync::mpsc::channel(32);

    // Start IPC server for CLI and AI agent access.
    let mut router = Router::new();
    router.register(
        "workspace",
        std::sync::Arc::new(wmux_ipc::handlers::workspace::WorkspaceHandler::new(
            app_state.clone(),
        )),
    );
    router.register(
        "surface",
        std::sync::Arc::new(wmux_ipc::handlers::surface::SurfaceHandler::new(
            app_state.clone(),
        )),
    );
    router.register(
        "browser",
        std::sync::Arc::new(wmux_ipc::handlers::browser::BrowserHandler::new(
            browser_cmd_tx,
        )),
    );
    router.register(
        "sidebar",
        std::sync::Arc::new(wmux_ipc::handlers::sidebar::SidebarHandler::new(
            app_state.clone(),
        )),
    );
    let router = std::sync::Arc::new(router);
    // WmuxOnly mode: only child processes of wmux can connect (most secure default).
    // Auth secret is not needed for WmuxOnly — PID ancestry check is used instead.
    let (ipc_server, _ipc_handle) = IpcServer::new(ipc_pipe.clone(), SecurityMode::WmuxOnly, None);
    rt.spawn(async move {
        if let Err(e) = ipc_server.run(router).await {
            tracing::error!(error = %e, "IPC server failed");
        }
    });
    tracing::info!(pipe = %ipc_pipe, "IPC server started");

    // Attempt to load previous session for restore during UI init.
    let session = rt.block_on(async {
        match wmux_core::load_session().await {
            Ok(Some(session)) => {
                tracing::info!(
                    workspace_count = session.workspaces.len(),
                    "session loaded, will restore on first frame"
                );
                Some(session)
            }
            Ok(None) => {
                tracing::debug!("no session to restore, starting fresh");
                None
            }
            Err(e) => {
                tracing::warn!(error = %e, "session restore failed, starting fresh");
                None
            }
        }
    });

    App::run(
        rt.handle().clone(),
        app_state.clone(),
        app_event_rx,
        session,
        browser_cmd_rx,
    )
    .context("application terminated with error")?;

    // Graceful shutdown: signal the actor to stop, then wait for it to
    // complete its final session save before force-exiting the process.
    app_state.shutdown();
    let _ = rt.block_on(actor_handle);

    // Force-exit the process. The tokio runtime has spawn_blocking tasks
    // (PTY reader, exit watcher) that block on synchronous I/O (File::read,
    // child.wait). These cannot be cancelled and would hang during normal
    // drop-based cleanup because Windows does not kill child processes when
    // the parent exits. std::process::exit terminates immediately — the OS
    // cleans up all handles, pipes, and child processes.
    // This is the standard pattern used by Alacritty, WezTerm, and other
    // terminal emulators.
    std::process::exit(0);
}
