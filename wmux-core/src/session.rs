use std::collections::HashMap;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::pane_registry::PaneRegistry;
use crate::pane_tree::PaneTree;
use crate::process_detect::{
    list_recent_claude_sessions, query_claude_session_ids_from_cmdline, ProcessSnapshot,
};
use crate::surface::SplitDirection;
use crate::types::PaneId;
use crate::workspace_manager::WorkspaceManager;

// Scrollback limits before serialization (ADR-0009).
const MAX_SCROLLBACK_LINES: usize = 4000;
const MAX_SCROLLBACK_CHARS: usize = 400_000;

/// Marker value stored in `claude_session_id` when Claude Code is detected
/// but the exact session UUID is unknown. At restore, this triggers
/// `claude --continue` (resume most recent session for the CWD).
pub const CLAUDE_CONTINUE_MARKER: &str = "__continue__";

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
        /// Claude Code session marker. When present, the pane was running Claude
        /// Code at save time and should be restored with `claude --continue` (or
        /// `claude --resume <id>` if a specific session ID is stored).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        claude_session_id: Option<String>,
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
    pane_pids: &HashMap<PaneId, u32>,
    known_claude_sessions: &HashMap<PaneId, String>,
    sidebar_width: u16,
    window: Option<WindowGeometry>,
) -> SessionState {
    let active_workspace_index = workspace_manager.active_index();

    // Capture a single process snapshot for all panes (one kernel call).
    let proc_snapshot = if pane_pids.is_empty() {
        None
    } else {
        Some(ProcessSnapshot::capture())
    };

    // Resolve Claude Code session UUIDs.
    // Priority: known sessions (from --resume at restore time) > filesystem heuristic.
    let claude_uuids = match proc_snapshot.as_ref() {
        Some(snap) => {
            resolve_claude_session_uuids(pane_pids, pane_cwds, snap, known_claude_sessions)
        }
        None => HashMap::new(),
    };

    let workspaces = workspace_manager
        .iter()
        .map(|ws| {
            let pane_tree = ws.pane_tree().map(|tree| {
                snapshot_pane_tree(
                    tree,
                    registry,
                    pane_cwds,
                    pane_pids,
                    proc_snapshot.as_ref(),
                    &claude_uuids,
                )
            });
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

/// Resolve Claude Code session UUIDs for panes that are running Claude.
///
/// Uses two strategies in priority order:
/// 1. **Known sessions** (`known_claude_sessions`): UUIDs remembered from the
///    previous restore cycle (`claude --resume <uuid>`). These are exact matches.
/// 2. **Filesystem heuristic**: For panes without a known UUID, groups by CWD and
///    lists the N most recently modified `.jsonl` files, excluding already-claimed UUIDs.
fn resolve_claude_session_uuids(
    pane_pids: &HashMap<PaneId, u32>,
    pane_cwds: &HashMap<PaneId, PathBuf>,
    proc_snapshot: &ProcessSnapshot,
    known_claude_sessions: &HashMap<PaneId, String>,
) -> HashMap<PaneId, String> {
    // 1. Find all panes running Claude Code.
    let claude_panes: Vec<PaneId> = pane_pids
        .iter()
        .filter(|(_, &pid)| proc_snapshot.has_claude_descendant(pid))
        .map(|(&pane_id, _)| pane_id)
        .collect();

    if claude_panes.is_empty() {
        return HashMap::new();
    }

    let mut result: HashMap<PaneId, String> = HashMap::new();

    // 2. Use known session UUIDs first (exact match from restore).
    let mut claimed_uuids: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut unresolved_panes: Vec<PaneId> = Vec::new();
    for &pane_id in &claude_panes {
        if let Some(uuid) = known_claude_sessions.get(&pane_id) {
            tracing::debug!(pane_id = %pane_id, uuid = %uuid, "using known Claude session UUID");
            claimed_uuids.insert(uuid.clone());
            result.insert(pane_id, uuid.clone());
        } else {
            unresolved_panes.push(pane_id);
        }
    }

    // 3. For remaining panes, resolve via WMI command-line query.
    // This gives exact PID→UUID mapping for restored sessions (--resume <uuid>).
    if !unresolved_panes.is_empty() {
        // Find the Claude PID for each unresolved pane.
        let mut pane_to_claude_pid: Vec<(PaneId, u32)> = Vec::new();
        for &pane_id in &unresolved_panes {
            if let Some(&root_pid) = pane_pids.get(&pane_id) {
                if let Some(claude_pid) = proc_snapshot.find_claude_pid(root_pid) {
                    pane_to_claude_pid.push((pane_id, claude_pid));
                }
            }
        }

        if !pane_to_claude_pid.is_empty() {
            let claude_pids: Vec<u32> = pane_to_claude_pid.iter().map(|(_, pid)| *pid).collect();
            let cmdline_uuids = query_claude_session_ids_from_cmdline(&claude_pids);

            let mut newly_resolved = Vec::new();
            for &(pane_id, claude_pid) in &pane_to_claude_pid {
                if let Some(uuid) = cmdline_uuids.get(&claude_pid) {
                    tracing::debug!(pane_id = %pane_id, uuid = %uuid, claude_pid, "resolved UUID from command line");
                    claimed_uuids.insert(uuid.clone());
                    result.insert(pane_id, uuid.clone());
                    newly_resolved.push(pane_id);
                }
            }

            // Remove resolved panes from the unresolved list.
            unresolved_panes.retain(|p| !newly_resolved.contains(p));
        }
    }

    // 4. For still-unresolved panes, resolve via filesystem heuristic.
    if !unresolved_panes.is_empty() {
        let mut by_cwd: HashMap<String, Vec<PaneId>> = HashMap::new();
        for pane_id in &unresolved_panes {
            if let Some(cwd) = pane_cwds.get(pane_id) {
                let cwd_str = cwd.to_string_lossy().into_owned();
                by_cwd.entry(cwd_str).or_default().push(*pane_id);
            }
        }

        for (cwd, pane_ids) in &by_cwd {
            // Request extra candidates to account for already-claimed UUIDs.
            let uuids = list_recent_claude_sessions(cwd, pane_ids.len() + claimed_uuids.len());
            // Filter out UUIDs already assigned to known-session panes.
            let available: Vec<&String> = uuids
                .iter()
                .filter(|u| !claimed_uuids.contains(*u))
                .collect();
            for (pane_id, uuid) in pane_ids.iter().zip(available.iter()) {
                result.insert(*pane_id, (*uuid).clone());
            }
            let resolved = available.len().min(pane_ids.len());
            let unresolved = pane_ids.len().saturating_sub(resolved);
            if unresolved > 0 {
                tracing::warn!(
                    cwd = cwd,
                    resolved,
                    unresolved,
                    "fewer Claude session files than panes, some will use --continue"
                );
            }
        }
    }

    tracing::debug!(
        total_claude_panes = claude_panes.len(),
        resolved = result.len(),
        "Claude session UUID resolution complete"
    );

    result
}

/// Recursively convert a `PaneTree` into a `PaneTreeSnapshot`, reading
/// scrollback from the registry and per-pane CWDs, applying truncation limits.
/// Detects Claude Code sessions via the process snapshot when available.
fn snapshot_pane_tree(
    tree: &PaneTree,
    registry: &PaneRegistry,
    pane_cwds: &HashMap<PaneId, PathBuf>,
    pane_pids: &HashMap<PaneId, u32>,
    proc_snapshot: Option<&ProcessSnapshot>,
    claude_uuids: &HashMap<PaneId, String>,
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

            // Detect Claude Code running in this pane via process tree inspection.
            // Use resolved UUID if available, otherwise fall back to __continue__.
            let claude_session_id = pane_pids.get(pane_id).and_then(|&pid| match proc_snapshot {
                Some(snap) if snap.has_claude_descendant(pid) => {
                    if let Some(uuid) = claude_uuids.get(pane_id) {
                        tracing::debug!(pane_id = %pane_id, uuid = %uuid, "resolved Claude session UUID");
                        Some(uuid.clone())
                    } else {
                        tracing::debug!(pane_id = %pane_id, pid, "Claude detected but UUID unresolved, using --continue fallback");
                        Some(CLAUDE_CONTINUE_MARKER.to_owned())
                    }
                }
                _ => None,
            });

            PaneTreeSnapshot::Leaf {
                surface_id,
                cwd,
                scrollback_text,
                claude_session_id,
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
                first: Box::new(snapshot_pane_tree(
                    first,
                    registry,
                    pane_cwds,
                    pane_pids,
                    proc_snapshot,
                    claude_uuids,
                )),
                second: Box::new(snapshot_pane_tree(
                    second,
                    registry,
                    pane_cwds,
                    pane_pids,
                    proc_snapshot,
                    claude_uuids,
                )),
            }
        }
    }
}

// ─── Restore helpers ─────────────────────────────────────────────────────────

/// Data extracted from the first (leftmost) leaf of a snapshot tree.
pub struct FirstLeafData<'a> {
    pub cwd: Option<&'a str>,
    pub scrollback_text: Option<&'a str>,
    pub claude_session_id: Option<&'a str>,
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
            claude_session_id,
            ..
        } => FirstLeafData {
            cwd: cwd.as_deref(),
            scrollback_text: scrollback_text.as_deref(),
            claude_session_id: claude_session_id.as_deref(),
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

    // Sanitize claude_session_id values — only allow the known marker or UUID format.
    // Defense-in-depth: prevents arbitrary strings from being passed to CLI args in future.
    for ws in &mut state.workspaces {
        if let Some(ref mut tree) = ws.pane_tree {
            sanitize_claude_session_ids(tree);
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

/// Recursively sanitize `claude_session_id` values in a pane tree.
///
/// Only allows the `CLAUDE_CONTINUE_MARKER` or UUID-format strings (hex + hyphens).
/// Rejects anything else to prevent command injection via tampered session.json.
fn sanitize_claude_session_ids(tree: &mut PaneTreeSnapshot) {
    match tree {
        PaneTreeSnapshot::Leaf {
            claude_session_id, ..
        } => {
            if let Some(ref val) = claude_session_id {
                let valid = val == CLAUDE_CONTINUE_MARKER
                    || (val.len() <= 40 && val.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
                if !valid {
                    tracing::warn!(
                        value = val,
                        "session restore: rejected suspicious claude_session_id"
                    );
                    *claude_session_id = None;
                }
            }
        }
        PaneTreeSnapshot::Split { first, second, .. } => {
            sanitize_claude_session_ids(first);
            sanitize_claude_session_ids(second);
        }
    }
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

        let state = build_session_state(
            &wm,
            &registry,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            0,
            None,
        );

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

        let state = build_session_state(
            &wm,
            &registry,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            0,
            None,
        );

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

        let state = build_session_state(
            &wm,
            &registry,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            0,
            None,
        );

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
                    claude_session_id: None,
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
                        claude_session_id: None,
                    }),
                    second: Box::new(PaneTreeSnapshot::Leaf {
                        surface_id: "surf-2".to_string(),
                        cwd: None,
                        scrollback_text: None,
                        claude_session_id: None,
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
        let state = build_session_state(
            &wm,
            &registry,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            0,
            None,
        );
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

        let state = build_session_state(
            &wm,
            &registry,
            &cwds,
            &HashMap::new(),
            &HashMap::new(),
            260,
            None,
        );
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

        let state = build_session_state(
            &wm,
            &registry,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            0,
            Some(geom),
        );
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
                    claude_session_id: Some("__continue__".to_string()),
                }),
                second: Box::new(PaneTreeSnapshot::Leaf {
                    surface_id: "b".to_string(),
                    cwd: None,
                    scrollback_text: None,
                    claude_session_id: None,
                }),
            }),
            second: Box::new(PaneTreeSnapshot::Leaf {
                surface_id: "c".to_string(),
                cwd: None,
                scrollback_text: None,
                claude_session_id: None,
            }),
        };

        let leaf = first_leaf(&tree);
        assert_eq!(leaf.cwd, Some("/deep/leaf"));
        assert_eq!(leaf.scrollback_text, Some("deep content"));
        assert_eq!(leaf.claude_session_id, Some("__continue__"));
    }

    #[test]
    fn claude_session_id_backward_compat() {
        // Old session files without claude_session_id should deserialize fine.
        let json = r#"{"Leaf":{"surface_id":"s1","cwd":"/tmp","scrollback_text":null}}"#;
        let tree: PaneTreeSnapshot = serde_json::from_str(json).unwrap();
        if let PaneTreeSnapshot::Leaf {
            claude_session_id, ..
        } = &tree
        {
            assert!(claude_session_id.is_none());
        } else {
            panic!("expected Leaf variant");
        }
    }

    #[test]
    fn claude_session_id_skipped_when_none() {
        let tree = PaneTreeSnapshot::Leaf {
            surface_id: "s1".to_string(),
            cwd: None,
            scrollback_text: None,
            claude_session_id: None,
        };
        let json = serde_json::to_string(&tree).unwrap();
        assert!(
            !json.contains("claude_session_id"),
            "None claude_session_id should be skipped in JSON"
        );
    }

    #[test]
    fn claude_session_id_serialized_when_present() {
        let tree = PaneTreeSnapshot::Leaf {
            surface_id: "s1".to_string(),
            cwd: None,
            scrollback_text: None,
            claude_session_id: Some("__continue__".to_string()),
        };
        let json = serde_json::to_string(&tree).unwrap();
        assert!(json.contains("claude_session_id"));
        assert!(json.contains("__continue__"));
    }

    #[test]
    fn window_geometry_maximized_defaults_false() {
        // Backward compat: old session files without `maximized` field.
        let json = r#"{"x":0,"y":0,"width":800,"height":600}"#;
        let geom: WindowGeometry = serde_json::from_str(json).unwrap();
        assert!(!geom.maximized);
    }
}
