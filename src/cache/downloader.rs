use anyhow::{Context, Result};
use futures::StreamExt;
use reqwest::Client;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, mpsc};
use tokio::time::{interval, timeout};
use tracing::{debug, error, info, warn};

use super::config::FileCacheConfig;
use super::metadata::MediaCacheKey;
use super::state_machine::{CacheStateMachine, DownloadState};
use super::stats::DownloaderStats;
use super::storage::CacheStorage;

/// Progress information for a download
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    /// Cache key being downloaded
    _cache_key: MediaCacheKey,
    /// Current download state
    pub state: DownloadState,
    /// Total file size in bytes (if known)
    pub total_size: Option<u64>,
    /// Bytes downloaded so far
    pub downloaded_bytes: u64,
    /// Download speed in bytes per second
    pub speed_bps: u64,
    /// Estimated time remaining
    pub eta_seconds: Option<u64>,
    /// Error message if failed
    pub error: Option<String>,
}

impl DownloadProgress {
    pub fn new(cache_key: MediaCacheKey) -> Self {
        Self {
            _cache_key: cache_key,
            state: DownloadState::NotStarted,
            total_size: None,
            downloaded_bytes: 0,
            speed_bps: 0,
            eta_seconds: None,
            error: None,
        }
    }

    /// Calculate download progress as percentage (0.0 to 1.0)
    pub fn progress_percent(&self) -> f64 {
        if let Some(total) = self.total_size
            && total > 0
        {
            return self.downloaded_bytes as f64 / total as f64;
        }
        0.0
    }

    /// Update progress with new downloaded bytes
    pub fn update(&mut self, new_bytes: u64, speed: u64) {
        self.downloaded_bytes = new_bytes;
        self.speed_bps = speed;

        if let Some(total) = self.total_size {
            if total > new_bytes && speed > 0 {
                let remaining_bytes = total - new_bytes;
                self.eta_seconds = Some(remaining_bytes / speed);
            } else {
                self.eta_seconds = None;
            }
        }
    }
}

/// Commands for the progressive downloader
#[derive(Debug)]
pub enum DownloadCommand {
    /// Start downloading a media file
    StartDownload {
        cache_key: MediaCacheKey,
        url: String,
        priority: DownloadPriority,
        respond_to: mpsc::UnboundedSender<Result<()>>,
    },
    /// Cancel a download
    CancelDownload {
        cache_key: MediaCacheKey,
        respond_to: mpsc::UnboundedSender<Result<()>>,
    },
    /// Get download progress
    GetProgress {
        cache_key: MediaCacheKey,
        respond_to: mpsc::UnboundedSender<Option<DownloadProgress>>,
    },
    /// Shutdown the downloader
    Shutdown,
}

/// Download priority levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum DownloadPriority {
    Urgent = 3, // For playback-required downloads
}

/// Progressive downloader for media files
pub struct ProgressiveDownloader {
    config: FileCacheConfig,
    storage: Arc<RwLock<CacheStorage>>,
    http_client: Client,
    active_downloads: Arc<RwLock<std::collections::HashMap<MediaCacheKey, DownloadProgress>>>,
    state_machine: Arc<CacheStateMachine>,
    command_receiver: mpsc::UnboundedReceiver<DownloadCommand>,
    download_semaphore: Arc<tokio::sync::Semaphore>,
    stats: DownloaderStats,
}

impl ProgressiveDownloader {
    /// Create a new progressive downloader
    pub fn new(
        config: FileCacheConfig,
        storage: Arc<RwLock<CacheStorage>>,
        state_machine: Arc<CacheStateMachine>,
        command_receiver: mpsc::UnboundedReceiver<DownloadCommand>,
    ) -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(config.download_timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        let download_semaphore = Arc::new(tokio::sync::Semaphore::new(
            config.max_concurrent_downloads as usize,
        ));

        Self {
            config,
            storage,
            http_client,
            active_downloads: Arc::new(RwLock::new(std::collections::HashMap::new())),
            state_machine,
            command_receiver,
            download_semaphore,
            stats: DownloaderStats::new(),
        }
    }

