use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use sysinfo::Disks;
use tracing::{debug, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCacheConfig {
    /// Maximum cache size in megabytes (fixed limit)
    pub max_size_mb: u64,

    /// Minimum free disk space to reserve in megabytes (absolute value)
    /// Used in dynamic limit calculation: min(fixed_max, total - min_free_reserve)
    pub min_free_reserve_mb: Option<u64>,

    /// Minimum free disk space to reserve as percentage of total disk space (0-100)
    /// Used if min_free_reserve_mb is not set
    pub min_free_reserve_percent: u8,

    /// Threshold percentage (0-100) at which cleanup is triggered
    /// Default: 90% means cleanup when cache reaches 90% of dynamic limit
    pub cleanup_threshold_percent: u8,

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
            max_size_mb: 10000,            // 10GB default (fixed maximum)
            min_free_reserve_mb: None,     // Use percentage by default
            min_free_reserve_percent: 5,   // Reserve 5% of total disk space
            cleanup_threshold_percent: 90, // Trigger cleanup at 90% of dynamic limit
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

        if self.min_free_reserve_percent > 100 {
            return Err(anyhow::anyhow!(
                "min_free_reserve_percent must be between 0 and 100"
            ));
        }

        if self.cleanup_threshold_percent == 0 || self.cleanup_threshold_percent > 100 {
            return Err(anyhow::anyhow!(
                "cleanup_threshold_percent must be between 1 and 100"
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

    /// Get disk space information for the cache directory
    /// Returns (total_bytes, available_bytes, free_bytes)
    pub fn get_disk_space_info(&self) -> Result<DiskSpaceInfo> {
        let cache_dir = self.cache_directory()?;

        let disks = Disks::new_with_refreshed_list();

        // Find the disk that contains the cache directory
        let mut best_match: Option<&sysinfo::Disk> = None;
        let mut best_match_len = 0;

        for disk in disks.list() {
            let mount_point = disk.mount_point();
            if let Ok(_stripped) = cache_dir.strip_prefix(mount_point) {
                let mount_len = mount_point.as_os_str().len();
                if mount_len > best_match_len {
                    best_match = Some(disk);
                    best_match_len = mount_len;
                }
            }
        }

        if let Some(disk) = best_match {
            let total = disk.total_space();
            let available = disk.available_space();

            debug!(
                "Disk space for {:?}: total={} bytes ({} GB), available={} bytes ({} GB)",
                disk.mount_point(),
                total,
                total / (1024 * 1024 * 1024),
                available,
                available / (1024 * 1024 * 1024)
            );

            Ok(DiskSpaceInfo {
                total_bytes: total,
                available_bytes: available,
                mount_point: disk.mount_point().to_path_buf(),
            })
        } else {
            warn!("Could not find disk for cache directory {:?}", cache_dir);
            Err(anyhow::anyhow!(
                "Could not determine disk space for cache directory"
            ))
        }
    }

    /// Calculate the minimum free space to reserve in bytes
    fn calculate_min_free_reserve(&self, total_disk_bytes: u64) -> u64 {
        if let Some(reserve_mb) = self.min_free_reserve_mb {
            // Use absolute value if set
            reserve_mb * 1024 * 1024
        } else {
            // Use percentage of total disk space
            (total_disk_bytes * self.min_free_reserve_percent as u64) / 100
        }
    }

    /// Calculate effective max size in bytes based on disk space
    /// Formula: min(fixed_max, total_disk - min_free_reserve)
    ///
    /// This method takes disk space info and calculates the dynamic cache limit
    /// by ensuring we don't exceed the fixed maximum and always leave enough
    /// free space on the disk.
    pub fn calculate_dynamic_cache_limit(&self, disk_info: &DiskSpaceInfo) -> DynamicCacheLimit {
        let fixed_max_bytes = self.max_size_mb * 1024 * 1024;
        let min_free_reserve = self.calculate_min_free_reserve(disk_info.total_bytes);

        // Calculate dynamic limit: total - min_free_reserve
        let dynamic_limit = disk_info.total_bytes.saturating_sub(min_free_reserve);

        // Take the smaller of fixed max and dynamic limit
        let effective_limit = std::cmp::min(fixed_max_bytes, dynamic_limit);

        // Calculate cleanup threshold (e.g., 90% of effective limit)
        let cleanup_threshold = (effective_limit * self.cleanup_threshold_percent as u64) / 100;

        let is_limited_by_disk = effective_limit < fixed_max_bytes;

        if is_limited_by_disk {
            warn!(
                "Cache limit constrained by disk space: effective={} MB ({}%), fixed_max={} MB, disk_total={} MB, min_reserve={} MB ({}%)",
                effective_limit / (1024 * 1024),
                (effective_limit * 100) / disk_info.total_bytes,
                fixed_max_bytes / (1024 * 1024),
                disk_info.total_bytes / (1024 * 1024),
                min_free_reserve / (1024 * 1024),
                self.min_free_reserve_percent
            );
        } else {
            debug!(
                "Cache limit using fixed maximum: effective={} MB, dynamic_limit={} MB",
                effective_limit / (1024 * 1024),
                dynamic_limit / (1024 * 1024)
            );
        }

        DynamicCacheLimit {
            effective_limit_bytes: effective_limit,
            fixed_max_bytes,
            dynamic_limit_bytes: dynamic_limit,
            cleanup_threshold_bytes: cleanup_threshold,
            min_free_reserve_bytes: min_free_reserve,
            is_limited_by_disk,
        }
    }

    /// Get chunk size in bytes (config stores in KB for clarity)
    pub fn chunk_size_bytes(&self) -> u64 {
        self.chunk_size_kb as u64 * 1024
    }

    /// Check if disk space is critically low and return warning level
    /// Returns (is_critical, warning_message)
    ///
    /// Warning levels:
    /// - Critical: < 5% free space or < 1GB available
    /// - Warning: < 10% free space or < 5GB available
    /// - Info: < 20% free space
    pub fn check_disk_space_status(&self) -> Result<DiskSpaceStatus> {
        let disk_info = self.get_disk_space_info()?;
        let available_gb = disk_info.available_bytes / (1024 * 1024 * 1024);
        let available_percent = (disk_info.available_bytes * 100) / disk_info.total_bytes;

        const CRITICAL_PERCENT: u64 = 5;
        const CRITICAL_GB: u64 = 1;
        const WARNING_PERCENT: u64 = 10;
        const WARNING_GB: u64 = 5;
        const INFO_PERCENT: u64 = 20;

        if available_percent < CRITICAL_PERCENT || available_gb < CRITICAL_GB {
            Ok(DiskSpaceStatus::Critical {
                available_bytes: disk_info.available_bytes,
                available_percent,
                message: format!(
                    "CRITICAL: Disk space critically low! Only {} GB ({:?}%) available on {:?}",
                    available_gb, available_percent, disk_info.mount_point
                ),
            })
        } else if available_percent < WARNING_PERCENT || available_gb < WARNING_GB {
            Ok(DiskSpaceStatus::Warning {
                available_bytes: disk_info.available_bytes,
                available_percent,
                message: format!(
                    "WARNING: Disk space low. {} GB ({:?}%) available on {:?}",
                    available_gb, available_percent, disk_info.mount_point
                ),
            })
        } else if available_percent < INFO_PERCENT {
            Ok(DiskSpaceStatus::Info {
                available_bytes: disk_info.available_bytes,
                available_percent,
                message: format!(
                    "INFO: Disk space getting low. {} GB ({:?}%) available on {:?}",
                    available_gb, available_percent, disk_info.mount_point
                ),
            })
        } else {
            Ok(DiskSpaceStatus::Healthy {
                available_bytes: disk_info.available_bytes,
                available_percent,
            })
        }
    }
}

/// Information about disk space
#[derive(Debug, Clone)]
pub struct DiskSpaceInfo {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub mount_point: PathBuf,
}

/// Dynamic cache limit information
#[derive(Debug, Clone)]
pub struct DynamicCacheLimit {
    /// The effective cache limit to use (min of fixed_max and dynamic_limit)
    pub effective_limit_bytes: u64,
    /// The configured fixed maximum cache size
    pub fixed_max_bytes: u64,
    /// The dynamic limit based on disk space (total - min_free_reserve)
    pub dynamic_limit_bytes: u64,
    /// The threshold at which to trigger cleanup (e.g., 90% of effective limit)
    pub cleanup_threshold_bytes: u64,
    /// The minimum free space to reserve on disk
    pub min_free_reserve_bytes: u64,
    /// Whether the effective limit is constrained by disk space (true) or fixed max (false)
    pub is_limited_by_disk: bool,
}

/// Disk space status levels
#[derive(Debug, Clone)]
pub enum DiskSpaceStatus {
    /// Disk space is healthy (> 20% free)
    Healthy {
        available_bytes: u64,
        available_percent: u64,
    },
    /// Disk space is getting low (< 20% free or < 10GB)
    Info {
        available_bytes: u64,
        available_percent: u64,
        message: String,
    },
    /// Disk space is low (< 10% free or < 5GB)
    Warning {
        available_bytes: u64,
        available_percent: u64,
        message: String,
    },
    /// Disk space is critically low (< 5% free or < 1GB)
    Critical {
        available_bytes: u64,
        available_percent: u64,
        message: String,
    },
}
