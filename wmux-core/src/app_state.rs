use std::collections::HashMap;
use std::fmt;
use std::time::Instant;

use tokio::sync::{mpsc, oneshot};
use tokio::task::{JoinHandle, JoinSet};

use crate::cell::Row;
use crate::error::CoreError;
use crate::event::TerminalEvent;
use crate::grid::Grid;
use crate::metadata_store::{LogEntry, LogLevel, MetadataSnapshot, MetadataStore, StatusEntry};
use crate::mode::TerminalMode;
use crate::notification::{Notification, NotificationEvent, NotificationSource, NotificationStore};
use crate::pane_registry::{PaneRegistry, PaneState};
use crate::pane_tree::PaneTree;
use crate::rect::Rect;
use crate::surface::{PanelKind, SplitDirection};
use crate::surface_manager::Surface;
use crate::types::{PaneId, SurfaceId, WorkspaceId};
use crate::workspace_manager::WorkspaceManager;

// TODO(L2_16): route through i18n system when wmux-config i18n is implemented.
const PROCESS_EXITED_MSG: &str = "\r\n[Process exited]\r\n";
const PROCESS_EXITED_ERROR_MSG: &str = "\r\n[Process exited with error]\r\n";

/// Nominal viewport used for focus navigation when the real size is unknown.
const FOCUS_NAV_VIEWPORT: Rect = Rect {
    x: 0.0,
    y: 0.0,
    width: 1920.0,
    height: 1080.0,
};

// ─── Focus Direction ─────────────────────────────────────────────────────────

