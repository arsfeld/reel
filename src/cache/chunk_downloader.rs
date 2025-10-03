use crate::cache::config::{DiskSpaceStatus, FileCacheConfig};
use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::db::entities::{CacheChunkModel, CacheEntryModel};
use crate::db::repository::CacheRepository;

use super::chunk_store::{ChunkStore, calculate_chunk_range};

/// Configuration for chunk download retries
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Initial delay before first retry
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Multiplier for exponential backoff
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
        }
    }
}

/// Downloads specific byte ranges from upstream servers and records chunks in database
pub struct ChunkDownloader {
    client: Client,
    repository: Arc<dyn CacheRepository>,
    chunk_store: Arc<ChunkStore>,
    chunk_size: u64,
    retry_config: RetryConfig,
    cache_config: Option<FileCacheConfig>,
}

impl ChunkDownloader {
    /// Create a new ChunkDownloader
    pub fn new(
        client: Client,
        repository: Arc<dyn CacheRepository>,
        chunk_store: Arc<ChunkStore>,
        chunk_size: u64,
    ) -> Self {
        Self {
            client,
            repository,
            chunk_store,
            chunk_size,
            retry_config: RetryConfig::default(),
            cache_config: None,
        }
    }

    /// Create a ChunkDownloader with custom retry configuration
    pub fn with_retry_config(mut self, retry_config: RetryConfig) -> Self {
        self.retry_config = retry_config;
        self
    }

    /// Set cache configuration for disk space monitoring
    pub fn with_cache_config(mut self, cache_config: FileCacheConfig) -> Self {
        self.cache_config = Some(cache_config);
        self
    }

    /// Download a specific chunk and record in database
    /// Returns a JoinHandle that can be awaited or cancelled
    pub async fn download_chunk(
        self: Arc<Self>,
        entry_id: i32,
        chunk_index: u64,
    ) -> Result<JoinHandle<Result<()>>> {
        let handle =
            tokio::spawn(async move { self.download_chunk_internal(entry_id, chunk_index).await });

        Ok(handle)
    }

    /// Internal implementation of chunk download with retry logic
    async fn download_chunk_internal(&self, entry_id: i32, chunk_index: u64) -> Result<()> {
        let mut attempt = 0;
        let mut delay = self.retry_config.initial_delay;

        loop {
            attempt += 1;

            match self.try_download_chunk(entry_id, chunk_index).await {
                Ok(()) => {
                    if attempt > 1 {
                        info!(
                            "Successfully downloaded chunk {} for entry {} after {} attempts",
                            chunk_index, entry_id, attempt
                        );
                    }
                    return Ok(());
                }
                Err(e) => {
                    if attempt >= self.retry_config.max_retries {
                        error!(
                            "Failed to download chunk {} for entry {} after {} attempts: {}",
                            chunk_index, entry_id, attempt, e
                        );
                        return Err(e);
                    }

                    warn!(
                        "Attempt {} failed for chunk {} (entry {}): {}. Retrying in {:?}...",
                        attempt, chunk_index, entry_id, e, delay
                    );

                    tokio::time::sleep(delay).await;

                    // Exponential backoff
                    delay = Duration::from_secs_f64(
                        (delay.as_secs_f64() * self.retry_config.backoff_multiplier)
                            .min(self.retry_config.max_delay.as_secs_f64()),
                    );
                }
            }
        }
    }

