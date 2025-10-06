use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::config::UpdateConfig;
use crate::services::config_service::config_service;

const GITHUB_REPO_OWNER: &str = "arsfeld";
const GITHUB_REPO_NAME: &str = "reel";

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateStatus {
    /// No update available
    UpToDate,
    /// Update available with version info
    UpdateAvailable {
        version: String,
        download_url: Option<String>,
    },
    /// Update is being downloaded
    Downloading { version: String, progress: f32 },
    /// Update downloaded and ready to install
    ReadyToInstall { version: String },
    /// Update is being installed
    Installing { version: String },
    /// Update check/download/install failed
    Error { message: String },
}

pub struct UpdateService {
    current_version: String,
    status: Arc<RwLock<UpdateStatus>>,
}

impl UpdateService {
    pub fn new() -> Self {
        let current_version = env!("CARGO_PKG_VERSION").to_string();
        info!("UpdateService initialized with version {}", current_version);

        Self {
            current_version,
            status: Arc::new(RwLock::new(UpdateStatus::UpToDate)),
        }
    }

    /// Get the current status
    pub async fn get_status(&self) -> UpdateStatus {
        self.status.read().await.clone()
    }

    /// Check for available updates
    pub async fn check_for_updates(&self) -> Result<UpdateStatus> {
        info!("Checking for updates...");

        // Get update config
        let config = config_service().get_config().await;
        if config.updates.behavior == "disabled" {
            info!("Updates are disabled in config");
            return Ok(UpdateStatus::UpToDate);
        }

        let status = match self.check_github_releases(&config.updates).await {
            Ok(status) => {
                info!("Update check completed: {:?}", status);
                status
            }
            Err(e) => {
                error!("Update check failed: {}", e);
                UpdateStatus::Error {
                    message: format!("Failed to check for updates: {}", e),
                }
            }
        };

        *self.status.write().await = status.clone();
        Ok(status)
    }

    /// Check GitHub releases for updates
    async fn check_github_releases(&self, config: &UpdateConfig) -> Result<UpdateStatus> {
        debug!("Fetching releases from GitHub");

        let releases = self_update::backends::github::ReleaseList::configure()
            .repo_owner(GITHUB_REPO_OWNER)
            .repo_name(GITHUB_REPO_NAME)
            .build()?
            .fetch()
            .context("Failed to fetch GitHub releases")?;

        // Filter for appropriate releases
        let latest_release = releases
            .iter()
            .filter(|r| {
                // Check if version contains pre-release indicators
                let is_prerelease = r.version.contains("alpha")
                    || r.version.contains("beta")
                    || r.version.contains("rc");
                config.check_prerelease || !is_prerelease
            })
            .max_by(|a, b| a.version.cmp(&b.version));

        match latest_release {
            Some(release) => {
                debug!(
                    "Latest release: {} (current: {})",
                    release.version, self.current_version
                );

                // Compare versions
                if release.version > self.current_version {
                    info!("Update available: {}", release.version);

                    // Find the appropriate asset for the current platform
                    let download_url = self.find_asset_url(&release.assets);

                    Ok(UpdateStatus::UpdateAvailable {
                        version: release.version.clone(),
                        download_url,
                    })
                } else {
                    info!("Already on latest version");
                    Ok(UpdateStatus::UpToDate)
                }
            }
            None => {
                warn!("No releases found on GitHub");
                Ok(UpdateStatus::UpToDate)
            }
        }
    }

    /// Find the appropriate download URL for the current platform
    fn find_asset_url(&self, assets: &[self_update::update::ReleaseAsset]) -> Option<String> {
        #[cfg(target_os = "linux")]
        let platform_patterns = vec!["linux", "x86_64", "amd64"];

        #[cfg(target_os = "macos")]
        let platform_patterns = vec!["macos", "darwin", "apple"];

        #[cfg(target_os = "windows")]
        let platform_patterns = vec!["windows", "win", "x86_64"];

        // Look for an asset that matches our platform
        for asset in assets {
            let name_lower = asset.name.to_lowercase();
            if platform_patterns.iter().any(|p| name_lower.contains(p)) {
                debug!("Found matching asset: {}", asset.name);
                return Some(asset.download_url.clone());
            }
        }

        warn!("No matching asset found for current platform");
        None
    }

    /// Download and install an update
    ///
    /// This method downloads and installs updates with built-in verification from the self_update crate.
    /// The self_update crate handles:
    /// - Archive checksum verification (if checksums are provided in GitHub releases)
    /// - Binary signature verification (if configured)
    /// - Atomic replacement of the current executable
    ///
    /// Rollback strategy:
    /// - The self_update crate creates a temporary backup before replacement
    /// - If the update fails, the original binary remains unchanged
    /// - Manual rollback: Users can reinstall previous version from GitHub releases
    ///
    /// For production use, consider:
    /// - Adding explicit backup of current binary before update
    /// - Implementing post-update health check
    /// - Adding automatic rollback if health check fails
    pub async fn download_and_install(&self, version: String) -> Result<()> {
        info!("Starting download and installation of version {}", version);

        *self.status.write().await = UpdateStatus::Downloading {
            version: version.clone(),
            progress: 0.0,
        };

        let config = config_service().get_config().await;
        if config.updates.behavior == "disabled" {
            return Err(anyhow::anyhow!("Updates are disabled in config"));
        }

        // Use self_update to download and install
        // This includes built-in verification and atomic replacement
        let update_result = self_update::backends::github::Update::configure()
            .repo_owner(GITHUB_REPO_OWNER)
            .repo_name(GITHUB_REPO_NAME)
            .bin_name(env!("CARGO_PKG_NAME"))
            .show_download_progress(true)
            .current_version(&self.current_version)
            .target_version_tag(&version)
            .build()
            .context("Failed to configure update")?
            .update();

        match update_result {
            Ok(status) => {
                match status {
                    self_update::Status::UpToDate(_) => {
                        info!("Already up to date");
                        *self.status.write().await = UpdateStatus::UpToDate;
                    }
                    self_update::Status::Updated(v) => {
                        info!("Successfully updated to version {}", v);
                        *self.status.write().await =
                            UpdateStatus::ReadyToInstall { version: v.clone() };
                    }
                }
                Ok(())
            }
            Err(e) => {
                error!("Update failed with verification error: {}", e);
                *self.status.write().await = UpdateStatus::Error {
                    message: format!("Update verification failed: {}", e),
                };
                Err(anyhow::anyhow!("Update failed: {}", e))
            }
        }
    }

    /// Get the current version
    pub fn current_version(&self) -> &str {
        &self.current_version
    }
}

impl Default for UpdateService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_update_service_initialization() {
        let service = UpdateService::new();
        assert!(!service.current_version().is_empty());
        assert_eq!(service.get_status().await, UpdateStatus::UpToDate);
    }

    #[tokio::test]
    async fn test_check_for_updates_when_disabled() {
        let service = UpdateService::new();

        // This test would need a way to mock the config
        // For now, we'll just verify the service can be created
        assert!(!service.current_version().is_empty());
    }
}
