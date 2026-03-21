use crate::error::CoreError;
use crate::surface::PanelKind;
use crate::types::{PaneId, SurfaceId};

/// A single surface (tab) within a pane.
///
/// Tracks identity and metadata. Terminal state and PTY channels live in
/// the backing [`crate::pane_registry::PaneState`] identified by `pane_id`.
/// Secondary surfaces (tabs beyond the first) are "hidden panes" — registered
/// in the [`crate::pane_registry::PaneRegistry`] but NOT in the layout tree.
#[derive(Debug, Clone)]
pub struct Surface {
    pub id: SurfaceId,
    pub title: String,
    pub kind: PanelKind,
    /// The PaneId of the backing PaneState that owns this surface's Terminal and PTY.
    pub pane_id: PaneId,
}

impl Surface {
    /// Create a new terminal surface with the given title and backing pane.
    #[must_use]
    pub fn new(title: impl Into<String>, pane_id: PaneId) -> Self {
        Self {
            id: SurfaceId::new(),
            title: title.into(),
            kind: PanelKind::Terminal,
            pane_id,
        }
    }

    /// Create a new surface with an explicit kind, title, and backing pane.
    #[must_use]
    pub fn with_kind(title: impl Into<String>, kind: PanelKind, pane_id: PaneId) -> Self {
        Self {
            id: SurfaceId::new(),
            title: title.into(),
            kind,
            pane_id,
        }
    }
}

/// Manages the list of surfaces (tabs) for a single pane.
///
/// Invariant: `surfaces` is never empty. The `active_index` always points
/// to a valid element.
#[derive(Debug)]
pub struct SurfaceManager {
    surfaces: Vec<Surface>,
    active_index: usize,
}

impl SurfaceManager {
    /// Create a new manager with a single initial surface.
    #[must_use]
    pub fn new(initial_surface: Surface) -> Self {
        tracing::debug!(
            surface_id = %initial_surface.id,
            title = %initial_surface.title,
            "SurfaceManager created",
        );
        Self {
            surfaces: vec![initial_surface],
            active_index: 0,
        }
    }

    /// Append a surface to the end of the list without switching focus.
    pub fn add(&mut self, surface: Surface) {
        tracing::debug!(
            surface_id = %surface.id,
            title = %surface.title,
            "surface added",
        );
        self.surfaces.push(surface);
    }

    /// Remove a surface by ID, adjusting the active index if necessary.
    ///
    /// Returns the removed surface, or `None` if it was not found.
    /// After removal, callers **must** check `is_empty()` — if true, the pane
    /// should be closed because `active()` will panic on an empty manager.
    pub fn remove(&mut self, id: SurfaceId) -> Option<Surface> {
        let pos = self.surfaces.iter().position(|s| s.id == id)?;
        let removed = self.surfaces.remove(pos);
        tracing::debug!(surface_id = %id, "surface removed");

        if self.surfaces.is_empty() {
            // Caller must close the pane.
            self.active_index = 0;
        } else if self.active_index >= self.surfaces.len() {
            // Active was at the end; move back one.
            self.active_index = self.surfaces.len() - 1;
        } else if pos < self.active_index {
            // A surface before active was removed; keep pointing to same surface.
            self.active_index -= 1;
        }
        // pos == active_index: removed the active surface — active_index now
        // points to the next surface (or was clamped above). No change needed.

        Some(removed)
    }

    /// Return a reference to the currently active surface, or `None` if empty.
    #[must_use]
    pub fn active(&self) -> Option<&Surface> {
        self.surfaces.get(self.active_index)
    }

    /// Return a mutable reference to the currently active surface, or `None` if empty.
    pub fn active_mut(&mut self) -> Option<&mut Surface> {
        self.surfaces.get_mut(self.active_index)
    }

    /// Return the ID of the currently active surface.
    #[must_use]
    pub fn active_id(&self) -> Option<SurfaceId> {
        self.surfaces.get(self.active_index).map(|s| s.id)
    }

    /// Return the index of the currently active surface.
    #[must_use]
    pub fn active_index(&self) -> usize {
        self.active_index
    }

    /// Cycle active surface forward or backward (wraps around).
    ///
    /// No-op if there is only one surface.
    pub fn cycle(&mut self, forward: bool) {
        let len = self.surfaces.len();
        if len <= 1 {
            return;
        }
        if forward {
            self.active_index = (self.active_index + 1) % len;
        } else {
            self.active_index = self.active_index.checked_sub(1).unwrap_or(len - 1);
        }
        tracing::debug!(active_index = self.active_index, "surface cycled");
    }

