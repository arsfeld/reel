/// Download state for cache entries
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadState {
    /// File not yet started, no data exists
    NotStarted,
    /// Initializing download (fetching headers, creating file)
    Initializing,
    /// Actively downloading data
    Downloading,
    /// Download paused by user or system
    Paused,
    /// Download complete, all data available
    Complete,
    /// Download failed with error
    Failed(String),
}

impl DownloadState {
    /// Check if state allows serving partial data
    pub fn can_serve_partial(&self) -> bool {
        matches!(self, Self::Downloading | Self::Paused | Self::Complete)
    }

    /// Check if download is active
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Initializing | Self::Downloading)
    }
}

/// Information about a download's state (derived from database)
#[derive(Debug, Clone)]
pub struct DownloadStateInfo {
    pub cache_key: super::metadata::MediaCacheKey,
    pub state: DownloadState,
    pub total_size: Option<u64>,
    pub downloaded_bytes: u64,
    pub last_updated: chrono::DateTime<chrono::Utc>,
    pub minimum_playback_bytes: u64,
    pub cache_entry_id: Option<i32>,
}

impl DownloadStateInfo {
    /// Create new state info
    pub fn new(cache_key: super::metadata::MediaCacheKey) -> Self {
        Self {
            cache_key,
            state: DownloadState::NotStarted,
            total_size: None,
            downloaded_bytes: 0,
            last_updated: chrono::Utc::now(),
            minimum_playback_bytes: 1024 * 1024, // 1MB default
            cache_entry_id: None,
        }
    }

    /// Check if enough data is available for playback
    pub fn has_minimum_data(&self) -> bool {
        self.downloaded_bytes >= self.minimum_playback_bytes
    }

    /// Calculate progress percentage
    pub fn progress_percent(&self) -> f64 {
        if let Some(total) = self.total_size {
            if total > 0 {
                return (self.downloaded_bytes as f64 / total as f64) * 100.0;
            }
        }
        0.0
    }
}
