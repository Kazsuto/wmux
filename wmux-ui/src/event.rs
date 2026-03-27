/// Custom events sent from background tasks to the winit event loop.
///
/// Used by the AppState actor (via the event forwarding task) and the PTY
/// bridge to wake the event loop for rendering, process exit, and notifications.
#[derive(Debug)]
pub enum WmuxEvent {
    /// A pane has new content — request a redraw to fetch fresh render data.
    PtyOutput,
    /// The shell process in a specific pane exited.
    PtyExited {
        pane_id: wmux_core::PaneId,
        success: bool,
    },
    /// A notification should be shown as a Windows Toast.
    ShowToast(Box<wmux_core::Notification>),
    /// Focus moved to a new pane (e.g., after a split). UI must update `focused_pane`.
    FocusPane(wmux_core::PaneId),
    /// A browser command forwarded from the IPC handler to the UI thread.
    /// Processed on the STA thread where BrowserManager lives.
    BrowserCommand(wmux_core::BrowserCommand),
    /// Deferred WebView2 panel creation — sent after the actor registers the surface.
    /// Must be processed in user_event() where the Win32 message pump is active.
    CreateBrowserPanel {
        surface_id: wmux_core::SurfaceId,
        url: String,
    },
}
