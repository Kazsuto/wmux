use std::fmt;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::{SurfaceId, WorkspaceId};

/// Unique identifier for a notification.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(transparent)]
pub struct NotificationId(Uuid);

impl NotificationId {
    /// Create a new random identifier.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Wrap an existing UUID.
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Return the inner UUID.
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for NotificationId {
    /// Returns a nil (all-zeros) identifier. Use [`Self::new`] for random IDs.
    fn default() -> Self {
        Self(Uuid::nil())
    }
}

impl fmt::Debug for NotificationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NotificationId({})", self.0)
    }
}

impl fmt::Display for NotificationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// State of a notification in the store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationState {
    /// Notification has been received but not yet presented.
    Received,
    /// Notification is unread.
    Unread,
    /// Notification has been read.
    Read,
    /// Notification has been cleared by the user.
    Cleared,
}

/// Source of a notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationSource {
    /// Notification from OSC 9/99/777 escape sequence.
    Osc,
    /// Notification from API call.
    Api,
    /// Notification from internal system.
    Internal,
}

/// A notification message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    /// Unique identifier for this notification.
    pub id: NotificationId,
    /// Optional title.
    pub title: Option<String>,
    /// Main notification body.
    pub body: String,
    /// Optional subtitle.
    pub subtitle: Option<String>,
    /// Source of the notification.
    pub source: NotificationSource,
    /// Workspace that generated this notification (if any).
    pub source_workspace: Option<WorkspaceId>,
    /// Surface (tab) that generated this notification (if any).
    pub source_surface: Option<SurfaceId>,
    /// When the notification was created.
    pub timestamp: SystemTime,
    /// Current state of the notification.
    pub state: NotificationState,
}

/// Event emitted when notification state changes.
#[derive(Debug, Clone)]
pub enum NotificationEvent {
    /// A new notification was added to the store.
    Added {
        /// ID of the new notification.
        notification_id: NotificationId,
        /// Whether the notification was suppressed (desktop alert not shown).
        suppressed: bool,
    },
    /// A notification's state transitioned.
    StateChanged {
        /// ID of the notification.
        notification_id: NotificationId,
        /// Previous state.
        old_state: NotificationState,
        /// New state.
        new_state: NotificationState,
    },
    /// A notification was cleared.
    Cleared {
        /// ID of the cleared notification.
        notification_id: NotificationId,
    },
}

/// In-memory store for notifications with capacity management.
#[derive(Debug)]
pub struct NotificationStore {
    notifications: Vec<Notification>,
    max_capacity: usize,
}

impl NotificationStore {
    /// Default maximum capacity for the notification store.
    pub const DEFAULT_MAX_CAPACITY: usize = 200;

    /// Create a new notification store with default capacity.
    #[must_use]
    pub fn new() -> Self {
        Self {
            notifications: Vec::new(),
            max_capacity: Self::DEFAULT_MAX_CAPACITY,
        }
    }

    /// Create a new notification store with a specific capacity.
    ///
    /// Capacity must be at least 1. A value of 0 is clamped to 1.
    #[must_use]
    pub fn with_capacity(max: usize) -> Self {
        Self {
            notifications: Vec::new(),
            max_capacity: max.max(1),
        }
    }

    /// Add a new notification to the store.
    ///
    /// If the store is at capacity, evicts the oldest cleared notification first,
    /// then the oldest notification regardless.
    #[must_use]
    pub fn add(
        &mut self,
        title: Option<String>,
        body: String,
        subtitle: Option<String>,
        source: NotificationSource,
        workspace: Option<WorkspaceId>,
        surface: Option<SurfaceId>,
    ) -> (NotificationId, NotificationEvent) {
        // Enforce capacity
        if self.notifications.len() >= self.max_capacity {
            // Try to evict oldest cleared notification
            if let Some(pos) = self
                .notifications
                .iter()
                .position(|n| n.state == NotificationState::Cleared)
            {
                self.notifications.remove(pos);
            } else {
                // Evict oldest regardless
                self.notifications.remove(0);
            }
        }

        let id = NotificationId::new();
        let notification = Notification {
            id,
            title,
            body,
            subtitle,
            source,
            source_workspace: workspace,
            source_surface: surface,
            timestamp: SystemTime::now(),
            state: NotificationState::Received,
        };

        self.notifications.push(notification);

        let event = NotificationEvent::Added {
            notification_id: id,
            suppressed: false,
        };

        (id, event)
    }

