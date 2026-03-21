use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::pane_tree::PaneTree;
use crate::types::WorkspaceId;

/// Metadata for a workspace, populated over time by sidebar and shell integration tasks.
///
/// All fields default to empty/None — they are filled in by Tasks L2_14 (sidebar metadata),
/// L3_13 (git integration), and L3_14 (port scanning).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceMetadata {
    /// Active git branch in the workspace CWD.
    pub git_branch: Option<String>,
    /// Current working directory of the most-recently-active pane.
    pub cwd: Option<PathBuf>,
    /// Detected listening ports (populated by a later task).
    pub ports: Vec<u16>,
    /// Whether the git index has uncommitted changes.
    pub git_dirty: bool,
}

/// A workspace — the primary organizational unit in wmux.
///
/// Each workspace owns an independent pane layout (`PaneTree`). Switching
/// workspaces switches the entire visible pane layout. The `pane_tree` starts
/// as `None` and is populated when the first pane is registered.
#[derive(Debug)]
pub struct Workspace {
    /// Stable unique identifier for this workspace.
    pub id: WorkspaceId,
    /// Display name shown in the sidebar.
    pub name: String,
    /// Layout tree — `None` until the first pane is registered.
    pub pane_tree: Option<PaneTree>,
    /// Metadata (git, cwd, ports) populated by later tasks.
    pub metadata: WorkspaceMetadata,
    /// 0-based creation order, used for stable sort in the sidebar.
    pub creation_order: usize,
}

impl Workspace {
    /// Create a new workspace with the given name and creation order.
    /// The `pane_tree` starts as `None`.
    #[must_use]
    pub fn new(name: impl Into<String>, creation_order: usize) -> Self {
        let id = WorkspaceId::new();
        let name = name.into();
        tracing::debug!(workspace_id = %id, name = %name, creation_order, "workspace created");
        Self {
            id,
            name,
            pane_tree: None,
            metadata: WorkspaceMetadata::default(),
            creation_order,
        }
    }

    /// Return the workspace's stable ID.
    #[must_use]
    pub fn id(&self) -> WorkspaceId {
        self.id
    }

    /// Return the display name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the creation order index.
    #[must_use]
    pub fn creation_order(&self) -> usize {
        self.creation_order
    }

    /// Return a reference to the pane tree, if any pane has been registered.
    #[must_use]
    pub fn pane_tree(&self) -> Option<&PaneTree> {
        self.pane_tree.as_ref()
    }

    /// Return a mutable reference to the pane tree, if any pane has been registered.
    pub fn pane_tree_mut(&mut self) -> Option<&mut PaneTree> {
        self.pane_tree.as_mut()
    }

    /// Return a reference to the workspace metadata.
    #[must_use]
    pub fn metadata(&self) -> &WorkspaceMetadata {
        &self.metadata
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_workspace_has_no_pane_tree() {
        let ws = Workspace::new("test", 0);
        assert!(ws.pane_tree.is_none());
        assert_eq!(ws.name(), "test");
        assert_eq!(ws.creation_order(), 0);
    }

    #[test]
    fn workspace_id_is_unique() {
        let a = Workspace::new("a", 0);
        let b = Workspace::new("b", 1);
        assert_ne!(a.id(), b.id());
    }

    #[test]
    fn workspace_metadata_default_is_empty() {
        let meta = WorkspaceMetadata::default();
        assert!(meta.git_branch.is_none());
        assert!(meta.cwd.is_none());
        assert!(meta.ports.is_empty());
        assert!(!meta.git_dirty);
    }

    #[test]
    fn pane_tree_starts_none_can_be_set() {
        let mut ws = Workspace::new("ws", 0);
        assert!(ws.pane_tree().is_none());

        let pane_id = crate::types::PaneId::new();
        ws.pane_tree = Some(PaneTree::new(pane_id));
        assert!(ws.pane_tree().is_some());
        assert!(ws.pane_tree().unwrap().find_pane(pane_id));
    }

    #[test]
    fn workspace_name_getter() {
        let ws = Workspace::new("My Workspace", 2);
        assert_eq!(ws.name(), "My Workspace");
    }
}
