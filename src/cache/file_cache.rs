use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, error, info};

use super::config::FileCacheConfig;
use super::downloader::{DownloadPriority, ProgressiveDownloader, ProgressiveDownloaderHandle};
use super::metadata::MediaCacheKey;
use super::proxy::CacheProxy;
use super::storage::{CacheStats, CacheStorage};
use crate::models::{MediaItemId, SourceId, StreamInfo};

/// Commands for the file cache
#[derive(Debug)]
pub enum FileCacheCommand {
    /// Get a cached stream URL, starting download if necessary
    GetCachedStream {
        source_id: SourceId,
        media_id: MediaItemId,
        original_stream: StreamInfo,
        respond_to: mpsc::UnboundedSender<Result<CachedStreamInfo>>,
    },
    /// Pre-cache a media file for offline viewing
    PreCache {
        source_id: SourceId,
        media_id: MediaItemId,
        stream_info: StreamInfo,
        priority: DownloadPriority,
        respond_to: mpsc::UnboundedSender<Result<()>>,
    },
    /// Check if media is cached and available
    IsCached {
        source_id: SourceId,
        media_id: MediaItemId,
        quality: String,
        respond_to: mpsc::UnboundedSender<bool>,
    },
    /// Remove media from cache
    RemoveFromCache {
        source_id: SourceId,
        media_id: MediaItemId,
        quality: String,
        respond_to: mpsc::UnboundedSender<Result<()>>,
    },
    /// Get cache statistics
    GetStats {
        respond_to: mpsc::UnboundedSender<CacheStats>,
    },
    /// Clear entire cache
    ClearCache {
        respond_to: mpsc::UnboundedSender<Result<()>>,
    },
    /// Cleanup cache to fit within limits
    CleanupCache {
        respond_to: mpsc::UnboundedSender<Result<()>>,
    },
    /// Shutdown the cache
    Shutdown,
}

/// Information about a cached stream
#[derive(Debug, Clone)]
pub struct CachedStreamInfo {
    /// Original stream information
    pub original_stream: StreamInfo,
    /// Local file URL for cached content
    pub cached_url: Option<String>,
    /// Whether the file is fully cached
    pub is_complete: bool,
    /// Download progress (0.0 to 1.0)
    pub progress: f64,
    /// Estimated download completion time in seconds
    pub eta_seconds: Option<u64>,
}

impl CachedStreamInfo {
    /// Get the best URL to use for playback (cached if available, otherwise original)
    pub fn playback_url(&self) -> &str {
        self.cached_url
            .as_ref()
            .unwrap_or(&self.original_stream.url)
    }
}

/// Main file cache implementation
pub struct FileCache {
    config: FileCacheConfig,
    storage: Arc<RwLock<CacheStorage>>,
    downloader_handle: ProgressiveDownloaderHandle,
    proxy: Arc<CacheProxy>,
    command_receiver: mpsc::UnboundedReceiver<FileCacheCommand>,
}

impl FileCache {
    /// Create a new file cache
    pub async fn new(config: FileCacheConfig) -> Result<(FileCacheHandle, Self)> {
        // Validate configuration
        config.validate().context("Invalid cache configuration")?;

        // Create storage
        let storage = Arc::new(RwLock::new(
            CacheStorage::new(config.clone())
                .await
                .context("Failed to initialize cache storage")?,
        ));

        // Create proxy server
        let proxy = Arc::new(CacheProxy::new(storage.clone()));
        proxy
            .clone()
            .start()
            .await
            .context("Failed to start cache proxy server")?;
        info!("Cache proxy server started on port {}", proxy.port());

        // Create downloader
        let (download_cmd_tx, download_cmd_rx) = mpsc::unbounded_channel();
        let downloader_handle = ProgressiveDownloaderHandle::new(download_cmd_tx);

        let downloader =
            ProgressiveDownloader::new(config.clone(), storage.clone(), download_cmd_rx);

        // Spawn downloader task with error handling
        tokio::spawn(async move {
            info!("🔄 Starting progressive downloader task");
            downloader.run().await;
            error!("🔄 Progressive downloader task has exited unexpectedly!");
        });

        // Create command channel
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();

        let handle = FileCacheHandle::new(cmd_tx);
        let file_cache = Self {
            config,
            storage,
            downloader_handle,
            proxy,
            command_receiver: cmd_rx,
        };

        Ok((handle, file_cache))
    }

