use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::metadata::MediaCacheKey;
use crate::db::entities::CacheEntryModel;
use crate::db::repository::cache_repository::CacheRepository;

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
    /// Convert to string for database storage
    pub fn to_db_string(&self) -> String {
        match self {
            Self::NotStarted => "not_started".to_string(),
            Self::Initializing => "initializing".to_string(),
            Self::Downloading => "downloading".to_string(),
            Self::Paused => "paused".to_string(),
            Self::Complete => "complete".to_string(),
            Self::Failed(msg) => format!("failed:{}", msg),
        }
    }

    /// Parse from database string
    pub fn from_db_string(s: &str) -> Self {
        if s.starts_with("failed:") {
            Self::Failed(s[7..].to_string())
        } else {
            match s {
                "not_started" => Self::NotStarted,
                "initializing" => Self::Initializing,
                "downloading" => Self::Downloading,
                "paused" => Self::Paused,
                "complete" => Self::Complete,
                _ => Self::NotStarted,
            }
        }
    }

    /// Check if state allows serving partial data
    pub fn can_serve_partial(&self) -> bool {
        matches!(self, Self::Downloading | Self::Paused | Self::Complete)
    }

    /// Check if download is active
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Initializing | Self::Downloading)
    }
}

/// State transition event
#[derive(Debug, Clone)]
pub struct StateTransition {
    pub from: DownloadState,
    pub to: DownloadState,
    pub timestamp: DateTime<Utc>,
    pub reason: Option<String>,
}

/// Information about a download's state
#[derive(Debug, Clone)]
pub struct DownloadStateInfo {
    pub cache_key: MediaCacheKey,
    pub state: DownloadState,
    pub total_size: Option<u64>,
    pub downloaded_bytes: u64,
    pub last_updated: DateTime<Utc>,
    pub transitions: Vec<StateTransition>,
    pub minimum_playback_bytes: u64,
    pub cache_entry_id: Option<i32>,
}

impl DownloadStateInfo {
    /// Create new state info
    pub fn new(cache_key: MediaCacheKey) -> Self {
        Self {
            cache_key,
            state: DownloadState::NotStarted,
            total_size: None,
            downloaded_bytes: 0,
            last_updated: Utc::now(),
            transitions: Vec::new(),
            minimum_playback_bytes: 1024 * 1024, // 1MB default
            cache_entry_id: None,
        }
    }

    /// Check if enough data is available for playback
    pub fn has_minimum_data(&self) -> bool {
        self.downloaded_bytes >= self.minimum_playback_bytes
    }

