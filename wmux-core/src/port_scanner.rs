use std::collections::HashSet;
use std::time::Duration;

/// Default port range to report (common dev server ports).
const PORT_RANGE_MIN: u16 = 1024;
const PORT_RANGE_MAX: u16 = 65535;

/// Scan for TCP ports currently in LISTENING state.
///
/// Uses `netstat -an` to discover listening ports. Filters to the
/// configurable port range (default: 1024-65535).
///
/// Returns an empty Vec on error (non-fatal — port detection is best-effort).
pub async fn scan_listening_ports() -> Vec<u16> {
    scan_listening_ports_in_range(PORT_RANGE_MIN, PORT_RANGE_MAX).await
}

/// Scan for listening TCP ports within a specific range.
pub async fn scan_listening_ports_in_range(min_port: u16, max_port: u16) -> Vec<u16> {
    let output = match tokio::time::timeout(
        Duration::from_secs(3),
        tokio::process::Command::new("netstat")
            .args(["-an", "-p", "TCP"])
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
    let mut ports: HashSet<u16> = HashSet::new();

    for line in text.lines() {
        // Lines look like: "  TCP    0.0.0.0:3000           0.0.0.0:0              LISTENING"
        // or:               "  TCP    [::]:8080              [::]:0                 LISTENING"
        let trimmed = line.trim();
        if !trimmed.contains("LISTENING") {
            continue;
        }
        let mut fields = trimmed.split_whitespace();
        let Some("TCP") = fields.next() else {
            continue;
        };
        let Some(addr) = fields.next() else {
            continue;
        };
        // Extract port from address (last colon-separated segment)
        if let Some(port_str) = addr.rsplit(':').next() {
            if let Ok(port) = port_str.parse::<u16>() {
                if port >= min_port && port <= max_port {
                    ports.insert(port);
                }
            }
        }
    }

    let mut result: Vec<u16> = ports.into_iter().collect();
    result.sort_unstable();
    tracing::debug!(count = result.len(), "listening ports detected");
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_netstat_line() {
        // Simulate parsing logic
        let line = "  TCP    0.0.0.0:3000           0.0.0.0:0              LISTENING";
        let trimmed = line.trim();
        assert!(trimmed.contains("LISTENING"));
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        assert_eq!(parts[0], "TCP");
        let port_str = parts[1].rsplit(':').next().unwrap();
        let port: u16 = port_str.parse().unwrap();
        assert_eq!(port, 3000);
    }

    #[test]
    fn parse_ipv6_netstat_line() {
        let line = "  TCP    [::]:8080              [::]:0                 LISTENING";
        let trimmed = line.trim();
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        let port_str = parts[1].rsplit(':').next().unwrap();
        let port: u16 = port_str.parse().unwrap();
        assert_eq!(port, 8080);
    }

    #[test]
    fn non_listening_line_ignored() {
        let line = "  TCP    127.0.0.1:5000     127.0.0.1:59876     ESTABLISHED";
        assert!(!line.contains("LISTENING"));
    }

    #[tokio::test]
    async fn scan_does_not_panic() {
        // Just verify it runs without panicking
        let _ = scan_listening_ports().await;
    }
}