    /// Run the file cache event loop
    pub async fn run(mut self) {
        info!("🗄️ FileCache: Starting event loop");
        info!("🗄️ FileCache: Entering command loop...");
        let mut command_count = 0;

        while let Some(command) = self.command_receiver.recv().await {
            command_count += 1;
            debug!(
                "FileCache received command #{}: {:?}",
                command_count,
                std::mem::discriminant(&command)
            );
            match command {
                FileCacheCommand::GetCachedStream {
                    source_id,
                    media_id,
                    original_stream,
                    respond_to,
                } => {
                    let result = self
                        .get_cached_stream(source_id, media_id, original_stream)
                        .await;
                    let _ = respond_to.send(result);
                }
                FileCacheCommand::PreCache {
                    source_id,
                    media_id,
                    stream_info,
                    priority,
                    respond_to,
                } => {
                    let result = self
                        .pre_cache(source_id, media_id, stream_info, priority)
                        .await;
                    let _ = respond_to.send(result);
                }
                FileCacheCommand::IsCached {
                    source_id,
                    media_id,
                    quality,
                    respond_to,
                } => {
                    let result = self.is_cached(&source_id, &media_id, &quality).await;
                    let _ = respond_to.send(result);
                }
                FileCacheCommand::RemoveFromCache {
                    source_id,
                    media_id,
                    quality,
                    respond_to,
                } => {
                    let result = self
                        .remove_from_cache(&source_id, &media_id, &quality)
                        .await;
                    let _ = respond_to.send(result);
                }
                FileCacheCommand::GetStats { respond_to } => {
                    let stats = self.get_stats().await;
                    let _ = respond_to.send(stats);
                }
                FileCacheCommand::ClearCache { respond_to } => {
                    let result = self.clear_cache().await;
                    let _ = respond_to.send(result);
                }
                FileCacheCommand::CleanupCache { respond_to } => {
                    let result = self.cleanup_cache().await;
                    let _ = respond_to.send(result);
                }
                FileCacheCommand::Shutdown => {
                    info!("🗄️ FileCache: Shutting down");
                    let _ = self.downloader_handle.shutdown();
                    break;
                }
            }
        }
    }

    /// Get cached stream information, starting download if necessary
    async fn get_cached_stream(
        &self,
        source_id: SourceId,
        media_id: MediaItemId,
        original_stream: StreamInfo,
    ) -> Result<CachedStreamInfo> {
        let quality = Self::determine_quality(&original_stream);
        let cache_key = MediaCacheKey::new(source_id.clone(), media_id.clone(), quality);

        // ALWAYS create or get cache entry
        let entry = {
            let mut storage = self.storage.write().await;

            // Get existing entry or create new one
            if let Some(entry) = storage.get_entry(&cache_key) {
                entry
            } else {
                // Create new cache entry
                storage
                    .create_entry(cache_key.clone(), original_stream.url.clone())
                    .await
                    .context("Failed to create cache entry")?
            }
        };

        // Start/resume download with high priority
        let _ = self
            .downloader_handle
            .start_download(
                cache_key.clone(),
                original_stream.url.clone(),
                DownloadPriority::Urgent,
            )
            .await;

        // Get download progress
        let progress_info = self
            .downloader_handle
            .get_progress(cache_key.clone())
            .await?;
        let (progress, eta_seconds, is_complete) = if let Some(info) = progress_info {
            (
                info.progress_percent(),
                info.eta_seconds,
                info.state == super::downloader::DownloadState::Completed,
            )
        } else {
            (entry.metadata.progress(), None, entry.metadata.is_complete)
        };

        // ALWAYS register with proxy and return proxy URL
        let proxy_url = self.proxy.register_stream(cache_key).await;

        info!(
            "🎬 Serving media through cache proxy: {} (complete: {}, progress: {:.1}%)",
            proxy_url,
            is_complete,
            progress * 100.0
        );

        Ok(CachedStreamInfo {
            original_stream,
            cached_url: Some(proxy_url), // ALWAYS use proxy URL
            is_complete,
            progress,
            eta_seconds,
        })
    }

