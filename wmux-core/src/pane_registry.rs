use std::collections::HashMap;

use tokio::sync::mpsc;

use crate::event::TerminalEvent;
use crate::surface_manager::SurfaceManager;
use crate::terminal::Terminal;
use crate::types::PaneId;

/// Per-pane state owned by the AppState actor.
///
/// Each pane bundles its terminal instance with the channel handles
/// needed to communicate with the PTY bridge task.
pub struct PaneState {
    /// Terminal state machine (grid, scrollback, VTE parser, modes).
    pub terminal: Terminal,

    /// Receiver for terminal events (DSR responses, notifications, etc.).
    pub terminal_event_rx: mpsc::Receiver<TerminalEvent>,

    /// Send input bytes to the PTY bridge task.
    pub pty_write_tx: mpsc::Sender<Vec<u8>>,

    /// Send resize commands to the PTY bridge task.
    pub pty_resize_tx: mpsc::Sender<(u16, u16)>,

    /// Whether the shell process has exited.
    pub process_exited: bool,

    /// Surface (tab) manager for this pane.
    pub surfaces: SurfaceManager,
}

/// Registry of all active panes, with focus tracking.
pub struct PaneRegistry {
    panes: HashMap<PaneId, PaneState>,
    focused: Option<PaneId>,
}

impl PaneRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            panes: HashMap::new(),
            focused: None,
        }
    }

    /// Register a new pane. If this is the first pane, it becomes focused.
    pub fn register(&mut self, id: PaneId, state: PaneState) {
        let is_first = self.panes.is_empty();
        self.panes.insert(id, state);
        if is_first {
            self.focused = Some(id);
        }
        tracing::info!(pane_id = %id, total = self.panes.len(), "pane registered");
    }

    /// Remove a pane. If the focused pane is removed, focus moves to another pane.
    pub fn remove(&mut self, id: PaneId) -> Option<PaneState> {
        let removed = self.panes.remove(&id);
        if self.focused == Some(id) {
            self.focused = self.panes.keys().next().copied();
        }
        if removed.is_some() {
            tracing::info!(pane_id = %id, total = self.panes.len(), "pane removed");
        }
        removed
    }

    /// Get a reference to a pane's state.
    #[inline]
    #[must_use]
    pub fn get(&self, id: PaneId) -> Option<&PaneState> {
        self.panes.get(&id)
    }

    /// Get a mutable reference to a pane's state.
    #[inline]
    pub fn get_mut(&mut self, id: PaneId) -> Option<&mut PaneState> {
        self.panes.get_mut(&id)
    }

    /// Get a reference to the currently focused pane's state.
    #[must_use]
    pub fn focused_pane(&self) -> Option<(PaneId, &PaneState)> {
        let id = self.focused?;
        self.panes.get(&id).map(|state| (id, state))
    }

    /// Get a mutable reference to the currently focused pane's state.
    pub fn focused_pane_mut(&mut self) -> Option<(PaneId, &mut PaneState)> {
        let id = self.focused?;
        self.panes.get_mut(&id).map(|state| (id, state))
    }

    /// Get the ID of the currently focused pane.
    #[must_use]
    pub fn focused_id(&self) -> Option<PaneId> {
        self.focused
    }

    /// Set focus to a specific pane. Returns false if the pane doesn't exist.
    pub fn set_focused(&mut self, id: PaneId) -> bool {
        if self.panes.contains_key(&id) {
            self.focused = Some(id);
            tracing::debug!(pane_id = %id, "focus changed");
            true
        } else {
            false
        }
    }

    /// Number of active panes.
    #[must_use]
    pub fn pane_count(&self) -> usize {
        self.panes.len()
    }
}

impl Default for PaneRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface_manager::Surface;

    fn make_channels() -> (
        mpsc::Receiver<TerminalEvent>,
        mpsc::Sender<Vec<u8>>,
        mpsc::Sender<(u16, u16)>,
    ) {
        let (_event_tx, event_rx) = mpsc::channel(16);
        let (write_tx, _write_rx) = mpsc::channel(16);
        let (resize_tx, _resize_rx) = mpsc::channel(4);
        (event_rx, write_tx, resize_tx)
    }

    fn make_pane_state() -> PaneState {
        let (terminal, terminal_event_rx) = {
            let (event_tx, event_rx) = mpsc::channel(16);
            let mut terminal = Terminal::new(80, 24);
            terminal.set_event_sender(event_tx);
            (terminal, event_rx)
        };
        let (_, write_tx, resize_tx) = make_channels();
        PaneState {
            terminal,
            terminal_event_rx,
            pty_write_tx: write_tx,
            pty_resize_tx: resize_tx,
            process_exited: false,
            surfaces: SurfaceManager::new(Surface::new("shell")),
        }
    }

    #[test]
    fn register_first_pane_auto_focuses() {
        let mut reg = PaneRegistry::new();
        let id = PaneId::new();
        reg.register(id, make_pane_state());
        assert_eq!(reg.focused_id(), Some(id));
        assert_eq!(reg.pane_count(), 1);
    }

    #[test]
    fn register_second_pane_keeps_focus() {
        let mut reg = PaneRegistry::new();
        let id1 = PaneId::new();
        let id2 = PaneId::new();
        reg.register(id1, make_pane_state());
        reg.register(id2, make_pane_state());
        assert_eq!(reg.focused_id(), Some(id1));
        assert_eq!(reg.pane_count(), 2);
    }

    #[test]
    fn remove_focused_moves_focus() {
        let mut reg = PaneRegistry::new();
        let id1 = PaneId::new();
        let id2 = PaneId::new();
        reg.register(id1, make_pane_state());
        reg.register(id2, make_pane_state());
        reg.remove(id1);
        assert_eq!(reg.focused_id(), Some(id2));
        assert_eq!(reg.pane_count(), 1);
    }

    #[test]
    fn remove_last_pane_clears_focus() {
        let mut reg = PaneRegistry::new();
        let id = PaneId::new();
        reg.register(id, make_pane_state());
        reg.remove(id);
        assert_eq!(reg.focused_id(), None);
        assert_eq!(reg.pane_count(), 0);
    }

    #[test]
    fn set_focused_nonexistent_returns_false() {
        let mut reg = PaneRegistry::new();
        assert!(!reg.set_focused(PaneId::new()));
    }

    #[test]
    fn get_returns_pane() {
        let mut reg = PaneRegistry::new();
        let id = PaneId::new();
        reg.register(id, make_pane_state());
        assert!(reg.get(id).is_some());
        assert!(reg.get(PaneId::new()).is_none());
    }

    #[test]
    fn focused_pane_returns_state() {
        let mut reg = PaneRegistry::new();
        let id = PaneId::new();
        reg.register(id, make_pane_state());
        let (fid, _state) = reg.focused_pane().unwrap();
        assert_eq!(fid, id);
    }
}