    /// Check if specific byte range is available
    pub fn has_byte_range(&self, start: u64, end: u64) -> bool {
        if !self.state.can_serve_partial() {
            return false;
        }

        // For complete downloads, all ranges within total size are available
        if self.state == DownloadState::Complete {
            if let Some(total) = self.total_size {
                return end < total;
            }
        }

        // For partial downloads, check if requested range is within downloaded bytes
        end < self.downloaded_bytes
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

    /// Record a state transition
    fn add_transition(&mut self, from: DownloadState, to: DownloadState, reason: Option<String>) {
        self.transitions.push(StateTransition {
            from,
            to: to.clone(),
            timestamp: Utc::now(),
            reason,
        });
        self.state = to;
        self.last_updated = Utc::now();
    }
}

/// State machine for managing download states
pub struct CacheStateMachine {
    states: Arc<RwLock<HashMap<MediaCacheKey, DownloadStateInfo>>>,
    repository: Arc<dyn CacheRepository>,
    state_waiters: Arc<RwLock<HashMap<MediaCacheKey, Vec<tokio::sync::oneshot::Sender<()>>>>>,
}

impl CacheStateMachine {
    /// Create a new state machine
    pub fn new(repository: Arc<dyn CacheRepository>) -> Self {
        Self {
            states: Arc::new(RwLock::new(HashMap::new())),
            repository,
            state_waiters: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Initialize state from database
    pub async fn initialize(&self) -> Result<()> {
        info!("Initializing cache state machine from database");

        let entries = self.repository.list_cache_entries().await?;
        let mut states = self.states.write().await;

        for entry in entries {
            let cache_key = MediaCacheKey::new(
                entry.source_id.clone().into(),
                entry.media_id.clone().into(),
                entry.quality.clone(),
            );

            let state = if entry.is_complete {
                DownloadState::Complete
            } else if entry.downloaded_bytes > 0 {
                DownloadState::Paused
            } else {
                DownloadState::NotStarted
            };

            let mut info = DownloadStateInfo::new(cache_key.clone());
            info.state = state;
            info.total_size = if let Some(expected) = entry.expected_total_size {
                if expected > 0 {
                    Some(expected as u64)
                } else {
                    None
                }
            } else {
                None
            };
            info.downloaded_bytes = entry.downloaded_bytes as u64;
            info.cache_entry_id = Some(entry.id);

            states.insert(cache_key, info);
        }

        info!("Loaded {} cache states from database", states.len());
        Ok(())
    }

    /// Get state info for a cache key
    pub async fn get_state(&self, cache_key: &MediaCacheKey) -> Option<DownloadStateInfo> {
        let states = self.states.read().await;
        states.get(cache_key).cloned()
    }

    /// Get or create state info
    pub async fn get_or_create_state(&self, cache_key: &MediaCacheKey) -> DownloadStateInfo {
        let mut states = self.states.write().await;
        states
            .entry(cache_key.clone())
            .or_insert_with(|| DownloadStateInfo::new(cache_key.clone()))
            .clone()
    }

    /// Transition to a new state
    pub async fn transition(
        &self,
        cache_key: &MediaCacheKey,
        to_state: DownloadState,
        reason: Option<String>,
    ) -> Result<()> {
        let mut states = self.states.write().await;
        let info = states
            .entry(cache_key.clone())
            .or_insert_with(|| DownloadStateInfo::new(cache_key.clone()));

        let from_state = info.state.clone();

        // Validate transition
        if !Self::is_valid_transition(&from_state, &to_state) {
            return Err(anyhow!(
                "Invalid state transition from {:?} to {:?}",
                from_state,
                to_state
            ));
        }

        debug!(
            "State transition for {:?}: {:?} -> {:?} (reason: {:?})",
            cache_key, from_state, to_state, reason
        );

        info.add_transition(from_state, to_state.clone(), reason);

        // Persist to database if we have a cache entry
        if let Some(entry_id) = info.cache_entry_id {
            // Update download status in queue if transitioning to failed
            if let DownloadState::Failed(ref msg) = to_state {
                if let Ok(Some(queue_item)) = self
                    .repository
                    .find_in_queue(cache_key.media_id.as_str(), cache_key.source_id.as_str())
                    .await
                {
                    let _ = self
                        .repository
                        .update_download_status(queue_item.id, format!("failed:{}", msg))
                        .await;
                }
            }
        }

        // Notify any waiters
        self.notify_waiters(cache_key).await;

        Ok(())
    }

    /// Update download progress
    pub async fn update_progress(
        &self,
        cache_key: &MediaCacheKey,
        downloaded_bytes: u64,
        total_size: Option<u64>,
    ) -> Result<()> {
        let mut states = self.states.write().await;
        let info = states
            .entry(cache_key.clone())
            .or_insert_with(|| DownloadStateInfo::new(cache_key.clone()));

        info.downloaded_bytes = downloaded_bytes;
        if let Some(total) = total_size {
            info.total_size = Some(total);
        }
        info.last_updated = Utc::now();

        // Persist to database if we have a cache entry
        if let Some(entry_id) = info.cache_entry_id {
            let is_complete = info
                .total_size
                .map(|t| downloaded_bytes >= t)
                .unwrap_or(false);
            self.repository
                .update_download_progress(entry_id, downloaded_bytes as i64, is_complete)
                .await?;
        }

        // Notify waiters if we have enough data
        if info.has_minimum_data() {
            self.notify_waiters(cache_key).await;
        }

        Ok(())
    }

    /// Set cache entry ID for a cache key
    pub async fn set_cache_entry_id(&self, cache_key: &MediaCacheKey, entry_id: i32) -> Result<()> {
        let mut states = self.states.write().await;
        let info = states
            .entry(cache_key.clone())
            .or_insert_with(|| DownloadStateInfo::new(cache_key.clone()));

        info.cache_entry_id = Some(entry_id);
        Ok(())
    }

    /// Wait for minimum data to be available (with timeout)
    pub async fn wait_for_data(
        &self,
        cache_key: &MediaCacheKey,
        timeout: std::time::Duration,
    ) -> Result<bool> {
        // First check if data is already available
        {
            let states = self.states.read().await;
            if let Some(info) = states.get(cache_key) {
                if info.has_minimum_data() || info.state == DownloadState::Complete {
                    return Ok(true);
                }
                if matches!(info.state, DownloadState::Failed(_)) {
                    return Ok(false);
                }
            }
        }

        // Register a waiter
        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut waiters = self.state_waiters.write().await;
            waiters
                .entry(cache_key.clone())
                .or_insert_with(Vec::new)
                .push(tx);
        }

        // Wait with timeout
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(_)) => {
                // Check again if data is available
                let states = self.states.read().await;
                Ok(states
                    .get(cache_key)
                    .map(|info| info.has_minimum_data())
                    .unwrap_or(false))
            }
            Ok(Err(_)) => Ok(false), // Channel closed
            Err(_) => Ok(false),     // Timeout
        }
    }