    /// Pre-cache a media file for offline viewing
    async fn pre_cache(
        &self,
        source_id: SourceId,
        media_id: MediaItemId,
        stream_info: StreamInfo,
        priority: DownloadPriority,
    ) -> Result<()> {
        let quality = Self::determine_quality(&stream_info);
        let cache_key = MediaCacheKey::new(source_id, media_id, quality);

        // Check if already fully cached
        if self
            .is_cached(
                &cache_key.source_id,
                &cache_key.media_id,
                &cache_key.quality,
            )
            .await
        {
            debug!("Media already cached: {:?}", cache_key);
            return Ok(());
        }

        // Start download
        self.downloader_handle
            .start_download(cache_key, stream_info.url, priority)
            .await
            .context("Failed to start pre-cache download")
    }

    /// Check if media is cached and complete
    async fn is_cached(&self, source_id: &SourceId, media_id: &MediaItemId, quality: &str) -> bool {
        let cache_key =
            MediaCacheKey::new(source_id.clone(), media_id.clone(), quality.to_string());
        let storage = self.storage.read().await;
        storage.is_complete(&cache_key)
    }

    /// Remove media from cache
    async fn remove_from_cache(
        &self,
        source_id: &SourceId,
        media_id: &MediaItemId,
        quality: &str,
    ) -> Result<()> {
        let cache_key =
            MediaCacheKey::new(source_id.clone(), media_id.clone(), quality.to_string());

        // Cancel any active download
        let _ = self
            .downloader_handle
            .cancel_download(cache_key.clone())
            .await;

        // Remove from storage
        let mut storage = self.storage.write().await;
        storage
            .remove_entry(&cache_key)
            .await
            .context("Failed to remove cache entry")
    }

    /// Get cache statistics
    async fn get_stats(&self) -> CacheStats {
        let storage = self.storage.read().await;
        storage.get_stats()
    }

    /// Clear entire cache
    async fn clear_cache(&self) -> Result<()> {
        info!("🗑️ Clearing entire cache");

        // Get all entries
        let entries = {
            let storage = self.storage.read().await;
            storage.list_entries()
        };

        // Cancel all downloads and remove all entries
        for entry in entries {
            let _ = self
                .downloader_handle
                .cancel_download(entry.key.clone())
                .await;

            let mut storage = self.storage.write().await;
            let _ = storage.remove_entry(&entry.key).await;
        }

        info!("🗑️ Cache cleared successfully");
        Ok(())
    }

    /// Cleanup cache to fit within configured limits
    async fn cleanup_cache(&self) -> Result<()> {
        let mut storage = self.storage.write().await;
        storage
            .cleanup_cache()
            .await
            .context("Failed to cleanup cache")
    }

    /// Determine quality string from stream info
    fn determine_quality(stream_info: &StreamInfo) -> String {
        // Use resolution as quality indicator
        let height = stream_info.resolution.height;
        match height {
            h if h >= 2160 => "4K".to_string(),
            h if h >= 1440 => "1440p".to_string(),
            h if h >= 1080 => "1080p".to_string(),
            h if h >= 720 => "720p".to_string(),
            h if h >= 480 => "480p".to_string(),
            _ => "SD".to_string(),
        }
    }
}

/// Handle for communicating with the file cache
#[derive(Debug, Clone)]
pub struct FileCacheHandle {
    command_sender: mpsc::UnboundedSender<FileCacheCommand>,
}

impl FileCacheHandle {
    /// Create a new file cache handle
    pub fn new(command_sender: mpsc::UnboundedSender<FileCacheCommand>) -> Self {
        Self { command_sender }
    }

