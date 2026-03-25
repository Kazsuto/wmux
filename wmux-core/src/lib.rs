pub mod app_state;
pub mod cell;
pub mod color;
pub mod command_registry;
pub mod cursor;
pub mod error;
pub mod event;
pub mod git_detector;
pub mod grid;
pub mod metadata_store;
pub mod mode;
pub mod notification;
pub mod pane_registry;
pub mod pane_tree;
pub mod port_scanner;
pub mod rect;
pub mod remote;
pub mod scrollback;
pub mod selection;
pub mod session;
pub mod surface;
pub mod surface_manager;
pub mod terminal;
pub mod types;
pub(crate) mod vte_handler;
pub mod workspace;
pub mod workspace_manager;

pub use app_state::{
    AppCommand, AppEvent, AppStateHandle, BrowserCommand, FocusDirection, PaneRenderData,
    PaneSurfaceInfo, WorkspaceSnapshot,
};
pub use cell::{Cell, CellFlags, Row};
pub use color::Color;
pub use command_registry::{CommandEntry, CommandRegistry, SearchResult};
pub use cursor::{CursorShape, CursorState};
pub use error::CoreError;
pub use event::{Hyperlink, PromptMark, TerminalEvent};
pub use git_detector::{detect_git, GitInfo};
pub use grid::Grid;
pub use metadata_store::{
    LogEntry, LogLevel, MetadataSnapshot, MetadataStore, ProgressState, StatusEntry,
};
pub use mode::TerminalMode;
pub use notification::{
    Notification, NotificationEvent, NotificationId, NotificationSeverity, NotificationSource,
    NotificationState, NotificationStore,
};
pub use pane_registry::{PaneRegistry, PaneState};
pub use pane_tree::{LayoutDivider, PaneTree};
pub use port_scanner::scan_listening_ports;
pub use rect::Rect;
pub use remote::{ReconnectBackoff, RemoteConfig, RemoteConnectionState, RemoteError};
pub use scrollback::Scrollback;
pub use selection::{Selection, SelectionMode, SelectionPoint};
pub use session::{
    build_session_state, first_leaf, load_session, save_session, session_file_path, FirstLeafData,
    PaneTreeSnapshot, SessionState, WindowGeometry, SESSION_VERSION,
};
pub use surface::{PanelKind, SplitDirection, SurfaceInfo};
pub use surface_manager::{Surface, SurfaceManager};
pub use terminal::Terminal;
pub use types::{PaneId, SplitId, SurfaceId, WindowId, WorkspaceId};
pub use workspace::{Workspace, WorkspaceKind, WorkspaceMetadata};
pub use workspace_manager::WorkspaceManager;