    /// Run the downloader event loop
    pub async fn run(mut self) {
        info!("🔄 ProgressiveDownloader: Starting event loop");

        // Start periodic stats reporting if enabled
        let stats_handle = if self.config.enable_stats {
            Some(self.start_stats_reporting())
        } else {
            None
        };

        while let Some(command) = self.command_receiver.recv().await {
            match command {
                DownloadCommand::StartDownload {
                    cache_key,
                    url,
                    priority,
                    respond_to,
                } => {
                    let result = self.start_download(cache_key, url, priority).await;
                    let _ = respond_to.send(result);
                }
                DownloadCommand::CancelDownload {
                    cache_key,
                    respond_to,
                } => {
                    let result = self.cancel_download(&cache_key).await;
                    let _ = respond_to.send(result);
                }
                DownloadCommand::GetProgress {
                    cache_key,
                    respond_to,
                } => {
                    let progress = self.get_progress(&cache_key).await;
                    let _ = respond_to.send(progress);
                }
                DownloadCommand::Shutdown => {
                    info!("🔄 ProgressiveDownloader: Shutting down");
                    if let Some(handle) = stats_handle {
                        handle.abort();
                    }
                    break;
                }
            }
        }
    }

    /// Start periodic stats reporting
    fn start_stats_reporting(&self) -> tokio::task::JoinHandle<()> {
        let stats = self.stats.clone();
        let active_downloads = self.active_downloads.clone();
        let interval_secs = self.config.stats_interval_secs;
        let state_machine = self.state_machine.clone();

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(interval_secs));
            ticker.tick().await; // Skip first immediate tick