/// Directional navigation for focus movement between adjacent panes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusDirection {
    Up,
    Down,
    Left,
    Right,
}

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

    // ─── Workspace commands ───────────────────────────────────────────────────
    /// Create a new workspace with the given name. Returns its ID.
    CreateWorkspace {
        name: String,
        reply: oneshot::Sender<WorkspaceId>,
    },

    /// Switch the active workspace by 0-based index.
    SwitchWorkspace { index: usize },

    /// Close a workspace (and all its panes) by ID.
    CloseWorkspace { id: WorkspaceId },

    /// Rename a workspace by ID.
    RenameWorkspace { id: WorkspaceId, name: String },

    /// Reorder a workspace from one index to another.
    ReorderWorkspace { from: usize, to: usize },

    /// Toggle zoom on a pane (zoomed pane fills the entire viewport).
    ToggleZoom { pane_id: PaneId },

    /// Move focus to the adjacent pane in the given direction.
    NavigateFocus { direction: FocusDirection },

    // ─── Surface (tab) commands ───────────────────────────────────────────────
    /// Create a new surface in a pane. Returns the new surface's ID.
    ///
    /// `backing_pane_id` is the PaneId of the hidden PaneState (with its own
    /// Terminal and PTY) that will back the new surface.
    CreateSurface {
        pane_id: PaneId,
        backing_pane_id: PaneId,
        reply: oneshot::Sender<Result<SurfaceId, CoreError>>,
    },

    /// Close a surface in a pane. If the pane has no surfaces left, closes the pane.
    CloseSurface {
        pane_id: PaneId,
        surface_id: SurfaceId,
    },

    /// Cycle the active surface in a pane (forward or backward).
    CycleSurface { pane_id: PaneId, forward: bool },

    // ─── IPC Query commands ───────────────────────────────────────────────
    /// Get the focused pane ID.
    GetFocusedPaneId {
        reply: oneshot::Sender<Option<PaneId>>,
    },

    /// Find which pane contains a given surface.
    FindPaneForSurface {
        surface_id: SurfaceId,
        reply: oneshot::Sender<Option<PaneId>>,
    },

    /// Read terminal text content from a pane's grid and scrollback.
    ReadText {
        pane_id: PaneId,
        start: Option<i32>,
        end: Option<i32>,
        reply: oneshot::Sender<String>,
    },

    /// List all workspaces with their metadata.
    ListWorkspaces {
        reply: oneshot::Sender<Vec<WorkspaceSnapshot>>,
    },

    /// Get the current (active) workspace info.
    GetCurrentWorkspace {
        reply: oneshot::Sender<WorkspaceSnapshot>,
    },

    /// Switch workspace by ID. Returns true if found.
    SelectWorkspaceById {
        id: WorkspaceId,
        reply: oneshot::Sender<bool>,
    },

    /// List surfaces across all panes in a workspace.
    ListSurfaces {
        workspace_id: Option<WorkspaceId>,
        reply: oneshot::Sender<Vec<PaneSurfaceInfo>>,
    },

    /// Focus a specific surface within a pane.
    FocusSurface {
        pane_id: PaneId,
        surface_id: SurfaceId,
    },

    /// Switch to a surface by index within a pane.
    SwitchSurfaceIndex { pane_id: PaneId, index: usize },

    /// Reorder surfaces within a pane (drag-and-drop).
    ReorderSurface {
        pane_id: PaneId,
        from: usize,
        to: usize,
    },

    /// Resize a split by setting the ratio on a pane's parent split node.
    ResizeSplit { pane_id: PaneId, ratio: f32 },

    // ─── Internal commands (sent by actor to itself) ───────────────────────
    /// Update git info for a workspace (sent by git detection task).
    UpdateGitInfo {
        workspace_id: WorkspaceId,
        branch: Option<String>,
        dirty: bool,
    },

    /// Update detected listening ports for a workspace.
    UpdatePorts {
        workspace_id: WorkspaceId,
        ports: Vec<u16>,
    },

    // ─── Sidebar metadata commands ───────────────────────────────────────────
    /// Set a sidebar status entry for the active workspace.
    SidebarSetStatus {
        key: String,
        value: String,
        icon: Option<String>,
        color: Option<String>,
        pid: Option<u32>,
    },

    /// Clear a sidebar status entry by key.
    SidebarClearStatus { key: String },

    /// List sidebar statuses for the active workspace.
    SidebarListStatus {
        reply: oneshot::Sender<Vec<StatusEntry>>,
    },

    /// Set sidebar progress bar.
    SidebarSetProgress { value: f32, label: Option<String> },

    /// Clear sidebar progress bar.
    SidebarClearProgress,

    /// Add a sidebar log entry.
    SidebarAddLog {
        level: String,
        source: String,
        message: String,
    },

    /// List sidebar log entries.
    SidebarListLog {
        limit: usize,
        reply: oneshot::Sender<Vec<LogEntry>>,
    },

    /// Clear sidebar log.
    SidebarClearLog,

    /// Get full sidebar metadata state.
    SidebarState {
        reply: oneshot::Sender<MetadataSnapshot>,
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
            Self::CreateWorkspace { name, .. } => f
                .debug_struct("CreateWorkspace")
                .field("name", name)
                .finish_non_exhaustive(),
            Self::SwitchWorkspace { index } => f
                .debug_struct("SwitchWorkspace")
                .field("index", index)
                .finish(),
            Self::CloseWorkspace { id } => {
                f.debug_struct("CloseWorkspace").field("id", id).finish()
            }
            Self::RenameWorkspace { id, name } => f
                .debug_struct("RenameWorkspace")
                .field("id", id)
                .field("name", name)
                .finish(),
            Self::ReorderWorkspace { from, to } => f
                .debug_struct("ReorderWorkspace")
                .field("from", from)
                .field("to", to)
                .finish(),
            Self::ToggleZoom { pane_id } => f
                .debug_struct("ToggleZoom")
                .field("pane_id", pane_id)
                .finish(),
            Self::NavigateFocus { direction } => f
                .debug_struct("NavigateFocus")
                .field("direction", direction)
                .finish(),
            Self::CreateSurface {
                pane_id,
                backing_pane_id,
                ..
            } => f
                .debug_struct("CreateSurface")
                .field("pane_id", pane_id)
                .field("backing_pane_id", backing_pane_id)
                .finish_non_exhaustive(),
            Self::CloseSurface {
                pane_id,
                surface_id,
            } => f
                .debug_struct("CloseSurface")
                .field("pane_id", pane_id)
                .field("surface_id", surface_id)
                .finish(),
            Self::CycleSurface { pane_id, forward } => f
                .debug_struct("CycleSurface")
                .field("pane_id", pane_id)
                .field("forward", forward)
                .finish(),
            Self::GetFocusedPaneId { .. } => write!(f, "GetFocusedPaneId"),
            Self::FindPaneForSurface { surface_id, .. } => f
                .debug_struct("FindPaneForSurface")
                .field("surface_id", surface_id)
                .finish_non_exhaustive(),
            Self::ReadText {
                pane_id,
                start,
                end,
                ..
            } => f
                .debug_struct("ReadText")
                .field("pane_id", pane_id)
                .field("start", start)
                .field("end", end)
                .finish_non_exhaustive(),
            Self::ListWorkspaces { .. } => write!(f, "ListWorkspaces"),
            Self::GetCurrentWorkspace { .. } => write!(f, "GetCurrentWorkspace"),
            Self::SelectWorkspaceById { id, .. } => f
                .debug_struct("SelectWorkspaceById")
                .field("id", id)
                .finish_non_exhaustive(),
            Self::ListSurfaces { workspace_id, .. } => f
                .debug_struct("ListSurfaces")
                .field("workspace_id", workspace_id)
                .finish_non_exhaustive(),
            Self::FocusSurface {
                pane_id,
                surface_id,
            } => f
                .debug_struct("FocusSurface")
                .field("pane_id", pane_id)
                .field("surface_id", surface_id)
                .finish(),
            Self::SwitchSurfaceIndex { pane_id, index } => f
                .debug_struct("SwitchSurfaceIndex")
                .field("pane_id", pane_id)
                .field("index", index)
                .finish(),
            Self::ReorderSurface { pane_id, from, to } => f
                .debug_struct("ReorderSurface")
                .field("pane_id", pane_id)
                .field("from", from)
                .field("to", to)
                .finish(),
            Self::ResizeSplit { pane_id, ratio } => f
                .debug_struct("ResizeSplit")
                .field("pane_id", pane_id)
                .field("ratio", ratio)
                .finish(),
            Self::UpdateGitInfo {
                workspace_id,
                branch,
                dirty,
            } => f
                .debug_struct("UpdateGitInfo")
                .field("workspace_id", workspace_id)
                .field("branch", branch)
                .field("dirty", dirty)
                .finish(),
            Self::UpdatePorts {
                workspace_id,
                ports,
            } => f
                .debug_struct("UpdatePorts")
                .field("workspace_id", workspace_id)
                .field("count", &ports.len())
                .finish(),
            Self::SidebarSetStatus { key, .. } => f
                .debug_struct("SidebarSetStatus")
                .field("key", key)
                .finish_non_exhaustive(),
            Self::SidebarClearStatus { key } => f
                .debug_struct("SidebarClearStatus")
                .field("key", key)
                .finish(),
            Self::SidebarListStatus { .. } => write!(f, "SidebarListStatus"),
            Self::SidebarSetProgress { value, .. } => f
                .debug_struct("SidebarSetProgress")
                .field("value", value)
                .finish_non_exhaustive(),
            Self::SidebarClearProgress => write!(f, "SidebarClearProgress"),
            Self::SidebarAddLog { level, source, .. } => f
                .debug_struct("SidebarAddLog")
                .field("level", level)
                .field("source", source)
                .finish_non_exhaustive(),
            Self::SidebarListLog { limit, .. } => f
                .debug_struct("SidebarListLog")
                .field("limit", limit)
                .finish_non_exhaustive(),
            Self::SidebarClearLog => write!(f, "SidebarClearLog"),
            Self::SidebarState { .. } => write!(f, "SidebarState"),
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

    /// A new workspace was created.
    WorkspaceCreated { id: WorkspaceId },

    /// The active workspace changed.
    WorkspaceSwitched { index: usize, id: WorkspaceId },

    /// A workspace was closed.
    WorkspaceClosed { id: WorkspaceId },

    /// The focused pane changed (via navigation or programmatic focus).
    FocusChanged { pane_id: PaneId },
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
    /// Number of surfaces (tabs) in this pane.
    pub surface_count: usize,
    /// Display titles for each surface.
    pub surface_titles: Vec<String>,
    /// Index of the currently active surface.
    pub active_surface: usize,
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
            .field("surface_count", &self.surface_count)
            .field("active_surface", &self.active_surface)
            .finish_non_exhaustive()
    }
}

// ─── IPC Snapshots ──────────────────────────────────────────────────────

/// Snapshot of a workspace for IPC queries.
#[derive(Debug, Clone)]
pub struct WorkspaceSnapshot {
    pub id: WorkspaceId,
    pub name: String,
    pub active: bool,
    pub pane_count: usize,
}

