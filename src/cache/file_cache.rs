use anyhow::{Context, Result};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, error, info, trace, warn};

use super::chunk_manager::{ChunkManager, Priority};
use super::chunk_store::ChunkStore;
use super::config::FileCacheConfig;
use super::metadata::MediaCacheKey;
use super::proxy::CacheProxy;
use super::state_computer::StateComputer;
use super::state_types::DownloadState;
use super::storage::{CacheStats, CacheStorage};
use crate::db::repository::cache_repository::{CacheRepository, CacheRepositoryImpl};
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
        priority: Priority,
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
    state_computer: Arc<StateComputer>,
    chunk_manager: Arc<ChunkManager>,
    proxy: Arc<CacheProxy>,
    command_receiver: mpsc::UnboundedReceiver<FileCacheCommand>,
}

impl FileCache {
    /// Create a new file cache with database connection
    pub async fn new(
        config: FileCacheConfig,
        db: Arc<DatabaseConnection>,
    ) -> Result<(FileCacheHandle, Self)> {
        // Validate configuration
        config.validate().context("Invalid cache configuration")?;

        // Create repository and state computer
        let repository: Arc<dyn CacheRepository> = Arc::new(CacheRepositoryImpl::new(db.clone()));
        let state_computer = Arc::new(StateComputer::new(repository.clone()));

        // Create storage
        let storage = Arc::new(RwLock::new(
            CacheStorage::new(config.clone())
                .await
                .context("Failed to initialize cache storage")?,
        ));

        // Create ChunkManager and ChunkStore for new chunk-based caching
        // Chunk size from config (default: 10MB as specified in CACHE_DESIGN.md)
        // Benefits: 4GB file = 400 chunks vs 2,048 with 2MB, reduces DB overhead by ~5x
        let chunk_size_bytes = config.chunk_size_bytes();
        let cache_dir = config
            .cache_directory()
            .context("Failed to get cache directory")?;
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.download_timeout_secs))
            .build()
            .context("Failed to create HTTP client")?;

        let chunk_manager = Arc::new(ChunkManager::with_client(
            repository.clone(),
            chunk_size_bytes,
            client,
            cache_dir.clone(),
            config.max_concurrent_downloads as usize,
        ));

        let chunk_store = Arc::new(ChunkStore::new(cache_dir));

        // Create proxy server with stats configuration and state computer
        let proxy = Arc::new(CacheProxy::with_config(
            storage.clone(),
            state_computer.clone(),
            repository.clone(),
            chunk_manager.clone(),
            chunk_store.clone(),
            config.enable_stats,
            config.stats_interval_secs,
        ));
        proxy
            .clone()
            .start()
            .await
            .context("Failed to start cache proxy server")?;
        info!("Cache proxy server started on port {}", proxy.port());

        // Create command channel
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();

        let handle = FileCacheHandle::new(cmd_tx);
        let file_cache = Self {
            config,
            storage,
            state_computer,
            chunk_manager,
            proxy,
            command_receiver: cmd_rx,
        };

        Ok((handle, file_cache))
    }

    /// Run the file cache event loop
    pub async fn run(mut self) {
        debug!("FileCache event loop started");
        let mut command_count = 0;

        while let Some(command) = self.command_receiver.recv().await {
            command_count += 1;
            trace!(
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
                    // Chunk downloads will naturally stop when the application exits
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
                debug!(
                    "Found existing cache entry for {:?}: is_complete={}, expected_total_size={}, downloaded_bytes={}",
                    cache_key,
                    entry.metadata.is_complete,
                    entry.metadata.expected_total_size,
                    entry.metadata.downloaded_bytes
                );

                // Check if this is an invalid cache entry that needs cleanup
                // Invalid = incomplete file without expected_total_size
                if !entry.metadata.is_complete && entry.metadata.expected_total_size == 0 {
                    warn!(
                        "Found invalid cache entry (incomplete with no total size) for {:?}, removing and recreating",
                        cache_key
                    );

                    // Remove the invalid entry
                    if let Err(e) = storage.remove_entry(&cache_key).await {
                        warn!("Failed to remove invalid cache entry: {}", e);
                    }

                    // Note: No need to remove from state computer - state is derived from database

                    // Create fresh entry
                    debug!(
                        "Creating fresh cache entry after cleanup for {:?}",
                        cache_key
                    );
                    storage
                        .create_entry(cache_key.clone(), original_stream.url.clone())
                        .await
                        .context("Failed to create cache entry")?
                } else {
                    debug!("Using existing valid cache entry for {:?}", cache_key);
                    entry
                }
            } else {
                trace!(
                    "No existing cache entry found, creating new one for {:?}",
                    cache_key
                );
                // Create new cache entry
                storage
                    .create_entry(cache_key.clone(), original_stream.url.clone())
                    .await
                    .context("Failed to create cache entry")?
            }
        };

        // Ensure database entry exists with expected_total_size (Option 2: Eager initialization)
        // This fixes the issue where GStreamer gets HTTP 500 because expected_total_size is missing
        self.ensure_database_entry(&cache_key, &original_stream.url, &entry.file_path)
            .await
            .context("Failed to ensure database entry")?;

        // Note: We don't explicitly start downloads here anymore.
        // The chunk-based system will download chunks on-demand when the proxy serves data.

        // Get download progress from state computer
        let state_info = self.state_computer.get_state(&cache_key).await;
        let (progress, eta_seconds, is_complete) = if let Some(info) = &state_info {
            (
                info.progress_percent() / 100.0, // Convert from percentage to 0-1 range
                None,                            // TODO: Could calculate ETA from download speed
                info.state == DownloadState::Complete,
            )
        } else {
            // Fall back to storage metadata if no state info
            (entry.metadata.progress(), None, entry.metadata.is_complete)
        };

        // ALWAYS register with proxy and return proxy URL
        let proxy_url = self.proxy.register_stream(cache_key).await;

        trace!(
            "Serving cached stream: complete={}, progress={:.1}%",
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
        _priority: Priority,
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

        // Note: Pre-caching is now handled by chunk_manager background fill
        // We just need to ensure the cache entry exists
        let mut storage = self.storage.write().await;
        storage
            .create_entry(cache_key.clone(), stream_info.url.clone())
            .await
            .context("Failed to create cache entry for pre-cache")?;

        Ok(())
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

        // Note: Chunk downloads will be cancelled when cache entry is removed from database

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

        // Remove all entries (downloads will be cancelled automatically)
        for entry in entries {
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

    /// Ensure database entry exists with expected_total_size via HEAD request
    /// This is critical for the chunk-based cache to work with the proxy
    async fn ensure_database_entry(
        &self,
        cache_key: &MediaCacheKey,
        original_url: &str,
        file_path: &std::path::Path,
    ) -> Result<()> {
        // Get repository from chunk_manager
        let repository = self.chunk_manager.repository();

        // Check if database entry already exists with valid expected_total_size
        if let Some(entry) = repository
            .find_cache_entry(
                &cache_key.source_id.to_string(),
                &cache_key.media_id.to_string(),
                &cache_key.quality,
            )
            .await?
        {
            if entry.expected_total_size.is_some() && entry.expected_total_size.unwrap() > 0 {
                debug!(
                    "Database entry already exists with expected_total_size={:?}",
                    entry.expected_total_size
                );
                return Ok(());
            }
            // Entry exists but missing expected_total_size - will update below
            debug!("Database entry exists but missing expected_total_size, updating...");
        }

        // Make HEAD request to get Content-Length
        debug!(
            "Making HEAD request to get Content-Length for {:?}",
            original_url
        );
        let client = self.chunk_manager.client();
        let response = client
            .head(original_url)
            .send()
            .await
            .with_context(|| format!("Failed to make HEAD request to {}", original_url))?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "HEAD request failed with status: {}",
                response.status()
            ));
        }

        let content_length = response
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<i64>().ok())
            .ok_or_else(|| anyhow::anyhow!("Content-Length header missing or invalid"))?;

        debug!(
            "Got Content-Length: {} bytes for {:?}",
            content_length, cache_key
        );

        // Check if entry exists - update or insert
        if let Some(mut entry) = repository
            .find_cache_entry(
                &cache_key.source_id.to_string(),
                &cache_key.media_id.to_string(),
                &cache_key.quality,
            )
            .await?
        {
            // Update existing entry
            entry.expected_total_size = Some(content_length);
            repository.update_cache_entry(entry).await?;
            debug!("Updated database entry with expected_total_size");
        } else {
            // Create new database entry
            use crate::db::entities::CacheEntryModel;
            use chrono::Utc;

            let entry = CacheEntryModel {
                id: 0, // Will be auto-generated
                source_id: cache_key.source_id.to_string(),
                media_id: cache_key.media_id.to_string(),
                quality: cache_key.quality.clone(),
                original_url: original_url.to_string(),
                file_path: file_path.to_string_lossy().to_string(),
                file_size: 0,
                expected_total_size: Some(content_length),
                downloaded_bytes: 0,
                is_complete: false,
                priority: 0,
                created_at: Utc::now().naive_utc(),
                last_accessed: Utc::now().naive_utc(),
                last_modified: Utc::now().naive_utc(),
                access_count: 0,
                mime_type: None, // Could extract from Content-Type header
                video_codec: None,
                audio_codec: None,
                container: None,
                resolution_width: None,
                resolution_height: None,
                bitrate: None,
                duration_secs: None,
                etag: None,
                expires_at: None,
            };

            repository.insert_cache_entry(entry).await?;
            debug!("Inserted database entry with expected_total_size");
        }

        // CRITICAL FIX: Verify the database update is visible before returning
        // This prevents race condition where proxy queries before commit is visible
        trace!("Verifying database entry is visible after update...");
        let verification_entry = match repository
            .find_cache_entry(
                &cache_key.source_id.to_string(),
                &cache_key.media_id.to_string(),
                &cache_key.quality,
            )
            .await
        {
            Ok(Some(entry)) => entry,
            Ok(None) => {
                error!("VERIFICATION FAILED: Cache entry disappeared after update");
                return Err(anyhow::anyhow!("Cache entry disappeared after update"));
            }
            Err(e) => {
                error!("VERIFICATION FAILED: Database query error: {}", e);
                return Err(e.into());
            }
        };

        match verification_entry.expected_total_size {
            Some(size) if size > 0 => {
                trace!(
                    "✅ VERIFICATION SUCCESS: Database entry has expected_total_size={} and is visible",
                    size
                );
                Ok(())
            }
            Some(size) => {
                error!(
                    "❌ VERIFICATION FAILED: expected_total_size is zero: {}",
                    size
                );
                Err(anyhow::anyhow!(
                    "Database update not visible: expected_total_size is zero"
                ))
            }
            None => {
                error!("❌ VERIFICATION FAILED: expected_total_size is None");
                Err(anyhow::anyhow!(
                    "Database update not visible: expected_total_size is missing"
                ))
            }
        }
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
        trace!(
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
        priority: Priority,
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
