use crate::error::CoreError;
use crate::types::WorkspaceId;
use crate::workspace::Workspace;

// TODO: route through i18n system when available.
const DEFAULT_WORKSPACE_NAME: &str = "Workspace 1";

/// Manages the list of workspaces and tracks the active workspace index.
///
/// Invariant: `workspaces` is never empty. `active_index` always points to a
/// valid element.
pub struct WorkspaceManager {
    workspaces: Vec<Workspace>,
    active_index: usize,
    /// Monotonically increasing counter for `creation_order`.
    next_creation_order: usize,
}

impl WorkspaceManager {
    /// Create a manager with one default workspace.
    #[must_use]
    pub fn new() -> Self {
        let default_ws = Workspace::new(DEFAULT_WORKSPACE_NAME, 0);
        tracing::info!(
            workspace_id = %default_ws.id(),
            "WorkspaceManager created with default workspace",
        );
        Self {
            workspaces: vec![default_ws],
            active_index: 0,
            next_creation_order: 1,
        }
    }

    /// Add a new workspace with the given name, return its ID.
    ///
    /// The newly created workspace does not become active.
    pub fn create(&mut self, name: impl Into<String>) -> WorkspaceId {
        let name = name.into();
        let order = self.next_creation_order;
        self.next_creation_order += 1;
        let ws = Workspace::new(name, order);
        let id = ws.id();
        tracing::info!(workspace_id = %id, name = %ws.name(), "workspace created");
        self.workspaces.push(ws);
        id
    }

    /// Return a reference to the currently active workspace.
    #[must_use]
    pub fn active(&self) -> &Workspace {
        &self.workspaces[self.active_index]
    }

    /// Return a mutable reference to the currently active workspace.
    pub fn active_mut(&mut self) -> &mut Workspace {
        &mut self.workspaces[self.active_index]
    }

    /// Return the ID of the currently active workspace.
    #[must_use]
    pub fn active_id(&self) -> WorkspaceId {
        self.workspaces[self.active_index].id()
    }

    /// Return the 0-based index of the currently active workspace.
    #[must_use]
    pub fn active_index(&self) -> usize {
        self.active_index
    }

    /// Switch the active workspace by 0-based index.
    ///
    /// Returns `false` if the index is out of bounds (no change is made).
    pub fn switch_to_index(&mut self, index: usize) -> bool {
        if index >= self.workspaces.len() {
            return false;
        }
        self.active_index = index;
        tracing::debug!(
            active_index = index,
            workspace_id = %self.workspaces[index].id(),
            "workspace switched by index",
        );
        true
    }

    /// Switch the active workspace by ID.
    ///
    /// Returns `false` if no workspace with that ID exists.
    pub fn switch_to_id(&mut self, id: WorkspaceId) -> bool {
        if let Some(pos) = self.workspaces.iter().position(|ws| ws.id() == id) {
            self.active_index = pos;
            tracing::debug!(
                workspace_id = %id,
                active_index = pos,
                "workspace switched by id",
            );
            true
        } else {
            false
        }
    }

    /// Close the workspace with the given ID.
    ///
    /// - If this is the last workspace, a new empty default workspace is created
    ///   before removing it, preserving the invariant that at least one workspace
    ///   always exists.
    /// - If the active workspace is closed, focus shifts to the next workspace
    ///   (or the previous one if there is no next).
    /// - Returns `Err(CoreError::WorkspaceNotFound)` if no such workspace exists.
    pub fn close(&mut self, id: WorkspaceId) -> Result<(), CoreError> {
        let pos = self
            .workspaces
            .iter()
            .position(|ws| ws.id() == id)
            .ok_or_else(|| CoreError::WorkspaceNotFound {
                workspace_id: id.to_string(),
            })?;

        // Ensure the invariant by creating a replacement before removing.
        if self.workspaces.len() == 1 {
            let order = self.next_creation_order;
            self.next_creation_order += 1;
            let replacement = Workspace::new(DEFAULT_WORKSPACE_NAME, order);
            tracing::info!(
                workspace_id = %replacement.id(),
                "created replacement workspace (was last)",
            );
            self.workspaces.push(replacement);
            // Active index stays 0 — we'll remove index 0 and the replacement
            // (now at index 1) will be clamped to 0 below.
        }

        self.workspaces.remove(pos);
        tracing::info!(workspace_id = %id, "workspace closed");

        // Adjust active_index to stay valid.
        if self.active_index >= self.workspaces.len() {
            self.active_index = self.workspaces.len() - 1;
        } else if pos < self.active_index {
            // A workspace before active was removed; keep pointing to same workspace.
            self.active_index -= 1;
        }
        // pos == active_index: removed the active workspace — active_index now
        // points to the next workspace (or was clamped above).

        Ok(())
    }

