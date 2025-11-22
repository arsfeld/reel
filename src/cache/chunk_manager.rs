use crate::db::repository::CacheRepository;
use anyhow::Result;
use reqwest::Client;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Notify, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

use super::chunk_downloader::ChunkDownloader;
use super::chunk_store::ChunkStore;

/// Priority levels for chunk downloads
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
    CRITICAL = 0, // Needed NOW for playback
    HIGH = 1,     // Next few chunks for smooth playback
    MEDIUM = 2,   // User-requested pre-cache
    LOW = 3,      // Background sequential fill
}

/// A chunk download request with priority
#[derive(Debug, Clone, PartialEq, Eq)]
struct ChunkRequest {
    entry_id: i32,
    chunk_index: u64,
    priority: Priority,
    requested_at: Instant,
}

impl ChunkRequest {
    fn new(entry_id: i32, chunk_index: u64, priority: Priority) -> Self {
        Self {
            entry_id,
            chunk_index,
            priority,
            requested_at: Instant::now(),
        }
    }
}

impl Ord for ChunkRequest {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority first (lower number = higher priority)
        // Reverse comparison so lower priority numbers (CRITICAL=0) compare as greater
        // Then FIFO (earlier requests first)
        other
            .priority
            .cmp(&self.priority)
            .then_with(|| self.requested_at.cmp(&other.requested_at))
    }
}

impl PartialOrd for ChunkRequest {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Coordinates chunk operations: availability queries, download requests, and event notifications
pub struct ChunkManager {
    repository: Arc<dyn CacheRepository>,
    priority_queue: Arc<Mutex<BinaryHeap<ChunkRequest>>>,
    chunk_waiters: Arc<RwLock<HashMap<(i32, u64), Arc<Notify>>>>,
    chunk_size: u64,
    downloader: Arc<ChunkDownloader>,
    chunk_store: Arc<ChunkStore>,
    active_downloads: Arc<Mutex<HashMap<(i32, u64), JoinHandle<Result<()>>>>>,
    max_concurrent_downloads: usize,
}

/// Callback handler for chunk download completion
struct ChunkManagerCallback {
    active_downloads: Arc<Mutex<HashMap<(i32, u64), JoinHandle<Result<()>>>>>,
    chunk_waiters: Arc<RwLock<HashMap<(i32, u64), Arc<Notify>>>>,
    priority_queue: Arc<Mutex<BinaryHeap<ChunkRequest>>>,
    downloader: Arc<ChunkDownloader>,
    max_concurrent_downloads: usize,
}

impl ChunkManagerCallback {
    async fn notify_chunk_available(&self, entry_id: i32, chunk_index: u64) {
        let key = (entry_id, chunk_index);
        let waiters = self.chunk_waiters.read().await;

        if let Some(notify) = waiters.get(&key) {
            notify.notify_waiters();
        }
    }

    /// Dispatch the next download from the priority queue if under concurrent limit
    /// Returns Ok(Some((entry_id, chunk_index))) if a download was dispatched, Ok(None) if not
    async fn dispatch_next_download(&self) -> Result<Option<(i32, u64)>> {
        let mut active = self.active_downloads.lock().await;

        // Check concurrent limit
        if active.len() >= self.max_concurrent_downloads {
            return Ok(None);
        }

        // Get highest priority request
        let request = {
            let mut queue = self.priority_queue.lock().await;
            queue.pop()
        };

        if let Some(req) = request {
            debug!(
                "Dispatching chunk {} for entry {} from callback",
                req.chunk_index, req.entry_id
            );

            // Spawn download task
            let entry_id = req.entry_id;
            let chunk_index = req.chunk_index;
            let downloader = self.downloader.clone();

            // Release the lock before awaiting
            drop(active);

            let handle = downloader.download_chunk(entry_id, chunk_index).await?;

            // Re-acquire lock to store handle
            let mut active = self.active_downloads.lock().await;
            active.insert((entry_id, chunk_index), handle);

            Ok(Some((entry_id, chunk_index)))
        } else {
            Ok(None)
        }
    }

