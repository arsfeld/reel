use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCacheConfig {
    /// Maximum cache size in megabytes
    pub max_size_mb: u64,

    /// Maximum cache size in percentage of available disk space (0-100)
    pub max_size_percent: u8,

    /// Enable progressive download during playback
    pub progressive_download: bool,

    /// Chunk size for progressive download in bytes
    pub chunk_size_kb: u32,

    /// Minimum bytes to cache before starting playback
    pub initial_buffer_kb: u32,

    /// Enable aggressive caching (cache entire file during playback)
    pub aggressive_caching: bool,

    /// Directory to store cached files
    pub cache_directory: Option<PathBuf>,

    /// Enable cache compression for video files
    pub enable_compression: bool,

    /// Number of parallel download connections
    pub max_concurrent_downloads: u32,

    /// Download timeout in seconds
    pub download_timeout_secs: u64,

    /// Maximum number of files to keep in cache
    pub max_files_count: u32,

    /// Enable periodic stats reporting
    pub enable_stats: bool,

    /// Stats reporting interval in seconds
    pub stats_interval_secs: u64,

    // ===== Chunk-based cache configuration =====
    /// Number of chunks to download ahead of playback position
    /// Used for smooth playback without buffering (default: 20 chunks = 200MB with 10MB chunks)
    pub lookahead_chunks: usize,

    /// Enable background sequential fill to complete partial downloads
    /// When enabled, chunks are downloaded in background with LOW priority
    pub enable_background_fill: bool,
}

impl Default for FileCacheConfig {
    fn default() -> Self {
        Self {
            max_size_mb: 5000,    // 5GB default
            max_size_percent: 10, // 10% of available disk space
            progressive_download: true,
            chunk_size_kb: 10240, // 10MB chunks (reduced overhead for large files)
            initial_buffer_kb: 20480, // 20MB initial buffer
            aggressive_caching: false,
            cache_directory: None, // Will be set to platform-specific default
            enable_compression: false, // Disabled by default to avoid re-encoding overhead
            max_concurrent_downloads: 3,
            download_timeout_secs: 300, // 5 minutes
            max_files_count: 1000,
            enable_stats: true,
            stats_interval_secs: 30, // 30 seconds default
            lookahead_chunks: 20,    // 20 chunks = 200MB with 10MB chunks
            enable_background_fill: true,
        }
    }
}

impl FileCacheConfig {
    /// Get the cache directory path, using platform-specific defaults if not set
    pub fn cache_directory(&self) -> Result<PathBuf> {
        if let Some(ref dir) = self.cache_directory {
            Ok(dir.clone())
        } else {
            Self::default_cache_directory()
        }
    }

    /// Get platform-specific default cache directory
    pub fn default_cache_directory() -> Result<PathBuf> {
        #[cfg(target_os = "macos")]
        {
            // On macOS, use ~/Library/Caches/Reel/
            let cache_dir = dirs::cache_dir()
                .or_else(|| dirs::home_dir().map(|h| h.join("Library/Caches")))
                .ok_or_else(|| anyhow::anyhow!("Failed to get cache directory"))?;
            Ok(cache_dir.join("Reel").join("media"))
        }
        #[cfg(not(target_os = "macos"))]
        {
            // On Linux and other platforms, use ~/.cache/reel/media/
            let cache_dir = dirs::cache_dir()
                .ok_or_else(|| anyhow::anyhow!("Failed to get cache directory"))?;
            Ok(cache_dir.join("reel").join("media"))
        }
    }

    /// Validate configuration values
    pub fn validate(&self) -> Result<()> {
        if self.max_size_mb == 0 {
            return Err(anyhow::anyhow!("max_size_mb must be greater than 0"));
        }

        if self.max_size_percent > 100 {
            return Err(anyhow::anyhow!(
                "max_size_percent must be between 0 and 100"
            ));
        }

        if self.chunk_size_kb == 0 {
            return Err(anyhow::anyhow!("chunk_size_kb must be greater than 0"));
        }

        if self.initial_buffer_kb == 0 {
            return Err(anyhow::anyhow!("initial_buffer_kb must be greater than 0"));
        }

        if self.max_concurrent_downloads == 0 {
            return Err(anyhow::anyhow!(
                "max_concurrent_downloads must be greater than 0"
            ));
        }

        if self.max_files_count == 0 {
            return Err(anyhow::anyhow!("max_files_count must be greater than 0"));
        }

        Ok(())
    }

    /// Calculate effective max size in bytes based on available disk space
    pub fn effective_max_size_bytes(&self, available_space_bytes: u64) -> u64 {
        let max_size_bytes = self.max_size_mb * 1024 * 1024;
        let percent_bytes = (available_space_bytes * self.max_size_percent as u64) / 100;

        std::cmp::min(max_size_bytes, percent_bytes)
    }

    /// Get chunk size in bytes (config stores in KB for clarity)
    pub fn chunk_size_bytes(&self) -> u64 {
        self.chunk_size_kb as u64 * 1024
    }
}
