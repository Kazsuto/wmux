use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::error::CoreError;
use crate::metadata_store::{LogEntry, MetadataSnapshot, StatusEntry};
use crate::rect::Rect;
use crate::surface::SplitDirection;
use crate::types::{PaneId, SurfaceId, WorkspaceId};

use super::{
    AppCommand, AppEvent, FocusDirection, PaneRenderData, PaneSurfaceInfo, WorkspaceSnapshot,
};
use crate::pane_registry::PaneState;

// ─── Handle ──────────────────────────────────────────────────────────────────

/// Client handle for sending commands to the AppState actor.
///
/// Cloneable — multiple parts of the system can hold a handle (UI, IPC, etc.).
#[derive(Debug, Clone)]
pub struct AppStateHandle {
    pub(super) cmd_tx: mpsc::Sender<AppCommand>,
}

impl AppStateHandle {
    pub(super) fn new(cmd_tx: mpsc::Sender<AppCommand>) -> Self {
        Self { cmd_tx }
    }

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

    /// Close and remove a pane. Fire-and-forget.
    pub fn close_pane(&self, pane_id: PaneId) {
        if self
            .cmd_tx
            .try_send(AppCommand::ClosePane { pane_id })
            .is_err()
        {
            tracing::warn!(pane_id = %pane_id, "command channel full, ClosePane dropped");
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

    /// Set the initial CWD for a pane (used during session restore).
    /// Fire-and-forget.
    pub fn set_pane_initial_cwd(&self, pane_id: PaneId, cwd: std::path::PathBuf) {
        let _ = self.cmd_tx.try_send(AppCommand::SetPaneInitialCwd {
            pane_id,
            initial_cwd: cwd,
        });
    }

    /// Request render data for a pane. Blocks until the actor responds.
    pub async fn get_render_data(&self, pane_id: PaneId) -> Option<PaneRenderData> {
        let (tx, rx) = tokio::sync::oneshot::channel();
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
        let (tx, rx) = tokio::sync::oneshot::channel();
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
        let (tx, rx) = tokio::sync::oneshot::channel();
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

    /// Get the current layout as pane-rect pairs with divider metadata.
    pub async fn get_layout(
        &self,
        viewport: Rect,
    ) -> (Vec<(PaneId, Rect)>, Vec<crate::pane_tree::LayoutDivider>) {
        let (tx, rx) = tokio::sync::oneshot::channel();
        if self
            .cmd_tx
            .send(AppCommand::GetLayout {
                viewport,
                reply: tx,
            })
            .await
            .is_err()
        {
            return (Vec::new(), Vec::new());
        }
        rx.await.unwrap_or_default()
    }

    /// Toggle zoom on a pane. Fire-and-forget.
    pub fn toggle_zoom(&self, pane_id: PaneId) {
        if self
            .cmd_tx
            .try_send(AppCommand::ToggleZoom { pane_id })
            .is_err()
        {
            tracing::warn!(pane_id = %pane_id, "command channel full, ToggleZoom dropped");
        }
    }

    /// Move focus to the adjacent pane in the given direction. Fire-and-forget.
    pub fn navigate_focus(&self, direction: FocusDirection) {
        if self
            .cmd_tx
            .try_send(AppCommand::NavigateFocus { direction })
            .is_err()
        {
            tracing::warn!("command channel full, NavigateFocus dropped");
        }
    }

    // ─── Workspace handle methods ─────────────────────────────────────────────

    /// Create a new workspace with the given name. Returns the new workspace's ID.
    pub async fn create_workspace(&self, name: String) -> Option<WorkspaceId> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.cmd_tx
            .send(AppCommand::CreateWorkspace { name, reply: tx })
            .await
            .ok()?;
        rx.await.ok()
    }

    /// Switch the active workspace by 0-based index. Fire-and-forget.
    pub fn switch_workspace(&self, index: usize) {
        let _ = self.cmd_tx.try_send(AppCommand::SwitchWorkspace { index });
    }

    /// Close a workspace by ID. Fire-and-forget.
    pub fn close_workspace(&self, id: WorkspaceId) {
        let _ = self.cmd_tx.try_send(AppCommand::CloseWorkspace { id });
    }

    /// Rename a workspace by ID. Fire-and-forget.
    pub fn rename_workspace(&self, id: WorkspaceId, name: String) {
        let _ = self
            .cmd_tx
            .try_send(AppCommand::RenameWorkspace { id, name });
    }

    /// Reorder a workspace from one index to another. Fire-and-forget.
    pub fn reorder_workspace(&self, from: usize, to: usize) {
        let _ = self
            .cmd_tx
            .try_send(AppCommand::ReorderWorkspace { from, to });
    }

    // ─── Surface handle methods ────────────────────────────────────────────────

    /// Create a new surface in a pane. Returns the new surface's ID.
    ///
    /// `backing_pane_id` must already be registered via `register_pane`.
    /// It will become a "hidden pane" backing this surface's Terminal and PTY.
    pub async fn create_surface(
        &self,
        pane_id: PaneId,
        backing_pane_id: PaneId,
    ) -> Result<SurfaceId, CoreError> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.cmd_tx
            .send(AppCommand::CreateSurface {
                pane_id,
                backing_pane_id,
                reply: tx,
            })
            .await
            .map_err(|_| CoreError::PaneNotFound {
                pane_id: pane_id.to_string(),
            })?;
        rx.await.map_err(|_| CoreError::PaneNotFound {
            pane_id: pane_id.to_string(),
        })?
    }

    /// Create a new browser surface in a pane. Returns the SurfaceId.
    ///
    /// Like `create_surface` but creates a `PanelKind::Browser` surface.
    pub async fn create_browser_surface(
        &self,
        pane_id: PaneId,
        backing_pane_id: PaneId,
    ) -> Result<SurfaceId, CoreError> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.cmd_tx
            .send(AppCommand::CreateBrowserSurface {
                pane_id,
                backing_pane_id,
                reply: tx,
            })
            .await
            .map_err(|_| CoreError::PaneNotFound {
                pane_id: pane_id.to_string(),
            })?;
        rx.await.map_err(|_| CoreError::PaneNotFound {
            pane_id: pane_id.to_string(),
        })?
    }

