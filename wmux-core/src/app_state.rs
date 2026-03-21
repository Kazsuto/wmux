use std::fmt;

use tokio::sync::{mpsc, oneshot};

use crate::cell::Row;
use crate::error::CoreError;
use crate::event::TerminalEvent;
use crate::grid::Grid;
use crate::mode::TerminalMode;
use crate::notification::{Notification, NotificationEvent, NotificationSource, NotificationStore};
use crate::pane_registry::{PaneRegistry, PaneState};
use crate::pane_tree::PaneTree;
use crate::rect::Rect;
use crate::surface::SplitDirection;
use crate::types::PaneId;

/// Channel capacity for the main command channel (ADR-0008).
const CMD_CHANNEL_CAPACITY: usize = 256;

// ─── Commands ────────────────────────────────────────────────────────────────

/// Commands sent to the AppState actor. All state mutations go through here.
pub enum AppCommand {
    /// Register a pre-created pane with its terminal and PTY bridge channels.
    RegisterPane {
        pane_id: PaneId,
        state: Box<PaneState>,
    },

    /// Close and remove a pane.
    ClosePane { pane_id: PaneId },

    /// Process raw PTY output bytes into a pane's terminal.
    ProcessPtyOutput { pane_id: PaneId, data: Vec<u8> },

    /// Send input bytes to a pane's PTY.
    SendInput { pane_id: PaneId, data: Vec<u8> },

    /// Resize a pane's terminal and PTY.
    ResizePane {
        pane_id: PaneId,
        cols: u16,
        rows: u16,
    },

    /// Set focus to a specific pane.
    FocusPane { pane_id: PaneId },

    /// Request render data for a pane. Returns a snapshot via oneshot.
    GetRenderData {
        pane_id: PaneId,
        reply: oneshot::Sender<Option<PaneRenderData>>,
    },

    /// Scroll the viewport of a pane (positive = up, negative = down).
    ScrollViewport { pane_id: PaneId, delta: i32 },

    /// Reset the viewport to bottom (live terminal).
    ResetViewport { pane_id: PaneId },

    /// Mark a pane's process as exited.
    MarkExited { pane_id: PaneId, success: bool },

    /// Extract selected text from a pane's grid using a Selection.
    ExtractSelection {
        pane_id: PaneId,
        selection: crate::selection::Selection,
        reply: oneshot::Sender<String>,
    },

    /// Split a pane, creating a new pane. Returns the new PaneId.
    SplitPane {
        pane_id: PaneId,
        direction: SplitDirection,
        reply: oneshot::Sender<Result<PaneId, CoreError>>,
    },

    /// Swap two panes in the layout tree.
    SwapPanes { a: PaneId, b: PaneId },

    /// Get the current layout as pane-rect pairs.
    GetLayout {
        viewport: Rect,
        reply: oneshot::Sender<Vec<(PaneId, Rect)>>,
    },

    /// Shut down the actor.
    Shutdown,
}

impl fmt::Debug for AppCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RegisterPane { pane_id, .. } => f
                .debug_struct("RegisterPane")
                .field("pane_id", pane_id)
                .finish_non_exhaustive(),
            Self::ClosePane { pane_id } => f
                .debug_struct("ClosePane")
                .field("pane_id", pane_id)
                .finish(),
            Self::ProcessPtyOutput { pane_id, data } => f
                .debug_struct("ProcessPtyOutput")
                .field("pane_id", pane_id)
                .field("len", &data.len())
                .finish(),
            Self::SendInput { pane_id, data } => f
                .debug_struct("SendInput")
                .field("pane_id", pane_id)
                .field("len", &data.len())
                .finish(),
            Self::ResizePane {
                pane_id,
                cols,
                rows,
            } => f
                .debug_struct("ResizePane")
                .field("pane_id", pane_id)
                .field("cols", cols)
                .field("rows", rows)
                .finish(),
            Self::FocusPane { pane_id } => f
                .debug_struct("FocusPane")
                .field("pane_id", pane_id)
                .finish(),
            Self::GetRenderData { pane_id, .. } => f
                .debug_struct("GetRenderData")
                .field("pane_id", pane_id)
                .finish_non_exhaustive(),
            Self::ScrollViewport { pane_id, delta } => f
                .debug_struct("ScrollViewport")
                .field("pane_id", pane_id)
                .field("delta", delta)
                .finish(),
            Self::ResetViewport { pane_id } => f
                .debug_struct("ResetViewport")
                .field("pane_id", pane_id)
                .finish(),
            Self::MarkExited { pane_id, success } => f
                .debug_struct("MarkExited")
                .field("pane_id", pane_id)
                .field("success", success)
                .finish(),
            Self::ExtractSelection { pane_id, .. } => f
                .debug_struct("ExtractSelection")
                .field("pane_id", pane_id)
                .finish_non_exhaustive(),
            Self::SplitPane {
                pane_id, direction, ..
            } => f
                .debug_struct("SplitPane")
                .field("pane_id", pane_id)
                .field("direction", direction)
                .finish_non_exhaustive(),
            Self::SwapPanes { a, b } => f
                .debug_struct("SwapPanes")
                .field("a", a)
                .field("b", b)
                .finish(),
            Self::GetLayout { viewport, .. } => f
                .debug_struct("GetLayout")
                .field("viewport", viewport)
                .finish_non_exhaustive(),
            Self::Shutdown => write!(f, "Shutdown"),
        }
    }
}