    async fn handle_chunk_completion(&self, entry_id: i32, chunk_index: u64) {
        // Wait for download to complete
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Remove from active downloads and get the handle
        let handle = {
            let mut active = self.active_downloads.lock().await;
            active.remove(&(entry_id, chunk_index))
        };

        // Process the result (lock is now released)
        if let Some(handle) = handle {
            match handle.await {
                Ok(Ok(())) => {
                    info!(
                        "Chunk {} for entry {} downloaded successfully",
                        chunk_index, entry_id
                    );
                    // Notify waiters
                    self.notify_chunk_available(entry_id, chunk_index).await;
                }
                Ok(Err(e)) => {
                    warn!(
                        "Failed to download chunk {} for entry {}: {}",
                        chunk_index, entry_id, e
                    );
                }
                Err(e) => {
                    warn!(
                        "Download task for chunk {} (entry {}) was cancelled or panicked: {}",
                        chunk_index, entry_id, e
                    );
                }
            }
        }

        // Try to dispatch pending downloads from the queue until we hit the concurrency limit
        loop {
            match self.dispatch_next_download().await {
                Ok(Some(_)) => {
                    // Successfully dispatched a download, continue trying to dispatch more
                    continue;
                }
                Ok(None) => {
                    // Hit concurrency limit or queue is empty, stop dispatching
                    break;
                }
                Err(e) => {
                    warn!("Failed to dispatch next download: {}", e);
                    break;
                }
            }
        }
    }
}

impl ChunkManager {
    /// Create a new ChunkManager with all dependencies
    pub fn new(
        repository: Arc<dyn CacheRepository>,
        chunk_size_bytes: u64,
        downloader: Arc<ChunkDownloader>,
        chunk_store: Arc<ChunkStore>,
        max_concurrent_downloads: usize,
    ) -> Self {
        Self {
            repository,
            priority_queue: Arc::new(Mutex::new(BinaryHeap::new())),
            chunk_waiters: Arc::new(RwLock::new(HashMap::new())),
            chunk_size: chunk_size_bytes,
            downloader,
            chunk_store,
            active_downloads: Arc::new(Mutex::new(HashMap::new())),
            max_concurrent_downloads,
        }
    }

    /// Create a new ChunkManager with HTTP client and cache directory
    /// Convenience constructor that creates downloader and chunk_store
    pub fn with_client(
        repository: Arc<dyn CacheRepository>,
        chunk_size_bytes: u64,
        client: Client,
        cache_dir: std::path::PathBuf,
        max_concurrent_downloads: usize,
    ) -> Self {
        let chunk_store = Arc::new(ChunkStore::new(cache_dir));
        let downloader = Arc::new(ChunkDownloader::new(
            client,
            repository.clone(),
            chunk_store.clone(),
            chunk_size_bytes,
        ));

        Self::new(
            repository,
            chunk_size_bytes,
            downloader,
            chunk_store,
            max_concurrent_downloads,
        )
    }

    /// Get the configured chunk size in bytes
    pub fn chunk_size(&self) -> u64 {
        self.chunk_size
    }

    /// Calculate chunk index from byte offset
    pub fn byte_to_chunk_index(&self, byte_offset: u64) -> u64 {
        byte_offset / self.chunk_size
    }

    /// Calculate start byte of a chunk
    pub fn chunk_start_byte(&self, chunk_index: u64) -> u64 {
        chunk_index * self.chunk_size
    }

    /// Calculate end byte of a chunk (inclusive)
    pub fn chunk_end_byte(&self, chunk_index: u64, file_size: u64) -> u64 {
        let start = self.chunk_start_byte(chunk_index);
        let end = start + self.chunk_size - 1;
        std::cmp::min(end, file_size - 1)
    }

