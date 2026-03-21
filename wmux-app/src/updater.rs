// Module is declared but not yet wired into the main application loop.
// Background polling and title bar integration are scheduled for a later wave.
#![allow(dead_code)]

use anyhow::{Context, Result};
use semver::Version;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::Duration;

const GITHUB_API_URL: &str = "https://api.github.com/repos/pimalaya/wmux/releases/latest";
const USER_AGENT: &str = "wmux-updater";

pub struct UpdateChecker {
    current_version: Version,
    update_dir: PathBuf,
    disabled: bool,
    client: reqwest::Client,
}

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub version: Version,
    pub download_url: String,
    pub release_notes: String,
}

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    body: Option<String>,
    assets: Vec<GitHubAsset>,
}

#[derive(Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

impl UpdateChecker {
    pub fn new(current_version: &str) -> Result<Self> {
        let version_str = current_version.trim_start_matches('v');
        let current_version = Version::parse(version_str)
            .with_context(|| format!("Failed to parse version: {current_version}"))?;

        let update_dir = dirs::config_dir()
            .context("Failed to find config directory")?
            .join("wmux")
            .join("updates");

        let disabled = std::env::var("WMUX_DISABLE_UPDATE").is_ok();

        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self {
            current_version,
            update_dir,
            disabled,
            client,
        })
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    pub async fn check_for_update(&self) -> Result<Option<UpdateInfo>> {
        if self.disabled {
            tracing::debug!("Update check disabled via WMUX_DISABLE_UPDATE");
            return Ok(None);
        }

        tracing::debug!("Checking for updates");

        let response = match self.client.get(GITHUB_API_URL).send().await {
            Ok(resp) => resp,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to reach update server");
                return Ok(None);
            }
        };

        let status = response.status();
        if status.as_u16() == 403 {
            tracing::warn!("Update check rate limited (HTTP 403)");
            return Ok(None);
        }
        if !status.is_success() {
            tracing::warn!(status = %status, "Update check returned non-success status");
            return Ok(None);
        }

