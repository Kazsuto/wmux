use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};

/// Process names recognized as Claude Code executables (lowercased).
const CLAUDE_PROCESS_NAMES: &[&str] = &["claude", "claude.exe", "claude.cmd"];

/// RAII wrapper for a Win32 ToolHelp snapshot handle.
///
/// Ensures `CloseHandle` is called even on early returns or panics.
struct ToolhelpGuard(HANDLE);

impl Drop for ToolhelpGuard {
    fn drop(&mut self) {
        // SAFETY: self.0 is a valid handle returned by CreateToolhelp32Snapshot.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

/// Snapshot of all running processes, indexed by parent PID.
///
/// Used during session auto-save to detect if a pane is running Claude Code.
/// Best-effort: returns empty on failure (process detection is non-critical).
pub struct ProcessSnapshot {
    /// parent_pid → list of (child_pid, executable_name_lowercase)
    children: HashMap<u32, Vec<(u32, String)>>,
    /// pid → executable_name_lowercase (for root PID lookups)
    names: HashMap<u32, String>,
}

impl ProcessSnapshot {
    /// Capture a snapshot of all running processes.
    ///
    /// Uses `CreateToolhelp32Snapshot` — a single kernel call that returns an
    /// in-memory snapshot. Iteration via `Process32FirstW`/`Process32NextW` is
    /// fast (no additional syscalls).
    pub fn capture() -> Self {
        let mut children: HashMap<u32, Vec<(u32, String)>> = HashMap::new();
        let mut names: HashMap<u32, String> = HashMap::new();

        // SAFETY: TH32CS_SNAPPROCESS with pid 0 captures all processes.
        // The returned handle is wrapped in ToolhelpGuard for RAII cleanup.
        let Ok(snap) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
            tracing::debug!("CreateToolhelp32Snapshot failed, process detection unavailable");
            return Self { children, names };
        };
        let _guard = ToolhelpGuard(snap);

        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        // SAFETY: snap is a valid snapshot handle, entry is correctly sized.
        if unsafe { Process32FirstW(snap, &mut entry) }.is_err() {
            return Self { children, names };
        }

        loop {
            let name = String::from_utf16_lossy(
                &entry.szExeFile[..entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len())],
            )
            .to_ascii_lowercase();

            names.insert(entry.th32ProcessID, name.clone());
            children
                .entry(entry.th32ParentProcessID)
                .or_default()
                .push((entry.th32ProcessID, name));

            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            // SAFETY: snap is a valid snapshot handle, entry is correctly sized.
            if unsafe { Process32NextW(snap, &mut entry) }.is_err() {
                break;
            }
        }

        Self { children, names }
    }

    /// Check if `root_pid` itself OR any of its descendants is a Claude Code process.
    ///
    /// Checks the root PID's own name first (handles the case where Claude was
    /// spawned directly, not through a shell), then performs a DFS through the
    /// process tree looking for `claude`, `claude.exe`, or `claude.cmd`.
    pub fn has_claude_descendant(&self, root_pid: u32) -> bool {
        // Check the root PID itself — handles restored panes where claude is
        // spawned directly (not as a child of a shell).
        if let Some(name) = self.names.get(&root_pid) {
            if CLAUDE_PROCESS_NAMES.contains(&name.as_str()) {
                return true;
            }
        }

        let mut stack = vec![root_pid];
        // Guard against cycles (zombie PIDs re-used as parents).
        let mut visited = std::collections::HashSet::new();

        while let Some(pid) = stack.pop() {
            if !visited.insert(pid) {
                continue;
            }
            if let Some(kids) = self.children.get(&pid) {
                for (child_pid, name) in kids {
                    if CLAUDE_PROCESS_NAMES.contains(&name.as_str()) {
                        return true;
                    }
                    stack.push(*child_pid);
                }
            }
        }

        false
    }

    /// Find the PID of the Claude Code process at or under `root_pid`.
    ///
    /// Same logic as `has_claude_descendant` but returns the matched PID
    /// instead of a boolean. Used for WMI command-line correlation.
    pub fn find_claude_pid(&self, root_pid: u32) -> Option<u32> {
        if let Some(name) = self.names.get(&root_pid) {
            if CLAUDE_PROCESS_NAMES.contains(&name.as_str()) {
                return Some(root_pid);
            }
        }

        let mut stack = vec![root_pid];
        let mut visited = std::collections::HashSet::new();

        while let Some(pid) = stack.pop() {
            if !visited.insert(pid) {
                continue;
            }
            if let Some(kids) = self.children.get(&pid) {
                for (child_pid, name) in kids {
                    if CLAUDE_PROCESS_NAMES.contains(&name.as_str()) {
                        return Some(*child_pid);
                    }
                    stack.push(*child_pid);
                }
            }
        }

        None
    }
}

