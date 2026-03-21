/// Custom events sent from background tasks to the winit event loop.
///
/// Used by the PTY bridge task to wake the event loop when
/// terminal output arrives or the child process exits.
#[derive(Debug, Clone)]
pub enum WmuxEvent {
    /// PTY output is available — drain the output channel and rerender.
    PtyOutput,
    /// The shell process exited.
    PtyExited { success: bool },
}