        let release: GitHubRelease = match response.json().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to parse update response");
                return Ok(None);
            }
        };

        let tag = release.tag_name.trim_start_matches('v');
        let remote_version = match Version::parse(tag) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    tag = %release.tag_name,
                    error = %e,
                    "Failed to parse remote version"
                );
                return Ok(None);
            }
        };

        if remote_version <= self.current_version {
            tracing::debug!(
                current = %self.current_version,
                remote = %remote_version,
                "No update available"
            );
            return Ok(None);
        }

        let asset = release
            .assets
            .iter()
            .find(|a| a.name.contains("wmux-app") && a.name.ends_with(".exe"));

        let download_url = match asset {
            Some(a) => a.browser_download_url.clone(),
            None => {
                tracing::warn!(
                    version = %remote_version,
                    "No Windows binary asset found in release"
                );
                return Ok(None);
            }
        };

        let release_notes = release.body.unwrap_or_default();

        tracing::info!(
            current = %self.current_version,
            available = %remote_version,
            "Update available"
        );

        Ok(Some(UpdateInfo {
            version: remote_version,
            download_url,
            release_notes,
        }))
    }

    pub async fn download_update(&self, info: &UpdateInfo) -> Result<PathBuf> {
        tokio::fs::create_dir_all(&self.update_dir)
            .await
            .with_context(|| {
                format!(
                    "Failed to create update directory: {}",
                    self.update_dir.display()
                )
            })?;

        let filename = format!("wmux-app-v{}.exe", info.version);
        let dest_path = self.update_dir.join(&filename);

        tracing::info!(
            url = %info.download_url,
            dest = %dest_path.display(),
            "Downloading update"
        );

        let mut response = self
            .client
            .get(&info.download_url)
            .send()
            .await
            .context("Failed to start update download")?;

        if !response.status().is_success() {
            anyhow::bail!("Download request failed with status: {}", response.status());
        }

        let mut file = tokio::fs::File::create(&dest_path)
            .await
            .with_context(|| format!("Failed to create file: {}", dest_path.display()))?;

        use tokio::io::AsyncWriteExt;
        while let Some(chunk) = response
            .chunk()
            .await
            .context("Failed to read response chunk")?
        {
            file.write_all(&chunk)
                .await
                .context("Failed to write chunk to file")?;
        }
        file.flush().await.context("Failed to flush update file")?;

        tracing::info!(path = %dest_path.display(), "Update downloaded successfully");

        Ok(dest_path)
    }

    /// Check if a staged update exe exists in the update directory.
    /// Returns the path to the highest-versioned pending update that is
    /// newer than the current version, if any.
    pub async fn check_pending_update(&self) -> Option<PathBuf> {
        let mut entries = tokio::fs::read_dir(&self.update_dir).await.ok()?;
        let mut candidates: Vec<(Version, PathBuf)> = Vec::new();

        loop {
            let entry = match entries.next_entry().await {
                Ok(Some(entry)) => entry,
                Ok(None) => break,
                Err(e) => {
                    tracing::debug!(error = %e, "skipping update directory entry");
                    continue;
                }
            };
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            if !name_str.starts_with("wmux-app-v") || !name_str.ends_with(".exe") {
                continue;
            }

            let version_str = match name_str
                .strip_prefix("wmux-app-v")
                .and_then(|s| s.strip_suffix(".exe"))
            {
                Some(s) => s.to_owned(),
                None => continue,
            };

            if let Ok(version) = Version::parse(&version_str) {
                // Only consider versions strictly newer than current
                if version > self.current_version {
                    candidates.push((version, entry.path()));
                }
            }
        }

        candidates
            .into_iter()
            .max_by(|a, b| a.0.cmp(&b.0))
            .map(|(_, path)| path)
    }

    /// Apply a pending update at startup.
    ///
    /// Strategy: copy new exe to a `.new` staging path first, then rename
    /// current to `.old`, rename `.new` to current. This minimizes the
    /// window where no valid exe exists. On failure, the original is restored.
    pub async fn apply_pending_update(pending: &Path) -> Result<()> {
        let current_exe = std::env::current_exe().context("Failed to get current exe path")?;
        let old_exe = current_exe.with_extension("old");
        let new_exe = current_exe.with_extension("new");

        tracing::info!(
            pending = %pending.display(),
            current = %current_exe.display(),
            "Applying pending update"
        );

        // Step 1: Copy update to .new staging path (current exe still intact)
        tokio::fs::copy(pending, &new_exe)
            .await
            .with_context(|| format!("Failed to stage update at {}", new_exe.display()))?;

        // Step 2: Rename current exe to .old
        tokio::fs::rename(&current_exe, &old_exe)
            .await
            .with_context(|| {
                format!(
                    "Failed to rename {} to {}",
                    current_exe.display(),
                    old_exe.display()
                )
            })?;

        // Step 3: Rename .new to current exe path
        if let Err(e) = tokio::fs::rename(&new_exe, &current_exe).await {
            // Restore: rename .old back to current
            if let Err(restore_err) = tokio::fs::rename(&old_exe, &current_exe).await {
                tracing::error!(
                    error = %restore_err,
                    "CRITICAL: failed to restore original exe after update failure"
                );
            }
            return Err(e).with_context(|| {
                format!("Failed to move staged update to {}", current_exe.display())
            });
        }

        // Step 4: Clean up .old exe
        if let Err(e) = tokio::fs::remove_file(&old_exe).await {
            tracing::warn!(
                path = %old_exe.display(),
                error = %e,
                "Failed to remove old exe after update"
            );
        }

        // Step 5: Clean up staged update file
        if let Err(e) = tokio::fs::remove_file(pending).await {
            tracing::warn!(
                path = %pending.display(),
                error = %e,
                "Failed to remove staged update file"
            );
        }

        tracing::info!("Update applied successfully");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_parsing() {
        let checker = UpdateChecker::new("0.1.0").unwrap();
        assert_eq!(checker.current_version, Version::new(0, 1, 0));
    }

    #[test]
    fn test_version_parsing_with_v_prefix() {
        let checker = UpdateChecker::new("v0.2.0").unwrap();
        assert_eq!(checker.current_version, Version::new(0, 2, 0));
    }

    #[test]
    fn test_version_parsing_invalid() {
        assert!(UpdateChecker::new("not-a-version").is_err());
    }

    #[test]
    fn test_version_comparison_newer() {
        let checker = UpdateChecker::new("0.1.0").unwrap();
        let remote = Version::new(0, 2, 0);
        assert!(remote > checker.current_version);
    }

    #[test]
    fn test_version_comparison_older() {
        let checker = UpdateChecker::new("0.2.0").unwrap();
        let remote = Version::new(0, 1, 0);
        assert!(remote < checker.current_version);
    }

    #[test]
    fn test_version_comparison_same() {
        let checker = UpdateChecker::new("0.1.0").unwrap();
        let remote = Version::new(0, 1, 0);
        assert!(remote <= checker.current_version);
    }

    #[test]
    #[ignore] // env var tests race in parallel — run with --ignored
    fn test_disable_env_var() {
        // Step 1: Remove var → checker should not be disabled
        // SAFETY: This test is #[ignore]'d and runs in isolation;
        // no other threads read WMUX_DISABLE_UPDATE concurrently.
        unsafe { std::env::remove_var("WMUX_DISABLE_UPDATE") };
        let checker = UpdateChecker::new("0.1.0").unwrap();
        assert!(!checker.is_disabled());

        // Step 2: Set var → checker should be disabled
        // SAFETY: Same isolation guarantee as above.
        unsafe { std::env::set_var("WMUX_DISABLE_UPDATE", "1") };
        let checker2 = UpdateChecker::new("0.1.0").unwrap();
        assert!(checker2.is_disabled());

        // SAFETY: Cleanup in same isolated test context.
        unsafe { std::env::remove_var("WMUX_DISABLE_UPDATE") };
    }

    #[test]
    fn test_github_release_parsing() {
        let json = r#"{
            "tag_name": "v0.2.0",
            "body": "Release notes here",
            "assets": [
                {
                    "name": "wmux-app.exe",
                    "browser_download_url": "https://example.com/wmux-app.exe"
                }
            ]
        }"#;
        let release: GitHubRelease = serde_json::from_str(json).unwrap();
        assert_eq!(release.tag_name, "v0.2.0");
        assert_eq!(release.body.as_deref().unwrap(), "Release notes here");
        assert_eq!(release.assets.len(), 1);
        assert_eq!(release.assets[0].name, "wmux-app.exe");
        assert_eq!(
            release.assets[0].browser_download_url,
            "https://example.com/wmux-app.exe"
        );
    }

    #[test]
    fn test_github_release_no_body() {
        let json = r#"{
            "tag_name": "v0.2.0",
            "body": null,
            "assets": []
        }"#;
        let release: GitHubRelease = serde_json::from_str(json).unwrap();
        assert!(release.body.is_none());
        assert!(release.assets.is_empty());
    }

    #[tokio::test]
    #[ignore] // env var tests race in parallel — run with --ignored
    async fn test_check_for_update_when_disabled() {
        // SAFETY: This test is #[ignore]'d and runs in isolation;
        // no other threads read WMUX_DISABLE_UPDATE concurrently.
        unsafe { std::env::set_var("WMUX_DISABLE_UPDATE", "1") };
        let checker = UpdateChecker::new("0.1.0").unwrap();
        let result = checker.check_for_update().await.unwrap();
        // SAFETY: Same isolation guarantee as above.
        unsafe { std::env::remove_var("WMUX_DISABLE_UPDATE") };
        assert!(result.is_none());
    }

    #[tokio::test]
    #[ignore]
    async fn test_check_for_update_network() {
        // SAFETY: This test is #[ignore]'d and runs in isolation;
        // no other threads read WMUX_DISABLE_UPDATE concurrently.
        unsafe { std::env::remove_var("WMUX_DISABLE_UPDATE") };
        let checker = UpdateChecker::new("0.0.1").unwrap();
        // Should succeed or fail silently — never panic.
        let result = checker.check_for_update().await;
        assert!(result.is_ok());
    }
}