// ─── Claude session command-line resolution ──────────────────────────────────

/// Batch-query Claude process command lines to extract session UUIDs.
///
/// Uses `wmic` to read the `CommandLine` of each PID, then extracts
/// `--resume <uuid>` or `--session-id <uuid>` arguments. Returns a map
/// from PID to session UUID.
///
/// Best-effort: returns empty HashMap on any error (wmic unavailable, etc.).
pub fn query_claude_session_ids_from_cmdline(pids: &[u32]) -> HashMap<u32, String> {
    if pids.is_empty() {
        return HashMap::new();
    }

    // Build WMI filter: "ProcessId=X or ProcessId=Y or ..."
    let filter = pids
        .iter()
        .map(|pid| format!("ProcessId={pid}"))
        .collect::<Vec<_>>()
        .join(" or ");

    let output = match std::process::Command::new("wmic")
        .args([
            "process",
            "where",
            &filter,
            "get",
            "ProcessId,CommandLine",
            "/format:csv",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => {
            tracing::debug!("wmic query failed, skipping command-line session resolution");
            return HashMap::new();
        }
    };

    let text = String::from_utf8_lossy(&output.stdout);
    let mut result = HashMap::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("Node,") {
            continue;
        }

        // CSV format: Node,CommandLine,ProcessId
        // CommandLine may contain commas, so split from the RIGHT for PID.
        let Some(last_comma) = line.rfind(',') else {
            continue;
        };
        let pid_str = line[last_comma + 1..].trim();
        let Ok(pid) = pid_str.parse::<u32>() else {
            continue;
        };

        // Extract UUID from --resume or --session-id in the command line.
        let cmdline = &line[..last_comma];
        if let Some(uuid) = extract_uuid_from_cmdline(cmdline) {
            result.insert(pid, uuid);
        }
    }

    if !result.is_empty() {
        tracing::debug!(
            count = result.len(),
            "resolved Claude session UUIDs from command lines"
        );
    }

    result
}

/// Extract a session UUID from a Claude command line.
///
/// Looks for `--resume <uuid>` or `--session-id <uuid>` patterns.
fn extract_uuid_from_cmdline(cmdline: &str) -> Option<String> {
    for flag in &["--resume", "--session-id", "-r"] {
        if let Some(pos) = cmdline.find(flag) {
            let after = &cmdline[pos + flag.len()..];
            let after = after.trim_start();
            // Take the next token (up to whitespace or end).
            let token: String = after.chars().take_while(|c| !c.is_whitespace()).collect();
            // Validate UUID format (36 chars, hex + hyphens).
            if token.len() == 36 && token.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
                return Some(token);
            }
        }
    }
    None
}

// ─── Claude session filesystem resolution ────────────────────────────────────

/// Encode a CWD path the way Claude Code does for its session directory names:
/// every non-ASCII-alphanumeric character is replaced by a hyphen.
///
/// Example: `F:/Workspaces/wmux` → `F--Workspaces-wmux`
pub fn encode_cwd_for_claude(cwd: &str) -> String {
    cwd.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// List the `count` most recently modified Claude Code session UUIDs for a CWD.
///
/// Reads `~/.claude/projects/<encoded_cwd>/` for `*.jsonl` files whose names
/// are valid UUIDs (36 chars, hex + hyphens). Returns UUIDs sorted by
/// modification time (most recent first), up to `count`.
///
/// Uses `std::fs` (synchronous) intentionally — the sessions directory is
/// small (typically <200 files) and this runs during session auto-save which
/// is already synchronous on the actor thread.
///
/// Returns an empty Vec on any I/O error (best-effort, non-critical).
pub fn list_recent_claude_sessions(cwd: &str, count: usize) -> Vec<String> {
    if count == 0 {
        return Vec::new();
    }

    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };

    let encoded = encode_cwd_for_claude(cwd);
    let sessions_dir = home.join(".claude").join("projects").join(&encoded);

    let entries = match std::fs::read_dir(&sessions_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut candidates: Vec<(SystemTime, String)> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        // Standard UUID is 36 chars (8-4-4-4-12 hex with hyphens).
        if stem.len() != 36 || !stem.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        candidates.push((modified, stem.to_owned()));
    }

    // Sort by modification time (most recent first). Active sessions are
    // naturally the most recently modified — no time cutoff needed. The caller
    // requests exactly N candidates, so old sessions are excluded by count.
    candidates.sort_unstable_by(|a, b| b.0.cmp(&a.0));
    candidates
        .into_iter()
        .take(count)
        .map(|(_, uuid)| uuid)
        .collect()
}

