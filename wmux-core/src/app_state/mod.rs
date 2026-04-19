mod actor;
mod handle;

pub use handle::AppStateHandle;

use std::fmt;

use crate::cell::Row;
use crate::grid::Grid;
use crate::metadata_store::{LogEntry, MetadataSnapshot, StatusEntry};
use crate::mode::TerminalMode;
use crate::notification::Notification;
use crate::pane_registry::PaneState;
use crate::rect::Rect;
use crate::surface::{PanelKind, SplitDirection};
use crate::types::{PaneId, SplitId, SurfaceId, WorkspaceId};

// TODO(L2_16): route through i18n system when wmux-config i18n is implemented.
pub(super) const PROCESS_EXITED_MSG: &str = "\r\n[Process exited]\r\n";
pub(super) const PROCESS_EXITED_ERROR_MSG: &str = "\r\n[Process exited with error]\r\n";

/// Nominal viewport used for focus navigation when the real size is unknown.
pub(super) const FOCUS_NAV_VIEWPORT: Rect = Rect {
    x: 0.0,
    y: 0.0,
    width: 1920.0,
    height: 1080.0,
};

/// Channel capacity for the main command channel (ADR-0008).
pub(super) const CMD_CHANNEL_CAPACITY: usize = 256;

// ─── Focus Direction ─────────────────────────────────────────────────────────