// ─── Events ──────────────────────────────────────────────────────────────────

/// Events emitted by the actor to notify the UI of state changes.
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// A pane has new content and needs a redraw.
    PaneNeedsRedraw(PaneId),

    /// A notification was added to the store.
    NotificationAdded {
        notification: Box<Notification>,
        suppressed: bool,
    },

    /// A pane's process exited.
    PaneExited { pane_id: PaneId, success: bool },
}

// ─── Render Snapshot ─────────────────────────────────────────────────────────

/// Render data snapshot for a single pane.
///
/// Contains a cloned grid and the scrollback rows visible in the current
/// viewport. The UI thread uses this to update the `TerminalRenderer`
/// without holding any reference to actor-owned state.
pub struct PaneRenderData {
    /// Cloned grid (cells + cursor). Dirty flags are in `dirty_rows`.
    pub grid: Grid,
    /// Indices of rows that changed since last snapshot.
    pub dirty_rows: Vec<u16>,
    /// Viewport offset from bottom (0 = live terminal).
    pub viewport_offset: usize,
    /// Total scrollback length (for scroll calculations).
    pub scrollback_len: usize,
    /// Scrollback rows visible in the current viewport (when scrolled up).
    /// Index 0 = topmost visible scrollback row.
    pub scrollback_visible_rows: Vec<Row>,
    /// Terminal mode flags (MOUSE_REPORTING, BRACKETED_PASTE, etc.).
    pub modes: TerminalMode,
    /// Whether the shell process has exited.
    pub process_exited: bool,
}

impl fmt::Debug for PaneRenderData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PaneRenderData")
            .field("grid", &self.grid)
            .field("dirty_rows_count", &self.dirty_rows.len())
            .field("viewport_offset", &self.viewport_offset)
            .field("scrollback_len", &self.scrollback_len)
            .field(
                "scrollback_visible_count",
                &self.scrollback_visible_rows.len(),
            )
            .field("modes", &self.modes)
            .field("process_exited", &self.process_exited)
            .finish_non_exhaustive()
    }
}

// ─── Handle ──────────────────────────────────────────────────────────────────

/// Client handle for sending commands to the AppState actor.
///
/// Cloneable — multiple parts of the system can hold a handle (UI, IPC, etc.).
#[derive(Debug, Clone)]
pub struct AppStateHandle {
    cmd_tx: mpsc::Sender<AppCommand>,
}

impl AppStateHandle {
    /// Register a pre-created pane. Fire-and-forget.
    pub fn register_pane(&self, pane_id: PaneId, state: PaneState) {
        if self
            .cmd_tx
            .try_send(AppCommand::RegisterPane {
                pane_id,
                state: Box::new(state),
            })
            .is_err()
        {
            tracing::warn!(pane_id = %pane_id, "command channel full, RegisterPane dropped");
        }
    }

    /// Process PTY output for a pane. Fire-and-forget.
    #[inline]
    pub fn process_pty_output(&self, pane_id: PaneId, data: Vec<u8>) {
        if self
            .cmd_tx
            .try_send(AppCommand::ProcessPtyOutput { pane_id, data })
            .is_err()
        {
            tracing::warn!(pane_id = %pane_id, "command channel full, ProcessPtyOutput dropped");
        }
    }

