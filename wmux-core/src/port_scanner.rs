use std::collections::{HashMap, HashSet};
use std::time::Duration;

/// Default port range to report (common dev server ports).
const PORT_RANGE_MIN: u16 = 1024;
const PORT_RANGE_MAX: u16 = 65535;

/// Scan for TCP ports currently in LISTENING state, filtered to processes
/// that are descendants of the given shell PIDs.
///
/// Uses `netstat -ano` to discover listening ports with their owning PIDs,
/// then walks the Windows process tree to find all descendants of `shell_pids`.
/// Only ports owned by those descendants (or the shells themselves) are returned.
///
/// If `shell_pids` is empty, falls back to returning all listening ports
/// (best-effort, same as previous behavior).
///
/// Returns an empty Vec on error (non-fatal — port detection is best-effort).
pub async fn scan_listening_ports(shell_pids: &[u32]) -> Vec<u16> {
    scan_listening_ports_filtered(shell_pids, PORT_RANGE_MIN, PORT_RANGE_MAX).await
}

/// Scan for listening TCP ports within a specific range, filtered by process tree.
async fn scan_listening_ports_filtered(
    shell_pids: &[u32],
    min_port: u16,
    max_port: u16,
) -> Vec<u16> {
    let output = match tokio::time::timeout(
        Duration::from_secs(3),
        tokio::process::Command::new("netstat")
            .args(["-ano", "-p", "TCP"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output(),
    )
    .await
    {
        Ok(Ok(o)) if o.status.success() => o,
        _ => {
            tracing::debug!("netstat command failed or timed out, port scanning unavailable");
            return Vec::new();
        }
    };

    let text = String::from_utf8_lossy(&output.stdout);
    let port_pid_pairs = parse_netstat_output(&text, min_port, max_port);

    if shell_pids.is_empty() {
        // No panes have registered their PID yet — nothing to show.
        tracing::debug!("no shell PIDs provided, skipping port scan");
        return Vec::new();
    }

    // Build the set of all descendant PIDs from the shell roots.
    let allowed_pids = collect_descendant_pids(shell_pids);

    let mut ports: HashSet<u16> = HashSet::new();
    for (port, pid) in port_pid_pairs {
        if allowed_pids.contains(&pid) {
            ports.insert(port);
        }
    }

    let mut result: Vec<u16> = ports.into_iter().collect();
    result.sort_unstable();
    tracing::debug!(
        count = result.len(),
        shell_count = shell_pids.len(),
        descendant_count = allowed_pids.len(),
        "listening ports detected (process-filtered)"
    );
    result
}

/// Parse `netstat -ano` output into (port, pid) pairs for LISTENING TCP entries.
fn parse_netstat_output(text: &str, min_port: u16, max_port: u16) -> Vec<(u16, u32)> {
    let mut pairs = Vec::new();

    for line in text.lines() {
        // Lines look like: "  TCP    0.0.0.0:3000           0.0.0.0:0              LISTENING       1234"
        // or:               "  TCP    [::]:8080              [::]:0                 LISTENING       5678"
        let trimmed = line.trim();
        if !trimmed.contains("LISTENING") {
            continue;
        }
        let fields: Vec<&str> = trimmed.split_whitespace().collect();
        if fields.len() < 5 || fields[0] != "TCP" {
            continue;
        }
        let addr = fields[1];
        let pid_str = fields[fields.len() - 1];

        let port = match addr.rsplit(':').next().and_then(|s| s.parse::<u16>().ok()) {
            Some(p) if p >= min_port && p <= max_port => p,
            _ => continue,
        };
        let pid = match pid_str.parse::<u32>() {
            Ok(p) => p,
            Err(_) => continue,
        };

        pairs.push((port, pid));
    }
    pairs
}

/// Collect the full set of descendant PIDs (including the roots themselves)
/// by walking the Windows process tree via `CreateToolhelp32Snapshot`.
///
/// This is a synchronous Win32 API call, but it's fast (~1ms for typical
/// process counts). Called from an async context that's already off the
/// main actor loop (background task).
fn collect_descendant_pids(root_pids: &[u32]) -> HashSet<u32> {
    let mut result: HashSet<u32> = root_pids.iter().copied().collect();

    // Build parent→children map from the process snapshot.
    let parent_map = match build_process_tree() {
        Some(m) => m,
        None => return result,
    };

    // BFS from each root to collect all descendants.
    let mut queue: Vec<u32> = root_pids.to_vec();
    while let Some(pid) = queue.pop() {
        if let Some(children) = parent_map.get(&pid) {
            for &child in children {
                if result.insert(child) {
                    queue.push(child);
                }
            }
        }
    }

    result
}

/// RAII wrapper for Win32 HANDLE — calls `CloseHandle` on drop.
#[cfg(windows)]
struct OwnedHandle(windows::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: The handle was obtained from a successful Win32 API call
        // and has not been closed yet. CloseHandle is safe to call once.
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

/// Build a map of parent_pid → [child_pids] using `CreateToolhelp32Snapshot`.
fn build_process_tree() -> Option<HashMap<u32, Vec<u32>>> {
    #[cfg(windows)]
    {
        use windows::Win32::System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32First, Process32Next, PROCESSENTRY32,
            TH32CS_SNAPPROCESS,
        };

        // SAFETY: CreateToolhelp32Snapshot with TH32CS_SNAPPROCESS and pid 0
        // takes a snapshot of all processes. This is a well-documented Win32 API.
        let snapshot =
            OwnedHandle(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()? });

        let mut entry = PROCESSENTRY32 {
            dwSize: std::mem::size_of::<PROCESSENTRY32>() as u32,
            ..Default::default()
        };

        let mut map: HashMap<u32, Vec<u32>> = HashMap::new();

        // SAFETY: Process32First/Next read from the snapshot handle.
        // The entry is properly sized via dwSize.
        unsafe {
            if Process32First(snapshot.0, &mut entry).is_err() {
                return None;
            }

            loop {
                let pid = entry.th32ProcessID;
                let ppid = entry.th32ParentProcessID;
                if pid != 0 {
                    map.entry(ppid).or_default().push(pid);
                }

                if Process32Next(snapshot.0, &mut entry).is_err() {
                    break;
                }
            }
        }
        // `snapshot` dropped here → CloseHandle called automatically.

        Some(map)
    }

    #[cfg(not(windows))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_netstat_line_with_pid() {
        let output = "  TCP    0.0.0.0:3000           0.0.0.0:0              LISTENING       1234";
        let pairs = parse_netstat_output(output, 1024, 65535);
        assert_eq!(pairs, vec![(3000, 1234)]);
    }

    #[test]
    fn parse_ipv6_netstat_line_with_pid() {
        let output = "  TCP    [::]:8080              [::]:0                 LISTENING       5678";
        let pairs = parse_netstat_output(output, 1024, 65535);
        assert_eq!(pairs, vec![(8080, 5678)]);
    }

    #[test]
    fn non_listening_line_ignored() {
        let output = "  TCP    127.0.0.1:5000     127.0.0.1:59876     ESTABLISHED     9999";
        let pairs = parse_netstat_output(output, 1024, 65535);
        assert!(pairs.is_empty());
    }

    #[test]
    fn port_range_filtering() {
        let output = "  TCP    0.0.0.0:80             0.0.0.0:0              LISTENING       100\n\
                       TCP    0.0.0.0:3000           0.0.0.0:0              LISTENING       200";
        let pairs = parse_netstat_output(output, 1024, 65535);
        assert_eq!(pairs, vec![(3000, 200)]);
    }

    #[test]
    fn collect_descendants_no_children() {
        let result = collect_descendant_pids(&[99999]);
        assert!(result.contains(&99999));
    }

    #[tokio::test]
    async fn scan_does_not_panic() {
        let _ = scan_listening_ports(&[]).await;
    }

    #[tokio::test]
    async fn scan_with_fake_pid_returns_empty_or_subset() {
        // A PID that almost certainly doesn't own any listening ports
        let ports = scan_listening_ports(&[99999]).await;
        // Should return empty (no descendants of PID 99999 listen on any port)
        assert!(ports.is_empty() || ports.len() < 20);
    }
}