    /// Try to download a chunk once (no retries)
    async fn try_download_chunk(&self, entry_id: i32, chunk_index: u64) -> Result<()> {
        // AC #5: Proactively check disk space before download
        if let Some(ref config) = self.cache_config {
            match config.check_disk_space_status() {
                Ok(status) => match status {
                    DiskSpaceStatus::Critical { message, .. } => {
                        error!("{}", message);
                        warn!(
                            "Attempting emergency cleanup before download due to critical disk space"
                        );
                        // Trigger emergency cleanup
                        if let Err(e) = self.emergency_cleanup().await {
                            error!("Emergency cleanup failed: {}", e);
                            return Err(anyhow!(
                                "Cannot download chunk: disk space critically low and cleanup failed"
                            ));
                        }
                    }
                    DiskSpaceStatus::Warning { message, .. } => {
                        warn!("{}", message);
                    }
                    DiskSpaceStatus::Info { message, .. } => {
                        info!("{}", message);
                    }
                    DiskSpaceStatus::Healthy { .. } => {
                        // All good, continue
                    }
                },
                Err(e) => {
                    debug!("Could not check disk space status: {}", e);
                }
            }
        }

        // 1. Get cache entry from database
        let entries = self.repository.list_cache_entries().await?;
        let entry = entries
            .into_iter()
            .find(|e| e.id == entry_id)
            .ok_or_else(|| anyhow!("Cache entry {} not found", entry_id))?;

        // 2. Calculate byte range for this chunk
        let (start_byte, end_byte) = calculate_chunk_range(chunk_index, self.chunk_size, &entry);

        // 3. Make HTTP range request
        let response = self
            .client
            .get(&entry.original_url)
            .header("Range", format!("bytes={}-{}", start_byte, end_byte))
            .send()
            .await
            .with_context(|| {
                format!(
                    "Failed to send HTTP request for chunk {} (entry {})",
                    chunk_index, entry_id
                )
            })?;

        // 4. Check response status
        let status = response.status();
        if !status.is_success() && status != reqwest::StatusCode::PARTIAL_CONTENT {
            return Err(anyhow!(
                "HTTP request failed with status {} for chunk {} (entry {})",
                status,
                chunk_index,
                entry_id
            ));
        }

        // 5. Read response data
        let data = response.bytes().await.with_context(|| {
            format!(
                "Failed to read response body for chunk {} (entry {})",
                chunk_index, entry_id
            )
        })?;

        let data_len = data.len();

        // 6. Write to chunk store with disk space error handling
        // AC #2: Trigger emergency cleanup when disk full
        // AC #3: Retry write after cleanup
        let write_result = self
            .chunk_store
            .write_chunk(entry_id, chunk_index, self.chunk_size, &data)
            .await;

        if let Err(e) = write_result {
            // Check if this is a disk full error
            if e.to_string().contains("DISK_FULL") {
                warn!(
                    "Disk full detected while writing chunk {} for entry {}. Attempting emergency cleanup...",
                    chunk_index, entry_id
                );

                // AC #2: Trigger emergency cleanup
                match self.emergency_cleanup().await {
                    Ok(freed_bytes) => {
                        info!(
                            "Emergency cleanup freed {} MB. Retrying write for chunk {} (entry {})...",
                            freed_bytes / (1024 * 1024),
                            chunk_index,
                            entry_id
                        );

                        // AC #3: Retry write after cleanup
                        self.chunk_store
                            .write_chunk(entry_id, chunk_index, self.chunk_size, &data)
                            .await
                            .with_context(|| {
                                format!(
                                    "Failed to write chunk {} even after emergency cleanup (entry {})",
                                    chunk_index, entry_id
                                )
                            })?;

                        info!(
                            "Successfully wrote chunk {} after emergency cleanup",
                            chunk_index
                        );
                    }
                    Err(cleanup_err) => {
                        error!(
                            "Emergency cleanup failed: {}. Cannot write chunk {} (entry {})",
                            cleanup_err, chunk_index, entry_id
                        );
                        // AC #4: Fall back to passthrough (error propagates up, caller handles fallback)
                        return Err(e).with_context(|| {
                            format!(
                                "Disk full and emergency cleanup failed for chunk {} (entry {})",
                                chunk_index, entry_id
                            )
                        });
                    }
                }
            } else {
                // Not a disk full error, just propagate
                return Err(e).with_context(|| {
                    format!(
                        "Failed to write chunk {} to disk (entry {})",
                        chunk_index, entry_id
                    )
                });
            }
        }

        // 7. Record chunk in database
        let chunk = CacheChunkModel {
            id: 0, // Will be assigned by database
            cache_entry_id: entry_id,
            start_byte: start_byte as i64,
            end_byte: end_byte as i64,
            downloaded_at: Utc::now().naive_utc(),
        };

        self.repository
            .add_cache_chunk(chunk)
            .await
            .with_context(|| {
                format!(
                    "Failed to record chunk {} in database (entry {})",
                    chunk_index, entry_id
                )
            })?;

        // 8. Check if download is complete
        // Use downloaded bytes instead of chunk count since chunks are now merged
        let downloaded_bytes = self.repository.get_downloaded_bytes(entry_id).await?;
        let total_size = entry.expected_total_size.unwrap_or(0);

        if downloaded_bytes >= total_size && total_size > 0 {
            info!(
                "Download complete for entry {} ({} bytes downloaded)",
                entry_id, downloaded_bytes
            );

            // Mark entry as complete
            let mut updated_entry = entry;
            updated_entry.is_complete = true;
            updated_entry.downloaded_bytes = total_size;

            self.repository
                .update_cache_entry(updated_entry)
                .await
                .with_context(|| format!("Failed to mark entry {} as complete", entry_id))?;
        }

        info!(
            "Downloaded chunk {} for entry {} ({} bytes)",
            chunk_index, entry_id, data_len
        );

        Ok(())
    }

