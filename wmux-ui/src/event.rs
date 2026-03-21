/// Custom events sent from background tasks to the winit event loop.
///
/// Used by the AppState actor (via the event forwarding task) and the PTY
/// bridge to wake the event loop for rendering, process exit, and notifications.
#[derive(Debug, Clone)]
pub enum WmuxEvent {
    /// A pane has new content — request a redraw to fetch fresh render data.
    PtyOutput,
    /// The shell process exited.
    PtyExited { success: bool },
    /// A notification should be shown as a Windows Toast.
    ShowToast(Box<wmux_core::Notification>),
}
