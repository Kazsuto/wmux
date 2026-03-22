use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

/// Log severity level for sidebar log entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Info,
    Progress,
    Success,
    Warning,
    Error,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Info => f.write_str("info"),
            Self::Progress => f.write_str("progress"),
            Self::Success => f.write_str("success"),
            Self::Warning => f.write_str("warning"),
            Self::Error => f.write_str("error"),
        }
    }
}

/// A status badge entry displayed in the sidebar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusEntry {
    pub key: String,
    pub value: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    /// PID of the process that set this status (for sweep).
    pub pid: Option<u32>,
}

/// Progress bar state for sidebar display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressState {
    /// Progress value between 0.0 and 1.0.
    pub value: f32,
    /// Optional label displayed alongside the progress bar.
    pub label: Option<String>,
}

/// A timestamped log entry for the sidebar activity log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: SystemTime,
    pub level: LogLevel,
    pub source: String,
    pub message: String,
}

/// Per-workspace metadata store for sidebar status, progress, and logs.
///
/// Each workspace has its own independent MetadataStore instance.
#[derive(Debug)]
pub struct MetadataStore {
    statuses: HashMap<String, StatusEntry>,
    progress: Option<ProgressState>,
    log: VecDeque<LogEntry>,
    max_log_entries: usize,
}

impl MetadataStore {
    /// Default maximum log entries per workspace.
    pub const DEFAULT_MAX_LOG: usize = 1000;
    /// Maximum number of status entries in the sidebar.
    const MAX_STATUSES: usize = 256;

    #[must_use]
    pub fn new() -> Self {
        Self {
            statuses: HashMap::with_capacity(16),
            progress: None,
            log: VecDeque::with_capacity(128),
            max_log_entries: Self::DEFAULT_MAX_LOG,
        }
    }

    /// Set or update a status entry by key.
    ///
    /// Returns `true` if the entry was stored, `false` if rejected (capacity limit reached).
    pub fn set_status(&mut self, entry: StatusEntry) -> bool {
        if !self.statuses.contains_key(&entry.key) && self.statuses.len() >= Self::MAX_STATUSES {
            tracing::warn!("sidebar status limit reached, rejecting new entry");
            return false;
        }
        tracing::debug!(key = %entry.key, value = %entry.value, "sidebar status set");
        self.statuses.insert(entry.key.clone(), entry);
        true
    }

    /// Remove a status entry by key. Returns true if it existed.
    pub fn clear_status(&mut self, key: &str) -> bool {
        let removed = self.statuses.remove(key).is_some();
        if removed {
            tracing::debug!(key = %key, "sidebar status cleared");
        }
        removed
    }

    /// List all current status entries.
    #[must_use]
    pub fn list_status(&self) -> Vec<&StatusEntry> {
        self.statuses.values().collect()
    }

    /// Set the progress bar state.
    pub fn set_progress(&mut self, value: f32, label: Option<String>) {
        let clamped = value.clamp(0.0, 1.0);
        tracing::debug!(value = clamped, ?label, "sidebar progress set");
        self.progress = Some(ProgressState {
            value: clamped,
            label,
        });
    }

    /// Clear the progress bar.
    pub fn clear_progress(&mut self) {
        if self.progress.is_some() {
            tracing::debug!("sidebar progress cleared");
        }
        self.progress = None;
    }

    /// Get the current progress state.
    #[must_use]
    pub fn progress(&self) -> Option<&ProgressState> {
        self.progress.as_ref()
    }

    /// Add a log entry. Enforces max_log_entries capacity.
    pub fn add_log(&mut self, level: LogLevel, source: String, message: String) {
        if self.log.len() >= self.max_log_entries {
            let _ = self.log.pop_front();
        }
        self.log.push_back(LogEntry {
            timestamp: SystemTime::now(),
            level,
            source,
            message,
        });
    }

    /// List log entries, newest first, up to `limit`.
    #[must_use]
    pub fn list_log(&self, limit: usize) -> Vec<&LogEntry> {
        self.log.iter().rev().take(limit).collect()
    }

    /// Clear all log entries.
    pub fn clear_log(&mut self) {
        self.log.clear();
        tracing::debug!("sidebar log cleared");
    }

    /// Return the full metadata state as a serializable snapshot.
    #[must_use]
    pub fn state(&self) -> MetadataSnapshot {
        MetadataSnapshot {
            statuses: self.statuses.values().cloned().collect(),
            progress: self.progress.clone(),
            log_count: self.log.len(),
        }
    }

    /// Remove status entries whose PID is no longer running.
    /// Returns the keys that were removed.
    #[cfg(windows)]
    pub fn sweep_dead_pids(&mut self) -> Vec<String> {
        let dead_keys: Vec<String> = self
            .statuses
            .iter()
            .filter_map(|(key, entry)| {
                entry
                    .pid
                    .filter(|&pid| !is_process_alive(pid))
                    .map(|_| key.clone())
            })
            .collect();

        for key in &dead_keys {
            self.statuses.remove(key);
            tracing::info!(key = %key, "swept dead PID status");
        }
        dead_keys
    }

    /// Stub for non-Windows (tests).
    #[cfg(not(windows))]
    pub fn sweep_dead_pids(&mut self) -> Vec<String> {
        Vec::new()
    }
}

impl Default for MetadataStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Serializable snapshot of metadata state for IPC responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataSnapshot {
    pub statuses: Vec<StatusEntry>,
    pub progress: Option<ProgressState>,
    pub log_count: usize,
}

