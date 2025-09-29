use anyhow::{Context, Result};
use futures::StreamExt;
use reqwest::{Client, Response};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, mpsc};
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

use super::config::FileCacheConfig;
use super::metadata::MediaCacheKey;
use super::storage::CacheStorage;

/// Download state for a media file
#[derive(Debug, Clone, PartialEq)]
pub enum DownloadState {
    /// Download is queued but not started
    Queued,
    /// Download is initializing (fetching headers, etc.)
    Initializing,
    /// Download is actively downloading
    Downloading,
    /// Download is paused
    Paused,
    /// Download completed successfully
    Completed,
    /// Download failed with error
    Failed(String),
    /// Download was cancelled
    Cancelled,
}

/// Progress information for a download
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    /// Cache key being downloaded
    pub cache_key: MediaCacheKey,
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
            cache_key,
            state: DownloadState::Queued,
            total_size: None,
            downloaded_bytes: 0,
            speed_bps: 0,
            eta_seconds: None,
            error: None,
        }
    }

    /// Calculate download progress as percentage (0.0 to 1.0)
    pub fn progress_percent(&self) -> f64 {
        if let Some(total) = self.total_size {
            if total > 0 {
                return self.downloaded_bytes as f64 / total as f64;
            }
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
    /// Pause a download
    PauseDownload {
        cache_key: MediaCacheKey,
        respond_to: mpsc::UnboundedSender<Result<()>>,
    },
    /// Resume a paused download
    ResumeDownload {
        cache_key: MediaCacheKey,
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
    /// List all active downloads
    ListDownloads {
        respond_to: mpsc::UnboundedSender<Vec<DownloadProgress>>,
    },
    /// Shutdown the downloader
    Shutdown,
}

/// Download priority levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum DownloadPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Urgent = 3, // For playback-required downloads
}

/// Progressive downloader for media files
pub struct ProgressiveDownloader {
    config: FileCacheConfig,
    storage: Arc<RwLock<CacheStorage>>,
    http_client: Client,
    active_downloads: Arc<RwLock<std::collections::HashMap<MediaCacheKey, DownloadProgress>>>,
    command_receiver: mpsc::UnboundedReceiver<DownloadCommand>,
    download_semaphore: Arc<tokio::sync::Semaphore>,
}

impl ProgressiveDownloader {
    /// Create a new progressive downloader
    pub fn new(
        config: FileCacheConfig,
        storage: Arc<RwLock<CacheStorage>>,
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
            command_receiver,
            download_semaphore,
        }
    }

    /// Run the downloader event loop
    pub async fn run(mut self) {
        info!("🔄 ProgressiveDownloader: Starting event loop");

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
                DownloadCommand::PauseDownload {
                    cache_key,
                    respond_to,
                } => {
                    let result = self.pause_download(&cache_key).await;
                    let _ = respond_to.send(result);
                }
                DownloadCommand::ResumeDownload {
                    cache_key,
                    respond_to,
                } => {
                    let result = self.resume_download(&cache_key).await;
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
                DownloadCommand::ListDownloads { respond_to } => {
                    let downloads = self.list_downloads().await;
                    let _ = respond_to.send(downloads);
                }
                DownloadCommand::Shutdown => {
                    info!("🔄 ProgressiveDownloader: Shutting down");
                    break;
                }
            }
        }
    }

    /// Start downloading a media file
    async fn start_download(
        &mut self,
        cache_key: MediaCacheKey,
        url: String,
        priority: DownloadPriority,
    ) -> Result<()> {
        debug!("Starting download for cache key: {:?}", cache_key);

        // Check if already downloading
        {
            let downloads = self.active_downloads.read().await;
            if let Some(progress) = downloads.get(&cache_key) {
                match progress.state {
                    DownloadState::Downloading | DownloadState::Initializing => {
                        return Err(anyhow::anyhow!("Download already in progress"));
                    }
                    DownloadState::Completed => {
                        return Err(anyhow::anyhow!("Download already completed"));
                    }
                    _ => {} // Can restart failed, cancelled, or paused downloads
                }
            }
        }

        // Initialize progress tracking
        let mut progress = DownloadProgress::new(cache_key.clone());
        progress.state = DownloadState::Initializing;

        {
            let mut downloads = self.active_downloads.write().await;
            downloads.insert(cache_key.clone(), progress);
        }

        // Spawn download task
        let storage = self.storage.clone();
        let client = self.http_client.clone();
        let config = self.config.clone();
        let active_downloads = self.active_downloads.clone();
        let semaphore = self.download_semaphore.clone();

        tokio::spawn(async move {
            let _permit = semaphore.acquire().await.expect("Semaphore closed");

            let result = Self::download_file(
                cache_key.clone(),
                url,
                storage,
                client,
                config,
                active_downloads.clone(),
            )
            .await;

            // Update final state
            let mut downloads = active_downloads.write().await;
            if let Some(progress) = downloads.get_mut(&cache_key) {
                match result {
                    Ok(_) => {
                        progress.state = DownloadState::Completed;
                        info!("✅ Download completed for cache key: {:?}", cache_key);
                    }
                    Err(e) => {
                        progress.state = DownloadState::Failed(e.to_string());
                        progress.error = Some(e.to_string());
                        error!("❌ Download failed for cache key: {:?} - {}", cache_key, e);
                    }
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

        // Update progress with total size
        {
            let mut downloads = active_downloads.write().await;
            if let Some(progress) = downloads.get_mut(&cache_key) {
                progress.total_size = total_size;
                progress.state = DownloadState::Downloading;
                progress.downloaded_bytes = start_byte;
            }
        }

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

        info!("Beginning download stream loop...");
        while let Some(chunk_result) = stream.next().await {
            chunks_received += 1;
            // Check if download was cancelled or paused
            {
                let downloads = active_downloads.read().await;
                if let Some(progress) = downloads.get(&cache_key) {
                    match progress.state {
                        DownloadState::Cancelled => {
                            return Err(anyhow::anyhow!("Download cancelled"));
                        }
                        DownloadState::Paused => {
                            // Wait for resume signal
                            while downloads.get(&cache_key).map(|p| &p.state)
                                == Some(&DownloadState::Paused)
                            {
                                tokio::time::sleep(Duration::from_millis(100)).await;
                            }
                        }
                        _ => {}
                    }
                }
            }

            let chunk = chunk_result.context("Failed to read chunk from response")?;
            let chunk_len = chunk.len();
            debug!(
                "Received chunk #{} with {} bytes",
                chunks_received, chunk_len
            );
            buffer.extend_from_slice(&chunk);

            // Write buffer to cache when it reaches chunk size
            if buffer.len() >= chunk_size {
                let data_to_write = buffer.clone();
                buffer.clear();
                info!(
                    "Writing {} bytes to cache at offset {}",
                    data_to_write.len(),
                    offset
                );

                {
                    let mut storage_guard = storage.write().await;
                    storage_guard
                        .write_to_entry(&cache_key, offset, &data_to_write)
                        .await
                        .context("Failed to write to cache")?;
                }

                offset += data_to_write.len() as u64;
                bytes_since_last_update += data_to_write.len() as u64;
                info!("Successfully wrote chunk, new offset: {}", offset);

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
            info!("Final chunk written successfully");
        }

        info!(
            "Download completed successfully for cache key: {:?}, total chunks: {}",
            cache_key, chunks_received
        );
        Ok(())
    }

    /// Pause a download
    async fn pause_download(&self, cache_key: &MediaCacheKey) -> Result<()> {
        let mut downloads = self.active_downloads.write().await;
        if let Some(progress) = downloads.get_mut(cache_key) {
            if progress.state == DownloadState::Downloading {
                progress.state = DownloadState::Paused;
                debug!("Paused download for cache key: {:?}", cache_key);
                Ok(())
            } else {
                Err(anyhow::anyhow!("Download is not in progress"))
            }
        } else {
            Err(anyhow::anyhow!("Download not found"))
        }
    }

    /// Resume a paused download
    async fn resume_download(&self, cache_key: &MediaCacheKey) -> Result<()> {
        let mut downloads = self.active_downloads.write().await;
        if let Some(progress) = downloads.get_mut(cache_key) {
            if progress.state == DownloadState::Paused {
                progress.state = DownloadState::Downloading;
                debug!("Resumed download for cache key: {:?}", cache_key);
                Ok(())
            } else {
                Err(anyhow::anyhow!("Download is not paused"))
            }
        } else {
            Err(anyhow::anyhow!("Download not found"))
        }
    }

    /// Cancel a download
    async fn cancel_download(&self, cache_key: &MediaCacheKey) -> Result<()> {
        let mut downloads = self.active_downloads.write().await;
        if let Some(progress) = downloads.get_mut(cache_key) {
            match progress.state {
                DownloadState::Downloading
                | DownloadState::Paused
                | DownloadState::Initializing => {
                    progress.state = DownloadState::Cancelled;
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

    /// List all active downloads
    async fn list_downloads(&self) -> Vec<DownloadProgress> {
        let downloads = self.active_downloads.read().await;
        downloads.values().cloned().collect()
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

    /// Pause a download
    pub async fn pause_download(&self, cache_key: MediaCacheKey) -> Result<()> {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        self.command_sender
            .send(DownloadCommand::PauseDownload {
                cache_key,
                respond_to: sender,
            })
            .map_err(|_| anyhow::anyhow!("Downloader disconnected"))?;

        receiver
            .recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("No response from downloader"))?
    }

    /// Resume a paused download
    pub async fn resume_download(&self, cache_key: MediaCacheKey) -> Result<()> {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        self.command_sender
            .send(DownloadCommand::ResumeDownload {
                cache_key,
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

    /// List all active downloads
    pub async fn list_downloads(&self) -> Result<Vec<DownloadProgress>> {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        self.command_sender
            .send(DownloadCommand::ListDownloads { respond_to: sender })
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
        assert_eq!(progress.eta_seconds, Some(5)); // (1000-500)/100 = 5
    }

    #[test]
    fn test_download_priority_ordering() {
        assert!(DownloadPriority::Urgent > DownloadPriority::High);
        assert!(DownloadPriority::High > DownloadPriority::Normal);
        assert!(DownloadPriority::Normal > DownloadPriority::Low);
    }
}
