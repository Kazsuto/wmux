use std::collections::HashMap;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::pane_registry::PaneRegistry;
use crate::pane_tree::PaneTree;
use crate::surface::SplitDirection;
use crate::types::PaneId;
use crate::workspace_manager::WorkspaceManager;

// Scrollback limits before serialization (ADR-0009).
const MAX_SCROLLBACK_LINES: usize = 4000;
const MAX_SCROLLBACK_CHARS: usize = 400_000;

// ─── Session Schema ───────────────────────────────────────────────────────────

/// Top-level session state — matches Architecture §6 schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub version: u32,
    pub workspaces: Vec<WorkspaceSnapshot>,
    pub active_workspace_index: usize,
    pub sidebar_width: u16,
    pub window: Option<WindowGeometry>,
}

/// Snapshot of a single workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSnapshot {
    pub id: String,
    pub name: String,
    pub pane_tree: Option<PaneTreeSnapshot>,
}

/// Recursive snapshot of the pane layout tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PaneTreeSnapshot {
    Leaf {
        surface_id: String,
        cwd: Option<String>,
        scrollback_text: Option<String>,
    },
    Split {
        /// "horizontal" or "vertical"
        direction: String,
        ratio: f32,
        first: Box<PaneTreeSnapshot>,
        second: Box<PaneTreeSnapshot>,
    },
}

/// Saved window position and size.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowGeometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub maximized: bool,
}

// ─── Build helpers ────────────────────────────────────────────────────────────

/// Build a `SessionState` from the current actor-owned state.
///
/// This is called from the auto-save timer inside the actor loop and
/// runs synchronously (serialization is fast, ~5-20ms for typical sessions).
pub fn build_session_state(
    workspace_manager: &WorkspaceManager,
    registry: &PaneRegistry,
    pane_cwds: &HashMap<PaneId, PathBuf>,
    sidebar_width: u16,
    window: Option<WindowGeometry>,
) -> SessionState {
    let active_workspace_index = workspace_manager.active_index();

    let workspaces = workspace_manager
        .iter()
        .map(|ws| {
            let pane_tree = ws
                .pane_tree()
                .map(|tree| snapshot_pane_tree(tree, registry, pane_cwds));
            WorkspaceSnapshot {
                id: ws.id().to_string(),
                name: ws.name().to_owned(),
                pane_tree,
            }
        })
        .collect();

    SessionState {
        version: 1,
        workspaces,
        active_workspace_index,
        sidebar_width,
        window,
    }
}

/// Recursively convert a `PaneTree` into a `PaneTreeSnapshot`, reading
/// scrollback from the registry and per-pane CWDs, applying truncation limits.
fn snapshot_pane_tree(
    tree: &PaneTree,
    registry: &PaneRegistry,
    pane_cwds: &HashMap<PaneId, PathBuf>,
) -> PaneTreeSnapshot {
    match tree {
        PaneTree::Leaf(pane_id) => {
            let (surface_id, scrollback_text) = if let Some(pane) = registry.get(*pane_id) {
                let active_surface_id = pane
                    .surfaces
                    .active()
                    .map(|s| s.id.to_string())
                    .unwrap_or_else(|| pane_id.to_string());

                let scrollback = pane.terminal.scrollback();
                let raw_text = if scrollback.is_empty() {
                    None
                } else {
                    let len = scrollback.len();
                    // Take at most MAX_SCROLLBACK_LINES from the end.
                    let start = len.saturating_sub(MAX_SCROLLBACK_LINES) as isize;
                    let text = scrollback.read_text(start, len as isize);
                    // Enforce character limit — keep the last MAX_SCROLLBACK_CHARS chars.
                    let char_count = text.chars().count();
                    let truncated = if char_count > MAX_SCROLLBACK_CHARS {
                        let skip = char_count - MAX_SCROLLBACK_CHARS;
                        let boundary = text.char_indices().nth(skip).map(|(i, _)| i).unwrap_or(0);
                        text[boundary..].to_owned()
                    } else {
                        text
                    };
                    if truncated.is_empty() {
                        None
                    } else {
                        Some(truncated)
                    }
                };

                (active_surface_id, raw_text)
            } else {
                (pane_id.to_string(), None)
            };

            let cwd = pane_cwds
                .get(pane_id)
                .map(|p| p.to_string_lossy().into_owned());

            PaneTreeSnapshot::Leaf {
                surface_id,
                cwd,
                scrollback_text,
            }
        }
        PaneTree::Split {
            direction,
            ratio,
            first,
            second,
        } => {
            let direction_str = match direction {
                SplitDirection::Horizontal => "horizontal",
                SplitDirection::Vertical => "vertical",
            };
            PaneTreeSnapshot::Split {
                direction: direction_str.to_owned(),
                ratio: *ratio,
                first: Box::new(snapshot_pane_tree(first, registry, pane_cwds)),
                second: Box::new(snapshot_pane_tree(second, registry, pane_cwds)),
            }
        }
    }
}

