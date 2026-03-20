use serde::{Deserialize, Serialize};

use crate::types::SurfaceId;

/// Direction of a pane split.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SplitDirection {
    #[default]
    Horizontal,
    Vertical,
}

/// The kind of content a panel hosts.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PanelKind {
    #[default]
    Terminal,
    Browser,
}

/// Metadata describing a surface (tab) within a pane.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SurfaceInfo {
    pub id: SurfaceId,
    pub kind: PanelKind,
    pub title: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_direction_is_copy() {
        let d = SplitDirection::Horizontal;
        let d2 = d;
        assert_eq!(d, d2);
    }

    #[test]
    fn panel_kind_is_copy() {
        let k = PanelKind::Terminal;
        let k2 = k;
        assert_eq!(k, k2);
    }

    #[test]
    fn surface_info_creation() {
        let info = SurfaceInfo {
            id: SurfaceId::new(),
            kind: PanelKind::Browser,
            title: "Google".into(),
        };
        assert_eq!(info.kind, PanelKind::Browser);
        assert_eq!(info.title, "Google");
    }

    #[test]
    fn serde_roundtrip() {
        let info = SurfaceInfo {
            id: SurfaceId::new(),
            kind: PanelKind::Terminal,
            title: "bash".into(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let back: SurfaceInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(info, back);
    }
}