    /// Calculate total number of chunks for an entry
    fn calculate_total_chunks(&self, entry: &CacheEntryModel) -> u64 {
        let file_size = entry.expected_total_size.unwrap_or(0) as u64;
        (file_size + self.chunk_size - 1) / self.chunk_size
    }

    /// Get chunk size
    pub fn chunk_size(&self) -> u64 {
        self.chunk_size
    }

    /// Get a reference to the HTTP client
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Perform emergency cache cleanup to free disk space
    /// Returns the number of bytes freed
    async fn emergency_cleanup(&self) -> Result<i64> {
        info!("Starting emergency cache cleanup due to disk space exhaustion");

        // Get current cache entries sorted by last access time (LRU)
        let entries = self.repository.get_entries_for_cleanup(10).await?;

        if entries.is_empty() {
            warn!("No cache entries available for emergency cleanup");
            return Ok(0);
        }

        let mut total_freed = 0i64;

        // Delete oldest entries until we free at least 1GB or run out of entries
        const TARGET_FREE_BYTES: i64 = 1024 * 1024 * 1024; // 1 GB

        for entry in entries {
            let entry_id = entry.id;
            let entry_size = entry.downloaded_bytes;

            info!(
                "Emergency cleanup: Deleting cache entry {} (size: {} MB)",
                entry_id,
                entry_size / (1024 * 1024)
            );

            // Delete chunks for this entry
            if let Err(e) = self.repository.delete_chunks_for_entry(entry_id).await {
                warn!("Failed to delete chunks for entry {}: {}", entry_id, e);
                continue;
            }

            // Delete the entry itself
            if let Err(e) = self.repository.delete_cache_entry(entry_id).await {
                warn!("Failed to delete cache entry {}: {}", entry_id, e);
                continue;
            }

            // Delete the file from chunk store
            if let Err(e) = self.chunk_store.delete_file(entry_id).await {
                warn!("Failed to delete cache file for entry {}: {}", entry_id, e);
                // Continue anyway - database is cleaned up
            }

            total_freed += entry_size;

            // Check if we've freed enough space
            if total_freed >= TARGET_FREE_BYTES {
                info!(
                    "Emergency cleanup: Freed {} MB (target reached)",
                    total_freed / (1024 * 1024)
                );
                break;
            }
        }

        if total_freed > 0 {
            info!(
                "Emergency cleanup completed: {} MB freed",
                total_freed / (1024 * 1024)
            );
        } else {
            warn!("Emergency cleanup freed no space");
        }

        Ok(total_freed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::entities::CacheEntryModel;
    use async_trait::async_trait;
    use chrono::Utc;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::Mutex as AsyncMutex;

    // Mock repository for testing
    struct MockCacheRepository {
        entries: Arc<AsyncMutex<Vec<CacheEntryModel>>>,
        chunks: Arc<AsyncMutex<Vec<CacheChunkModel>>>,
    }

    impl MockCacheRepository {
        fn new() -> Self {
            Self {
                entries: Arc::new(AsyncMutex::new(Vec::new())),
                chunks: Arc::new(AsyncMutex::new(Vec::new())),
            }
        }

        async fn add_entry(&self, entry: CacheEntryModel) {
            let mut entries = self.entries.lock().await;
            entries.push(entry);
        }
    }

    #[async_trait]
    impl CacheRepository for MockCacheRepository {
        async fn find_cache_entry(
            &self,
            _source_id: &str,
            _media_id: &str,
            _quality: &str,
        ) -> Result<Option<CacheEntryModel>> {
            Ok(None)
        }

        async fn insert_cache_entry(&self, entry: CacheEntryModel) -> Result<CacheEntryModel> {
            self.add_entry(entry.clone()).await;
            Ok(entry)
        }

        async fn update_cache_entry(&self, entry: CacheEntryModel) -> Result<CacheEntryModel> {
            let mut entries = self.entries.lock().await;
            if let Some(existing) = entries.iter_mut().find(|e| e.id == entry.id) {
                *existing = entry.clone();
            }
            Ok(entry)
        }

        async fn delete_cache_entry(&self, _id: i32) -> Result<()> {
            Ok(())
        }

        async fn list_cache_entries(&self) -> Result<Vec<CacheEntryModel>> {
            let entries = self.entries.lock().await;
            Ok(entries.clone())
        }

        async fn mark_cache_accessed(&self, _id: i32) -> Result<()> {
            Ok(())
        }

        async fn update_download_progress(
            &self,
            _id: i32,
            _downloaded_bytes: i64,
            _is_complete: bool,
        ) -> Result<()> {
            Ok(())
        }

        async fn find_cache_entries_by_media(
            &self,
            _media_id: &str,
        ) -> Result<Vec<CacheEntryModel>> {
            Ok(Vec::new())
        }

        async fn find_cache_entries_by_source(
            &self,
            _source_id: &str,
        ) -> Result<Vec<CacheEntryModel>> {
            Ok(Vec::new())
        }

        async fn add_cache_chunk(&self, chunk: CacheChunkModel) -> Result<CacheChunkModel> {
            let mut chunks = self.chunks.lock().await;
            let mut chunk_with_id = chunk.clone();
            chunk_with_id.id = chunks.len() as i32 + 1;
            chunks.push(chunk_with_id.clone());
            Ok(chunk_with_id)
        }

        async fn get_chunks_for_entry(&self, cache_entry_id: i32) -> Result<Vec<CacheChunkModel>> {
            let chunks = self.chunks.lock().await;
            Ok(chunks
                .iter()
                .filter(|c| c.cache_entry_id == cache_entry_id)
                .cloned()
                .collect())
        }

        async fn delete_chunks_for_entry(&self, _cache_entry_id: i32) -> Result<()> {
            Ok(())
        }

        async fn has_byte_range(&self, cache_entry_id: i32, start: i64, end: i64) -> Result<bool> {
            let chunks = self.get_chunks_for_entry(cache_entry_id).await?;
            for chunk in chunks {
                if chunk.start_byte <= start && chunk.end_byte >= end {
                    return Ok(true);
                }
            }
            Ok(false)
        }

        async fn add_to_download_queue(
            &self,
            item: crate::db::entities::CacheDownloadQueueModel,
        ) -> Result<crate::db::entities::CacheDownloadQueueModel> {
            Ok(item)
        }

        async fn get_pending_downloads(
            &self,
        ) -> Result<Vec<crate::db::entities::CacheDownloadQueueModel>> {
            Ok(Vec::new())
        }

        async fn update_download_status(&self, _id: i32, _status: String) -> Result<()> {
            Ok(())
        }

        async fn increment_retry_count(&self, _id: i32) -> Result<()> {
            Ok(())
        }

        async fn remove_from_queue(&self, _id: i32) -> Result<()> {
            Ok(())
        }

        async fn find_in_queue(
            &self,
            _media_id: &str,
            _source_id: &str,
        ) -> Result<Option<crate::db::entities::CacheDownloadQueueModel>> {
            Ok(None)
        }

        async fn get_cache_statistics(
            &self,
        ) -> Result<Option<crate::db::entities::CacheStatisticsModel>> {
            Ok(None)
        }

        async fn update_cache_statistics(
            &self,
            stats: crate::db::entities::CacheStatisticsModel,
        ) -> Result<crate::db::entities::CacheStatisticsModel> {
            Ok(stats)
        }

        async fn increment_cache_hit(&self) -> Result<()> {
            Ok(())
        }

        async fn increment_cache_miss(&self) -> Result<()> {
            Ok(())
        }

        async fn update_cache_size(&self, _total_size: i64, _file_count: i32) -> Result<()> {
            Ok(())
        }

        async fn add_cache_headers(
            &self,
            _headers: Vec<crate::db::entities::CacheHeaderModel>,
        ) -> Result<()> {
            Ok(())
        }

        async fn get_headers_for_entry(
            &self,
            _cache_entry_id: i32,
        ) -> Result<Vec<crate::db::entities::CacheHeaderModel>> {
            Ok(Vec::new())
        }

        async fn delete_headers_for_entry(&self, _cache_entry_id: i32) -> Result<()> {
            Ok(())
        }

        async fn add_quality_variant(
            &self,
            variant: crate::db::entities::CacheQualityVariantModel,
        ) -> Result<crate::db::entities::CacheQualityVariantModel> {
            Ok(variant)
        }

        async fn find_quality_variants(
            &self,
            _media_id: &str,
            _source_id: &str,
        ) -> Result<Vec<crate::db::entities::CacheQualityVariantModel>> {
            Ok(Vec::new())
        }

        async fn delete_quality_variants(&self, _media_id: &str, _source_id: &str) -> Result<()> {
            Ok(())
        }

        async fn get_entries_for_cleanup(&self, _limit: usize) -> Result<Vec<CacheEntryModel>> {
            Ok(Vec::new())
        }

        async fn delete_old_entries(&self, _days_old: i64) -> Result<u64> {
            Ok(0)
        }

        async fn get_downloaded_bytes(&self, cache_entry_id: i32) -> Result<i64> {
            let chunks = self.get_chunks_for_entry(cache_entry_id).await?;
            Ok(chunks.iter().map(|c| c.end_byte - c.start_byte + 1).sum())
        }

        async fn get_chunk_count(&self, cache_entry_id: i32) -> Result<usize> {
            let chunks = self.get_chunks_for_entry(cache_entry_id).await?;
            Ok(chunks.len())
        }

        async fn has_pending_downloads(&self, _source_id: &str, _media_id: &str) -> Result<bool> {
            Ok(false)
        }
    }

    fn create_test_entry(id: i32, url: &str, size: i64) -> CacheEntryModel {
        CacheEntryModel {
            id,
            source_id: "test".to_string(),
            media_id: format!("media_{}", id),
            quality: "1080p".to_string(),
            original_url: url.to_string(),
            file_path: format!("/tmp/test_{}.cache", id),
            file_size: 0,
            expected_total_size: Some(size),
            downloaded_bytes: 0,
            is_complete: false,
            priority: 0,
            created_at: Utc::now().naive_utc(),
            last_accessed: Utc::now().naive_utc(),
            last_modified: Utc::now().naive_utc(),
            access_count: 0,
            mime_type: Some("video/mp4".to_string()),
            video_codec: None,
            audio_codec: None,
            container: None,
            resolution_width: None,
            resolution_height: None,
            bitrate: None,
            duration_secs: None,
            etag: None,
            expires_at: None,
        }
    }

    #[test]
    fn test_calculate_total_chunks() {
        let chunk_size = 2 * 1024 * 1024; // 2MB
        let client = Client::new();
        let repo = Arc::new(MockCacheRepository::new());
        let temp_dir = TempDir::new().unwrap();
        let chunk_store = Arc::new(ChunkStore::new(temp_dir.path().to_path_buf()));

        let downloader = ChunkDownloader::new(client, repo, chunk_store, chunk_size);

        // Test file exactly divisible by chunk size
        let entry = create_test_entry(1, "http://test", chunk_size as i64 * 10);
        assert_eq!(downloader.calculate_total_chunks(&entry), 10);

        // Test file with partial last chunk
        let entry = create_test_entry(2, "http://test", chunk_size as i64 * 10 + 1000);
        assert_eq!(downloader.calculate_total_chunks(&entry), 11);

        // Test small file (less than one chunk)
        let entry = create_test_entry(3, "http://test", 1000);
        assert_eq!(downloader.calculate_total_chunks(&entry), 1);
    }

    #[test]
    fn test_retry_config_defaults() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_delay, Duration::from_millis(500));
        assert_eq!(config.max_delay, Duration::from_secs(30));
        assert_eq!(config.backoff_multiplier, 2.0);
    }
}