    /// Check if a specific chunk is available in the database
    pub async fn has_chunk(&self, entry_id: i32, chunk_index: u64) -> Result<bool> {
        let start_byte = self.chunk_start_byte(chunk_index) as i64;
        let end_byte = (start_byte + self.chunk_size as i64 - 1) as i64;

        // Get all chunks for this entry
        let chunks = self.repository.get_chunks_for_entry(entry_id).await?;

        // Check if any chunk covers this range
        for chunk in chunks {
            if chunk.start_byte <= start_byte && chunk.end_byte >= end_byte {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Check if an entire byte range is available (all chunks covering the range)
    /// This is more sophisticated than the repository's has_byte_range - it checks
    /// for contiguous coverage across multiple chunks
    pub async fn has_byte_range(&self, entry_id: i32, start: u64, end: u64) -> Result<bool> {
        // Get all chunks for this entry
        let chunks = self.repository.get_chunks_for_entry(entry_id).await?;

        if chunks.is_empty() {
            return Ok(false);
        }

        // Sort chunks by start_byte
        let mut sorted_chunks = chunks;
        sorted_chunks.sort_by_key(|c| c.start_byte);

        // Check for contiguous coverage
        let mut covered_up_to = start as i64;

        for chunk in sorted_chunks {
            // Skip chunks that end before our range starts
            if chunk.end_byte < start as i64 {
                continue;
            }

            // If there's a gap, range is not fully covered
            if chunk.start_byte > covered_up_to {
                return Ok(false);
            }

            // Extend coverage
            covered_up_to = std::cmp::max(covered_up_to, chunk.end_byte + 1);

            // If we've covered the entire requested range, we're done
            if covered_up_to > end as i64 {
                return Ok(true);
            }
        }

        // Check if we covered the entire range
        Ok(covered_up_to > end as i64)
    }

    /// Request a chunk with given priority (may trigger download)
    /// This adds the request to the priority queue and may dispatch download immediately
    pub async fn request_chunk(
        &self,
        entry_id: i32,
        chunk_index: u64,
        priority: Priority,
    ) -> Result<()> {
        // Check if chunk is already available
        if self.has_chunk(entry_id, chunk_index).await? {
            // Notify any waiters
            self.notify_chunk_available(entry_id, chunk_index).await;
            return Ok(());
        }

        // Check if already downloading
        {
            let active = self.active_downloads.lock().await;
            if active.contains_key(&(entry_id, chunk_index)) {
                return Ok(());
            }
        }

        // Add to priority queue
        let request = ChunkRequest::new(entry_id, chunk_index, priority);
        {
            let mut queue = self.priority_queue.lock().await;
            queue.push(request);
        }

        // Try to dispatch download if under concurrent limit
        self.dispatch_next_download().await?;

        Ok(())
    }

    /// Dispatch the next download from the priority queue if under concurrent limit
    async fn dispatch_next_download(&self) -> Result<()> {
        let mut active = self.active_downloads.lock().await;

        // Check concurrent limit
        if active.len() >= self.max_concurrent_downloads {
            return Ok(());
        }

        // Get highest priority request
        let request = {
            let mut queue = self.priority_queue.lock().await;
            queue.pop()
        };

        if let Some(req) = request {
            // Spawn download task
            let downloader = self.downloader.clone();
            let chunk_manager_clone = self.clone_for_callback();
            let entry_id = req.entry_id;
            let chunk_index = req.chunk_index;

            let handle = downloader.download_chunk(entry_id, chunk_index).await?;

            // Store the handle
            active.insert((entry_id, chunk_index), handle);

            // Spawn a task to clean up completed downloads and notify waiters
            tokio::spawn(async move {
                chunk_manager_clone
                    .handle_chunk_completion(entry_id, chunk_index)
                    .await;
            });
        }

        Ok(())
    }

    /// Clone parts of ChunkManager needed for callbacks
    fn clone_for_callback(&self) -> Arc<ChunkManagerCallback> {
        Arc::new(ChunkManagerCallback {
            active_downloads: self.active_downloads.clone(),
            chunk_waiters: self.chunk_waiters.clone(),
            priority_queue: self.priority_queue.clone(),
            downloader: self.downloader.clone(),
            max_concurrent_downloads: self.max_concurrent_downloads,
        })
    }

    /// Request multiple chunks for a byte range
    pub async fn request_chunks_for_range(
        &self,
        entry_id: i32,
        start: u64,
        end: u64,
        _file_size: u64,
        priority: Priority,
    ) -> Result<()> {
        let start_chunk = self.byte_to_chunk_index(start);
        let end_chunk = self.byte_to_chunk_index(end);

        for chunk_index in start_chunk..=end_chunk {
            self.request_chunk(entry_id, chunk_index, priority).await?;
        }

        Ok(())
    }

    /// Wait for a specific chunk to become available (event-based, not polling)
    pub async fn wait_for_chunk(
        &self,
        entry_id: i32,
        chunk_index: u64,
        timeout: Duration,
    ) -> Result<()> {
        // Check if already available
        if self.has_chunk(entry_id, chunk_index).await? {
            return Ok(());
        }

        // Get or create notifier for this chunk
        let notify = {
            let mut waiters = self.chunk_waiters.write().await;
            waiters
                .entry((entry_id, chunk_index))
                .or_insert_with(|| Arc::new(Notify::new()))
                .clone()
        };

        // Wait with timeout
        tokio::select! {
            _ = notify.notified() => {
                Ok(())
            }
            _ = tokio::time::sleep(timeout) => {
                warn!("Timeout waiting for chunk {} for entry {}", chunk_index, entry_id);
                Err(anyhow::anyhow!("Timeout waiting for chunk"))
            }
        }
    }

    /// Wait for an entire byte range to become available
    pub async fn wait_for_range(
        &self,
        entry_id: i32,
        start: u64,
        end: u64,
        timeout: Duration,
    ) -> Result<()> {
        let start_chunk = self.byte_to_chunk_index(start);
        let end_chunk = self.byte_to_chunk_index(end);

        let deadline = Instant::now() + timeout;

        for chunk_index in start_chunk..=end_chunk {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(anyhow::anyhow!("Timeout waiting for range"));
            }

            self.wait_for_chunk(entry_id, chunk_index, remaining)
                .await?;
        }

        Ok(())
    }

    /// Get list of available chunk indices for an entry
    pub async fn get_available_chunks(&self, entry_id: i32) -> Result<Vec<u64>> {
        let chunks = self.repository.get_chunks_for_entry(entry_id).await?;

        // Convert byte ranges to chunk indices
        let mut chunk_indices = Vec::new();
        for chunk in chunks {
            let start_chunk = self.byte_to_chunk_index(chunk.start_byte as u64);
            let end_chunk = self.byte_to_chunk_index(chunk.end_byte as u64);

            for chunk_index in start_chunk..=end_chunk {
                if !chunk_indices.contains(&chunk_index) {
                    chunk_indices.push(chunk_index);
                }
            }
        }

        chunk_indices.sort_unstable();
        Ok(chunk_indices)
    }

    /// Cancel pending chunk requests for given chunks
    pub async fn cancel_requests(&self, entry_id: i32, chunk_indices: &[u64]) -> Result<()> {
        let mut queue = self.priority_queue.lock().await;

        // Filter out cancelled requests
        let filtered: Vec<ChunkRequest> = queue
            .drain()
            .filter(|req| !(req.entry_id == entry_id && chunk_indices.contains(&req.chunk_index)))
            .collect();

        // Rebuild heap
        *queue = filtered.into_iter().collect();

        info!(
            "Cancelled {} chunk requests for entry {}",
            chunk_indices.len(),
            entry_id
        );

        Ok(())
    }

    /// Notify waiters that a chunk is available
    async fn notify_chunk_available(&self, entry_id: i32, chunk_index: u64) {
        let key = (entry_id, chunk_index);
        let waiters = self.chunk_waiters.read().await;

        if let Some(notify) = waiters.get(&key) {
            notify.notify_waiters();
        }
    }

    /// Get the current size of the priority queue (for monitoring/testing)
    pub async fn queue_size(&self) -> usize {
        self.priority_queue.lock().await.len()
    }

    /// Get a reference to the repository
    pub fn repository(&self) -> &Arc<dyn CacheRepository> {
        &self.repository
    }

    /// Get a reference to the HTTP client
    pub fn client(&self) -> &Client {
        self.downloader.client()
    }

    /// Invalidate and retry downloading a byte range (for corrupted chunks)
    /// This deletes the chunks covering the range from the database and re-requests them
    pub async fn retry_range(
        &self,
        entry_id: i32,
        start: u64,
        end: u64,
        total_size: u64,
        timeout: Duration,
    ) -> Result<()> {
        // Delete chunks that overlap with this range
        self.repository
            .delete_chunks_in_range(entry_id, start as i64, end as i64)
            .await?;

        // Re-request the chunks for this range with HIGH priority
        self.request_chunks_for_range(entry_id, start, end, total_size, Priority::HIGH)
            .await?;

        // Wait for the range to become available
        self.wait_for_range(entry_id, start, end, timeout).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::entities::{CacheChunkModel, CacheEntryModel};
    use async_trait::async_trait;
    use std::sync::Arc;
    use tokio::sync::Mutex as AsyncMutex;

    // Mock repository for testing
    struct MockCacheRepository {
        chunks: Arc<AsyncMutex<Vec<CacheChunkModel>>>,
    }

    impl MockCacheRepository {
        fn new() -> Self {
            Self {
                chunks: Arc::new(AsyncMutex::new(Vec::new())),
            }
        }

        async fn add_chunk(&self, chunk: CacheChunkModel) {
            let mut chunks = self.chunks.lock().await;
            chunks.push(chunk);
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
            Ok(entry)
        }

        async fn update_cache_entry(&self, entry: CacheEntryModel) -> Result<CacheEntryModel> {
            Ok(entry)
        }

        async fn update_expected_total_size(
            &self,
            _id: i32,
            _expected_total_size: i64,
        ) -> Result<()> {
            Ok(())
        }

        async fn delete_cache_entry(&self, _id: i32) -> Result<()> {
            Ok(())
        }

        async fn list_cache_entries(&self) -> Result<Vec<CacheEntryModel>> {
            Ok(Vec::new())
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
            self.add_chunk(chunk.clone()).await;
            Ok(chunk)
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

        async fn delete_chunks_in_range(
            &self,
            _cache_entry_id: i32,
            _start: i64,
            _end: i64,
        ) -> Result<()> {
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

    // Test chunk size (2MB for tests to keep test data manageable)
    const TEST_CHUNK_SIZE: u64 = 2 * 1024 * 1024;

    fn create_chunk(entry_id: i32, start_byte: i64, end_byte: i64) -> CacheChunkModel {
        use chrono::Utc;
        CacheChunkModel {
            id: 0, // Will be assigned by database
            cache_entry_id: entry_id,
            start_byte,
            end_byte,
            downloaded_at: Utc::now().naive_utc(),
        }
    }

    // Helper to create a test ChunkManager with mock dependencies
    fn create_test_manager(repo: Arc<MockCacheRepository>) -> ChunkManager {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let chunk_store = Arc::new(ChunkStore::new(temp_dir.path().to_path_buf()));
        let client = Client::new();
        let downloader = Arc::new(ChunkDownloader::new(
            client,
            repo.clone(),
            chunk_store.clone(),
            TEST_CHUNK_SIZE,
        ));

        ChunkManager::new(repo, TEST_CHUNK_SIZE, downloader, chunk_store, 3)
    }

    #[test]
    fn test_chunk_calculations() {
        let repo = Arc::new(MockCacheRepository::new());
        let manager = create_test_manager(repo);

        // Test byte to chunk index
        assert_eq!(manager.byte_to_chunk_index(0), 0);
        assert_eq!(manager.byte_to_chunk_index(TEST_CHUNK_SIZE - 1), 0);
        assert_eq!(manager.byte_to_chunk_index(TEST_CHUNK_SIZE), 1);
        assert_eq!(manager.byte_to_chunk_index(TEST_CHUNK_SIZE * 2), 2);

        // Test chunk start byte
        assert_eq!(manager.chunk_start_byte(0), 0);
        assert_eq!(manager.chunk_start_byte(1), TEST_CHUNK_SIZE);
        assert_eq!(manager.chunk_start_byte(2), TEST_CHUNK_SIZE * 2);

        // Test chunk end byte
        let file_size = TEST_CHUNK_SIZE * 10;
        assert_eq!(manager.chunk_end_byte(0, file_size), TEST_CHUNK_SIZE - 1);
        assert_eq!(
            manager.chunk_end_byte(1, file_size),
            TEST_CHUNK_SIZE * 2 - 1
        );

        // Last chunk should be limited by file size
        let small_file = TEST_CHUNK_SIZE + 1000;
        assert_eq!(manager.chunk_end_byte(1, small_file), small_file - 1);
    }

    #[test]
    fn test_priority_ordering() {
        let mut requests = Vec::new();

        // Add requests with different priorities
        requests.push(ChunkRequest::new(1, 0, Priority::LOW));
        requests.push(ChunkRequest::new(1, 1, Priority::CRITICAL));
        requests.push(ChunkRequest::new(1, 2, Priority::HIGH));
        requests.push(ChunkRequest::new(1, 3, Priority::MEDIUM));

        // Create heap
        let mut heap: BinaryHeap<ChunkRequest> = requests.into_iter().collect();

        // Pop in priority order
        assert_eq!(heap.pop().unwrap().priority, Priority::CRITICAL);
        assert_eq!(heap.pop().unwrap().priority, Priority::HIGH);
        assert_eq!(heap.pop().unwrap().priority, Priority::MEDIUM);
        assert_eq!(heap.pop().unwrap().priority, Priority::LOW);
    }

    #[tokio::test]
    async fn test_has_chunk_single_chunk() {
        let repo = Arc::new(MockCacheRepository::new());
        let manager = create_test_manager(repo.clone());

        // Add chunk 0 (bytes 0 - TEST_CHUNK_SIZE-1)
        repo.add_chunk(create_chunk(1, 0, (TEST_CHUNK_SIZE - 1) as i64))
            .await;

        // Chunk 0 should be available
        assert!(manager.has_chunk(1, 0).await.unwrap());

        // Chunk 1 should not be available
        assert!(!manager.has_chunk(1, 1).await.unwrap());
    }

    #[tokio::test]
    async fn test_has_byte_range_single_chunk() {
        let repo = Arc::new(MockCacheRepository::new());
        let manager = create_test_manager(repo.clone());

        // Add chunk 0 (bytes 0 - TEST_CHUNK_SIZE-1)
        repo.add_chunk(create_chunk(1, 0, (TEST_CHUNK_SIZE - 1) as i64))
            .await;

        // Range within chunk 0 should be available
        assert!(manager.has_byte_range(1, 0, 1000).await.unwrap());
        assert!(
            manager
                .has_byte_range(1, 1000, TEST_CHUNK_SIZE - 1)
                .await
                .unwrap()
        );

        // Range spanning into chunk 1 should not be available
        assert!(
            !manager
                .has_byte_range(1, 0, TEST_CHUNK_SIZE + 1000)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn test_has_byte_range_multiple_contiguous_chunks() {
        let repo = Arc::new(MockCacheRepository::new());
        let manager = create_test_manager(repo.clone());

        // Add chunks 0, 1, 2 (contiguous)
        repo.add_chunk(create_chunk(1, 0, (TEST_CHUNK_SIZE - 1) as i64))
            .await;
        repo.add_chunk(create_chunk(
            1,
            TEST_CHUNK_SIZE as i64,
            (TEST_CHUNK_SIZE * 2 - 1) as i64,
        ))
        .await;
        repo.add_chunk(create_chunk(
            1,
            (TEST_CHUNK_SIZE * 2) as i64,
            (TEST_CHUNK_SIZE * 3 - 1) as i64,
        ))
        .await;

        // Range spanning all three chunks should be available
        assert!(
            manager
                .has_byte_range(1, 0, TEST_CHUNK_SIZE * 3 - 1)
                .await
                .unwrap()
        );

        // Range spanning chunks 0-1 should be available
        assert!(
            manager
                .has_byte_range(1, 0, TEST_CHUNK_SIZE * 2 - 1)
                .await
                .unwrap()
        );

        // Range spanning into non-existent chunk 3 should not be available
        assert!(
            !manager
                .has_byte_range(1, 0, TEST_CHUNK_SIZE * 3 + 1000)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn test_has_byte_range_gap_in_chunks() {
        let repo = Arc::new(MockCacheRepository::new());
        let manager = create_test_manager(repo.clone());

        // Add chunks 0 and 2 (gap at chunk 1)
        repo.add_chunk(create_chunk(1, 0, (TEST_CHUNK_SIZE - 1) as i64))
            .await;
        repo.add_chunk(create_chunk(
            1,
            (TEST_CHUNK_SIZE * 2) as i64,
            (TEST_CHUNK_SIZE * 3 - 1) as i64,
        ))
        .await;

        // Range spanning chunk 0 should be available
        assert!(
            manager
                .has_byte_range(1, 0, TEST_CHUNK_SIZE - 1)
                .await
                .unwrap()
        );

        // Range spanning chunk 2 should be available
        assert!(
            manager
                .has_byte_range(1, TEST_CHUNK_SIZE * 2, TEST_CHUNK_SIZE * 3 - 1)
                .await
                .unwrap()
        );

        // Range spanning chunks 0-1 should NOT be available (gap at chunk 1)
        assert!(
            !manager
                .has_byte_range(1, 0, TEST_CHUNK_SIZE * 2 - 1)
                .await
                .unwrap()
        );

        // Range spanning all three chunks should NOT be available (gap at chunk 1)
        assert!(
            !manager
                .has_byte_range(1, 0, TEST_CHUNK_SIZE * 3 - 1)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn test_request_chunk_with_priority() {
        let repo = Arc::new(MockCacheRepository::new());
        let manager = create_test_manager(repo.clone());

        // Request more chunks than the concurrent download limit (3)
        // so some remain in the queue
        manager.request_chunk(1, 0, Priority::LOW).await.unwrap();
        manager
            .request_chunk(1, 1, Priority::CRITICAL)
            .await
            .unwrap();
        manager.request_chunk(1, 2, Priority::HIGH).await.unwrap();
        manager.request_chunk(1, 3, Priority::MEDIUM).await.unwrap();
        manager.request_chunk(1, 4, Priority::LOW).await.unwrap();

        // Check queue size - should have 2 in queue (3 are being downloaded concurrently)
        assert_eq!(manager.queue_size().await, 2);
    }

    #[tokio::test]
    async fn test_request_chunk_already_available() {
        let repo = Arc::new(MockCacheRepository::new());
        let manager = create_test_manager(repo.clone());

        // Add chunk 0
        repo.add_chunk(create_chunk(1, 0, (TEST_CHUNK_SIZE - 1) as i64))
            .await;

        // Request chunk 0 (should not be added to queue since it's available)
        manager.request_chunk(1, 0, Priority::HIGH).await.unwrap();

        // Queue should be empty
        assert_eq!(manager.queue_size().await, 0);
    }

    #[tokio::test]
    async fn test_request_chunks_for_range() {
        let repo = Arc::new(MockCacheRepository::new());
        let manager = create_test_manager(repo.clone());

        let file_size = TEST_CHUNK_SIZE * 10;

        // Request chunks for range spanning chunks 0-4 (5 chunks total)
        // Max concurrent is 3, so 2 should remain in queue
        manager
            .request_chunks_for_range(1, 0, TEST_CHUNK_SIZE * 5 - 1, file_size, Priority::HIGH)
            .await
            .unwrap();

        // Should have 2 chunks in queue (3 are being downloaded concurrently)
        assert_eq!(manager.queue_size().await, 2);
    }

    #[tokio::test]
    async fn test_cancel_requests() {
        let repo = Arc::new(MockCacheRepository::new());
        let manager = create_test_manager(repo.clone());

        // Request chunks 0-5 (6 chunks total)
        // Max concurrent is 3, so 3 should remain in queue
        manager.request_chunk(1, 0, Priority::HIGH).await.unwrap();
        manager.request_chunk(1, 1, Priority::HIGH).await.unwrap();
        manager.request_chunk(1, 2, Priority::HIGH).await.unwrap();
        manager.request_chunk(1, 3, Priority::HIGH).await.unwrap();
        manager.request_chunk(1, 4, Priority::HIGH).await.unwrap();
        manager.request_chunk(1, 5, Priority::HIGH).await.unwrap();

        assert_eq!(manager.queue_size().await, 3);

        // Cancel chunks 3 and 5 (which are in the queue, not being downloaded)
        manager.cancel_requests(1, &[3, 5]).await.unwrap();

        // Should have 1 chunk left in queue (chunk 4)
        assert_eq!(manager.queue_size().await, 1);
    }

    #[tokio::test]
    async fn test_get_available_chunks() {
        let repo = Arc::new(MockCacheRepository::new());
        let manager = create_test_manager(repo.clone());

        // Add chunks 0, 2, 4 (gaps at 1, 3)
        repo.add_chunk(create_chunk(1, 0, (TEST_CHUNK_SIZE - 1) as i64))
            .await;
        repo.add_chunk(create_chunk(
            1,
            (TEST_CHUNK_SIZE * 2) as i64,
            (TEST_CHUNK_SIZE * 3 - 1) as i64,
        ))
        .await;
        repo.add_chunk(create_chunk(
            1,
            (TEST_CHUNK_SIZE * 4) as i64,
            (TEST_CHUNK_SIZE * 5 - 1) as i64,
        ))
        .await;

        let available = manager.get_available_chunks(1).await.unwrap();

        assert_eq!(available.len(), 3);
        assert!(available.contains(&0));
        assert!(available.contains(&2));
        assert!(available.contains(&4));
        assert!(!available.contains(&1));
        assert!(!available.contains(&3));
    }

    #[tokio::test]
    async fn test_wait_for_chunk_timeout() {
        let repo = Arc::new(MockCacheRepository::new());
        let manager = create_test_manager(repo.clone());

        // Wait for chunk that never arrives
        let result = manager
            .wait_for_chunk(1, 0, Duration::from_millis(100))
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_wait_for_chunk_already_available() {
        let repo = Arc::new(MockCacheRepository::new());
        let manager = create_test_manager(repo.clone());

        // Add chunk 0
        repo.add_chunk(create_chunk(1, 0, (TEST_CHUNK_SIZE - 1) as i64))
            .await;

        // Wait for chunk 0 (should return immediately)
        let result = manager.wait_for_chunk(1, 0, Duration::from_secs(1)).await;

        assert!(result.is_ok());
    }
}