    /// Close a surface in a pane. Fire-and-forget.
    pub fn close_surface(&self, pane_id: PaneId, surface_id: SurfaceId) {
        if self
            .cmd_tx
            .try_send(AppCommand::CloseSurface {
                pane_id,
                surface_id,
            })
            .is_err()
        {
            tracing::warn!(
                pane_id = %pane_id,
                surface_id = %surface_id,
                "command channel full, CloseSurface dropped"
            );
        }
    }

    /// Rename a surface (tab) in a pane. Fire-and-forget.
    pub fn rename_surface(&self, pane_id: PaneId, surface_id: SurfaceId, name: String) {
        if self
            .cmd_tx
            .try_send(AppCommand::RenameSurface {
                pane_id,
                surface_id,
                name,
            })
            .is_err()
        {
            tracing::warn!(
                pane_id = %pane_id,
                surface_id = %surface_id,
                "command channel full, RenameSurface dropped"
            );
        }
    }

    /// Cycle the active surface in a pane. Fire-and-forget.
    pub fn cycle_surface(&self, pane_id: PaneId, forward: bool) {
        if self
            .cmd_tx
            .try_send(AppCommand::CycleSurface { pane_id, forward })
            .is_err()
        {
            tracing::warn!(pane_id = %pane_id, "command channel full, CycleSurface dropped");
        }
    }

    // ─── IPC Query handle methods ────────────────────────────────────────