/// Snapshot of a surface for IPC queries.
#[derive(Debug, Clone)]
pub struct PaneSurfaceInfo {
    pub surface_id: SurfaceId,
    pub pane_id: PaneId,
    pub title: String,
    pub kind: PanelKind,
    pub active: bool,
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
        let (tx, rx) = oneshot::channel();
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
        let (tx, rx) = oneshot::channel();
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
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(AppCommand::GetFocusedPaneId { reply: tx })
            .await
            .ok()?;
        rx.await.ok()?
    }

    /// Find which pane contains a given surface.
    pub async fn find_pane_for_surface(&self, surface_id: SurfaceId) -> Option<PaneId> {
        let (tx, rx) = oneshot::channel();
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
        let (tx, rx) = oneshot::channel();
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
        let (tx, rx) = oneshot::channel();
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
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(AppCommand::GetCurrentWorkspace { reply: tx })
            .await
            .ok()?;
        rx.await.ok()
    }

    /// Switch workspace by ID. Returns true if found.
    pub async fn select_workspace_by_id(&self, id: WorkspaceId) -> bool {
        let (tx, rx) = oneshot::channel();
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
        let (tx, rx) = oneshot::channel();
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
        let (tx, rx) = oneshot::channel();
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
        let (tx, rx) = oneshot::channel();
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
        let (tx, rx) = oneshot::channel();
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
        let (cmd_tx, cmd_rx) = mpsc::channel(CMD_CHANNEL_CAPACITY);
        let actor = AppStateActor {
            registry: PaneRegistry::new(),
            notification_store: NotificationStore::new(),
            workspace_manager: WorkspaceManager::new(),
            metadata_stores: HashMap::new(),
            zoomed_pane: None,
            self_tx: cmd_tx.clone(),
            background_tasks: JoinSet::new(),
            last_git_detect: HashMap::new(),
            port_scan_in_flight: false,
            cmd_rx,
            event_tx,
        };
        let join_handle = tokio::spawn(actor.run());
        tracing::info!("AppState actor spawned");
        (Self { cmd_tx }, join_handle)
    }
}

// ─── Actor ───────────────────────────────────────────────────────────────────

/// The AppState actor. Runs in a dedicated tokio task and owns all mutable
/// application state. All mutations go through `AppCommand` messages.
struct AppStateActor {
    registry: PaneRegistry,
    notification_store: NotificationStore,
    workspace_manager: WorkspaceManager,
    /// Per-workspace sidebar metadata stores.
    metadata_stores: HashMap<WorkspaceId, MetadataStore>,
    /// Currently zoomed pane. When `Some(id)`, `GetLayout` returns only that
    /// pane at full-viewport rect. `None` means normal split layout.
    zoomed_pane: Option<PaneId>,
    /// Self-sender for internal commands (git detection results, port scan results).
    ///
    /// NOTE: This creates a reference cycle on the command channel — the actor
    /// holds a sender to its own receiver, preventing automatic shutdown via
    /// sender-drop detection. The actor MUST be shut down explicitly via
    /// `AppCommand::Shutdown`. Background tasks cloned from this sender are
    /// aborted on shutdown via `background_tasks`.
    self_tx: mpsc::Sender<AppCommand>,
    /// Background tasks spawned by the actor (git detection, port scanning, auto-save).
    /// Aborted on shutdown to prevent orphaned tasks.
    background_tasks: JoinSet<()>,
    /// Last git detection timestamp per workspace — debounce 500ms.
    last_git_detect: HashMap<WorkspaceId, Instant>,
    /// Whether a port scan is currently in flight (prevents overlapping scans).
    port_scan_in_flight: bool,
    cmd_rx: mpsc::Receiver<AppCommand>,
    event_tx: mpsc::Sender<AppEvent>,
}

