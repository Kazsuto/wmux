use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! define_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[repr(transparent)]
        pub struct $name(Uuid);

        impl $name {
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

        impl Default for $name {
            /// Returns a nil (all-zeros) identifier. Use [`Self::new`] for random IDs.
            fn default() -> Self {
                Self(Uuid::nil())
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl std::str::FromStr for $name {
            type Err = uuid::Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(Uuid::parse_str(s)?))
            }
        }
    };
}

define_id!(
    /// Unique identifier for a top-level window.
    WindowId
);

define_id!(
    /// Unique identifier for a workspace within a window.
    WorkspaceId
);

define_id!(
    /// Unique identifier for a pane (split region) within a workspace.
    PaneId
);

define_id!(
    /// Unique identifier for a surface (tab) within a pane.
    SurfaceId
);

define_id!(
    /// Unique identifier for a split node in the pane tree.
    SplitId
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        let a = WindowId::new();
        let b = WindowId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn id_display_matches_uuid() {
        let id = PaneId::new();
        assert_eq!(id.to_string(), id.as_uuid().to_string());
    }

    #[test]
    fn id_debug_includes_type_name() {
        let id = SurfaceId::new();
        let debug = format!("{:?}", id);
        assert!(debug.starts_with("SurfaceId("));
    }

    #[test]
    fn id_is_copy() {
        let id = WorkspaceId::new();
        let id2 = id;
        assert_eq!(id, id2);
    }

    #[test]
    fn id_hash_as_map_key() {
        use std::collections::HashMap;
        let mut map = HashMap::new();
        let id = WindowId::new();
        map.insert(id, "test");
        assert_eq!(map[&id], "test");
    }

    #[test]
    fn serde_roundtrip() {
        let id = PaneId::new();
        let json = serde_json::to_string(&id).unwrap();
        let back: PaneId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    fn _assert_send<T: Send>() {}
    fn _assert_sync<T: Sync>() {}

    #[test]
    fn ids_are_send_and_sync() {
        _assert_send::<WindowId>();
        _assert_sync::<WindowId>();
        _assert_send::<WorkspaceId>();
        _assert_sync::<WorkspaceId>();
        _assert_send::<PaneId>();
        _assert_sync::<PaneId>();
        _assert_send::<SurfaceId>();
        _assert_sync::<SurfaceId>();
        _assert_send::<SplitId>();
        _assert_sync::<SplitId>();
    }
}