    /// Get the currently focused pane ID.
    pub async fn get_focused_pane_id(&self) -> Option<PaneId> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.cmd_tx
            .send(AppCommand::GetFocusedPaneId { reply: tx })
            .await
            .ok()?;
        rx.await.ok()?
    }

    /// Find which pane contains a given surface.
    pub async fn find_pane_for_surface(&self, surface_id: SurfaceId) -> Option<PaneId> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.cmd_tx
            .send(AppCommand::FindPaneForSurface {
                surface_id,
                reply: tx,
            })
            .await
            .ok()?;
        rx.await.ok()?
    }

    /// Read terminal text from a pane's grid and scrollback.
    pub async fn read_text(
        &self,
        pane_id: PaneId,
        start: Option<i32>,
        end: Option<i32>,
    ) -> Option<String> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.cmd_tx
            .send(AppCommand::ReadText {
                pane_id,
                start,
                end,
                reply: tx,
            })
            .await
            .ok()?;
        rx.await.ok()
    }

    /// List all workspaces.
    pub async fn list_workspaces(&self) -> Vec<WorkspaceSnapshot> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        if self
            .cmd_tx
            .send(AppCommand::ListWorkspaces { reply: tx })
            .await
            .is_err()
        {
            return Vec::new();
        }
        rx.await.unwrap_or_default()
    }

    /// Get the current workspace info.
    pub async fn get_current_workspace(&self) -> Option<WorkspaceSnapshot> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.cmd_tx
            .send(AppCommand::GetCurrentWorkspace { reply: tx })
            .await
            .ok()?;
        rx.await.ok()
    }

    /// Switch workspace by ID. Returns true if found.
    pub async fn select_workspace_by_id(&self, id: WorkspaceId) -> bool {
        let (tx, rx) = tokio::sync::oneshot::channel();
        if self
            .cmd_tx
            .send(AppCommand::SelectWorkspaceById { id, reply: tx })
            .await
            .is_err()
        {
            return false;
        }
        rx.await.unwrap_or(false)
    }

    /// List all surfaces in a workspace (or active workspace if None).
    pub async fn list_surfaces(&self, workspace_id: Option<WorkspaceId>) -> Vec<PaneSurfaceInfo> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        if self
            .cmd_tx
            .send(AppCommand::ListSurfaces {
                workspace_id,
                reply: tx,
            })
            .await
            .is_err()
        {
            return Vec::new();
        }
        rx.await.unwrap_or_default()
    }

    /// Focus a specific surface within a pane. Fire-and-forget.
    pub fn focus_surface(&self, pane_id: PaneId, surface_id: SurfaceId) {
        let _ = self.cmd_tx.try_send(AppCommand::FocusSurface {
            pane_id,
            surface_id,
        });
    }

    /// Switch to a surface by index within a pane. Fire-and-forget.
    pub fn cycle_surface_to_index(&self, pane_id: PaneId, index: usize) {
        let _ = self
            .cmd_tx
            .try_send(AppCommand::SwitchSurfaceIndex { pane_id, index });
    }

    /// Reorder surfaces within a pane (drag-and-drop). Fire-and-forget.
    pub fn reorder_surface(&self, pane_id: PaneId, from: usize, to: usize) {
        let _ = self
            .cmd_tx
            .try_send(AppCommand::ReorderSurface { pane_id, from, to });
    }

    /// Resize a split ratio on a pane's parent split node. Fire-and-forget.
    pub fn resize_split(&self, pane_id: PaneId, ratio: f32) {
        let _ = self
            .cmd_tx
            .try_send(AppCommand::ResizeSplit { pane_id, ratio });
    }

    /// Resize a specific split node by its `SplitId`. Fire-and-forget.
    pub fn resize_split_by_id(&self, split_id: crate::types::SplitId, ratio: f32) {
        let _ = self
            .cmd_tx
            .try_send(AppCommand::ResizeSplitById { split_id, ratio });
    }

    // ─── Sidebar metadata handle methods ────────────────────────────────────

    /// Set a sidebar status entry. Fire-and-forget.
    pub fn sidebar_set_status(
        &self,
        key: String,
        value: String,
        icon: Option<String>,
        color: Option<String>,
        pid: Option<u32>,
    ) {
        let _ = self.cmd_tx.try_send(AppCommand::SidebarSetStatus {
            key,
            value,
            icon,
            color,
            pid,
        });
    }

    /// Clear a sidebar status entry. Fire-and-forget.
    pub fn sidebar_clear_status(&self, key: String) {
        let _ = self.cmd_tx.try_send(AppCommand::SidebarClearStatus { key });
    }

    /// List sidebar statuses for the active workspace.
    pub async fn sidebar_list_status(&self) -> Vec<StatusEntry> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        if self
            .cmd_tx
            .send(AppCommand::SidebarListStatus { reply: tx })
            .await
            .is_err()
        {
            return Vec::new();
        }
        rx.await.unwrap_or_default()
    }

    /// Set sidebar progress bar. Fire-and-forget.
    pub fn sidebar_set_progress(&self, value: f32, label: Option<String>) {
        let _ = self
            .cmd_tx
            .try_send(AppCommand::SidebarSetProgress { value, label });
    }

    /// Clear sidebar progress bar. Fire-and-forget.
    pub fn sidebar_clear_progress(&self) {
        let _ = self.cmd_tx.try_send(AppCommand::SidebarClearProgress);
    }

    /// Add a sidebar log entry. Fire-and-forget.
    pub fn sidebar_add_log(&self, level: String, source: String, message: String) {
        let _ = self.cmd_tx.try_send(AppCommand::SidebarAddLog {
            level,
            source,
            message,
        });
    }

    /// List sidebar log entries.
    pub async fn sidebar_list_log(&self, limit: usize) -> Vec<LogEntry> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        if self
            .cmd_tx
            .send(AppCommand::SidebarListLog { limit, reply: tx })
            .await
            .is_err()
        {
            return Vec::new();
        }
        rx.await.unwrap_or_default()
    }

    /// Clear sidebar log. Fire-and-forget.
    pub fn sidebar_clear_log(&self) {
        let _ = self.cmd_tx.try_send(AppCommand::SidebarClearLog);
    }

    /// Get full sidebar metadata state.
    pub async fn sidebar_state(&self) -> MetadataSnapshot {
        let (tx, rx) = tokio::sync::oneshot::channel();
        if self
            .cmd_tx
            .send(AppCommand::SidebarState { reply: tx })
            .await
            .is_err()
        {
            return MetadataSnapshot::empty();
        }
        rx.await.unwrap_or_else(|_| MetadataSnapshot::empty())
    }

    /// Update UI-owned state for session persistence. Fire-and-forget.
    pub fn update_ui_state(
        &self,
        sidebar_width: u16,
        window: Option<crate::session::WindowGeometry>,
    ) {
        let _ = self.cmd_tx.try_send(AppCommand::UpdateUiState {
            sidebar_width,
            window,
        });
    }

    /// Shut down the actor. Fire-and-forget.
    pub fn shutdown(&self) {
        let _ = self.cmd_tx.try_send(AppCommand::Shutdown);
    }

    /// Spawn the AppState actor on the tokio runtime.
    ///
    /// Returns the handle for sending commands and a `JoinHandle` for the
    /// actor task. Callers should `shutdown()` then `await` the join handle
    /// to ensure the final session save completes before process exit.
    #[must_use]
    pub fn spawn(event_tx: mpsc::Sender<AppEvent>) -> (Self, JoinHandle<()>) {
        super::actor::spawn_actor(event_tx)
    }
}