impl AppStateActor {
    async fn run(mut self) {
        tracing::info!("AppState actor loop started");

        let mut save_interval = tokio::time::interval(std::time::Duration::from_secs(8));
        save_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        let mut pid_sweep_interval = tokio::time::interval(std::time::Duration::from_secs(30));
        pid_sweep_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        let mut port_scan_interval = tokio::time::interval(std::time::Duration::from_secs(15));
        port_scan_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                cmd = self.cmd_rx.recv() => {
                    let Some(cmd) = cmd else { break; };
                    match cmd {
                AppCommand::RegisterPane { pane_id, state } => {
                    self.registry.register(pane_id, *state);
                    // Initialize the active workspace's pane_tree on first pane
                    // registration. Subsequent panes must be added via SplitPane.
                    let active = self.workspace_manager.active_mut();
                    if active.pane_tree.is_none() {
                        active.pane_tree = Some(PaneTree::new(pane_id));
                    }
                }
                AppCommand::ClosePane { pane_id } => {
                    self.close_pane_internal(pane_id);
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
                    let target = self.resolve_terminal_pane(pane_id);
                    if let Some(pane) = self.registry.get_mut(target) {
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
                    let target = self.resolve_terminal_pane(pane_id);
                    let text = self
                        .registry
                        .get(target)
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
                    let result = if let Some(tree) =
                        self.workspace_manager.active_mut().pane_tree.as_mut()
                    {
                        tree.split_pane(pane_id, direction)
                    } else {
                        Err(CoreError::PaneNotFound {
                            pane_id: pane_id.to_string(),
                        })
                    };
                    let _ = reply.send(result);
                }
                AppCommand::SwapPanes { a, b } => {
                    if let Some(tree) = self.workspace_manager.active_mut().pane_tree.as_mut() {
                        if let Err(e) = tree.swap_panes(a, b) {
                            tracing::warn!(error = %e, "SwapPanes failed");
                        }
                    }
                }
                AppCommand::GetLayout { viewport, reply } => {
                    let layout = if let Some(zoomed_id) = self.zoomed_pane {
                        // Validate zoomed pane still exists in the registry.
                        if self.registry.get(zoomed_id).is_some() {
                            vec![(zoomed_id, viewport)]
                        } else {
                            // Stale zoom reference — clear it and fall through.
                            self.zoomed_pane = None;
                            if let Some(tree) = self.workspace_manager.active().pane_tree.as_ref() {
                                tree.layout(viewport)
                            } else {
                                Vec::new()
                            }
                        }
                    } else if let Some(tree) = self.workspace_manager.active().pane_tree.as_ref() {
                        tree.layout(viewport)
                    } else {
                        Vec::new()
                    };
                    let _ = reply.send(layout);
                }

                AppCommand::ToggleZoom { pane_id } => {
                    match self.zoomed_pane {
                        Some(id) if id == pane_id => {
                            self.zoomed_pane = None;
                            tracing::info!(pane_id = %pane_id, "zoom cleared");
                        }
                        _ => {
                            self.zoomed_pane = Some(pane_id);
                            tracing::info!(pane_id = %pane_id, "pane zoomed");
                        }
                    }
                    let _ = self.event_tx.try_send(AppEvent::PaneNeedsRedraw(pane_id));
                }

                AppCommand::NavigateFocus { direction } => {
                    self.handle_navigate_focus(direction);
                }

                // ─── Workspace commands ───────────────────────────────────────
                AppCommand::CreateWorkspace { name, reply } => {
                    let id = self.workspace_manager.create(name);
                    // Auto-switch to the new workspace (it's the last one).
                    let new_index = self.workspace_manager.count() - 1;
                    self.zoomed_pane = None;
                    self.workspace_manager.switch_to_index(new_index);
                    tracing::info!(workspace_id = %id, index = new_index, "workspace created and switched");
                    let _ = self.event_tx.try_send(AppEvent::WorkspaceCreated { id });
                    let _ = self.event_tx.try_send(AppEvent::WorkspaceSwitched {
                        index: new_index,
                        id,
                    });
                    let _ = reply.send(id);
                }
                AppCommand::SwitchWorkspace { index } => {
                    // Clear zoom when switching workspaces — zoom is per-view, not per-workspace.
                    self.zoomed_pane = None;
                    if self.workspace_manager.switch_to_index(index) {
                        let id = self.workspace_manager.active_id();
                        tracing::info!(index, workspace_id = %id, "workspace switched");
                        let _ = self
                            .event_tx
                            .try_send(AppEvent::WorkspaceSwitched { index, id });
                    } else {
                        tracing::warn!(index, "SwitchWorkspace: index out of bounds");
                    }
                }
                AppCommand::CloseWorkspace { id } => match self.workspace_manager.close(id) {
                    Ok(()) => {
                        tracing::info!(workspace_id = %id, "workspace closed via command");
                        let _ = self.event_tx.try_send(AppEvent::WorkspaceClosed { id });
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, workspace_id = %id, "CloseWorkspace failed");
                    }
                },
                AppCommand::RenameWorkspace { id, name } => {
                    if let Err(e) = self.workspace_manager.rename(id, name) {
                        tracing::warn!(error = %e, workspace_id = %id, "RenameWorkspace failed");
                    }
                }
                AppCommand::ReorderWorkspace { from, to } => {
                    if let Err(e) = self.workspace_manager.reorder(from, to) {
                        tracing::warn!(error = %e, from, to, "ReorderWorkspace failed");
                    }
                }

                // ─── Surface commands ─────────────────────────────────────────
                AppCommand::CreateSurface { pane_id, backing_pane_id, reply } => {
                    let result = if let Some(pane) = self.registry.get_mut(pane_id) {
                        let surface = Surface::new("shell", backing_pane_id);
                        let id = surface.id;
                        pane.surfaces.add(surface);
                        // Switch to the newly created surface.
                        let _ = pane.surfaces.switch_to_id(id);
                        tracing::info!(
                            pane_id = %pane_id,
                            surface_id = %id,
                            backing = %backing_pane_id,
                            "surface created",
                        );
                        Ok(id)
                    } else {
                        Err(CoreError::PaneNotFound {
                            pane_id: pane_id.to_string(),
                        })
                    };
                    if result.is_ok() {
                        let _ = self.event_tx.try_send(AppEvent::PaneNeedsRedraw(pane_id));
                    }
                    let _ = reply.send(result);
                }

                AppCommand::CloseSurface {
                    pane_id,
                    surface_id,
                } => {
                    // Find the backing pane and determine if the pane becomes empty.
                    // Scoped borrow so we can call registry.remove afterwards.
                    let (backing_pane_id, pane_now_empty) =
                        if let Some(pane) = self.registry.get_mut(pane_id) {
                            let backing = pane.surfaces.find(surface_id).map(|s| s.pane_id);
                            pane.surfaces.remove(surface_id);
                            (backing, pane.surfaces.is_empty())
                        } else {
                            (None, false)
                        };

                    // Remove the hidden backing pane (stops its PTY).
                    if let Some(bpid) = backing_pane_id {
                        if bpid != pane_id {
                            self.registry.remove(bpid);
                        }
                    }

                    if pane_now_empty {
                        tracing::info!(pane_id = %pane_id, "last surface closed, closing pane");
                        self.close_pane_internal(pane_id);
                    } else if self.registry.get(pane_id).is_some() {
                        tracing::info!(
                            pane_id = %pane_id,
                            surface_id = %surface_id,
                            "surface closed"
                        );
                        // Force full re-render of the now-active surface.
                        let new_backing = self.resolve_terminal_pane(pane_id);
                        if let Some(bp) = self.registry.get_mut(new_backing) {
                            bp.terminal.grid_mut().mark_all_dirty();
                        }
                        let _ = self.event_tx.try_send(AppEvent::PaneNeedsRedraw(pane_id));
                    }
                }

                AppCommand::CycleSurface { pane_id, forward } => {
                    let backing_id = if let Some(pane) = self.registry.get_mut(pane_id) {
                        pane.surfaces.cycle(forward);
                        tracing::debug!(pane_id = %pane_id, forward, "surface cycled");
                        pane.surfaces.active().map(|s| s.pane_id)
                    } else {
                        None
                    };
                    // Force full re-render of the new backing terminal.
                    if let Some(bid) = backing_id {
                        if let Some(bp) = self.registry.get_mut(bid) {
                            bp.terminal.grid_mut().mark_all_dirty();
                        }
                    }
                    let _ = self.event_tx.try_send(AppEvent::PaneNeedsRedraw(pane_id));
                }

                // ─── IPC Query commands ───────────────────────────────────
                AppCommand::GetFocusedPaneId { reply } => {
                    let _ = reply.send(self.registry.focused_id());
                }
                AppCommand::FindPaneForSurface { surface_id, reply } => {
                    let result = self.registry.find_pane_for_surface(surface_id);
                    let _ = reply.send(result);
                }
                AppCommand::ReadText { pane_id, start, end, reply } => {
                    let text = self.build_read_text(pane_id, start, end);
                    let _ = reply.send(text);
                }
                AppCommand::ListWorkspaces { reply } => {
                    let active_id = self.workspace_manager.active_id();
                    let snapshots: Vec<WorkspaceSnapshot> = self
                        .workspace_manager
                        .iter()
                        .map(|ws| {
                            let pane_count = ws
                                .pane_tree
                                .as_ref()
                                .map_or(0, |t| t.pane_count());
                            WorkspaceSnapshot {
                                id: ws.id(),
                                name: ws.name().to_owned(),
                                active: ws.id() == active_id,
                                pane_count,
                            }
                        })
                        .collect();
                    let _ = reply.send(snapshots);
                }
                AppCommand::GetCurrentWorkspace { reply } => {
                    let ws = self.workspace_manager.active();
                    let pane_count = ws
                        .pane_tree
                        .as_ref()
                        .map_or(0, |t| t.pane_count());
                    let _ = reply.send(WorkspaceSnapshot {
                        id: ws.id(),
                        name: ws.name().to_owned(),
                        active: true,
                        pane_count,
                    });
                }
                AppCommand::SelectWorkspaceById { id, reply } => {
                    let ok = self.workspace_manager.switch_to_id(id);
                    if ok {
                        self.zoomed_pane = None;
                        let index = self.workspace_manager.active_index();
                        let _ = self
                            .event_tx
                            .try_send(AppEvent::WorkspaceSwitched { index, id });
                    }
                    let _ = reply.send(ok);
                }
                AppCommand::ListSurfaces { workspace_id, reply } => {
                    let infos = self.build_surface_list(workspace_id);
                    let _ = reply.send(infos);
                }
                AppCommand::FocusSurface { pane_id, surface_id } => {
                    let backing_id = if let Some(pane) = self.registry.get_mut(pane_id) {
                        let _ = pane.surfaces.switch_to_id(surface_id);
                        pane.surfaces.active().map(|s| s.pane_id)
                    } else {
                        None
                    };
                    if let Some(bid) = backing_id {
                        if let Some(bp) = self.registry.get_mut(bid) {
                            bp.terminal.grid_mut().mark_all_dirty();
                        }
                    }
                    let _ = self.event_tx.try_send(AppEvent::PaneNeedsRedraw(pane_id));
                }
                AppCommand::SwitchSurfaceIndex { pane_id, index } => {
                    let backing_id = if let Some(pane) = self.registry.get_mut(pane_id) {
                        pane.surfaces.switch_to(index);
                        pane.surfaces.active().map(|s| s.pane_id)
                    } else {
                        None
                    };
                    if let Some(bid) = backing_id {
                        if let Some(bp) = self.registry.get_mut(bid) {
                            bp.terminal.grid_mut().mark_all_dirty();
                        }
                    }
                    let _ = self.event_tx.try_send(AppEvent::PaneNeedsRedraw(pane_id));
                }
                AppCommand::ReorderSurface { pane_id, from, to } => {
                    if let Some(pane) = self.registry.get_mut(pane_id) {
                        pane.surfaces.reorder(from, to);
                        let _ = self.event_tx.try_send(AppEvent::PaneNeedsRedraw(pane_id));
                    }
                }
                AppCommand::ResizeSplit { pane_id, ratio } => {
                    if let Some(tree) = self.workspace_manager.active_mut().pane_tree.as_mut() {
                        if let Err(e) = tree.resize_split(pane_id, ratio) {
                            tracing::warn!(error = %e, pane_id = %pane_id, "ResizeSplit failed");
                        }
                    }
                }

                // ─── Internal commands ─────────────────────────────────────
                AppCommand::UpdateGitInfo { workspace_id, branch, dirty } => {
                    if let Some(ws) = self.workspace_manager.by_id_mut(workspace_id) {
                        ws.metadata.git_branch = branch;
                        ws.metadata.git_dirty = dirty;
                    }
                }
                AppCommand::UpdatePorts { workspace_id, ports } => {
                    self.port_scan_in_flight = false;
                    if let Some(ws) = self.workspace_manager.by_id_mut(workspace_id) {
                        ws.metadata.ports = ports;
                    }
                }

                // ─── Sidebar metadata commands ───────────────────────────
                AppCommand::SidebarSetStatus { key, value, icon, color, pid } => {
                    let ws_id = self.workspace_manager.active_id();
                    let store = self.metadata_stores.entry(ws_id).or_default();
                    store.set_status(StatusEntry { key, value, icon, color, pid });
                }
                AppCommand::SidebarClearStatus { key } => {
                    let ws_id = self.workspace_manager.active_id();
                    if let Some(store) = self.metadata_stores.get_mut(&ws_id) {
                        store.clear_status(&key);
                    }
                }
                AppCommand::SidebarListStatus { reply } => {
                    let ws_id = self.workspace_manager.active_id();
                    let statuses = self
                        .metadata_stores
                        .get(&ws_id)
                        .map(|s| s.list_status().into_iter().cloned().collect())
                        .unwrap_or_default();
                    let _ = reply.send(statuses);
                }
                AppCommand::SidebarSetProgress { value, label } => {
                    let ws_id = self.workspace_manager.active_id();
                    let store = self.metadata_stores.entry(ws_id).or_default();
                    store.set_progress(value, label);
                }
                AppCommand::SidebarClearProgress => {
                    let ws_id = self.workspace_manager.active_id();
                    if let Some(store) = self.metadata_stores.get_mut(&ws_id) {
                        store.clear_progress();
                    }
                }
                AppCommand::SidebarAddLog { level, source, message } => {
                    let ws_id = self.workspace_manager.active_id();
                    let store = self.metadata_stores.entry(ws_id).or_default();
                    let log_level = match level.as_str() {
                        "progress" => LogLevel::Progress,
                        "success" => LogLevel::Success,
                        "warning" | "warn" => LogLevel::Warning,
                        "error" => LogLevel::Error,
                        _ => LogLevel::Info,
                    };
                    store.add_log(log_level, source, message);
                }
                AppCommand::SidebarListLog { limit, reply } => {
                    let ws_id = self.workspace_manager.active_id();
                    let logs = self
                        .metadata_stores
                        .get(&ws_id)
                        .map(|s| s.list_log(limit).into_iter().cloned().collect())
                        .unwrap_or_default();
                    let _ = reply.send(logs);
                }
                AppCommand::SidebarClearLog => {
                    let ws_id = self.workspace_manager.active_id();
                    if let Some(store) = self.metadata_stores.get_mut(&ws_id) {
                        store.clear_log();
                    }
                }
                AppCommand::SidebarState { reply } => {
                    let ws_id = self.workspace_manager.active_id();
                    let snapshot = self
                        .metadata_stores
                        .get(&ws_id)
                        .map(|s| s.state())
                        .unwrap_or_else(MetadataSnapshot::empty);
                    let _ = reply.send(snapshot);
                }

                        AppCommand::Shutdown => {
                            tracing::info!("AppState actor shutting down");
                            self.background_tasks.abort_all();
                            break;
                        }
                    }
                }
                _ = save_interval.tick() => {
                    self.auto_save();
                }
                _ = pid_sweep_interval.tick() => {
                    for store in self.metadata_stores.values_mut() {
                        store.sweep_dead_pids();
                    }
                }
                _ = port_scan_interval.tick() => {
                    // Skip if a previous scan is still in flight.
                    if !self.port_scan_in_flight {
                        self.port_scan_in_flight = true;
                        let ws_id = self.workspace_manager.active_id();
                        let self_tx = self.self_tx.clone();
                        self.background_tasks.spawn(async move {
                            let ports = crate::port_scanner::scan_listening_ports().await;
                            let _ = self_tx.try_send(AppCommand::UpdatePorts {
                                workspace_id: ws_id,
                                ports,
                            });
                        });
                    }
                }
                // Drain completed background tasks to avoid unbounded growth.
                Some(result) = self.background_tasks.join_next(), if !self.background_tasks.is_empty() => {
                    if let Err(e) = result {
                        if e.is_panic() {
                            tracing::error!(error = ?e, "background task panicked");
                        }
                        // Cancelled tasks (from abort_all) are expected during shutdown.
                    }
                }
            }
        }
        // Final save on shutdown — must complete before the process exits.
        // Unlike periodic saves, this one is awaited to guarantee persistence.
        self.save_session_now().await;
        tracing::info!("AppState actor loop ended");
    }

    /// Periodic auto-save: fire-and-forget so the actor loop never awaits disk I/O.
    fn auto_save(&mut self) {
        let state = crate::session::build_session_state(&self.workspace_manager, &self.registry);
        self.background_tasks.spawn(async move {
            if let Err(e) = crate::session::save_session(&state).await {
                tracing::warn!(error = %e, "auto-save failed");
            } else {
                tracing::debug!("session auto-saved");
            }
        });
    }

    /// Save session state and wait for completion (used on shutdown).
    async fn save_session_now(&self) {
        let state = crate::session::build_session_state(&self.workspace_manager, &self.registry);
        if let Err(e) = crate::session::save_session(&state).await {
            tracing::warn!(error = %e, "final session save failed");
        } else {
            tracing::debug!("session saved on shutdown");
        }
    }

    /// Find which workspace contains a specific pane by searching all workspaces' pane trees.
    fn find_workspace_for_pane(&self, pane_id: PaneId) -> Option<WorkspaceId> {
        for ws in self.workspace_manager.iter() {
            if let Some(tree) = ws.pane_tree() {
                if tree.find_pane(pane_id) {
                    return Some(ws.id());
                }
            }
        }
        None
    }

    fn handle_pty_output(&mut self, pane_id: PaneId, data: &[u8]) {
        // Look up which workspace contains this pane early, before mutable borrows
        let ws_id = self.find_workspace_for_pane(pane_id);

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
                        None, // workspace — TODO(L2_07): pass workspace context for notification tracking
                        None, // surface — TODO(L2_07): pass surface context for notification tracking
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
                TerminalEvent::CwdChanged(cwd) => {
                    // Use the workspace ID found at the start of this function
                    let Some(cwd_ws_id) = ws_id else {
                        tracing::warn!(pane_id = %pane_id, "CwdChanged event but pane not in any workspace tree");
                        continue;
                    };

                    // Update workspace metadata with new CWD
                    if let Some(ws) = self.workspace_manager.by_id_mut(cwd_ws_id) {
                        ws.metadata.cwd = Some(cwd.clone());
                    }
                    tracing::debug!(workspace_id = %cwd_ws_id, cwd = %cwd.display(), "CWD changed");

                    // Debounce git detection: skip if less than 500ms since last detect
                    // for this workspace. Prevents process storms on rapid CWD changes.
                    let now = Instant::now();
                    let debounce = std::time::Duration::from_millis(500);
                    let should_detect = self
                        .last_git_detect
                        .get(&cwd_ws_id)
                        .is_none_or(|last| now.duration_since(*last) >= debounce);

                    if should_detect {
                        self.last_git_detect.insert(cwd_ws_id, now);
                        let cwd_clone = cwd.clone();
                        let self_tx = self.self_tx.clone();
                        self.background_tasks.spawn(async move {
                            if let Some(git_info) =
                                crate::git_detector::detect_git(&cwd_clone).await
                            {
                                let _ = self_tx.try_send(AppCommand::UpdateGitInfo {
                                    workspace_id: cwd_ws_id,
                                    branch: Some(git_info.branch),
                                    dirty: git_info.dirty,
                                });
                            } else {
                                let _ = self_tx.try_send(AppCommand::UpdateGitInfo {
                                    workspace_id: cwd_ws_id,
                                    branch: None,
                                    dirty: false,
                                });
                            }
                        });
                    }
                }
                TerminalEvent::PromptMark(_) => {
                    // Prompt marks currently informational only
                }
            }
        }

        // Signal UI to redraw.
        let _ = self.event_tx.try_send(AppEvent::PaneNeedsRedraw(pane_id));
    }

    /// Remove a pane from the registry, clear zoom if needed, and prune the
    /// pane tree.  Shared by `ClosePane` and `CloseSurface` (empty-pane path).
    fn close_pane_internal(&mut self, pane_id: PaneId) {
        // Remove hidden surface backing panes first (stops their PTYs).
        let hidden_ids: Vec<PaneId> = self
            .registry
            .get(pane_id)
            .map(|p| {
                p.surfaces
                    .iter()
                    .filter(|s| s.pane_id != pane_id)
                    .map(|s| s.pane_id)
                    .collect()
            })
            .unwrap_or_default();
        for hid in hidden_ids {
            self.registry.remove(hid);
        }

        self.registry.remove(pane_id);
        if self.zoomed_pane == Some(pane_id) {
            self.zoomed_pane = None;
        }
        if let Some(tree) = self.workspace_manager.active_mut().pane_tree.as_mut() {
            if let Err(e) = tree.close_pane(pane_id) {
                tracing::warn!(error = %e, "failed to close pane in tree");
            }
        }
    }

    /// Resolve the backing terminal pane for a layout pane.
    ///
    /// When a layout pane has an active surface whose `pane_id` is registered,
    /// returns that surface's `pane_id` (the hidden backing pane). Falls back to
    /// `layout_pane_id` if the pane has no surface, does not exist, or the
    /// surface's backing pane is not (yet) registered.
    fn resolve_terminal_pane(&self, layout_pane_id: PaneId) -> PaneId {
        if let Some(pane) = self.registry.get(layout_pane_id) {
            if let Some(surface) = pane.surfaces.active() {
                let backing = surface.pane_id;
                // Only redirect if the backing pane is actually registered.
                if backing != layout_pane_id && self.registry.get(backing).is_some() {
                    return backing;
                }
            }
        }
        layout_pane_id
    }

    fn handle_send_input(&self, pane_id: PaneId, data: Vec<u8>) {
        let target = self.resolve_terminal_pane(pane_id);
        if let Some(pane) = self.registry.get(target) {
            if !pane.process_exited && pane.pty_write_tx.try_send(data).is_err() {
                tracing::warn!(pane_id = %target, "PTY write channel full, input dropped");
            }
        }
    }

    fn handle_resize(&mut self, pane_id: PaneId, cols: u16, rows: u16) {
        let target = self.resolve_terminal_pane(pane_id);
        if let Some(pane) = self.registry.get_mut(target) {
            let old_cols = pane.terminal.cols();
            let old_rows = pane.terminal.rows();
            if cols != old_cols || rows != old_rows {
                pane.terminal.resize(cols, rows);
                if pane.pty_resize_tx.try_send((rows, cols)).is_err() {
                    tracing::warn!(pane_id = %target, "PTY resize channel full, resize dropped");
                }
                tracing::debug!(
                    pane_id = %target,
                    old_cols, old_rows, cols, rows,
                    "pane resized",
                );
            }
        }
    }

    fn handle_scroll(&mut self, pane_id: PaneId, delta: i32) {
        let target = self.resolve_terminal_pane(pane_id);
        if let Some(pane) = self.registry.get_mut(target) {
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
                PROCESS_EXITED_MSG
            } else {
                PROCESS_EXITED_ERROR_MSG
            };
            pane.terminal.process(msg.as_bytes());
            tracing::info!(pane_id = %pane_id, success, "pane process exited");
        }
        let _ = self
            .event_tx
            .try_send(AppEvent::PaneExited { pane_id, success });
    }

    /// Navigate focus to the adjacent pane in the given direction.
    ///
    /// Algorithm: compute the layout using a nominal viewport, find the focused
    /// pane's center, then for each other pane check whether its center is in
    /// the requested direction relative to the focused pane. The closest such
    /// pane (by Euclidean distance between centers) wins.
    fn handle_navigate_focus(&mut self, direction: FocusDirection) {
        let Some(focused_id) = self.registry.focused_id() else {
            return;
        };

        // Use the current pane_tree for layout; no need for real pixel coords.
        let Some(tree) = self.workspace_manager.active().pane_tree.as_ref() else {
            return;
        };

        let viewport = FOCUS_NAV_VIEWPORT;
        let layout = tree.layout(viewport);

        if layout.len() < 2 {
            return;
        }

        // Find focused pane's rect.
        let focused_rect = match layout.iter().find(|(id, _)| *id == focused_id) {
            Some((_, r)) => *r,
            None => return,
        };

        let fx = focused_rect.x + focused_rect.width / 2.0;
        let fy = focused_rect.y + focused_rect.height / 2.0;

        // Find the nearest pane in the given direction, prioritizing panes
        // that share the same row (Left/Right) or column (Up/Down).
        //
        // Two-pass algorithm:
        // 1. First pass: find candidates with overlap on the perpendicular axis
        //    (e.g., for Right, panes whose Y range overlaps the focused pane's Y range)
        // 2. Fallback: if no overlap candidates, use any pane in the direction
        let mut best_overlap: Option<(PaneId, f32)> = None;
        let mut best_any: Option<(PaneId, f32)> = None;

        for (id, rect) in &layout {
            if *id == focused_id {
                continue;
            }

            let cx = rect.x + rect.width / 2.0;
            let cy = rect.y + rect.height / 2.0;

            let in_direction = match direction {
                FocusDirection::Up => cy < fy,
                FocusDirection::Down => cy > fy,
                FocusDirection::Left => cx < fx,
                FocusDirection::Right => cx > fx,
            };

            if !in_direction {
                continue;
            }

            // Check if panes overlap on the perpendicular axis.
            let has_overlap = match direction {
                FocusDirection::Left | FocusDirection::Right => {
                    // Y ranges overlap?
                    let y1_start = focused_rect.y;
                    let y1_end = focused_rect.y + focused_rect.height;
                    let y2_start = rect.y;
                    let y2_end = rect.y + rect.height;
                    y1_start < y2_end && y2_start < y1_end
                }
                FocusDirection::Up | FocusDirection::Down => {
                    // X ranges overlap?
                    let x1_start = focused_rect.x;
                    let x1_end = focused_rect.x + focused_rect.width;
                    let x2_start = rect.x;
                    let x2_end = rect.x + rect.width;
                    x1_start < x2_end && x2_start < x1_end
                }
            };

            let dist = (cx - fx) * (cx - fx) + (cy - fy) * (cy - fy);

            if has_overlap {
                match best_overlap {
                    None => best_overlap = Some((*id, dist)),
                    Some((_, d)) if dist < d => best_overlap = Some((*id, dist)),
                    _ => {}
                }
            }
            match best_any {
                None => best_any = Some((*id, dist)),
                Some((_, d)) if dist < d => best_any = Some((*id, dist)),
                _ => {}
            }
        }

        let best = best_overlap.or(best_any);

        if let Some((target_id, _)) = best {
            if self.registry.set_focused(target_id) {
                tracing::info!(
                    from = %focused_id,
                    to = %target_id,
                    direction = ?direction,
                    "focus navigated",
                );
                let _ = self
                    .event_tx
                    .try_send(AppEvent::FocusChanged { pane_id: target_id });
                let _ = self.event_tx.try_send(AppEvent::PaneNeedsRedraw(target_id));
            }
        }
    }

    fn build_render_data(&mut self, pane_id: PaneId) -> Option<PaneRenderData> {
        // Resolve the backing terminal pane (may be a hidden pane for secondary surfaces).
        let terminal_pane_id = self.resolve_terminal_pane(pane_id);

        // Read surface metadata from the layout pane first (immutable borrow ends here).
        let (surface_count, surface_titles, active_surface) = {
            let pane = self.registry.get(pane_id)?;
            let sc = pane.surfaces.count();
            let st: Vec<String> = pane.surfaces.iter().map(|s| s.title.clone()).collect();
            let ai = pane.surfaces.active_index();
            (sc, st, ai)
        };

        // Get mutable access to the backing terminal pane (may differ from layout pane).
        let pane = self.registry.get_mut(terminal_pane_id)?;

        // Take dirty rows from the actor's grid (resets flags), then clone.
        let mut dirty_rows = Vec::with_capacity(pane.terminal.grid().rows() as usize);
        pane.terminal
            .grid_mut()
            .take_dirty_rows_into(&mut dirty_rows);
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
            surface_count,
            surface_titles,
            active_surface,
        })
    }

    /// Read terminal text content from a pane's grid and scrollback.
    ///
    /// If start/end are None, captures the visible grid only.
    /// Negative values are relative from the end of the combined buffer.
    fn build_read_text(&self, pane_id: PaneId, start: Option<i32>, end: Option<i32>) -> String {
        let terminal_pane_id = self.resolve_terminal_pane(pane_id);
        let Some(pane) = self.registry.get(terminal_pane_id) else {
            return String::new();
        };

        let grid = pane.terminal.grid();
        let scrollback = pane.terminal.scrollback();
        let grid_rows = grid.rows() as usize;
        let sb_len = scrollback.len();
        let total_lines = sb_len + grid_rows;

        // Resolve range
        let resolve = |val: i32| -> usize {
            if val < 0 {
                (total_lines as i64 + val as i64).max(0) as usize
            } else {
                (val as usize).min(total_lines)
            }
        };

        let (s, e) = match (start, end) {
            (Some(s), Some(e)) => (resolve(s), resolve(e)),
            (Some(s), None) => (resolve(s), total_lines),
            (None, Some(e)) => (sb_len, resolve(e)), // default start = visible grid start
            (None, None) => (sb_len, total_lines),   // visible grid only
        };

        if s >= e || s >= total_lines {
            return String::new();
        }

        let cols = grid.cols() as usize;
        let mut result = String::with_capacity((e - s) * (cols + 1));

        for line_idx in s..e {
            if line_idx > s {
                result.push('\n');
            }
            if line_idx < sb_len {
                // Scrollback row
                if let Some(row) = scrollback.get_row(line_idx) {
                    for cell in row {
                        result.push_str(cell.grapheme.as_str());
                    }
                }
            } else {
                // Grid row
                let grid_row = (line_idx - sb_len) as u16;
                if grid_row < grid.rows() {
                    for col in 0..grid.cols() {
                        result.push_str(grid.cell(col, grid_row).grapheme.as_str());
                    }
                }
            }
        }

        // Trim trailing whitespace per line
        result
            .lines()
            .map(|l| l.trim_end())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Build surface list for IPC ListSurfaces query.
    fn build_surface_list(&self, workspace_id: Option<WorkspaceId>) -> Vec<PaneSurfaceInfo> {
        let ws = if let Some(id) = workspace_id {
            match self.workspace_manager.by_id(id) {
                Some(ws) => ws,
                None => return Vec::new(),
            }
        } else {
            self.workspace_manager.active()
        };

        let pane_ids: Vec<PaneId> = ws
            .pane_tree
            .as_ref()
            .map_or_else(Vec::new, |t| t.pane_ids());

        let mut infos = Vec::new();
        for pane_id in pane_ids {
            if let Some(pane) = self.registry.get(pane_id) {
                let active_id = pane.surfaces.active_id();
                for surface in pane.surfaces.iter() {
                    infos.push(PaneSurfaceInfo {
                        surface_id: surface.id,
                        pane_id,
                        title: surface.title.clone(),
                        kind: surface.kind,
                        active: active_id == Some(surface.id),
                    });
                }
            }
        }
        infos
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
        let pane_id = PaneId::new();
        PaneState {
            terminal,
            terminal_event_rx: event_rx,
            pty_write_tx: write_tx,
            pty_resize_tx: resize_tx,
            process_exited: false,
            surfaces: crate::surface_manager::SurfaceManager::new(
                crate::surface_manager::Surface::new("shell", pane_id),
            ),
        }
    }

    #[tokio::test]
    async fn spawn_and_register_pane() {
        let (event_tx, mut event_rx) = mpsc::channel(64);
        let (handle, _actor_handle) = AppStateHandle::spawn(event_tx);

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
        let (handle, _actor_handle) = AppStateHandle::spawn(event_tx);

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
        let (handle, _actor_handle) = AppStateHandle::spawn(event_tx);

        let data = handle.get_render_data(PaneId::new()).await;
        assert!(data.is_none());

        handle.shutdown();
    }

    #[tokio::test]
    async fn mark_exited_sets_flag() {
        let (event_tx, mut event_rx) = mpsc::channel(64);
        let (handle, _actor_handle) = AppStateHandle::spawn(event_tx);

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
        let (handle, _actor_handle) = AppStateHandle::spawn(event_tx);
        handle.shutdown();
        tokio::task::yield_now().await;

        // After shutdown, commands should fail silently.
        let data = handle.get_render_data(PaneId::new()).await;
        assert!(data.is_none());
    }

    #[tokio::test]
    async fn create_workspace_returns_id() {
        let (event_tx, mut event_rx) = mpsc::channel(64);
        let (handle, _actor_handle) = AppStateHandle::spawn(event_tx);

        let id = handle.create_workspace("WS 2".to_string()).await;
        assert!(id.is_some());

        // Drain events.
        tokio::task::yield_now().await;
        let mut got_created = false;
        while let Ok(event) = event_rx.try_recv() {
            if matches!(event, AppEvent::WorkspaceCreated { .. }) {
                got_created = true;
            }
        }
        assert!(got_created);

        handle.shutdown();
    }

    #[tokio::test]
    async fn switch_workspace_emits_event() {
        let (event_tx, mut event_rx) = mpsc::channel(64);
        let (handle, _actor_handle) = AppStateHandle::spawn(event_tx);

        handle.create_workspace("WS 2".to_string()).await;
        tokio::task::yield_now().await;
        // Drain the WorkspaceCreated event.
        event_rx.try_recv().ok();

        handle.switch_workspace(1);
        tokio::task::yield_now().await;

        let mut got_switched = false;
        while let Ok(event) = event_rx.try_recv() {
            if matches!(event, AppEvent::WorkspaceSwitched { index: 1, .. }) {
                got_switched = true;
            }
        }
        assert!(got_switched);

        handle.shutdown();
    }

    #[tokio::test]
    async fn close_workspace_emits_event() {
        let (event_tx, mut event_rx) = mpsc::channel(64);
        let (handle, _actor_handle) = AppStateHandle::spawn(event_tx);

        let ws_id = handle.create_workspace("WS 2".to_string()).await.unwrap();
        tokio::task::yield_now().await;
        event_rx.try_recv().ok(); // drain WorkspaceCreated

        handle.close_workspace(ws_id);
        tokio::task::yield_now().await;

        let mut got_closed = false;
        while let Ok(event) = event_rx.try_recv() {
            if matches!(event, AppEvent::WorkspaceClosed { id } if id == ws_id) {
                got_closed = true;
            }
        }
        assert!(got_closed);

        handle.shutdown();
    }

    #[tokio::test]
    async fn pane_registered_in_active_workspace() {
        let (event_tx, _) = mpsc::channel(64);
        let (handle, _actor_handle) = AppStateHandle::spawn(event_tx);

        let pane_id = PaneId::new();
        handle.register_pane(pane_id, make_pane_state(80, 24));
        tokio::task::yield_now().await;

        // Layout should return the pane in the active workspace.
        let viewport = crate::rect::Rect::new(0.0, 0.0, 800.0, 600.0);
        let layout = handle.get_layout(viewport).await;
        assert_eq!(layout.len(), 1);
        assert_eq!(layout[0].0, pane_id);

        handle.shutdown();
    }
}
