use anyhow::Result;
use std::sync::Arc;
use tracing::{debug, error};

use super::metadata::MediaCacheKey;
use super::state_types::{DownloadState, DownloadStateInfo};
use crate::db::repository::cache_repository::CacheRepository;

/// Utility for computing download state from database
pub struct StateComputer {
    repository: Arc<dyn CacheRepository>,
}

impl StateComputer {
    /// Create a new state computer
    pub fn new(repository: Arc<dyn CacheRepository>) -> Self {
        Self { repository }
    }

    /// Get state info for a cache key by querying the database
    pub async fn get_state(&self, cache_key: &MediaCacheKey) -> Option<DownloadStateInfo> {
        // Try to find the cache entry in the database
        let entry_result = self
            .repository
            .find_cache_entry(
                cache_key.source_id.as_str(),
                cache_key.media_id.as_str(),
                &cache_key.quality,
            )
            .await;

        let entry = match entry_result {
            Ok(Some(e)) => e,
            Ok(None) => {
                debug!("No cache entry found for key: {:?}", cache_key);
                return None;
            }
            Err(e) => {
                error!("Error finding cache entry: {}", e);
                return None;
            }
        };

        // Compute state from database
        let state = self.compute_state(entry.id, &entry).await;

        // Get downloaded bytes from chunks
        let downloaded_bytes = self
            .repository
            .get_downloaded_bytes(entry.id)
            .await
            .unwrap_or(0) as u64;

        // Get total size
        let total_size = entry.expected_total_size.map(|s| s as u64);

        let mut info = DownloadStateInfo::new(cache_key.clone());
        info.state = state;
        info.downloaded_bytes = downloaded_bytes;
        info.total_size = total_size;
        info.cache_entry_id = Some(entry.id);
        info.last_updated = entry.last_modified.and_utc();

        Some(info)
    }

    /// Compute the download state for a cache entry
    async fn compute_state(
        &self,
        entry_id: i32,
        entry: &crate::db::entities::CacheEntryModel,
    ) -> DownloadState {
        // Check if complete
        if entry.is_complete {
            return DownloadState::Complete;
        }

        // Get chunk count
        let chunk_count = self.repository.get_chunk_count(entry_id).await.unwrap_or(0);

        // Check if we have any chunks
        if chunk_count == 0 {
            // No chunks - check if we have expected size
            if entry.expected_total_size.is_none() || entry.expected_total_size == Some(0) {
                return DownloadState::Initializing;
            } else {
                return DownloadState::NotStarted;
            }
        }

        // Has chunks - check if downloading or paused
        let has_pending = self
            .repository
            .has_pending_downloads(&entry.source_id, &entry.media_id)
            .await
            .unwrap_or(false);

        if has_pending {
            return DownloadState::Downloading;
        }

        // No pending downloads - check if complete based on size
        if let Some(expected_size) = entry.expected_total_size {
            let downloaded = self
                .repository
                .get_downloaded_bytes(entry_id)
                .await
                .unwrap_or(0);

            if downloaded >= expected_size {
                return DownloadState::Complete;
            }
        }

        // Has chunks but not downloading and not complete
        DownloadState::Paused
    }

    /// Check if state allows serving partial data
    pub async fn can_serve_partial(&self, cache_key: &MediaCacheKey) -> bool {
        if let Some(info) = self.get_state(cache_key).await {
            matches!(
                info.state,
                DownloadState::Downloading | DownloadState::Paused | DownloadState::Complete
            )
        } else {
            false
        }
    }

    /// Check if download is active
    pub async fn is_active(&self, cache_key: &MediaCacheKey) -> bool {
        if let Some(info) = self.get_state(cache_key).await {
            matches!(
                info.state,
                DownloadState::Initializing | DownloadState::Downloading
            )
        } else {
            false
        }
    }

    /// Check if specific byte range is available
    pub async fn has_byte_range(
        &self,
        cache_key: &MediaCacheKey,
        start: u64,
        end: u64,
    ) -> Result<bool> {
        // Get cache entry
        let entry = self
            .repository
            .find_cache_entry(
                cache_key.source_id.as_str(),
                cache_key.media_id.as_str(),
                &cache_key.quality,
            )
            .await?;

        let Some(entry) = entry else {
            return Ok(false);
        };

        // Query database for range availability
        self.repository
            .has_byte_range(entry.id, start as i64, end as i64)
            .await
    }

    /// Get download progress percentage
    pub async fn progress_percent(&self, cache_key: &MediaCacheKey) -> f64 {
        if let Some(info) = self.get_state(cache_key).await {
            if let Some(total) = info.total_size {
                if total > 0 {
                    return (info.downloaded_bytes as f64 / total as f64) * 100.0;
                }
            }
        }
        0.0
    }

    /// Check if enough data is available for playback
    pub async fn has_minimum_data(&self, cache_key: &MediaCacheKey) -> bool {
        if let Some(info) = self.get_state(cache_key).await {
            info.has_minimum_data()
        } else {
            false
        }
    }
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;
    use crate::models::{MediaItemId, SourceId};

    // Note: Full tests would require mocking the repository
    // For now, just test basic struct creation

    #[test]
    fn test_state_computer_creation() {
        // This test just verifies the module compiles correctly
        // Real tests would require repository mocking
    }
}