    /// Rename the workspace with the given ID.
    ///
    /// Returns `Err(CoreError::WorkspaceNotFound)` if no such workspace exists.
    pub fn rename(&mut self, id: WorkspaceId, name: impl Into<String>) -> Result<(), CoreError> {
        let name = name.into();
        let ws = self
            .workspaces
            .iter_mut()
            .find(|ws| ws.id() == id)
            .ok_or_else(|| CoreError::WorkspaceNotFound {
                workspace_id: id.to_string(),
            })?;
        tracing::info!(workspace_id = %id, old_name = %ws.name, new_name = %name, "workspace renamed");
        ws.name = name;
        Ok(())
    }

    /// Get a reference to a workspace by ID.
    #[must_use]
    pub fn by_id(&self, id: WorkspaceId) -> Option<&Workspace> {
        self.workspaces.iter().find(|ws| ws.id() == id)
    }

    /// Get a mutable reference to a workspace by ID.
    pub fn by_id_mut(&mut self, id: WorkspaceId) -> Option<&mut Workspace> {
        self.workspaces.iter_mut().find(|ws| ws.id() == id)
    }

    /// Return the total number of workspaces.
    #[must_use]
    pub fn count(&self) -> usize {
        self.workspaces.len()
    }

    /// Iterate over all workspaces in creation order.
    pub fn iter(&self) -> impl Iterator<Item = &Workspace> {
        self.workspaces.iter()
    }
}

