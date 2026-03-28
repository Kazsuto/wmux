// Module is declared but not yet wired into the main application loop.
// Background polling and title bar integration are scheduled for a later wave.

use anyhow::{Context, Result};
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Duration;

// TODO(release): Update to the correct GitHub repository before first release.
const GITHUB_API_URL: &str = "https://api.github.com/repos/pimalaya/wmux/releases/latest";
const USER_AGENT: &str = "wmux-updater";
/// Maximum download size (200 MB). Prevents disk exhaustion from malicious servers.
const MAX_DOWNLOAD_SIZE: u64 = 200 * 1024 * 1024;
/// Allowed HTTPS hosts for update downloads.
const ALLOWED_DOWNLOAD_HOSTS: &[&str] = &["github.com", "objects.githubusercontent.com"];

pub struct UpdateChecker {
    current_version: Version,
    update_dir: PathBuf,
    current_exe: PathBuf,
    disabled: bool,
    client: reqwest::Client,
}

#[expect(dead_code, reason = "consumed by UpdateChecker which is not yet wired")]
#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub version: Version,
    pub download_url: String,
    pub checksum_url: Option<String>,
    pub release_notes: String,
}

// Note: `deny_unknown_fields` is intentionally NOT used on these structs.
// The GitHub API returns many fields beyond what we deserialize (html_url,
// author, draft, prerelease, etc.). Denying them would break deserialization.
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