            loop {
                ticker.tick().await;

                // Get active download details
                let downloads = active_downloads.read().await;
                let mut active_details = Vec::new();
                let mut active_count = 0u64;
                let mut queued_count = 0u64;

                for (key, progress) in downloads.iter() {
                    // Get state from state machine
                    if let Some(state_info) = state_machine.get_state(&key).await {
                        match &state_info.state {
                            DownloadState::Downloading | DownloadState::Initializing => {
                                active_count += 1;
                                let name =
                                    format!("{}:{}", key.source_id.as_str(), key.media_id.as_str());
                                let progress_pct = progress.progress_percent();
                                active_details.push((name, progress.speed_bps, progress_pct));
                            }
                            DownloadState::NotStarted => {
                                queued_count += 1;
                            }
                            _ => {}
                        }
                    }
                }

                stats.set_active_downloads(active_count);
                stats.set_queued_downloads(queued_count);

                let report = stats.format_report(active_details);
                info!("{}", report);
            }
        })
    }

    /// Start downloading a media file
    async fn start_download(
        &mut self,
        cache_key: MediaCacheKey,
        url: String,
        _priority: DownloadPriority,
    ) -> Result<()> {
        debug!("Starting download for cache key: {:?}", cache_key);

        // Check if already downloading via state machine
        if let Some(state_info) = self.state_machine.get_state(&cache_key).await {
            match state_info.state {
                DownloadState::Downloading | DownloadState::Initializing => {
                    return Err(anyhow::anyhow!("Download already in progress"));
                }
                DownloadState::Complete => {
                    return Err(anyhow::anyhow!("Download already completed"));
                }
                _ => {} // Can restart failed or paused downloads
            }
        }

        // Initialize progress tracking
        let progress = DownloadProgress::new(cache_key.clone());
        {
            let mut downloads = self.active_downloads.write().await;
            downloads.insert(cache_key.clone(), progress);
        }

        // Transition state to initializing
        self.state_machine
            .transition(
                &cache_key,
                DownloadState::Initializing,
                Some("Starting download".to_string()),
            )
            .await?;

        // Increment stats
        self.stats.increment_started();

        // Spawn download task
        let storage = self.storage.clone();
        let client = self.http_client.clone();
        let config = self.config.clone();
        let active_downloads = self.active_downloads.clone();
        let semaphore = self.download_semaphore.clone();
        let stats = self.stats.clone();
        let state_machine = self.state_machine.clone();

        tokio::spawn(async move {
            let _permit = semaphore.acquire().await.expect("Semaphore closed");

            let result = Self::download_file(
                cache_key.clone(),
                url,
                storage,
                client,
                config,
                active_downloads.clone(),
                state_machine.clone(),
                stats.clone(),
            )
            .await;

            // Update final state in state machine
            match result {
                Ok(_) => {
                    state_machine
                        .transition(
                            &cache_key,
                            DownloadState::Complete,
                            Some("Download completed successfully".to_string()),
                        )
                        .await
                        .ok();
                    stats.increment_completed();
                    info!("✅ Download completed for cache key: {:?}", cache_key);
                }
                Err(e) => {
                    state_machine
                        .transition(
                            &cache_key,
                            DownloadState::Failed(e.to_string()),
                            Some("Download failed".to_string()),
                        )
                        .await
                        .ok();

                    let mut downloads = active_downloads.write().await;
                    if let Some(progress) = downloads.get_mut(&cache_key) {
                        progress.error = Some(e.to_string());
                    }

                    stats.increment_failed();
                    error!("❌ Download failed for cache key: {:?} - {}", cache_key, e);
                }
            }
        });

        Ok(())
    }

    /// Download a file with progress tracking
    async fn download_file(
        cache_key: MediaCacheKey,
        url: String,
        storage: Arc<RwLock<CacheStorage>>,
        client: Client,
        config: FileCacheConfig,
        active_downloads: Arc<RwLock<std::collections::HashMap<MediaCacheKey, DownloadProgress>>>,
        state_machine: Arc<CacheStateMachine>,
        stats: DownloaderStats,
    ) -> Result<()> {
        info!(
            "Starting download_file for key: {:?}, URL: {}",
            cache_key, url
        );

        // Create cache entry
        let entry = {
            debug!("Acquiring storage write lock");
            let mut storage_guard = storage.write().await;
            debug!("Storage write lock acquired, creating cache entry");

            let result = storage_guard
                .create_entry(cache_key.clone(), url.clone())
                .await;

            match result {
                Ok(entry) => {
                    debug!("Cache entry created successfully for key: {:?}", cache_key);
                    entry
                }
                Err(e) => {
                    error!(
                        "Failed to create cache entry for key: {:?}, error: {}",
                        cache_key, e
                    );
                    return Err(e).context("Failed to create cache entry");
                }
            }
        };
        debug!("Cache entry created, metadata: {:?}", entry.metadata);

        // Check if we can resume an existing download
        let start_byte = entry.metadata.downloaded_bytes;
        info!("Starting download from byte: {}", start_byte);

        // Make HTTP request with range header if resuming
        let mut request = client.get(&url);
        if start_byte > 0 {
            request = request.header("Range", format!("bytes={}-", start_byte));
            info!("Resuming download with Range header: bytes={}-", start_byte);
        }

        info!("Sending HTTP request to: {}", url);
        let response_result = timeout(
            Duration::from_secs(config.download_timeout_secs),
            request.send(),
        )
        .await;

        let response = match response_result {
            Ok(Ok(resp)) => {
                info!("HTTP request successful, status: {}", resp.status());
                resp
            }
            Ok(Err(e)) => {
                error!("Failed to send HTTP request: {}", e);
                return Err(anyhow::anyhow!("Failed to send HTTP request: {}", e));
            }
            Err(_) => {
                error!(
                    "HTTP request timeout after {} seconds",
                    config.download_timeout_secs
                );
                return Err(anyhow::anyhow!("Request timeout"));
            }
        };

        if !response.status().is_success() && response.status().as_u16() != 206 {
            error!("HTTP error response: {}", response.status());
            return Err(anyhow::anyhow!("HTTP error: {}", response.status()));
        }

        // Get content length
        let content_length = response.content_length();
        info!("Content length from response: {:?}", content_length);
        let total_size = if start_byte > 0 {
            content_length.map(|len| len + start_byte)
        } else {
            content_length
        };

        // Update metadata with expected total file size
        if let Some(expected_size) = total_size {
            let mut storage_guard = storage.write().await;
            storage_guard.set_expected_file_size(&cache_key, expected_size);
            info!(
                "Set expected file size in metadata: {} bytes",
                expected_size
            );
        }

        // Update progress with total size
        {
            let mut downloads = active_downloads.write().await;
            if let Some(progress) = downloads.get_mut(&cache_key) {
                progress.total_size = total_size;
                progress.downloaded_bytes = start_byte;
            }
        }

        // Transition to downloading state
        state_machine
            .transition(
                &cache_key,
                DownloadState::Downloading,
                Some("Starting data transfer".to_string()),
            )
            .await?;

        // Update state machine progress
        state_machine
            .update_progress(&cache_key, start_byte, total_size)
            .await?;

        // Stream the response and write to cache
        info!(
            "Starting to stream response, chunk size: {} KB",
            config.chunk_size_kb
        );
        let mut stream = response.bytes_stream();
        let mut offset = start_byte;
        let chunk_size = config.chunk_size_kb as usize * 1024;
        let mut buffer = Vec::with_capacity(chunk_size);
        let mut last_progress_update = Instant::now();
        let mut bytes_since_last_update = 0u64;
        let mut chunks_received = 0u64;
        let mut first_chunk_written = false;
        let download_start_time = Instant::now();

        info!("Beginning download stream loop...");
        while let Some(chunk_result) = stream.next().await {
            chunks_received += 1;
            // Check if download was cancelled or paused via state machine
            if let Some(state_info) = state_machine.get_state(&cache_key).await {
                match &state_info.state {
                    DownloadState::Failed(msg) => {
                        return Err(anyhow::anyhow!("Download failed: {}", msg));
                    }
                    DownloadState::Paused => {
                        // Wait for resume signal
                        loop {
                            tokio::time::sleep(Duration::from_millis(100)).await;
                            if let Some(info) = state_machine.get_state(&cache_key).await {
                                if info.state != DownloadState::Paused {
                                    break;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }

            let chunk = chunk_result.context("Failed to read chunk from response")?;
            buffer.extend_from_slice(&chunk);

            // Write buffer to cache when it reaches chunk size
            if buffer.len() >= chunk_size {
                let data_to_write = buffer.clone();
                buffer.clear();

                {
                    let mut storage_guard = storage.write().await;
                    storage_guard
                        .write_to_entry(&cache_key, offset, &data_to_write)
                        .await
                        .context("Failed to write to cache")?;
                }

                if !first_chunk_written {
                    let time_to_first_chunk = download_start_time.elapsed();
                    info!(
                        "First chunk written for {:?}: {} bytes after {:?} (including HTTP request time)",
                        cache_key,
                        data_to_write.len(),
                        time_to_first_chunk
                    );
                    first_chunk_written = true;
                }

                offset += data_to_write.len() as u64;
                bytes_since_last_update += data_to_write.len() as u64;

                // Update progress periodically
                let now = Instant::now();
                if now.duration_since(last_progress_update) >= Duration::from_millis(500) {
                    let elapsed_secs = now.duration_since(last_progress_update).as_secs_f64();
                    let speed = if elapsed_secs > 0.0 {
                        (bytes_since_last_update as f64 / elapsed_secs) as u64
                    } else {
                        0
                    };

                    {
                        let mut downloads = active_downloads.write().await;
                        if let Some(progress) = downloads.get_mut(&cache_key) {
                            progress.update(offset, speed);
                        }

                        // Update state machine progress
                        state_machine
                            .update_progress(&cache_key, offset, total_size)
                            .await
                            .ok();
                    }

                    last_progress_update = now;
                    bytes_since_last_update = 0;
                }
            }
        }

        // Write any remaining data in buffer
        if !buffer.is_empty() {
            info!(
                "Writing final {} bytes to cache at offset {}",
                buffer.len(),
                offset
            );
            let mut storage_guard = storage.write().await;
            storage_guard
                .write_to_entry(&cache_key, offset, &buffer)
                .await
                .context("Failed to write final chunk to cache")?;

            if !first_chunk_written {
                let time_to_first_chunk = download_start_time.elapsed();
                info!(
                    "First chunk (final buffer) written for {:?}: {} bytes after {:?}",
                    cache_key,
                    buffer.len(),
                    time_to_first_chunk
                );
            }

            offset += buffer.len() as u64;
            info!(
                "Final chunk written successfully, total downloaded: {} bytes",
                offset
            );
        }

        // Mark the download as complete in metadata
        {
            let mut storage_guard = storage.write().await;
            storage_guard.mark_download_complete(&cache_key, offset);
        }

        // Update final progress in state machine
        state_machine
            .update_progress(&cache_key, offset, Some(offset))
            .await?;

        // Update stats with total bytes downloaded
        stats.add_bytes_downloaded(offset);

        info!(
            "Download completed successfully for cache key: {:?}, total chunks: {}, total bytes: {}",
            cache_key, chunks_received, offset
        );
        Ok(())
    }

    /// Cancel a download
    async fn cancel_download(&self, cache_key: &MediaCacheKey) -> Result<()> {
        // Check current state
        if let Some(state_info) = self.state_machine.get_state(cache_key).await {
            match state_info.state {
                DownloadState::Downloading
                | DownloadState::Paused
                | DownloadState::Initializing => {
                    self.state_machine
                        .transition(
                            cache_key,
                            DownloadState::Failed("Cancelled by user".to_string()),
                            Some("User requested cancellation".to_string()),
                        )
                        .await?;

                    // Remove from active downloads
                    let mut downloads = self.active_downloads.write().await;
                    downloads.remove(cache_key);

                    debug!("Cancelled download for cache key: {:?}", cache_key);
                    Ok(())
                }
                _ => Err(anyhow::anyhow!(
                    "Download cannot be cancelled in current state"
                )),
            }
        } else {
            Err(anyhow::anyhow!("Download not found"))
        }
    }

    /// Get download progress
    async fn get_progress(&self, cache_key: &MediaCacheKey) -> Option<DownloadProgress> {
        let downloads = self.active_downloads.read().await;
        downloads.get(cache_key).cloned()
    }
}

/// Handle for communicating with the progressive downloader
#[derive(Debug, Clone)]
pub struct ProgressiveDownloaderHandle {
    command_sender: mpsc::UnboundedSender<DownloadCommand>,
}

impl ProgressiveDownloaderHandle {
    /// Create a new downloader handle
    pub fn new(command_sender: mpsc::UnboundedSender<DownloadCommand>) -> Self {
        Self { command_sender }
    }

    /// Start downloading a media file
    pub async fn start_download(
        &self,
        cache_key: MediaCacheKey,
        url: String,
        priority: DownloadPriority,
    ) -> Result<()> {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        self.command_sender
            .send(DownloadCommand::StartDownload {
                cache_key,
                url,
                priority,
                respond_to: sender,
            })
            .map_err(|_| anyhow::anyhow!("Downloader disconnected"))?;

        receiver
            .recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("No response from downloader"))?
    }

    /// Cancel a download
    pub async fn cancel_download(&self, cache_key: MediaCacheKey) -> Result<()> {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        self.command_sender
            .send(DownloadCommand::CancelDownload {
                cache_key,
                respond_to: sender,
            })
            .map_err(|_| anyhow::anyhow!("Downloader disconnected"))?;

        receiver
            .recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("No response from downloader"))?
    }

    /// Get download progress
    pub async fn get_progress(&self, cache_key: MediaCacheKey) -> Result<Option<DownloadProgress>> {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        self.command_sender
            .send(DownloadCommand::GetProgress {
                cache_key,
                respond_to: sender,
            })
            .map_err(|_| anyhow::anyhow!("Downloader disconnected"))?;

        receiver
            .recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("No response from downloader"))
    }

    /// Shutdown the downloader
    pub fn shutdown(&self) -> Result<()> {
        self.command_sender
            .send(DownloadCommand::Shutdown)
            .map_err(|_| anyhow::anyhow!("Downloader disconnected"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{MediaItemId, SourceId};

    #[test]
    fn test_download_progress() {
        let key = MediaCacheKey::new(SourceId::from("test"), MediaItemId::from("test"), "1080p");

        let mut progress = DownloadProgress::new(key);
        assert_eq!(progress.state, DownloadState::Queued);
        assert_eq!(progress.progress_percent(), 0.0);

        progress.total_size = Some(1000);
        progress.update(500, 100);
        assert_eq!(progress.progress_percent(), 0.5);
        assert_eq!(progress.speed_bps, 100);
    }
}