    /// Get cached stream information
    pub async fn get_cached_stream(
        &self,
        source_id: SourceId,
        media_id: MediaItemId,
        original_stream: StreamInfo,
    ) -> Result<CachedStreamInfo> {
        debug!(
            "FileCacheHandle::get_cached_stream called for media_id: {:?}",
            media_id
        );
        let (sender, mut receiver) = mpsc::unbounded_channel();
        self.command_sender
            .send(FileCacheCommand::GetCachedStream {
                source_id,
                media_id,
                original_stream,
                respond_to: sender,
            })
            .map_err(|e| {
                error!("Failed to send GetCachedStream command: {:?}", e);
                anyhow::anyhow!("File cache disconnected")
            })?;

        receiver
            .recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("No response from file cache"))?
    }

    /// Pre-cache media for offline viewing
    pub async fn pre_cache(
        &self,
        source_id: SourceId,
        media_id: MediaItemId,
        stream_info: StreamInfo,
        priority: DownloadPriority,
    ) -> Result<()> {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        self.command_sender
            .send(FileCacheCommand::PreCache {
                source_id,
                media_id,
                stream_info,
                priority,
                respond_to: sender,
            })
            .map_err(|_| anyhow::anyhow!("File cache disconnected"))?;

        receiver
            .recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("No response from file cache"))?
    }

    /// Check if media is cached
    pub async fn is_cached(
        &self,
        source_id: SourceId,
        media_id: MediaItemId,
        quality: String,
    ) -> Result<bool> {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        self.command_sender
            .send(FileCacheCommand::IsCached {
                source_id,
                media_id,
                quality,
                respond_to: sender,
            })
            .map_err(|_| anyhow::anyhow!("File cache disconnected"))?;

        receiver
            .recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("No response from file cache"))
    }

    /// Remove media from cache
    pub async fn remove_from_cache(
        &self,
        source_id: SourceId,
        media_id: MediaItemId,
        quality: String,
    ) -> Result<()> {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        self.command_sender
            .send(FileCacheCommand::RemoveFromCache {
                source_id,
                media_id,
                quality,
                respond_to: sender,
            })
            .map_err(|_| anyhow::anyhow!("File cache disconnected"))?;

        receiver
            .recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("No response from file cache"))?
    }

    /// Get cache statistics
    pub async fn get_stats(&self) -> Result<CacheStats> {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        self.command_sender
            .send(FileCacheCommand::GetStats { respond_to: sender })
            .map_err(|_| anyhow::anyhow!("File cache disconnected"))?;

        receiver
            .recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("No response from file cache"))
    }

    /// Clear entire cache
    pub async fn clear_cache(&self) -> Result<()> {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        self.command_sender
            .send(FileCacheCommand::ClearCache { respond_to: sender })
            .map_err(|_| anyhow::anyhow!("File cache disconnected"))?;

        receiver
            .recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("No response from file cache"))?
    }

    /// Cleanup cache
    pub async fn cleanup_cache(&self) -> Result<()> {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        self.command_sender
            .send(FileCacheCommand::CleanupCache { respond_to: sender })
            .map_err(|_| anyhow::anyhow!("File cache disconnected"))?;

        receiver
            .recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("No response from file cache"))?
    }

    /// Shutdown the file cache
    pub fn shutdown(&self) -> Result<()> {
        self.command_sender
            .send(FileCacheCommand::Shutdown)
            .map_err(|_| anyhow::anyhow!("File cache disconnected"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{QualityOption, Resolution};

    fn create_test_stream_info(height: u32) -> StreamInfo {
        StreamInfo {
            url: "http://test.com/video.mp4".to_string(),
            direct_play: true,
            video_codec: "h264".to_string(),
            audio_codec: "aac".to_string(),
            container: "mp4".to_string(),
            bitrate: 5000000,
            resolution: Resolution {
                width: height * 16 / 9,
                height,
            },
            quality_options: vec![],
        }
    }

    #[test]
    fn test_determine_quality() {
        assert_eq!(
            FileCache::determine_quality(&create_test_stream_info(2160)),
            "4K"
        );
        assert_eq!(
            FileCache::determine_quality(&create_test_stream_info(1440)),
            "1440p"
        );
        assert_eq!(
            FileCache::determine_quality(&create_test_stream_info(1080)),
            "1080p"
        );
        assert_eq!(
            FileCache::determine_quality(&create_test_stream_info(720)),
            "720p"
        );
        assert_eq!(
            FileCache::determine_quality(&create_test_stream_info(480)),
            "480p"
        );
        assert_eq!(
            FileCache::determine_quality(&create_test_stream_info(360)),
            "SD"
        );
    }

    #[test]
    fn test_cached_stream_info_playback_url() {
        let original_stream = create_test_stream_info(1080);

        // Test with cached URL
        let cached_info = CachedStreamInfo {
            original_stream: original_stream.clone(),
            cached_url: Some("file:///path/to/cached/file.mp4".to_string()),
            is_complete: true,
            progress: 1.0,
            eta_seconds: None,
        };
        assert_eq!(
            cached_info.playback_url(),
            "file:///path/to/cached/file.mp4"
        );

        // Test without cached URL (falls back to original)
        let uncached_info = CachedStreamInfo {
            original_stream: original_stream.clone(),
            cached_url: None,
            is_complete: false,
            progress: 0.5,
            eta_seconds: Some(300),
        };
        assert_eq!(uncached_info.playback_url(), "http://test.com/video.mp4");
    }
}