    /// Switch to the surface at the given index (clamped to valid range).
    pub fn switch_to(&mut self, index: usize) {
        self.active_index = index.min(self.surfaces.len().saturating_sub(1));
        tracing::debug!(active_index = self.active_index, "surface switched");
    }

    /// Switch to a surface by its ID.
    ///
    /// Returns an error if the surface is not found.
    pub fn switch_to_id(&mut self, id: SurfaceId) -> Result<(), CoreError> {
        let pos = self
            .surfaces
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| CoreError::SurfaceNotFound {
                surface_id: id.to_string(),
            })?;
        self.active_index = pos;
        tracing::debug!(surface_id = %id, active_index = pos, "surface switched by id");
        Ok(())
    }

    /// Total number of surfaces in this pane.
    #[must_use]
    pub fn count(&self) -> usize {
        self.surfaces.len()
    }

    /// Returns `true` if there are no surfaces (invariant violation state).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.surfaces.is_empty()
    }

    /// Iterate over all surfaces in order.
    pub fn iter(&self) -> impl Iterator<Item = &Surface> {
        self.surfaces.iter()
    }

    /// Find a surface by ID, returning a reference if found.
    #[must_use]
    pub fn find(&self, id: SurfaceId) -> Option<&Surface> {
        self.surfaces.iter().find(|s| s.id == id)
    }

    /// Find a surface by ID, returning a mutable reference if found.
    pub fn find_mut(&mut self, id: SurfaceId) -> Option<&mut Surface> {
        self.surfaces.iter_mut().find(|s| s.id == id)
    }

    /// Return a reference to the surface at a given index, or `None` if out of bounds.
    #[must_use]
    pub fn get_by_index(&self, index: usize) -> Option<&Surface> {
        self.surfaces.get(index)
    }

    /// Move a surface from `from_index` to `to_index`, keeping the active pointer
    /// pointing to the same surface.
    pub fn reorder(&mut self, from_index: usize, to_index: usize) {
        if from_index >= self.surfaces.len()
            || to_index >= self.surfaces.len()
            || from_index == to_index
        {
            return;
        }
        let surface = self.surfaces.remove(from_index);
        self.surfaces.insert(to_index, surface);
        // Adjust active_index to keep pointing to the same surface.
        if self.active_index == from_index {
            self.active_index = to_index;
        } else if from_index < self.active_index && to_index >= self.active_index {
            self.active_index -= 1;
        } else if from_index > self.active_index && to_index <= self.active_index {
            self.active_index += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::PanelKind;
    use crate::types::PaneId;

    fn make_surface(title: &str) -> Surface {
        Surface::new(title, PaneId::new())
    }

    #[test]
    fn new_manager_has_one_surface() {
        let s = make_surface("bash");
        let id = s.id;
        let mgr = SurfaceManager::new(s);
        assert_eq!(mgr.count(), 1);
        assert_eq!(mgr.active_id(), Some(id));
        assert_eq!(mgr.active_index(), 0);
    }

    #[test]
    fn add_surface_does_not_change_active() {
        let s1 = make_surface("bash");
        let s1_id = s1.id;
        let mut mgr = SurfaceManager::new(s1);

        let s2 = make_surface("zsh");
        mgr.add(s2);
        assert_eq!(mgr.count(), 2);
        assert_eq!(mgr.active_id(), Some(s1_id));
        assert_eq!(mgr.active_index(), 0);
    }

    #[test]
    fn cycle_forward_wraps() {
        let s1 = make_surface("a");
        let s2 = make_surface("b");
        let s3 = make_surface("c");
        let s1_id = s1.id;
        let s2_id = s2.id;
        let s3_id = s3.id;
        let mut mgr = SurfaceManager::new(s1);
        mgr.add(s2);
        mgr.add(s3);

        assert_eq!(mgr.active_id(), Some(s1_id));
        mgr.cycle(true);
        assert_eq!(mgr.active_id(), Some(s2_id));
        mgr.cycle(true);
        assert_eq!(mgr.active_id(), Some(s3_id));
        mgr.cycle(true);
        // Wraps back to first.
        assert_eq!(mgr.active_id(), Some(s1_id));
    }

    #[test]
    fn cycle_backward_wraps() {
        let s1 = make_surface("a");
        let s2 = make_surface("b");
        let s1_id = s1.id;
        let s2_id = s2.id;
        let mut mgr = SurfaceManager::new(s1);
        mgr.add(s2);

        assert_eq!(mgr.active_id(), Some(s1_id));
        // Backward from index 0 wraps to last.
        mgr.cycle(false);
        assert_eq!(mgr.active_id(), Some(s2_id));
    }

    #[test]
    fn cycle_single_surface_is_noop() {
        let s = make_surface("bash");
        let id = s.id;
        let mut mgr = SurfaceManager::new(s);
        mgr.cycle(true);
        assert_eq!(mgr.active_id(), Some(id));
        mgr.cycle(false);
        assert_eq!(mgr.active_id(), Some(id));
    }

    #[test]
    fn switch_to_clamped() {
        let s = make_surface("a");
        let id = s.id;
        let mut mgr = SurfaceManager::new(s);
        mgr.switch_to(100);
        // Clamped to the only valid index.
        assert_eq!(mgr.active_id(), Some(id));
        assert_eq!(mgr.active_index(), 0);
    }

    #[test]
    fn remove_non_active_preserves_active() {
        let s1 = make_surface("a");
        let s2 = make_surface("b");
        let s3 = make_surface("c");
        let s1_id = s1.id;
        let s2_id = s2.id;
        let s3_id = s3.id;
        let mut mgr = SurfaceManager::new(s1);
        mgr.add(s2);
        mgr.add(s3);

        // Switch to s2, then remove s3 (non-active, after active).
        mgr.switch_to(1);
        assert_eq!(mgr.active_id(), Some(s2_id));
        let removed = mgr.remove(s3_id);
        assert!(removed.is_some());
        assert_eq!(mgr.count(), 2);
        assert_eq!(mgr.active_id(), Some(s2_id));

        // Remove s1 (non-active, before active).
        let removed = mgr.remove(s1_id);
        assert!(removed.is_some());
        assert_eq!(mgr.count(), 1);
        // Still pointing to s2.
        assert_eq!(mgr.active_id(), Some(s2_id));
    }

    #[test]
    fn remove_active_surface_moves_to_next() {
        let s1 = make_surface("a");
        let s2 = make_surface("b");
        let s1_id = s1.id;
        let s2_id = s2.id;
        let mut mgr = SurfaceManager::new(s1);
        mgr.add(s2);

        // Remove active (s1 at index 0). s2 should become active.
        let removed = mgr.remove(s1_id);
        assert!(removed.is_some());
        assert_eq!(mgr.count(), 1);
        assert_eq!(mgr.active_id(), Some(s2_id));
    }

    #[test]
    fn remove_last_surface_leaves_empty() {
        let s = make_surface("a");
        let id = s.id;
        let mut mgr = SurfaceManager::new(s);
        let removed = mgr.remove(id);
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn remove_nonexistent_returns_none() {
        let s = make_surface("a");
        let mut mgr = SurfaceManager::new(s);
        let result = mgr.remove(SurfaceId::new());
        assert!(result.is_none());
        assert_eq!(mgr.count(), 1);
    }

    #[test]
    fn find_surface_by_id() {
        let s1 = make_surface("a");
        let s2 = make_surface("b");
        let s2_id = s2.id;
        let mut mgr = SurfaceManager::new(s1);
        mgr.add(s2);

        let found = mgr.find(s2_id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().title, "b");

        assert!(mgr.find(SurfaceId::new()).is_none());
    }

    #[test]
    fn switch_to_id_ok() {
        let s1 = make_surface("a");
        let s2 = make_surface("b");
        let s2_id = s2.id;
        let mut mgr = SurfaceManager::new(s1);
        mgr.add(s2);

        assert!(mgr.switch_to_id(s2_id).is_ok());
        assert_eq!(mgr.active_id(), Some(s2_id));
    }

    #[test]
    fn switch_to_id_not_found_returns_error() {
        let s = make_surface("a");
        let mut mgr = SurfaceManager::new(s);
        let result = mgr.switch_to_id(SurfaceId::new());
        assert!(matches!(result, Err(CoreError::SurfaceNotFound { .. })));
    }

    #[test]
    fn iter_returns_all_surfaces() {
        let s1 = make_surface("a");
        let s2 = make_surface("b");
        let mut mgr = SurfaceManager::new(s1);
        mgr.add(s2);
        let titles: Vec<_> = mgr.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, ["a", "b"]);
    }

    #[test]
    fn surface_with_kind() {
        let s = Surface::with_kind("browser tab", PanelKind::Browser, PaneId::new());
        assert_eq!(s.kind, PanelKind::Browser);
        assert_eq!(s.title, "browser tab");
    }

    #[test]
    fn active_mut_can_rename() {
        let s = make_surface("old");
        let mut mgr = SurfaceManager::new(s);
        mgr.active_mut().unwrap().title = "new".to_string();
        assert_eq!(mgr.active().unwrap().title, "new");
    }
}