// ─── Restore helpers ─────────────────────────────────────────────────────────

/// Data extracted from the first (leftmost) leaf of a snapshot tree.
pub struct FirstLeafData<'a> {
    pub cwd: Option<&'a str>,
    pub scrollback_text: Option<&'a str>,
}

/// Extract data from the first (leftmost / topmost) leaf of a pane tree snapshot.
///
/// Used during session restore to populate the root pane before building the
/// rest of the tree structure via split operations.
#[must_use]
pub fn first_leaf(snapshot: &PaneTreeSnapshot) -> FirstLeafData<'_> {
    match snapshot {
        PaneTreeSnapshot::Leaf {
            cwd,
            scrollback_text,
            ..
        } => FirstLeafData {
            cwd: cwd.as_deref(),
            scrollback_text: scrollback_text.as_deref(),
        },
        PaneTreeSnapshot::Split { first, .. } => first_leaf(first),
    }
}

/// Maximum pane tree nesting depth. Protects against pathological session files
/// that would cause excessive recursion during restore.
const MAX_PANE_TREE_DEPTH: usize = 16;

/// Check that a pane tree snapshot does not exceed the depth limit.
fn validate_tree_depth(tree: &PaneTreeSnapshot, depth: usize) -> bool {
    if depth > MAX_PANE_TREE_DEPTH {
        return false;
    }
    match tree {
        PaneTreeSnapshot::Leaf { .. } => true,
        PaneTreeSnapshot::Split { first, second, .. } => {
            validate_tree_depth(first, depth + 1) && validate_tree_depth(second, depth + 1)
        }
    }
}

// ─── File I/O ─────────────────────────────────────────────────────────────────

/// Return the session file path: `%APPDATA%/wmux/session.json`.
///
/// Returns `None` if the config directory cannot be determined.
#[must_use]
pub fn session_file_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("wmux").join("session.json"))
}

/// Atomically save session state to disk.
///
/// Writes to a `.tmp` file first, then renames it to avoid corruption on
/// crash during write (ADR-0009).
pub async fn save_session(state: &SessionState) -> Result<(), io::Error> {
    let path = session_file_path().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "could not determine config directory",
        )
    })?;

    // Ensure parent directory exists.
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    // Serialize to JSON.
    let json = serde_json::to_vec_pretty(state)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    // Write to temp file then rename (atomic on Windows via MoveFileExW).
    let tmp_path = path.with_extension("tmp");
    tokio::fs::write(&tmp_path, &json).await?;
    tokio::fs::rename(&tmp_path, &path).await?;

    Ok(())
}

/// Current session schema version.
pub const SESSION_VERSION: u32 = 1;