    /// Send input bytes to a pane's PTY. Fire-and-forget.
    #[inline]
    pub fn send_input(&self, pane_id: PaneId, data: Vec<u8>) {
        if self
            .cmd_tx
            .try_send(AppCommand::SendInput { pane_id, data })
            .is_err()
        {
            tracing::warn!(pane_id = %pane_id, "command channel full, SendInput dropped");
        }
    }

    /// Resize a pane's terminal and PTY. Fire-and-forget.
    pub fn resize_pane(&self, pane_id: PaneId, cols: u16, rows: u16) {
        if self
            .cmd_tx
            .try_send(AppCommand::ResizePane {
                pane_id,
                cols,
                rows,
            })
            .is_err()
        {
            tracing::warn!(pane_id = %pane_id, "command channel full, ResizePane dropped");
        }
    }

    /// Set focus to a pane. Fire-and-forget.
    pub fn focus_pane(&self, pane_id: PaneId) {
        let _ = self.cmd_tx.try_send(AppCommand::FocusPane { pane_id });
    }

    /// Scroll a pane's viewport. Fire-and-forget.
    #[inline]
    pub fn scroll_viewport(&self, pane_id: PaneId, delta: i32) {
        let _ = self
            .cmd_tx
            .try_send(AppCommand::ScrollViewport { pane_id, delta });
    }

    /// Reset a pane's viewport to bottom. Fire-and-forget.
    #[inline]
    pub fn reset_viewport(&self, pane_id: PaneId) {
        let _ = self.cmd_tx.try_send(AppCommand::ResetViewport { pane_id });
    }

    /// Mark a pane's process as exited. Fire-and-forget.
    pub fn mark_exited(&self, pane_id: PaneId, success: bool) {
        if self
            .cmd_tx
            .try_send(AppCommand::MarkExited { pane_id, success })
            .is_err()
        {
            tracing::warn!(pane_id = %pane_id, "command channel full, MarkExited dropped");
        }
    }

    /// Request render data for a pane. Blocks until the actor responds.
    pub async fn get_render_data(&self, pane_id: PaneId) -> Option<PaneRenderData> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(AppCommand::GetRenderData { pane_id, reply: tx })
            .await
            .ok()?;
        rx.await.ok()?
    }

    /// Extract selected text from a pane's grid. Blocks until the actor responds.
    pub async fn extract_selection(
        &self,
        pane_id: PaneId,
        selection: crate::selection::Selection,
    ) -> Option<String> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(AppCommand::ExtractSelection {
                pane_id,
                selection,
                reply: tx,
            })
            .await
            .ok()?;
        rx.await.ok()
    }

    /// Split a pane, creating a new pane in the given direction.
    /// Returns the new pane's ID.
    pub async fn split_pane(
        &self,
        pane_id: PaneId,
        direction: SplitDirection,
    ) -> Result<PaneId, CoreError> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(AppCommand::SplitPane {
                pane_id,
                direction,
                reply: tx,
            })
            .await
            .map_err(|_| CoreError::CannotSplit("actor shut down".to_string()))?;
        rx.await
            .map_err(|_| CoreError::CannotSplit("actor dropped reply".to_string()))?
    }

    /// Swap two panes in the layout tree. Fire-and-forget.
    pub fn swap_panes(&self, a: PaneId, b: PaneId) {
        if self
            .cmd_tx
            .try_send(AppCommand::SwapPanes { a, b })
            .is_err()
        {
            tracing::warn!("command channel full, SwapPanes dropped");
        }
    }

    /// Get the current layout as pane-rect pairs.
    pub async fn get_layout(&self, viewport: Rect) -> Vec<(PaneId, Rect)> {
        let (tx, rx) = oneshot::channel();
        if self
            .cmd_tx
            .send(AppCommand::GetLayout {
                viewport,
                reply: tx,
            })
            .await
            .is_err()
        {
            return Vec::new();
        }
        rx.await.unwrap_or_default()
    }

    /// Shut down the actor. Fire-and-forget.
    pub fn shutdown(&self) {
        let _ = self.cmd_tx.try_send(AppCommand::Shutdown);
    }

    /// Spawn the AppState actor on the tokio runtime.
    ///
    /// Returns the handle for sending commands and the event receiver
    /// for UI notifications.
    #[must_use]
    pub fn spawn(event_tx: mpsc::Sender<AppEvent>) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel(CMD_CHANNEL_CAPACITY);
        let actor = AppStateActor {
            registry: PaneRegistry::new(),
            notification_store: NotificationStore::new(),
            pane_tree: None,
            cmd_rx,
            event_tx,
        };
        tokio::spawn(actor.run());
        tracing::info!("AppState actor spawned");
        Self { cmd_tx }
    }
}