/// Claude sessions directory for a given CWD (for testing/debugging).
pub fn claude_sessions_dir(cwd: &str) -> Option<PathBuf> {
    dirs::home_dir().map(|home| {
        home.join(".claude")
            .join("projects")
            .join(encode_cwd_for_claude(cwd))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_succeeds() {
        // Should never panic — returns empty snapshot on failure.
        let snap = ProcessSnapshot::capture();
        // At minimum, the current process should appear as a child of something.
        assert!(
            !snap.children.is_empty(),
            "snapshot should contain at least one process"
        );
    }

    #[test]
    fn no_claude_in_test_process() {
        let snap = ProcessSnapshot::capture();
        let pid = std::process::id();
        // Test runner is not Claude Code.
        assert!(!snap.has_claude_descendant(pid));
    }

    #[test]
    fn nonexistent_pid_returns_false() {
        let snap = ProcessSnapshot::capture();
        assert!(!snap.has_claude_descendant(u32::MAX));
    }

    #[test]
    fn claude_process_names_includes_cmd() {
        assert!(CLAUDE_PROCESS_NAMES.contains(&"claude.cmd"));
    }

    #[test]
    fn encode_cwd_drive_letter() {
        assert_eq!(
            encode_cwd_for_claude("F:/Workspaces/wmux"),
            "F--Workspaces-wmux"
        );
    }

    #[test]
    fn encode_cwd_spaces_and_dots() {
        assert_eq!(
            encode_cwd_for_claude("C:/Users/WINDOWS 11/.claude"),
            "C--Users-WINDOWS-11--claude"
        );
    }

    #[test]
    fn encode_cwd_alphanumeric_only() {
        assert_eq!(encode_cwd_for_claude("abc123"), "abc123");
    }

    #[test]
    fn list_recent_sessions_nonexistent_dir() {
        let result = list_recent_claude_sessions("Z:/nonexistent/path/12345", 4);
        assert!(result.is_empty());
    }

    #[test]
    fn list_recent_sessions_zero_count() {
        let result = list_recent_claude_sessions("F:/Workspaces/wmux", 0);
        assert!(result.is_empty());
    }

    #[test]
    fn list_recent_sessions_real_dir() {
        // If Claude sessions exist for wmux, we should get valid UUIDs.
        let results = list_recent_claude_sessions("F:/Workspaces/wmux", 2);
        for uuid in &results {
            assert_eq!(uuid.len(), 36, "UUID should be 36 chars: {uuid}");
            assert!(
                uuid.chars().all(|c| c.is_ascii_hexdigit() || c == '-'),
                "UUID should be hex+hyphens: {uuid}"
            );
        }
    }

    #[test]
    fn extract_uuid_resume() {
        let cmdline = "claude --resume bb216252-86b5-421f-a0ad-88f0b66bae83";
        assert_eq!(
            extract_uuid_from_cmdline(cmdline),
            Some("bb216252-86b5-421f-a0ad-88f0b66bae83".to_string())
        );
    }

    #[test]
    fn extract_uuid_session_id() {
        let cmdline = r#""C:\Users\test\.local\bin\claude.exe" --session-id aabbccdd-1122-3344-5566-778899aabbcc"#;
        assert_eq!(
            extract_uuid_from_cmdline(cmdline),
            Some("aabbccdd-1122-3344-5566-778899aabbcc".to_string())
        );
    }

    #[test]
    fn extract_uuid_no_flag() {
        let cmdline = r#""C:\Users\WINDOWS 11\.local\bin\claude.exe""#;
        assert_eq!(extract_uuid_from_cmdline(cmdline), None);
    }
}