    /// Transition a notification's state.
    ///
    /// Only forward transitions are allowed: Received → Unread → Read → Cleared.
    /// Returns the emitted event if the notification was found and the transition
    /// is valid, None otherwise.
    pub fn transition(
        &mut self,
        id: NotificationId,
        new_state: NotificationState,
    ) -> Option<NotificationEvent> {
        self.notifications
            .iter_mut()
            .find(|n| n.id == id)
            .and_then(|n| {
                let old_state = n.state;
                if Self::is_valid_transition(old_state, new_state) {
                    n.state = new_state;
                    Some(NotificationEvent::StateChanged {
                        notification_id: id,
                        old_state,
                        new_state,
                    })
                } else {
                    tracing::warn!(
                        ?old_state,
                        ?new_state,
                        %id,
                        "invalid notification state transition",
                    );
                    None
                }
            })
    }

    /// Check if a state transition is valid (forward-only).
    fn is_valid_transition(from: NotificationState, to: NotificationState) -> bool {
        matches!(
            (from, to),
            (NotificationState::Received, NotificationState::Unread)
                | (NotificationState::Received, NotificationState::Read)
                | (NotificationState::Received, NotificationState::Cleared)
                | (NotificationState::Unread, NotificationState::Read)
                | (NotificationState::Unread, NotificationState::Cleared)
                | (NotificationState::Read, NotificationState::Cleared)
        )
    }

