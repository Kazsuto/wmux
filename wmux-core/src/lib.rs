pub mod app_state;
pub mod cell;
pub mod color;
pub mod cursor;
pub mod error;
pub mod event;
pub mod grid;
pub mod mode;
pub mod notification;
pub mod pane_registry;
pub mod pane_tree;
pub mod rect;
pub mod scrollback;
pub mod selection;
pub mod surface;
pub mod terminal;
pub mod types;
pub(crate) mod vte_handler;

pub use app_state::{AppCommand, AppEvent, AppStateHandle, PaneRenderData};
pub use cell::{Cell, CellFlags, Row};
pub use color::Color;
pub use cursor::{CursorShape, CursorState};
pub use error::CoreError;
pub use event::{Hyperlink, PromptMark, TerminalEvent};
pub use grid::Grid;
pub use mode::TerminalMode;
pub use notification::{
    Notification, NotificationEvent, NotificationId, NotificationSource, NotificationState,
    NotificationStore,
};
pub use pane_registry::{PaneRegistry, PaneState};
pub use pane_tree::PaneTree;
pub use rect::Rect;
pub use scrollback::Scrollback;
pub use selection::{Selection, SelectionMode, SelectionPoint};
pub use surface::{PanelKind, SplitDirection, SurfaceInfo};
pub use terminal::Terminal;
pub use types::{PaneId, SurfaceId, WindowId, WorkspaceId};