    /// Wait for specific byte range to be available
    pub async fn wait_for_range(
        &self,
        cache_key: &MediaCacheKey,
        start: u64,
        end: u64,
        timeout: std::time::Duration,
    ) -> Result<bool> {
        let deadline = tokio::time::Instant::now() + timeout;

        loop {
            // Check if range is available
            {
                let states = self.states.read().await;
                if let Some(info) = states.get(cache_key) {
                    if info.has_byte_range(start, end) {
                        return Ok(true);
                    }
                    if matches!(info.state, DownloadState::Failed(_)) {
                        return Ok(false);
                    }
                }
            }

            // Check if we've timed out
            if tokio::time::Instant::now() >= deadline {
                return Ok(false);
            }

            // Wait a bit before checking again
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }

    /// Notify waiters for a cache key
    async fn notify_waiters(&self, cache_key: &MediaCacheKey) {
        let mut waiters = self.state_waiters.write().await;
        if let Some(senders) = waiters.remove(cache_key) {
            for sender in senders {
                let _ = sender.send(());
            }
        }
    }

    /// Check if a state transition is valid
    fn is_valid_transition(from: &DownloadState, to: &DownloadState) -> bool {
        use DownloadState::*;

        match (from, to) {
            // From NotStarted
            (NotStarted, Initializing) => true,
            (NotStarted, Failed(_)) => true,

            // From Initializing
            (Initializing, Downloading) => true,
            (Initializing, Failed(_)) => true,
            (Initializing, Paused) => true,

            // From Downloading
            (Downloading, Paused) => true,
            (Downloading, Complete) => true,
            (Downloading, Failed(_)) => true,

            // From Paused
            (Paused, Downloading) => true,
            (Paused, Failed(_)) => true,

            // From Failed - can retry
            (Failed(_), Initializing) => true,

            // From Complete - no transitions except retry
            (Complete, Initializing) => true,

            // Invalid transitions
            _ => false,
        }
    }

    /// Get all active downloads
    pub async fn get_active_downloads(&self) -> Vec<DownloadStateInfo> {
        let states = self.states.read().await;
        states
            .values()
            .filter(|info| info.state.is_active())
            .cloned()
            .collect()
    }

    /// Clean up completed or failed downloads older than specified duration
    pub async fn cleanup_old_states(&self, older_than: chrono::Duration) -> Result<usize> {
        let cutoff = Utc::now() - older_than;
        let mut states = self.states.write().await;

        let keys_to_remove: Vec<_> = states
            .iter()
            .filter(|(_, info)| {
                matches!(
                    info.state,
                    DownloadState::Complete | DownloadState::Failed(_)
                ) && info.last_updated < cutoff
            })
            .map(|(k, _)| k.clone())
            .collect();

        let count = keys_to_remove.len();
        for key in keys_to_remove {
            states.remove(&key);
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{MediaItemId, SourceId};

    #[test]
    fn test_state_transitions() {
        assert!(CacheStateMachine::is_valid_transition(
            &DownloadState::NotStarted,
            &DownloadState::Initializing
        ));

        assert!(CacheStateMachine::is_valid_transition(
            &DownloadState::Downloading,
            &DownloadState::Complete
        ));

        assert!(!CacheStateMachine::is_valid_transition(
            &DownloadState::NotStarted,
            &DownloadState::Complete
        ));
    }

    #[test]
    fn test_download_state_info() {
        let key = MediaCacheKey::new(
            SourceId::from("source1"),
            MediaItemId::from("media1"),
            "1080p".to_string(),
        );

        let mut info = DownloadStateInfo::new(key);
        info.downloaded_bytes = 500;
        info.total_size = Some(1000);

        assert_eq!(info.progress_percent(), 50.0);
        assert!(!info.has_minimum_data()); // Default minimum is 1MB

        info.minimum_playback_bytes = 100;
        assert!(info.has_minimum_data());

        assert!(!info.has_byte_range(600, 700)); // Not downloaded yet
        assert!(info.has_byte_range(0, 400)); // Within downloaded range
    }
}