/// Directional navigation for focus movement between adjacent panes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusDirection {
    Up,
    Down,
    Left,
    Right,
}

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

    /// Store the child shell PID for a pane (used by process-aware port scanning).
    SetChildPid { pane_id: PaneId, pid: u32 },

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
        reply: tokio::sync::oneshot::Sender<Option<PaneRenderData>>,
    },

    /// Scroll the viewport of a pane (positive = up, negative = down).
    ScrollViewport { pane_id: PaneId, delta: i32 },

    /// Reset the viewport to bottom (live terminal).
    ResetViewport { pane_id: PaneId },

    /// Mark a pane's process as exited.
    MarkExited { pane_id: PaneId, success: bool },

    /// Set the initial CWD for a pane (used during session restore for panes
    /// without a shell to emit OSC 7).
    SetPaneInitialCwd {
        pane_id: PaneId,
        initial_cwd: std::path::PathBuf,
    },

    /// Extract selected text from a pane's grid using a Selection.
    ExtractSelection {
        pane_id: PaneId,
        selection: crate::selection::Selection,
        reply: tokio::sync::oneshot::Sender<String>,
    },

    /// Split a pane, creating a new pane. Returns the new PaneId.
    SplitPane {
        pane_id: PaneId,
        direction: SplitDirection,
        reply: tokio::sync::oneshot::Sender<Result<PaneId, crate::error::CoreError>>,
    },

    /// Swap two panes in the layout tree.
    SwapPanes { a: PaneId, b: PaneId },

    /// Get the current layout as pane-rect pairs with divider metadata.
    #[expect(
        clippy::type_complexity,
        reason = "oneshot reply with (Vec, Vec) tuple — extracting a type alias adds indirection without clarity"
    )]
    GetLayout {
        viewport: Rect,
        reply: tokio::sync::oneshot::Sender<(
            Vec<(PaneId, Rect)>,
            Vec<crate::pane_tree::LayoutDivider>,
        )>,
    },

    // ─── Workspace commands ───────────────────────────────────────────────────
    /// Create a new workspace with the given name. Returns its ID.
    CreateWorkspace {
        name: String,
        reply: tokio::sync::oneshot::Sender<WorkspaceId>,
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
        reply: tokio::sync::oneshot::Sender<Result<SurfaceId, crate::error::CoreError>>,
    },

    /// Create a new browser surface in a pane. Like CreateSurface but with PanelKind::Browser.
    CreateBrowserSurface {
        pane_id: PaneId,
        backing_pane_id: PaneId,
        reply: tokio::sync::oneshot::Sender<Result<SurfaceId, crate::error::CoreError>>,
    },

    /// Close a surface in a pane. If the pane has no surfaces left, closes the pane.
    CloseSurface {
        pane_id: PaneId,
        surface_id: SurfaceId,
    },

    /// Rename a surface (tab) in a pane.
    RenameSurface {
        pane_id: PaneId,
        surface_id: SurfaceId,
        name: String,
    },

    /// Cycle the active surface in a pane (forward or backward).
    CycleSurface { pane_id: PaneId, forward: bool },

    // ─── IPC Query commands ───────────────────────────────────────────────
    /// Get the focused pane ID.
    GetFocusedPaneId {
        reply: tokio::sync::oneshot::Sender<Option<PaneId>>,
    },

    /// Find which pane contains a given surface.
    FindPaneForSurface {
        surface_id: SurfaceId,
        reply: tokio::sync::oneshot::Sender<Option<PaneId>>,
    },

    /// Read terminal text content from a pane's grid and scrollback.
    ReadText {
        pane_id: PaneId,
        start: Option<i32>,
        end: Option<i32>,
        reply: tokio::sync::oneshot::Sender<String>,
    },

    /// List all workspaces with their metadata.
    ListWorkspaces {
        reply: tokio::sync::oneshot::Sender<Vec<WorkspaceSnapshot>>,
    },

    /// Get the current (active) workspace info.
    GetCurrentWorkspace {
        reply: tokio::sync::oneshot::Sender<WorkspaceSnapshot>,
    },

    /// Switch workspace by ID. Returns true if found.
    SelectWorkspaceById {
        id: WorkspaceId,
        reply: tokio::sync::oneshot::Sender<bool>,
    },

    /// List surfaces across all panes in a workspace.
    ListSurfaces {
        workspace_id: Option<WorkspaceId>,
        reply: tokio::sync::oneshot::Sender<Vec<PaneSurfaceInfo>>,
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

    /// Resize a specific split node by its `SplitId`.
    ResizeSplitById { split_id: SplitId, ratio: f32 },

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
        reply: tokio::sync::oneshot::Sender<Vec<StatusEntry>>,
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
        reply: tokio::sync::oneshot::Sender<Vec<LogEntry>>,
    },

    /// Clear sidebar log.
    SidebarClearLog,

    /// Get full sidebar metadata state.
    SidebarState {
        reply: tokio::sync::oneshot::Sender<MetadataSnapshot>,
    },

    /// Update UI-owned state (sidebar width, collapsed, window geometry) for session persistence.
    UpdateUiState {
        sidebar_width: u16,
        sidebar_collapsed: bool,
        window: Option<crate::session::WindowGeometry>,
    },

    // ─── Notification commands ─────────────────────────────────────────────
    /// List notifications (newest first, up to `limit`).
    ListNotifications {
        limit: usize,
        reply: tokio::sync::oneshot::Sender<Vec<Notification>>,
    },

    /// Clear all non-cleared notifications.
    ClearAllNotifications,

    /// Batched render-pull: collect all state the UI needs for one frame in a
    /// single actor round-trip.
    ///
    /// Prefer this over calling `GetLayout` + `GetRenderData`×N +
    /// `ListWorkspaces` + `ListNotifications` separately from the render loop.
    GetFrameSnapshot {
        /// Viewport rect used to compute the pane layout.
        viewport: Rect,
        /// Reply channel for the assembled snapshot.
        resp: tokio::sync::oneshot::Sender<FrameSnapshot>,
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
            Self::SetChildPid { pane_id, pid } => f
                .debug_struct("SetChildPid")
                .field("pane_id", pane_id)
                .field("pid", pid)
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
            Self::SetPaneInitialCwd {
                pane_id,
                initial_cwd,
            } => f
                .debug_struct("SetPaneInitialCwd")
                .field("pane_id", pane_id)
                .field("cwd", initial_cwd)
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
            Self::CreateBrowserSurface {
                pane_id,
                backing_pane_id,
                ..
            } => f
                .debug_struct("CreateBrowserSurface")
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
            Self::RenameSurface {
                pane_id,
                surface_id,
                name,
            } => f
                .debug_struct("RenameSurface")
                .field("pane_id", pane_id)
                .field("surface_id", surface_id)
                .field("name", name)
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
            Self::ResizeSplitById { split_id, ratio } => f
                .debug_struct("ResizeSplitById")
                .field("split_id", split_id)
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
            Self::UpdateUiState {
                sidebar_width,
                sidebar_collapsed,
                ..
            } => f
                .debug_struct("UpdateUiState")
                .field("sidebar_width", sidebar_width)
                .field("sidebar_collapsed", sidebar_collapsed)
                .finish_non_exhaustive(),
            Self::ListNotifications { limit, .. } => f
                .debug_struct("ListNotifications")
                .field("limit", limit)
                .finish_non_exhaustive(),
            Self::ClearAllNotifications => write!(f, "ClearAllNotifications"),
            Self::GetFrameSnapshot { viewport, .. } => f
                .debug_struct("GetFrameSnapshot")
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
    /// IDs of each surface (parallel to `surface_titles`).
    pub surface_ids: Vec<SurfaceId>,
    /// Panel kind for each surface (Terminal or Browser, parallel to `surface_titles`).
    pub surface_kinds: Vec<PanelKind>,
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
    pub unread_count: usize,
    pub cwd: Option<String>,
    pub git_branch: Option<String>,
    /// Detected listening ports for this workspace.
    pub ports: Vec<u16>,
    /// Whether the git index has uncommitted changes.
    pub git_dirty: bool,
    /// Text value of the first status entry (displayed as description in sidebar card).
    pub status_text: Option<String>,
    /// Status entry icons set via IPC (`sidebar.status.set --icon <name>`).
    /// Each tuple is `(key, icon_name)` for entries that have an icon.
    pub status_icons: Vec<(String, String)>,
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

// ─── Frame Snapshot ──────────────────────────────────────────────────────────

/// All actor-owned state the UI render loop needs for one frame.
///
/// Collected in a single actor round-trip to avoid per-frame async contention.
/// The render loop calls `get_frame_snapshot` once per frame instead of
/// issuing separate `list_workspaces`, `get_layout`, `get_render_data`×N,
/// and `list_notifications` requests.
#[derive(Debug, Default)]
pub struct FrameSnapshot {
    /// Workspace list (for sidebar, status bar, etc.).
    pub workspaces: Vec<WorkspaceSnapshot>,
    /// Pane layout: (PaneId, Rect) pairs for the current workspace.
    pub layout: Vec<(PaneId, Rect)>,
    /// Divider metadata for the current layout (used by the resize UI).
    pub layout_dividers: Vec<crate::pane_tree::LayoutDivider>,
    /// Render data keyed by pane ID.
    pub pane_data: std::collections::HashMap<PaneId, PaneRenderData>,
    /// Notifications (newest first, up to the requested limit).
    pub notifications: Vec<Notification>,
}

// ─── Browser Command Channel ────────────────────────────────────────────

/// A browser command sent from the IPC handler to the UI thread.
///
/// The IPC handler creates a oneshot reply channel and sends the command
/// through a tokio mpsc channel. A forwarding task reads from this channel
/// and sends `WmuxEvent::BrowserCommand` via the winit `EventLoopProxy`.
/// The UI thread processes the command on the STA thread (required for
/// WebView2 COM operations) and sends the result back via the oneshot.
pub struct BrowserCommand {
    /// The JSON-RPC method name (e.g., "open", "navigate", "eval").
    pub method: String,
    /// The JSON-RPC params object.
    pub params: serde_json::Value,
    /// Reply channel for the result.
    pub reply: tokio::sync::oneshot::Sender<Result<serde_json::Value, String>>,
}

impl fmt::Debug for BrowserCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrowserCommand")
            .field("method", &self.method)
            .finish_non_exhaustive()
    }
}