#[expect(dead_code, reason = "not yet wired into the application loop")]
impl UpdateChecker {
    /// Create a new update checker.
    ///
    /// `disabled` controls whether update checks are skipped entirely.
    /// The caller is responsible for reading `WMUX_DISABLE_UPDATE` env var
    /// (or any other source) and passing the result here.
    pub fn new(current_version: &str, disabled: bool) -> Result<Self> {
        let version_str = current_version.trim_start_matches('v');
        let current_version = Version::parse(version_str)
            .with_context(|| format!("Failed to parse version: {current_version}"))?;

        let update_dir = dirs::config_dir()
            .context("Failed to find config directory")?
            .join("wmux")
            .join("updates");

        // Resolve current exe path once at construction time (sync context).
        // This avoids blocking calls inside async methods later.
        let current_exe = std::env::current_exe().context("Failed to get current exe path")?;

        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                // Block protocol downgrade: only follow HTTPS redirects.
                if attempt.url().scheme() != "https" {
                    return attempt.error(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Redirect to non-HTTPS URL blocked",
                    ));
                }
                attempt.follow()
            }))
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self {
            current_version,
            update_dir,
            current_exe,
            disabled,
            client,
        })
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    pub async fn check_for_update(&self) -> Result<Option<UpdateInfo>> {
        if self.disabled {
            tracing::debug!("Update check disabled");
            return Ok(None);
        }

        tracing::debug!("Checking for updates");

        let client = &self.client;
        let response = match client.get(GITHUB_API_URL).send().await {
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

        // Look for a .sha256 checksum sidecar asset.
        let checksum_url = release
            .assets
            .iter()
            .find(|a| a.name.contains("wmux-app") && a.name.ends_with(".sha256"))
            .map(|a| a.browser_download_url.clone());

        let release_notes = release.body.unwrap_or_default();

        tracing::info!(
            current = %self.current_version,
            available = %remote_version,
            has_checksum = checksum_url.is_some(),
            "Update available"
        );

        Ok(Some(UpdateInfo {
            version: remote_version,
            download_url,
            checksum_url,
            release_notes,
        }))
    }

    /// Validate that a download URL is safe: HTTPS scheme and allowed host.
    fn validate_download_url(url: &str) -> Result<()> {
        let parsed: reqwest::Url = url
            .parse()
            .with_context(|| format!("Invalid download URL: {url}"))?;

        if parsed.scheme() != "https" {
            anyhow::bail!("Download URL must use HTTPS, got: {}", parsed.scheme());
        }

        let host = parsed.host_str().context("Download URL has no host")?;

        if !ALLOWED_DOWNLOAD_HOSTS
            .iter()
            .any(|allowed| host == *allowed || host.ends_with(&format!(".{allowed}")))
        {
            anyhow::bail!(
                "Download host '{host}' is not in the allowed list: {ALLOWED_DOWNLOAD_HOSTS:?}"
            );
        }

        Ok(())
    }

    /// Fetch the SHA-256 checksum from the sidecar asset URL.
    /// Expected format: `<hex-hash>  <filename>` or just `<hex-hash>`.
    async fn fetch_checksum(&self, checksum_url: &str) -> Result<String> {
        Self::validate_download_url(checksum_url)?;

        let client = &self.client;
        let response = client
            .get(checksum_url)
            .send()
            .await
            .context("Failed to fetch checksum file")?;

        if !response.status().is_success() {
            anyhow::bail!("Checksum fetch failed with status: {}", response.status());
        }

        let text = response
            .text()
            .await
            .context("Failed to read checksum response")?;

        // Parse: either "abcdef1234..." or "abcdef1234...  filename.exe"
        let hash = text
            .split_whitespace()
            .next()
            .context("Empty checksum file")?
            .to_lowercase();

        // Validate it looks like a hex SHA-256 hash (64 hex chars).
        if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
            anyhow::bail!("Invalid SHA-256 checksum format: {hash}");
        }

        Ok(hash)
    }

    pub async fn download_update(&self, info: &UpdateInfo) -> Result<PathBuf> {
        Self::validate_download_url(&info.download_url)?;

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

        let client = &self.client;
        let mut response = client
            .get(&info.download_url)
            .send()
            .await
            .context("Failed to start update download")?;

        if !response.status().is_success() {
            anyhow::bail!("Download request failed with status: {}", response.status());
        }

        // Reject obviously oversized downloads before streaming.
        if let Some(content_length) = response.content_length() {
            if content_length > MAX_DOWNLOAD_SIZE {
                anyhow::bail!(
                    "Download too large: {content_length} bytes (max {MAX_DOWNLOAD_SIZE})"
                );
            }
        }

        let mut file = tokio::fs::File::create(&dest_path)
            .await
            .with_context(|| format!("Failed to create file: {}", dest_path.display()))?;

        use tokio::io::AsyncWriteExt;
        let mut total_bytes: u64 = 0;
        let mut hasher = Sha256::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .context("Failed to read response chunk")?
        {
            total_bytes += chunk.len() as u64;
            if total_bytes > MAX_DOWNLOAD_SIZE {
                drop(file);
                let _ = tokio::fs::remove_file(&dest_path).await;
                anyhow::bail!("Download exceeded size limit: >{MAX_DOWNLOAD_SIZE} bytes");
            }
            hasher.update(&chunk);
            file.write_all(&chunk)
                .await
                .context("Failed to write chunk to file")?;
        }
        file.flush().await.context("Failed to flush update file")?;
        drop(file);

        // Reject empty downloads — a 0-byte file is never a valid executable.
        if total_bytes == 0 {
            let _ = tokio::fs::remove_file(&dest_path).await;
            anyhow::bail!("Download returned 0 bytes — refusing empty update");
        }

        // Verify SHA-256 checksum (computed incrementally during download).
        let actual_hash = format!("{:x}", hasher.finalize());
        if let Some(ref checksum_url) = info.checksum_url {
            let expected = self.fetch_checksum(checksum_url).await?;
            if actual_hash != expected {
                let _ = tokio::fs::remove_file(&dest_path).await;
                anyhow::bail!(
                    "Checksum mismatch: expected {expected}, got {actual_hash}. \
                     Downloaded file has been deleted."
                );
            }
            tracing::info!("SHA-256 checksum verified");
        } else {
            let _ = tokio::fs::remove_file(&dest_path).await;
            anyhow::bail!("Release has no .sha256 checksum sidecar — refusing unsigned update");
        }

        tracing::info!(
            path = %dest_path.display(),
            bytes = total_bytes,
            "Update downloaded successfully"
        );

        Ok(dest_path)
    }

    /// Check if a staged update exe exists in the update directory.
    /// Returns the path to the highest-versioned pending update that is
    /// newer than the current version, if any.
    ///
    /// Also performs recovery: if a previous update was interrupted (current
    /// exe missing but `.old` backup exists), restores the backup first.
    pub async fn check_pending_update(&self) -> Option<PathBuf> {
        // Recovery: if a previous apply was interrupted, restore the backup.
        let old_exe = self.current_exe.with_extension("old");
        let current_exists = tokio::fs::try_exists(&self.current_exe)
            .await
            .unwrap_or(true);
        let old_exists = tokio::fs::try_exists(&old_exe).await.unwrap_or(false);
        if !current_exists && old_exists {
            tracing::warn!(
                old = %old_exe.display(),
                current = %self.current_exe.display(),
                "Detected interrupted update, restoring backup"
            );
            if let Err(e) = tokio::fs::rename(&old_exe, &self.current_exe).await {
                tracing::error!(
                    error = %e,
                    "Failed to restore backup exe after interrupted update"
                );
            }
        }

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
    ///
    /// The `pending` path must be inside this checker's update directory and
    /// match the expected filename pattern (`wmux-app-v*.exe`).
    pub async fn apply_pending_update(&self, pending: &Path) -> Result<()> {
        // Validate that pending is inside our update directory.
        let canonical_pending = tokio::fs::canonicalize(pending)
            .await
            .with_context(|| format!("Failed to canonicalize: {}", pending.display()))?;
        let canonical_update_dir = tokio::fs::canonicalize(&self.update_dir)
            .await
            .with_context(|| {
                format!(
                    "Failed to canonicalize update dir: {}",
                    self.update_dir.display()
                )
            })?;
        if !canonical_pending.starts_with(&canonical_update_dir) {
            anyhow::bail!(
                "Pending update path is outside update directory: {}",
                pending.display()
            );
        }

        // Validate filename pattern.
        let filename = pending
            .file_name()
            .and_then(|n| n.to_str())
            .context("Pending update has no filename")?;
        if !filename.starts_with("wmux-app-v") || !filename.ends_with(".exe") {
            anyhow::bail!("Pending update filename does not match expected pattern: {filename}");
        }

        let old_exe = self.current_exe.with_extension("old");
        let new_exe = self.current_exe.with_extension("new");

        tracing::info!(
            pending = %pending.display(),
            current = %self.current_exe.display(),
            "Applying pending update"
        );

        // Step 1: Copy update to .new staging path (current exe still intact)
        tokio::fs::copy(pending, &new_exe)
            .await
            .with_context(|| format!("Failed to stage update at {}", new_exe.display()))?;

        // Step 2: Rename current exe to .old
        tokio::fs::rename(&self.current_exe, &old_exe)
            .await
            .with_context(|| {
                format!(
                    "Failed to rename {} to {}",
                    self.current_exe.display(),
                    old_exe.display()
                )
            })?;

        // Step 3: Rename .new to current exe path
        if let Err(e) = tokio::fs::rename(&new_exe, &self.current_exe).await {
            // Restore: rename .old back to current
            if let Err(restore_err) = tokio::fs::rename(&old_exe, &self.current_exe).await {
                tracing::error!(
                    error = %restore_err,
                    "CRITICAL: failed to restore original exe after update failure"
                );
            }
            return Err(e).with_context(|| {
                format!(
                    "Failed to move staged update to {}",
                    self.current_exe.display()
                )
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
        let checker = UpdateChecker::new("0.1.0", false).unwrap();
        assert_eq!(checker.current_version, Version::new(0, 1, 0));
    }

    #[test]
    fn test_version_parsing_with_v_prefix() {
        let checker = UpdateChecker::new("v0.2.0", false).unwrap();
        assert_eq!(checker.current_version, Version::new(0, 2, 0));
    }

    #[test]
    fn test_version_parsing_invalid() {
        assert!(UpdateChecker::new("not-a-version", false).is_err());
    }

    #[test]
    fn test_version_comparison_newer() {
        let checker = UpdateChecker::new("0.1.0", false).unwrap();
        let remote = Version::new(0, 2, 0);
        assert!(remote > checker.current_version);
    }

    #[test]
    fn test_version_comparison_older() {
        let checker = UpdateChecker::new("0.2.0", false).unwrap();
        let remote = Version::new(0, 1, 0);
        assert!(remote < checker.current_version);
    }

    #[test]
    fn test_version_comparison_same() {
        let checker = UpdateChecker::new("0.1.0", false).unwrap();
        let remote = Version::new(0, 1, 0);
        assert!(remote <= checker.current_version);
    }

    #[test]
    fn test_disabled_flag() {
        let enabled = UpdateChecker::new("0.1.0", false).unwrap();
        assert!(!enabled.is_disabled());

        let disabled = UpdateChecker::new("0.1.0", true).unwrap();
        assert!(disabled.is_disabled());
    }

    #[test]
    fn test_github_release_parsing() {
        let json = r#"{
            "tag_name": "v0.2.0",
            "body": "Release notes here",
            "assets": [
                {
                    "name": "wmux-app.exe",
                    "browser_download_url": "https://github.com/example/wmux/releases/download/v0.2.0/wmux-app.exe"
                }
            ]
        }"#;
        let release: GitHubRelease = serde_json::from_str(json).unwrap();
        assert_eq!(release.tag_name, "v0.2.0");
        assert_eq!(release.body.as_deref().unwrap(), "Release notes here");
        assert_eq!(release.assets.len(), 1);
        assert_eq!(release.assets[0].name, "wmux-app.exe");
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
    async fn test_check_for_update_when_disabled() {
        let checker = UpdateChecker::new("0.1.0", true).unwrap();
        let result = checker.check_for_update().await.unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_validate_download_url_https_github() {
        assert!(UpdateChecker::validate_download_url(
            "https://github.com/user/repo/releases/download/v1.0/app.exe"
        )
        .is_ok());
    }

    #[test]
    fn test_validate_download_url_https_githubusercontent() {
        assert!(UpdateChecker::validate_download_url(
            "https://objects.githubusercontent.com/some/path"
        )
        .is_ok());
    }

    #[test]
    fn test_validate_download_url_github_subdomain() {
        assert!(UpdateChecker::validate_download_url(
            "https://api.github.com/repos/user/repo/releases"
        )
        .is_ok());
    }

    #[test]
    fn test_validate_download_url_rejects_http() {
        assert!(UpdateChecker::validate_download_url(
            "http://github.com/user/repo/releases/download/v1.0/app.exe"
        )
        .is_err());
    }

    #[test]
    fn test_validate_download_url_rejects_unknown_host() {
        assert!(UpdateChecker::validate_download_url("https://evil.com/malware.exe").is_err());
    }

    #[test]
    fn test_validate_download_url_rejects_lookalike_host() {
        assert!(UpdateChecker::validate_download_url("https://notgithub.com/fake.exe").is_err());
    }

    #[tokio::test]
    #[ignore] // Hits the network — run with --ignored
    async fn test_check_for_update_network() {
        let checker = UpdateChecker::new("0.0.1", false).unwrap();
        // Should succeed or fail silently — never panic.
        let result = checker.check_for_update().await;
        assert!(result.is_ok());
    }
}