// ─── Actor ───────────────────────────────────────────────────────────────────

/// The AppState actor. Runs in a dedicated tokio task and owns all mutable
/// application state. All mutations go through `AppCommand` messages.
struct AppStateActor {
    registry: PaneRegistry,
    notification_store: NotificationStore,
    pane_tree: Option<PaneTree>,
    cmd_rx: mpsc::Receiver<AppCommand>,
    event_tx: mpsc::Sender<AppEvent>,
}

impl AppStateActor {
    async fn run(mut self) {
        tracing::info!("AppState actor loop started");
        while let Some(cmd) = self.cmd_rx.recv().await {
            match cmd {
                AppCommand::RegisterPane { pane_id, state } => {
                    self.registry.register(pane_id, *state);
                    // Initialize pane_tree on first pane registration.
                    // Subsequent panes must be added via SplitPane first, which
                    // creates a tree node and returns the new PaneId. The caller
                    // then calls RegisterPane with that ID to attach terminal state.
                    if self.pane_tree.is_none() {
                        self.pane_tree = Some(PaneTree::new(pane_id));
                    }
                }
                AppCommand::ClosePane { pane_id } => {
                    self.registry.remove(pane_id);
                    // Keep pane_tree in sync — remove the leaf and promote sibling.
                    if let Some(tree) = &mut self.pane_tree {
                        if let Err(e) = tree.close_pane(pane_id) {
                            tracing::warn!(error = %e, "failed to close pane in tree");
                        }
                    }
                }
                AppCommand::ProcessPtyOutput { pane_id, data } => {
                    self.handle_pty_output(pane_id, &data);
                }
                AppCommand::SendInput { pane_id, data } => {
                    self.handle_send_input(pane_id, data);
                }
                AppCommand::ResizePane {
                    pane_id,
                    cols,
                    rows,
                } => {
                    self.handle_resize(pane_id, cols, rows);
                }
                AppCommand::FocusPane { pane_id } => {
                    self.registry.set_focused(pane_id);
                }
                AppCommand::GetRenderData { pane_id, reply } => {
                    let data = self.build_render_data(pane_id);
                    let _ = reply.send(data);
                }
                AppCommand::ScrollViewport { pane_id, delta } => {
                    self.handle_scroll(pane_id, delta);
                }
                AppCommand::ResetViewport { pane_id } => {
                    if let Some(pane) = self.registry.get_mut(pane_id) {
                        pane.terminal.reset_viewport();
                    }
                }
                AppCommand::MarkExited { pane_id, success } => {
                    self.handle_exit(pane_id, success);
                }
                AppCommand::ExtractSelection {
                    pane_id,
                    selection,
                    reply,
                } => {
                    let text = self
                        .registry
                        .get(pane_id)
                        .map(|pane| {
                            selection.extract_text(pane.terminal.grid(), pane.terminal.scrollback())
                        })
                        .unwrap_or_default();
                    let _ = reply.send(text);
                }
                AppCommand::SplitPane {
                    pane_id,
                    direction,
                    reply,
                } => {
                    let result = if let Some(tree) = &mut self.pane_tree {
                        tree.split_pane(pane_id, direction)
                    } else {
                        Err(CoreError::PaneNotFound {
                            pane_id: pane_id.to_string(),
                        })
                    };
                    let _ = reply.send(result);
                }
                AppCommand::SwapPanes { a, b } => {
                    if let Some(tree) = &mut self.pane_tree {
                        if let Err(e) = tree.swap_panes(a, b) {
                            tracing::warn!(error = %e, "SwapPanes failed");
                        }
                    }
                }
                AppCommand::GetLayout { viewport, reply } => {
                    let layout = if let Some(tree) = &self.pane_tree {
                        tree.layout(viewport)
                    } else {
                        Vec::new()
                    };
                    let _ = reply.send(layout);
                }
                AppCommand::Shutdown => {
                    tracing::info!("AppState actor shutting down");
                    break;
                }
            }
        }
        tracing::info!("AppState actor loop ended");
    }