    /// Mark all received/unread notifications from a workspace as read.
    pub fn mark_workspace_read(&mut self, workspace_id: WorkspaceId) -> Vec<NotificationEvent> {
        self.notifications
            .iter_mut()
            .filter_map(|n| {
                if n.source_workspace == Some(workspace_id)
                    && (n.state == NotificationState::Received
                        || n.state == NotificationState::Unread)
                {
                    let old_state = n.state;
                    n.state = NotificationState::Read;
                    Some(NotificationEvent::StateChanged {
                        notification_id: n.id,
                        old_state,
                        new_state: NotificationState::Read,
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// Clear a notification by ID.
    ///
    /// Returns the emitted event if the notification was found, None otherwise.
    pub fn clear(&mut self, id: NotificationId) -> Option<NotificationEvent> {
        if let Some(n) = self.notifications.iter_mut().find(|n| n.id == id) {
            n.state = NotificationState::Cleared;
            Some(NotificationEvent::Cleared {
                notification_id: id,
            })
        } else {
            None
        }
    }

    /// Clear all non-cleared notifications.
    pub fn clear_all(&mut self) -> Vec<NotificationEvent> {
        self.notifications
            .iter_mut()
            .filter_map(|n| {
                if n.state != NotificationState::Cleared {
                    n.state = NotificationState::Cleared;
                    Some(NotificationEvent::Cleared {
                        notification_id: n.id,
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// Look up a notification by ID.
    #[must_use]
    pub fn get(&self, id: NotificationId) -> Option<&Notification> {
        self.notifications.iter().find(|n| n.id == id)
    }

    /// List notifications, optionally filtered by state, newest first.
    ///
    /// Returns up to `limit` notifications sorted by timestamp (newest first).
    #[must_use]
    pub fn list(&self, filter: Option<NotificationState>, limit: usize) -> Vec<&Notification> {
        let mut result: Vec<_> = self
            .notifications
            .iter()
            .filter(|n| filter.is_none_or(|f| n.state == f))
            .collect();

        result.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        result.truncate(limit);
        result
    }

    /// Count unread notifications (Received + Unread) for a workspace.
    #[must_use]
    pub fn unread_count(&self, workspace_id: WorkspaceId) -> usize {
        self.notifications
            .iter()
            .filter(|n| {
                n.source_workspace == Some(workspace_id)
                    && (n.state == NotificationState::Received
                        || n.state == NotificationState::Unread)
            })
            .count()
    }

    /// Count total unread notifications (Received + Unread).
    #[must_use]
    pub fn total_unread_count(&self) -> usize {
        self.notifications
            .iter()
            .filter(|n| {
                n.state == NotificationState::Received || n.state == NotificationState::Unread
            })
            .count()
    }

    /// Determine if a desktop alert should be suppressed for an incoming notification.
    ///
    /// Returns true if wmux is focused AND the notification's source workspace
    /// matches the active workspace (the user is already looking at it).
    #[must_use]
    pub fn should_suppress(
        &self,
        notification_workspace: Option<WorkspaceId>,
        active_workspace: Option<WorkspaceId>,
        wmux_focused: bool,
    ) -> bool {
        if !wmux_focused {
            return false;
        }

        // Suppress only if the notification comes from the workspace the user is viewing
        match (notification_workspace, active_workspace) {
            (Some(nw), Some(aw)) => nw == aw,
            _ => false,
        }
    }

    /// Return the number of notifications in the store.
    #[must_use]
    pub fn len(&self) -> usize {
        self.notifications.len()
    }

    /// Return true if the store is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.notifications.is_empty()
    }
}

impl Default for NotificationStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_notification() {
        let mut store = NotificationStore::new();
        let (id, event) = store.add(
            Some("Title".to_string()),
            "Body".to_string(),
            None,
            NotificationSource::Api,
            None,
            None,
        );

        assert!(matches!(
            event,
            NotificationEvent::Added {
                suppressed: false,
                ..
            }
        ));
        let notif = store.get(id).expect("notification should exist");
        assert_eq!(notif.title, Some("Title".to_string()));
        assert_eq!(notif.body, "Body");
        assert_eq!(notif.state, NotificationState::Received);
    }

    #[test]
    fn test_lifecycle_transitions() {
        let mut store = NotificationStore::new();
        let (id, _) = store.add(
            None,
            "test".to_string(),
            None,
            NotificationSource::Api,
            None,
            None,
        );

        // Received -> Unread
        let event = store.transition(id, NotificationState::Unread).unwrap();
        assert!(matches!(
            event,
            NotificationEvent::StateChanged {
                old_state: NotificationState::Received,
                new_state: NotificationState::Unread,
                ..
            }
        ));

        // Unread -> Read
        let event = store.transition(id, NotificationState::Read).unwrap();
        assert!(matches!(
            event,
            NotificationEvent::StateChanged {
                old_state: NotificationState::Unread,
                new_state: NotificationState::Read,
                ..
            }
        ));

        // Read -> Cleared
        let event = store.transition(id, NotificationState::Cleared).unwrap();
        assert!(matches!(
            event,
            NotificationEvent::StateChanged {
                old_state: NotificationState::Read,
                new_state: NotificationState::Cleared,
                ..
            }
        ));
    }

    #[test]
    fn test_invalid_transitions_rejected() {
        let mut store = NotificationStore::new();
        let (id, _) = store.add(
            None,
            "test".to_string(),
            None,
            NotificationSource::Api,
            None,
            None,
        );

        // Forward to Read
        store.transition(id, NotificationState::Read);

        // Backward Read -> Unread should be rejected
        assert!(store.transition(id, NotificationState::Unread).is_none());
        // Backward Read -> Received should be rejected
        assert!(store.transition(id, NotificationState::Received).is_none());
        // State should still be Read
        assert_eq!(store.get(id).unwrap().state, NotificationState::Read);

        // Forward to Cleared
        store.transition(id, NotificationState::Cleared);
        // Backward from Cleared should be rejected
        assert!(store.transition(id, NotificationState::Read).is_none());
    }

    #[test]
    fn test_with_capacity_zero_clamped() {
        let store = NotificationStore::with_capacity(0);
        // Capacity 0 is clamped to 1, so adding should work without panic
        let mut store = store;
        let (_id, _event) = store.add(
            None,
            "test".to_string(),
            None,
            NotificationSource::Api,
            None,
            None,
        );
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn test_mark_workspace_read() {
        let ws_id = WorkspaceId::new();
        let other_ws_id = WorkspaceId::new();

        let mut store = NotificationStore::new();
        let (id1, _) = store.add(
            None,
            "test1".to_string(),
            None,
            NotificationSource::Api,
            Some(ws_id),
            None,
        );
        let (id2, _) = store.add(
            None,
            "test2".to_string(),
            None,
            NotificationSource::Api,
            Some(other_ws_id),
            None,
        );
        let (id3, _) = store.add(
            None,
            "test3".to_string(),
            None,
            NotificationSource::Api,
            Some(ws_id),
            None,
        );

        // Transition some to Unread
        store.transition(id1, NotificationState::Unread);

        let events = store.mark_workspace_read(ws_id);
        assert_eq!(events.len(), 2); // id1 and id3

        // id2 should still be Received (different workspace)
        assert_eq!(store.get(id2).unwrap().state, NotificationState::Received);
        // id1 and id3 should be Read
        assert_eq!(store.get(id1).unwrap().state, NotificationState::Read);
        assert_eq!(store.get(id3).unwrap().state, NotificationState::Read);
    }

    #[test]
    fn test_clear_and_clear_all() {
        let mut store = NotificationStore::new();
        let (id1, _) = store.add(
            None,
            "test1".to_string(),
            None,
            NotificationSource::Api,
            None,
            None,
        );
        let (id2, _) = store.add(
            None,
            "test2".to_string(),
            None,
            NotificationSource::Api,
            None,
            None,
        );
        let (id3, _) = store.add(
            None,
            "test3".to_string(),
            None,
            NotificationSource::Api,
            None,
            None,
        );

        // Clear one
        let event = store.clear(id1).unwrap();
        assert!(matches!(event, NotificationEvent::Cleared { .. }));
        assert_eq!(store.get(id1).unwrap().state, NotificationState::Cleared);

        // Clear all remaining
        let events = store.clear_all();
        assert_eq!(events.len(), 2); // id2 and id3
        assert_eq!(store.get(id2).unwrap().state, NotificationState::Cleared);
        assert_eq!(store.get(id3).unwrap().state, NotificationState::Cleared);
    }

    #[test]
    fn test_unread_count() {
        let ws_id = WorkspaceId::new();
        let other_ws_id = WorkspaceId::new();

        let mut store = NotificationStore::new();
        let (id1, _) = store.add(
            None,
            "test1".to_string(),
            None,
            NotificationSource::Api,
            Some(ws_id),
            None,
        );
        let (_, _) = store.add(
            None,
            "test2".to_string(),
            None,
            NotificationSource::Api,
            Some(ws_id),
            None,
        );
        let (id3, _) = store.add(
            None,
            "test3".to_string(),
            None,
            NotificationSource::Api,
            Some(other_ws_id),
            None,
        );

        assert_eq!(store.unread_count(ws_id), 2);
        assert_eq!(store.unread_count(other_ws_id), 1);
        assert_eq!(store.total_unread_count(), 3);

        // Mark one as read
        store.transition(id1, NotificationState::Read);
        assert_eq!(store.unread_count(ws_id), 1);
        assert_eq!(store.total_unread_count(), 2);

        // Clear one
        store.clear(id3);
        assert_eq!(store.unread_count(other_ws_id), 0);
        assert_eq!(store.total_unread_count(), 1);
    }

    #[test]
    fn test_cap_enforcement() {
        let mut store = NotificationStore::with_capacity(10);

        // Add 15 notifications
        let mut ids = Vec::new();
        for i in 0..15 {
            let (id, _) = store.add(
                None,
                format!("notif {}", i),
                None,
                NotificationSource::Api,
                None,
                None,
            );
            ids.push(id);
        }

        // Store should have exactly 10
        assert_eq!(store.len(), 10);

        // The first 5 should have been evicted
        for i in 0..5 {
            assert!(store.get(ids[i]).is_none());
        }

        // The last 10 should exist
        for i in 5..15 {
            assert!(store.get(ids[i]).is_some());
        }
    }

    #[test]
    fn test_cap_enforcement_clears_cleared_first() {
        let mut store = NotificationStore::with_capacity(10);

        // Add 10 notifications
        let mut ids = Vec::new();
        for i in 0..10 {
            let (id, _) = store.add(
                None,
                format!("notif {}", i),
                None,
                NotificationSource::Api,
                None,
                None,
            );
            ids.push(id);
        }

        // Clear the oldest one
        store.clear(ids[0]);

        // Add one more (capacity exceeded)
        let (new_id, _) = store.add(
            None,
            "notif 10".to_string(),
            None,
            NotificationSource::Api,
            None,
            None,
        );

        // The cleared notification (ids[0]) should be gone
        assert!(store.get(ids[0]).is_none());
        // The second notification should still exist
        assert!(store.get(ids[1]).is_some());
        // New notification should exist
        assert!(store.get(new_id).is_some());
    }

    #[test]
    fn test_should_suppress() {
        let ws_id = WorkspaceId::new();
        let other_ws_id = WorkspaceId::new();

        let mut store = NotificationStore::new();
        let (_id, _) = store.add(
            None,
            "test".to_string(),
            None,
            NotificationSource::Api,
            Some(ws_id),
            None,
        );

        // Suppress when wmux focused and notification workspace matches active
        assert!(store.should_suppress(Some(ws_id), Some(ws_id), true));

        // Don't suppress when wmux not focused
        assert!(!store.should_suppress(Some(ws_id), Some(ws_id), false));

        // Don't suppress when workspace doesn't match
        assert!(!store.should_suppress(Some(ws_id), Some(other_ws_id), true));

        // Don't suppress when no active workspace
        assert!(!store.should_suppress(Some(ws_id), None, true));

        // Don't suppress when notification has no workspace
        assert!(!store.should_suppress(None, Some(ws_id), true));
    }

    #[test]
    fn test_list_with_filter() {
        let mut store = NotificationStore::new();
        let (id1, _) = store.add(
            None,
            "test1".to_string(),
            None,
            NotificationSource::Api,
            None,
            None,
        );
        let (id2, _) = store.add(
            None,
            "test2".to_string(),
            None,
            NotificationSource::Api,
            None,
            None,
        );
        let (id3, _) = store.add(
            None,
            "test3".to_string(),
            None,
            NotificationSource::Api,
            None,
            None,
        );

        // Mark one as read, one as cleared
        store.transition(id2, NotificationState::Read);
        store.clear(id3);

        // List received (should have id1)
        let received = store.list(Some(NotificationState::Received), 10);
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].id, id1);

        // List read (should have id2)
        let read = store.list(Some(NotificationState::Read), 10);
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].id, id2);

        // List all (should return newest first)
        let all = store.list(None, 10);
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].id, id3); // Most recent
        assert_eq!(all[1].id, id2);
        assert_eq!(all[2].id, id1); // Oldest
    }

    #[test]
    fn test_notification_id_uniqueness() {
        let id1 = NotificationId::new();
        let id2 = NotificationId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_notification_id_display() {
        let id = NotificationId::new();
        let uuid_str = id.as_uuid().to_string();
        assert_eq!(id.to_string(), uuid_str);
    }

    #[test]
    fn test_notification_id_debug() {
        let id = NotificationId::new();
        let debug_str = format!("{:?}", id);
        assert!(debug_str.starts_with("NotificationId("));
    }
}