/// Load session state from disk.
///
/// Returns `Ok(None)` if:
/// - The session file does not exist
/// - The file is corrupted or cannot be parsed
/// - The schema version does not match
///
/// Per ADR-0009: corrupted files NEVER cause a crash — log a warning and start fresh.
pub async fn load_session() -> Result<Option<SessionState>, io::Error> {
    let Some(path) = session_file_path() else {
        tracing::debug!("config directory not found, skipping session restore");
        return Ok(None);
    };

    let data = match tokio::fs::read(&path).await {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::debug!("no session file found, starting fresh");
            return Ok(None);
        }
        Err(e) => {
            tracing::warn!(
                path = %path.display(),
                error = %e,
                "failed to read session file, starting fresh"
            );
            return Ok(None);
        }
    };

    let mut state: SessionState = match serde_json::from_slice(&data) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(
                path = %path.display(),
                error = %e,
                "session file corrupted, starting fresh"
            );
            return Ok(None);
        }
    };

    if state.version != SESSION_VERSION {
        tracing::warn!(
            found = state.version,
            expected = SESSION_VERSION,
            "session schema version mismatch, starting fresh"
        );
        return Ok(None);
    }

    if state.workspaces.is_empty() {
        tracing::warn!("session has no workspaces, starting fresh");
        return Ok(None);
    }

    // Validate pane tree depth to prevent pathological session files.
    for ws in &state.workspaces {
        if let Some(ref tree) = ws.pane_tree {
            if !validate_tree_depth(tree, 0) {
                tracing::warn!(
                    workspace = %ws.name,
                    max_depth = MAX_PANE_TREE_DEPTH,
                    "pane tree too deep, starting fresh"
                );
                return Ok(None);
            }
        }
    }

    // Validate active_workspace_index is in bounds
    if state.active_workspace_index >= state.workspaces.len() {
        tracing::warn!(
            index = state.active_workspace_index,
            count = state.workspaces.len(),
            "active_workspace_index out of range, resetting to 0"
        );
        state.active_workspace_index = 0;
    }

    tracing::info!(
        workspace_count = state.workspaces.len(),
        active_index = state.active_workspace_index,
        "session loaded successfully"
    );

    Ok(Some(state))
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pane_registry::PaneState;
    use crate::surface_manager::{Surface, SurfaceManager};
    use crate::terminal::Terminal;
    use tokio::sync::mpsc;

    fn make_pane_state() -> PaneState {
        let (event_tx, event_rx) = mpsc::channel(16);
        let mut terminal = Terminal::new(80, 24);
        terminal.set_event_sender(event_tx);
        let (write_tx, _write_rx) = mpsc::channel::<Vec<u8>>(16);
        let (resize_tx, _resize_rx) = mpsc::channel::<(u16, u16)>(4);
        PaneState {
            terminal,
            terminal_event_rx: event_rx,
            pty_write_tx: write_tx,
            pty_resize_tx: resize_tx,
            process_exited: false,
            surfaces: SurfaceManager::new(Surface::new("shell", PaneId::new())),
        }
    }

    #[test]
    fn build_session_state_empty_workspace() {
        let wm = WorkspaceManager::new();
        let registry = PaneRegistry::new();

        let state = build_session_state(&wm, &registry, &HashMap::new(), 0, None);

        assert_eq!(state.version, 1);
        assert_eq!(state.workspaces.len(), 1);
        assert!(state.workspaces[0].pane_tree.is_none());
        assert_eq!(state.active_workspace_index, 0);
    }

    #[test]
    fn build_session_state_with_pane() {
        let mut wm = WorkspaceManager::new();
        let mut registry = PaneRegistry::new();

        let pane_id = PaneId::new();
        registry.register(pane_id, make_pane_state());

        let active = wm.active_mut();
        active.pane_tree = Some(crate::pane_tree::PaneTree::new(pane_id));

        let state = build_session_state(&wm, &registry, &HashMap::new(), 0, None);

        assert_eq!(state.version, 1);
        assert!(state.workspaces[0].pane_tree.is_some());

        // Check leaf node.
        if let Some(PaneTreeSnapshot::Leaf { .. }) = &state.workspaces[0].pane_tree {
        } else {
            panic!("expected Leaf variant");
        }
    }

    #[test]
    fn build_session_state_multiple_workspaces() {
        let mut wm = WorkspaceManager::new();
        wm.create("Second".to_string());
        wm.create("Third".to_string());
        let registry = PaneRegistry::new();

        let state = build_session_state(&wm, &registry, &HashMap::new(), 0, None);

        assert_eq!(state.workspaces.len(), 3);
        assert_eq!(state.workspaces[1].name, "Second");
        assert_eq!(state.workspaces[2].name, "Third");
    }

    #[test]
    fn session_state_serde_roundtrip() {
        let state = SessionState {
            version: 1,
            workspaces: vec![WorkspaceSnapshot {
                id: "ws-1".to_string(),
                name: "Main".to_string(),
                pane_tree: Some(PaneTreeSnapshot::Leaf {
                    surface_id: "surf-1".to_string(),
                    cwd: Some("/home/user".to_string()),
                    scrollback_text: Some("hello\nworld".to_string()),
                }),
            }],
            active_workspace_index: 0,
            sidebar_width: 200,
            window: Some(WindowGeometry {
                x: 100,
                y: 200,
                width: 1280,
                height: 720,
                maximized: false,
            }),
        };

        let json = serde_json::to_string(&state).unwrap();
        let back: SessionState = serde_json::from_str(&json).unwrap();

        assert_eq!(back.version, 1);
        assert_eq!(back.workspaces.len(), 1);
        assert_eq!(back.active_workspace_index, 0);
        assert_eq!(back.sidebar_width, 200);
        assert!(back.window.is_some());
        let w = back.window.unwrap();
        assert_eq!(w.x, 100);
        assert_eq!(w.width, 1280);
    }

    #[test]
    fn session_state_split_serde_roundtrip() {
        let state = SessionState {
            version: 1,
            workspaces: vec![WorkspaceSnapshot {
                id: "ws-1".to_string(),
                name: "Main".to_string(),
                pane_tree: Some(PaneTreeSnapshot::Split {
                    direction: "horizontal".to_string(),
                    ratio: 0.5,
                    first: Box::new(PaneTreeSnapshot::Leaf {
                        surface_id: "surf-1".to_string(),
                        cwd: None,
                        scrollback_text: None,
                    }),
                    second: Box::new(PaneTreeSnapshot::Leaf {
                        surface_id: "surf-2".to_string(),
                        cwd: None,
                        scrollback_text: None,
                    }),
                }),
            }],
            active_workspace_index: 0,
            sidebar_width: 0,
            window: None,
        };

        let json = serde_json::to_string(&state).unwrap();
        let back: SessionState = serde_json::from_str(&json).unwrap();

        if let Some(PaneTreeSnapshot::Split {
            direction, ratio, ..
        }) = &back.workspaces[0].pane_tree
        {
            assert_eq!(direction, "horizontal");
            assert!((ratio - 0.5_f32).abs() < f32::EPSILON);
        } else {
            panic!("expected Split variant");
        }
    }

    #[test]
    fn session_file_path_has_wmux_component() {
        let path = session_file_path();
        if let Some(p) = path {
            let as_str = p.to_string_lossy();
            assert!(
                as_str.contains("wmux"),
                "session path should contain 'wmux': {as_str}"
            );
            assert!(
                as_str.ends_with("session.json"),
                "session path should end with session.json: {as_str}"
            );
        }
        // If config_dir returns None (CI without home dir), test is skipped.
    }

    #[test]
    fn scrollback_truncation_constants() {
        // Verify the truncation constants are set to the spec requirements.
        assert_eq!(MAX_SCROLLBACK_LINES, 4000);
        assert_eq!(MAX_SCROLLBACK_CHARS, 400_000);
    }

    #[test]
    fn build_session_state_with_registered_pane() {
        let mut wm = WorkspaceManager::new();
        let mut registry = PaneRegistry::new();
        let pane_id = PaneId::new();

        registry.register(pane_id, make_pane_state());
        let active = wm.active_mut();
        active.pane_tree = Some(crate::pane_tree::PaneTree::new(pane_id));

        // Build state — should succeed even with empty scrollback.
        let state = build_session_state(&wm, &registry, &HashMap::new(), 0, None);
        assert_eq!(state.version, 1);
        assert!(state.workspaces[0].pane_tree.is_some());
    }

    #[test]
    fn build_session_state_captures_pane_cwd() {
        let mut wm = WorkspaceManager::new();
        let mut registry = PaneRegistry::new();
        let pane_id = PaneId::new();

        registry.register(pane_id, make_pane_state());
        let active = wm.active_mut();
        active.pane_tree = Some(crate::pane_tree::PaneTree::new(pane_id));

        let mut cwds = HashMap::new();
        cwds.insert(pane_id, PathBuf::from("F:/Workspaces/wmux"));

        let state = build_session_state(&wm, &registry, &cwds, 260, None);
        assert_eq!(state.sidebar_width, 260);

        if let Some(PaneTreeSnapshot::Leaf { cwd, .. }) = &state.workspaces[0].pane_tree {
            assert_eq!(cwd.as_deref(), Some("F:/Workspaces/wmux"));
        } else {
            panic!("expected Leaf with CWD");
        }
    }

    #[test]
    fn build_session_state_captures_window_geometry() {
        let wm = WorkspaceManager::new();
        let registry = PaneRegistry::new();
        let geom = WindowGeometry {
            x: 50,
            y: 100,
            width: 1920,
            height: 1080,
            maximized: true,
        };

        let state = build_session_state(&wm, &registry, &HashMap::new(), 0, Some(geom));
        let w = state.window.unwrap();
        assert_eq!(w.x, 50);
        assert_eq!(w.width, 1920);
        assert!(w.maximized);
    }

    #[test]
    fn first_leaf_extracts_leftmost_leaf() {
        let tree = PaneTreeSnapshot::Split {
            direction: "horizontal".to_string(),
            ratio: 0.5,
            first: Box::new(PaneTreeSnapshot::Split {
                direction: "vertical".to_string(),
                ratio: 0.5,
                first: Box::new(PaneTreeSnapshot::Leaf {
                    surface_id: "a".to_string(),
                    cwd: Some("/deep/leaf".to_string()),
                    scrollback_text: Some("deep content".to_string()),
                }),
                second: Box::new(PaneTreeSnapshot::Leaf {
                    surface_id: "b".to_string(),
                    cwd: None,
                    scrollback_text: None,
                }),
            }),
            second: Box::new(PaneTreeSnapshot::Leaf {
                surface_id: "c".to_string(),
                cwd: None,
                scrollback_text: None,
            }),
        };

        let leaf = first_leaf(&tree);
        assert_eq!(leaf.cwd, Some("/deep/leaf"));
        assert_eq!(leaf.scrollback_text, Some("deep content"));
    }

    #[test]
    fn window_geometry_maximized_defaults_false() {
        // Backward compat: old session files without `maximized` field.
        let json = r#"{"x":0,"y":0,"width":800,"height":600}"#;
        let geom: WindowGeometry = serde_json::from_str(json).unwrap();
        assert!(!geom.maximized);
    }
}