impl MetadataSnapshot {
    /// Create an empty snapshot (used as default when no store exists).
    #[must_use]
    pub fn empty() -> Self {
        Self {
            statuses: Vec::new(),
            progress: None,
            log_count: 0,
        }
    }
}

/// Check if a Windows process is still alive using OpenProcess.
#[cfg(windows)]
fn is_process_alive(pid: u32) -> bool {
    use std::os::windows::io::{FromRawHandle, OwnedHandle};
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    // SAFETY: OpenProcess with PROCESS_QUERY_LIMITED_INFORMATION is safe —
    // it either returns a valid handle or an error. The returned handle is
    // wrapped in OwnedHandle (RAII) which calls CloseHandle on drop, even
    // if a panic occurs between open and close.
    unsafe {
        match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(handle) => {
                let _owned = OwnedHandle::from_raw_handle(handle.0 as *mut _);
                true
            }
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_store_is_empty() {
        let store = MetadataStore::new();
        assert!(store.list_status().is_empty());
        assert!(store.progress().is_none());
        assert!(store.list_log(100).is_empty());
    }

    #[test]
    fn set_and_clear_status() {
        let mut store = MetadataStore::new();
        store.set_status(StatusEntry {
            key: "build".into(),
            value: "passing".into(),
            icon: Some("check".into()),
            color: Some("green".into()),
            pid: None,
        });
        assert_eq!(store.list_status().len(), 1);

        assert!(store.clear_status("build"));
        assert!(store.list_status().is_empty());
        assert!(!store.clear_status("nonexistent"));
    }

    #[test]
    fn set_status_overwrites() {
        let mut store = MetadataStore::new();
        store.set_status(StatusEntry {
            key: "build".into(),
            value: "failing".into(),
            icon: None,
            color: None,
            pid: None,
        });
        store.set_status(StatusEntry {
            key: "build".into(),
            value: "passing".into(),
            icon: None,
            color: None,
            pid: None,
        });
        assert_eq!(store.list_status().len(), 1);
        assert_eq!(store.list_status()[0].value, "passing");
    }

    #[test]
    fn progress_clamps_value() {
        let mut store = MetadataStore::new();
        store.set_progress(1.5, None);
        assert!((store.progress().unwrap().value - 1.0).abs() < f32::EPSILON);

        store.set_progress(-0.5, Some("test".into()));
        assert!(store.progress().unwrap().value.abs() < f32::EPSILON);

        store.clear_progress();
        assert!(store.progress().is_none());
    }

    #[test]
    fn log_entries_capped() {
        let mut store = MetadataStore::new();
        // Override capacity for test
        store.max_log_entries = 3;

        for i in 0..5 {
            store.add_log(LogLevel::Info, "test".into(), format!("msg {i}"));
        }

        assert_eq!(store.list_log(100).len(), 3);
        // Newest first
        assert_eq!(store.list_log(100)[0].message, "msg 4");
        assert_eq!(store.list_log(100)[2].message, "msg 2");
    }

    #[test]
    fn log_limit_respected() {
        let mut store = MetadataStore::new();
        for i in 0..10 {
            store.add_log(LogLevel::Info, "test".into(), format!("msg {i}"));
        }
        assert_eq!(store.list_log(3).len(), 3);
    }

    #[test]
    fn clear_log() {
        let mut store = MetadataStore::new();
        store.add_log(LogLevel::Warning, "test".into(), "warning".into());
        store.clear_log();
        assert!(store.list_log(100).is_empty());
    }

    #[test]
    fn state_snapshot() {
        let mut store = MetadataStore::new();
        store.set_status(StatusEntry {
            key: "k".into(),
            value: "v".into(),
            icon: None,
            color: None,
            pid: None,
        });
        store.set_progress(0.5, Some("half".into()));
        store.add_log(LogLevel::Success, "src".into(), "done".into());

        let snap = store.state();
        assert_eq!(snap.statuses.len(), 1);
        assert!(snap.progress.is_some());
        assert_eq!(snap.log_count, 1);
    }

    #[test]
    fn sweep_dead_pids_no_pids() {
        let mut store = MetadataStore::new();
        store.set_status(StatusEntry {
            key: "test".into(),
            value: "v".into(),
            icon: None,
            color: None,
            pid: None,
        });
        let removed = store.sweep_dead_pids();
        assert!(removed.is_empty());
        assert_eq!(store.list_status().len(), 1);
    }

    #[test]
    fn status_limit_enforced() {
        let mut store = MetadataStore::new();

        // Add MAX_STATUSES entries
        for i in 0..MetadataStore::MAX_STATUSES {
            store.set_status(StatusEntry {
                key: format!("status_{}", i),
                value: format!("v{}", i),
                icon: None,
                color: None,
                pid: None,
            });
        }
        assert_eq!(store.list_status().len(), MetadataStore::MAX_STATUSES);

        // Try to add one more — should be rejected
        assert!(!store.set_status(StatusEntry {
            key: "status_overflow".into(),
            value: "overflow".into(),
            icon: None,
            color: None,
            pid: None,
        }));
        assert_eq!(store.list_status().len(), MetadataStore::MAX_STATUSES);
        assert!(store
            .list_status()
            .iter()
            .all(|s| s.key != "status_overflow"));

        // Updating an existing key should still work
        assert!(store.set_status(StatusEntry {
            key: "status_0".into(),
            value: "updated".into(),
            icon: None,
            color: None,
            pid: None,
        }));
        assert_eq!(
            store
                .list_status()
                .iter()
                .find(|s| s.key == "status_0")
                .unwrap()
                .value,
            "updated"
        );
    }
}
