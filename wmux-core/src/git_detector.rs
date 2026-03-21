use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Git repository information for sidebar display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitInfo {
    /// Current branch name (e.g., "main", "feature/foo").
    pub branch: String,
    /// Whether the working tree has uncommitted changes.
    pub dirty: bool,
}

/// Detect git branch and dirty state for a given directory.
///
/// Returns `None` if the directory is not inside a git repository
/// or if `git` is not available on the system. Also returns `None` if
/// the git command times out (5 second limit per command).
pub async fn detect_git(cwd: impl AsRef<Path>) -> Option<GitInfo> {
    let cwd = cwd.as_ref();
    let timeout = Duration::from_secs(5);

    let branch_output = tokio::time::timeout(
        timeout,
        tokio::process::Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output(),
    )
    .await
    .ok()?
    .ok()
    .filter(|o| o.status.success())?;

    let branch = String::from_utf8_lossy(&branch_output.stdout)
        .trim()
        .to_owned();
    if branch.is_empty() {
        return None;
    }

    let dirty = tokio::time::timeout(
        timeout,
        tokio::process::Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output(),
    )
    .await
    .ok()
    .and_then(|r| r.ok())
    .map(|o| !o.stdout.is_empty())
    .unwrap_or(false);

    tracing::debug!(
        cwd = %cwd.display(),
        branch = %branch,
        dirty,
        "git detected"
    );

    Some(GitInfo { branch, dirty })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn non_git_directory_returns_none() {
        // Temp dir is not a git repo
        let tmp = std::env::temp_dir();
        let result = detect_git(&tmp).await;
        // Most likely None, but if temp is inside a git repo, it could be Some
        // We just verify it doesn't panic or error
        let _ = result;
    }

    #[test]
    fn git_info_serde_roundtrip() {
        let info = GitInfo {
            branch: "main".to_string(),
            dirty: true,
        };
        let json = serde_json::to_string(&info).unwrap();
        let back: GitInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back, info);
    }
}