impl Default for WorkspaceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_manager_has_one_workspace() {
        let mgr = WorkspaceManager::new();
        assert_eq!(mgr.count(), 1);
        assert_eq!(mgr.active_index(), 0);
        assert_eq!(mgr.active().name(), DEFAULT_WORKSPACE_NAME);
    }

    #[test]
    fn create_adds_workspace() {
        let mut mgr = WorkspaceManager::new();
        let id = mgr.create("Second".to_string());
        assert_eq!(mgr.count(), 2);
        // Active stays on first workspace.
        assert_eq!(mgr.active_index(), 0);
        assert!(mgr.by_id(id).is_some());
        assert_eq!(mgr.by_id(id).unwrap().name(), "Second");
    }

    #[test]
    fn create_multiple_workspaces() {
        let mut mgr = WorkspaceManager::new();
        let id2 = mgr.create("WS 2".to_string());
        let id3 = mgr.create("WS 3".to_string());
        assert_eq!(mgr.count(), 3);
        assert_ne!(id2, id3);
    }

    #[test]
    fn switch_to_index_valid() {
        let mut mgr = WorkspaceManager::new();
        mgr.create("Second".to_string());
        assert!(mgr.switch_to_index(1));
        assert_eq!(mgr.active_index(), 1);
        assert_eq!(mgr.active().name(), "Second");
    }

    #[test]
    fn switch_to_index_out_of_bounds_returns_false() {
        let mut mgr = WorkspaceManager::new();
        assert!(!mgr.switch_to_index(5));
        assert_eq!(mgr.active_index(), 0);
    }

    #[test]
    fn switch_to_id_valid() {
        let mut mgr = WorkspaceManager::new();
        let id = mgr.create("Second".to_string());
        assert!(mgr.switch_to_id(id));
        assert_eq!(mgr.active_id(), id);
    }

    #[test]
    fn switch_to_id_nonexistent_returns_false() {
        let mut mgr = WorkspaceManager::new();
        assert!(!mgr.switch_to_id(WorkspaceId::new()));
        assert_eq!(mgr.active_index(), 0);
    }

    #[test]
    fn close_last_workspace_creates_replacement() {
        let mut mgr = WorkspaceManager::new();
        let first_id = mgr.active_id();
        mgr.close(first_id).unwrap();
        // Should still have one workspace — the replacement.
        assert_eq!(mgr.count(), 1);
        assert_ne!(mgr.active_id(), first_id);
        assert_eq!(mgr.active().name(), DEFAULT_WORKSPACE_NAME);
    }

    #[test]
    fn close_nonexistent_returns_error() {
        let mut mgr = WorkspaceManager::new();
        let result = mgr.close(WorkspaceId::new());
        assert!(matches!(result, Err(CoreError::WorkspaceNotFound { .. })));
    }

    #[test]
    fn close_non_active_workspace() {
        let mut mgr = WorkspaceManager::new();
        let id2 = mgr.create("Second".to_string());
        // Close the non-active second workspace.
        mgr.close(id2).unwrap();
        assert_eq!(mgr.count(), 1);
        // Active stays at index 0.
        assert_eq!(mgr.active_index(), 0);
    }

    #[test]
    fn close_active_workspace_switches_focus() {
        let mut mgr = WorkspaceManager::new();
        let _id2 = mgr.create("Second".to_string());
        // Switch to second, then close it.
        mgr.switch_to_index(1);
        let id2 = mgr.active_id();
        mgr.close(id2).unwrap();
        // Should have switched to index 0 (clamped).
        assert_eq!(mgr.active_index(), 0);
        assert_eq!(mgr.count(), 1);
    }

    #[test]
    fn close_first_workspace_keeps_active_stable() {
        let mut mgr = WorkspaceManager::new();
        let first_id = mgr.active_id();
        let _id2 = mgr.create("Second".to_string());
        let _id3 = mgr.create("Third".to_string());
        // Switch to third.
        mgr.switch_to_index(2);
        // Close first (before active).
        mgr.close(first_id).unwrap();
        // active_index should have decremented but still point to third.
        assert_eq!(mgr.count(), 2);
        assert_eq!(mgr.active_index(), 1);
    }

    #[test]
    fn rename_workspace() {
        let mut mgr = WorkspaceManager::new();
        let id = mgr.active_id();
        mgr.rename(id, "My Renamed WS".to_string()).unwrap();
        assert_eq!(mgr.active().name(), "My Renamed WS");
    }

    #[test]
    fn rename_nonexistent_returns_error() {
        let mut mgr = WorkspaceManager::new();
        let result = mgr.rename(WorkspaceId::new(), "name".to_string());
        assert!(matches!(result, Err(CoreError::WorkspaceNotFound { .. })));
    }

    #[test]
    fn by_id_returns_correct_workspace() {
        let mut mgr = WorkspaceManager::new();
        let id = mgr.create("Target".to_string());
        let ws = mgr.by_id(id).unwrap();
        assert_eq!(ws.name(), "Target");
    }

    #[test]
    fn by_id_mut_allows_mutation() {
        let mut mgr = WorkspaceManager::new();
        let id = mgr.create("Original".to_string());
        mgr.by_id_mut(id).unwrap().name = "Modified".to_string();
        assert_eq!(mgr.by_id(id).unwrap().name(), "Modified");
    }

    #[test]
    fn iter_returns_all_workspaces() {
        let mut mgr = WorkspaceManager::new();
        mgr.create("WS2".to_string());
        mgr.create("WS3".to_string());
        let names: Vec<_> = mgr.iter().map(|ws| ws.name()).collect();
        assert_eq!(names, [DEFAULT_WORKSPACE_NAME, "WS2", "WS3"]);
    }

    #[test]
    fn active_id_matches_active_workspace() {
        let mut mgr = WorkspaceManager::new();
        let id2 = mgr.create("Second".to_string());
        mgr.switch_to_index(1);
        assert_eq!(mgr.active_id(), id2);
        assert_eq!(mgr.active().id(), id2);
    }

    #[test]
    fn creation_order_increments() {
        let mut mgr = WorkspaceManager::new();
        let id2 = mgr.create("WS2".to_string());
        let id3 = mgr.create("WS3".to_string());
        let ws1_order = mgr.by_id(mgr.workspaces[0].id()).unwrap().creation_order();
        let ws2_order = mgr.by_id(id2).unwrap().creation_order();
        let ws3_order = mgr.by_id(id3).unwrap().creation_order();
        assert!(ws1_order < ws2_order);
        assert!(ws2_order < ws3_order);
    }

    #[test]
    fn switch_to_ctrl9_goes_to_ninth_or_last() {
        let mut mgr = WorkspaceManager::new();
        for i in 2..=5 {
            mgr.create(format!("WS {i}"));
        }
        // Ctrl+9 behavior: go to 9th (index 8) if available, else go to last.
        let target_index = 8.min(mgr.count() - 1);
        assert!(mgr.switch_to_index(target_index));
        // 5 workspaces → last is index 4.
        assert_eq!(mgr.active_index(), 4);
    }
}