    fn handle_pty_output(&mut self, pane_id: PaneId, data: &[u8]) {
        let Some(pane) = self.registry.get_mut(pane_id) else {
            return;
        };

        // Process raw bytes through VTE parser → terminal state machine.
        pane.terminal.process(data);

        // Drain terminal events: forward PTY write-backs (DSR/DA1) and
        // collect notifications for the store.
        while let Ok(event) = pane.terminal_event_rx.try_recv() {
            match event {
                TerminalEvent::PtyWrite(bytes) => {
                    let _ = pane.pty_write_tx.try_send(bytes);
                }
                TerminalEvent::Notification { title, body, .. } => {
                    let (notif_id, event) = self.notification_store.add(
                        title,
                        body,
                        None, // subtitle
                        NotificationSource::Osc,
                        None, // workspace — TODO: track in L2_07
                        None, // surface — TODO: track in L2_07
                    );
                    if let NotificationEvent::Added { suppressed, .. } = &event {
                        if let Some(n) = self.notification_store.get(notif_id) {
                            let _ = self.event_tx.try_send(AppEvent::NotificationAdded {
                                notification: Box::new(n.clone()),
                                suppressed: *suppressed,
                            });
                        }
                    }
                }
                // CwdChanged, PromptMark — handled later (L2_14 sidebar metadata)
                _ => {}
            }
        }

        // Signal UI to redraw.
        let _ = self.event_tx.try_send(AppEvent::PaneNeedsRedraw(pane_id));
    }

    fn handle_send_input(&self, pane_id: PaneId, data: Vec<u8>) {
        if let Some(pane) = self.registry.get(pane_id) {
            if !pane.process_exited && pane.pty_write_tx.try_send(data).is_err() {
                tracing::warn!(pane_id = %pane_id, "PTY write channel full, input dropped");
            }
        }
    }

    fn handle_resize(&mut self, pane_id: PaneId, cols: u16, rows: u16) {
        if let Some(pane) = self.registry.get_mut(pane_id) {
            let old_cols = pane.terminal.cols();
            let old_rows = pane.terminal.rows();
            if cols != old_cols || rows != old_rows {
                pane.terminal.resize(cols, rows);
                if pane.pty_resize_tx.try_send((rows, cols)).is_err() {
                    tracing::warn!(pane_id = %pane_id, "PTY resize channel full, resize dropped");
                }
                tracing::debug!(
                    pane_id = %pane_id,
                    old_cols, old_rows, cols, rows,
                    "pane resized",
                );
            }
        }
    }

    fn handle_scroll(&mut self, pane_id: PaneId, delta: i32) {
        if let Some(pane) = self.registry.get_mut(pane_id) {
            if delta > 0 {
                pane.terminal.scroll_viewport_up(delta as usize);
            } else if delta < 0 {
                pane.terminal.scroll_viewport_down((-delta) as usize);
            }
        }
    }

    fn handle_exit(&mut self, pane_id: PaneId, success: bool) {
        if let Some(pane) = self.registry.get_mut(pane_id) {
            pane.process_exited = true;
            let msg = if success {
                "\r\n[Process exited]\r\n"
            } else {
                "\r\n[Process exited with error]\r\n"
            };
            pane.terminal.process(msg.as_bytes());
            tracing::info!(pane_id = %pane_id, success, "pane process exited");
        }
        let _ = self
            .event_tx
            .try_send(AppEvent::PaneExited { pane_id, success });
    }

    fn build_render_data(&mut self, pane_id: PaneId) -> Option<PaneRenderData> {
        let pane = self.registry.get_mut(pane_id)?;

        // Take dirty rows from the actor's grid (resets flags), then clone.
        let dirty_rows = pane.terminal.grid_mut().take_dirty_rows();
        let grid = pane.terminal.grid().clone();
        let modes = pane.terminal.modes();
        let viewport_offset = pane.terminal.viewport_offset();
        let scrollback = pane.terminal.scrollback();
        let scrollback_len = scrollback.len();

        // Extract scrollback rows visible in the current viewport.
        let scrollback_visible_rows = if viewport_offset > 0 {
            let rows = pane.terminal.rows() as usize;
            let sb_rows_shown = viewport_offset.min(rows);
            let start_idx = scrollback_len.saturating_sub(viewport_offset);
            let mut visible = Vec::with_capacity(sb_rows_shown);
            for i in 0..sb_rows_shown {
                if let Some(row) = scrollback.get_row(start_idx + i) {
                    visible.push(row.clone());
                }
            }
            visible
        } else {
            Vec::new()
        };

        Some(PaneRenderData {
            grid,
            dirty_rows,
            viewport_offset,
            scrollback_len,
            scrollback_visible_rows,
            modes,
            process_exited: pane.process_exited,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    fn make_pane_state(cols: u16, rows: u16) -> PaneState {
        let (event_tx, event_rx) = mpsc::channel(16);
        let mut terminal = crate::terminal::Terminal::new(cols, rows);
        terminal.set_event_sender(event_tx);
        let (write_tx, _write_rx) = mpsc::channel(16);
        let (resize_tx, _resize_rx) = mpsc::channel(4);
        PaneState {
            terminal,
            terminal_event_rx: event_rx,
            pty_write_tx: write_tx,
            pty_resize_tx: resize_tx,
            process_exited: false,
        }
    }

    #[tokio::test]
    async fn spawn_and_register_pane() {
        let (event_tx, mut event_rx) = mpsc::channel(64);
        let handle = AppStateHandle::spawn(event_tx);

        let pane_id = PaneId::new();
        handle.register_pane(pane_id, make_pane_state(80, 24));

        // Give actor time to process.
        tokio::task::yield_now().await;

        // Should be able to get render data.
        let data = handle.get_render_data(pane_id).await;
        assert!(data.is_some());

        let data = data.unwrap();
        assert_eq!(data.grid.cols(), 80);
        assert_eq!(data.grid.rows(), 24);
        assert!(!data.process_exited);

        // Cleanup: events may have been emitted.
        event_rx.try_recv().ok();

        handle.shutdown();
    }

    #[tokio::test]
    async fn process_pty_output_triggers_redraw() {
        let (event_tx, mut event_rx) = mpsc::channel(64);
        let handle = AppStateHandle::spawn(event_tx);

        let pane_id = PaneId::new();
        handle.register_pane(pane_id, make_pane_state(80, 24));
        tokio::task::yield_now().await;

        handle.process_pty_output(pane_id, b"Hello".to_vec());
        tokio::task::yield_now().await;

        // Should receive PaneNeedsRedraw event.
        let mut got_redraw = false;
        while let Ok(event) = event_rx.try_recv() {
            if matches!(event, AppEvent::PaneNeedsRedraw(id) if id == pane_id) {
                got_redraw = true;
            }
        }
        assert!(got_redraw);

        handle.shutdown();
    }

    #[tokio::test]
    async fn get_render_data_nonexistent_pane_returns_none() {
        let (event_tx, _event_rx) = mpsc::channel(64);
        let handle = AppStateHandle::spawn(event_tx);

        let data = handle.get_render_data(PaneId::new()).await;
        assert!(data.is_none());

        handle.shutdown();
    }

    #[tokio::test]
    async fn mark_exited_sets_flag() {
        let (event_tx, mut event_rx) = mpsc::channel(64);
        let handle = AppStateHandle::spawn(event_tx);

        let pane_id = PaneId::new();
        handle.register_pane(pane_id, make_pane_state(80, 24));
        tokio::task::yield_now().await;

        handle.mark_exited(pane_id, true);
        tokio::task::yield_now().await;

        let data = handle.get_render_data(pane_id).await.unwrap();
        assert!(data.process_exited);

        // Should receive PaneExited event.
        let mut got_exit = false;
        while let Ok(event) = event_rx.try_recv() {
            if matches!(&event, AppEvent::PaneExited { pane_id: id, success: true } if *id == pane_id)
            {
                got_exit = true;
            }
        }
        assert!(got_exit);

        handle.shutdown();
    }

    #[tokio::test]
    async fn shutdown_stops_actor() {
        let (event_tx, _event_rx) = mpsc::channel(64);
        let handle = AppStateHandle::spawn(event_tx);
        handle.shutdown();
        tokio::task::yield_now().await;

        // After shutdown, commands should fail silently.
        let data = handle.get_render_data(PaneId::new()).await;
        assert!(data.is_none());
    }
}
