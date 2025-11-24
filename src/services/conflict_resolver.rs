use crate::backends::traits::MediaBackend;
use crate::db::entities::SyncChangeType;
use crate::models::PlaybackProgress;
use anyhow::Result;
use async_trait::async_trait;
use tracing::{debug, info, warn};

/// Decision made by conflict resolver
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictResolution {
    /// Use local value (sync to backend)
    UseLocal,
    /// Use backend value (skip sync)
    UseBackend,
    /// Both values are the same (no conflict)
    NoConflict,
}

/// Trait for implementing different conflict resolution strategies
#[async_trait]
pub trait ConflictResolver: Send + Sync {
    /// Resolve a conflict for position update
    async fn resolve_position_conflict(
        &self,
        local_position_ms: i64,
        backend_state: &PlaybackProgress,
    ) -> ConflictResolution;

    /// Resolve a conflict for watch status
    async fn resolve_watch_status_conflict(
        &self,
        local_is_watched: bool,
        backend_state: &PlaybackProgress,
    ) -> ConflictResolution;

    /// Get the name of this strategy for logging
    fn strategy_name(&self) -> &'static str;
}

/// Local-Progressive Strategy: Use whichever position is further along
/// This is the recommended default strategy
pub struct LocalProgressiveStrategy;

impl LocalProgressiveStrategy {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LocalProgressiveStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ConflictResolver for LocalProgressiveStrategy {
    async fn resolve_position_conflict(
        &self,
        local_position_ms: i64,
        backend_state: &PlaybackProgress,
    ) -> ConflictResolution {
        let backend_position_ms = backend_state.position.map(|d| d.as_millis() as i64);

        match backend_position_ms {
            Some(backend_position_ms) => {
                if local_position_ms == backend_position_ms {
                    debug!(
                        "No position conflict: local={} == backend={}",
                        local_position_ms, backend_position_ms
                    );
                    ConflictResolution::NoConflict
                } else if local_position_ms > backend_position_ms {
                    info!(
                        "Position conflict resolved: local={} > backend={}, using local",
                        local_position_ms, backend_position_ms
                    );
                    ConflictResolution::UseLocal
                } else {
                    warn!(
                        "Position conflict resolved: local={} < backend={}, using backend (skipping sync)",
                        local_position_ms, backend_position_ms
                    );
                    ConflictResolution::UseBackend
                }
            }
            None => {
                debug!(
                    "No backend position, using local position={}",
                    local_position_ms
                );
                ConflictResolution::UseLocal
            }
        }
    }

    async fn resolve_watch_status_conflict(
        &self,
        local_is_watched: bool,
        backend_state: &PlaybackProgress,
    ) -> ConflictResolution {
        let backend_is_watched = backend_state.is_watched.unwrap_or(false);

        if local_is_watched == backend_is_watched {
            debug!(
                "No watch status conflict: local={} == backend={}",
                local_is_watched, backend_is_watched
            );
            ConflictResolution::NoConflict
        } else {
            // For watch status, always prefer local changes
            // Rationale: User explicitly marked it locally, so respect that
            info!(
                "Watch status conflict resolved: local={} != backend={}, using local",
                local_is_watched, backend_is_watched
            );
            ConflictResolution::UseLocal
        }
    }

    fn strategy_name(&self) -> &'static str {
        "Local-Progressive"
    }
}

/// Last-Write-Wins Strategy: Use the most recent change based on timestamp
pub struct LastWriteWinsStrategy;

impl LastWriteWinsStrategy {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LastWriteWinsStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ConflictResolver for LastWriteWinsStrategy {
    async fn resolve_position_conflict(
        &self,
        local_position_ms: i64,
        backend_state: &PlaybackProgress,
    ) -> ConflictResolution {
        let backend_position_ms = backend_state.position.map(|d| d.as_millis() as i64);

        match backend_position_ms {
            Some(backend_position_ms) => {
                if local_position_ms == backend_position_ms {
                    ConflictResolution::NoConflict
                } else {
                    // Check timestamp to determine which is more recent
                    // For now, assume local is more recent (we just created the change)
                    // A real implementation would compare timestamps from local vs backend
                    info!(
                        "Position conflict: local={} vs backend={}, using local (last-write-wins)",
                        local_position_ms, backend_position_ms
                    );
                    ConflictResolution::UseLocal
                }
            }
            None => ConflictResolution::UseLocal,
        }
    }

    async fn resolve_watch_status_conflict(
        &self,
        local_is_watched: bool,
        backend_state: &PlaybackProgress,
    ) -> ConflictResolution {
        let backend_is_watched = backend_state.is_watched.unwrap_or(false);

        if local_is_watched == backend_is_watched {
            ConflictResolution::NoConflict
        } else {
            // Use most recent change (assume local is more recent)
            info!(
                "Watch status conflict: local={} vs backend={}, using local (last-write-wins)",
                local_is_watched, backend_is_watched
            );
            ConflictResolution::UseLocal
        }
    }

    fn strategy_name(&self) -> &'static str {
        "Last-Write-Wins"
    }
}

/// Always-Local Strategy: Local changes always override backend
/// Simpler but may lose legitimate backend changes
pub struct AlwaysLocalStrategy;

impl AlwaysLocalStrategy {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AlwaysLocalStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ConflictResolver for AlwaysLocalStrategy {
    async fn resolve_position_conflict(
        &self,
        local_position_ms: i64,
        backend_state: &PlaybackProgress,
    ) -> ConflictResolution {
        let backend_position_ms = backend_state.position.map(|d| d.as_millis() as i64);

        match backend_position_ms {
            Some(backend_position_ms) if local_position_ms == backend_position_ms => {
                ConflictResolution::NoConflict
            }
            _ => {
                debug!(
                    "Using local position={} (always-local strategy)",
                    local_position_ms
                );
                ConflictResolution::UseLocal
            }
        }
    }

    async fn resolve_watch_status_conflict(
        &self,
        local_is_watched: bool,
        backend_state: &PlaybackProgress,
    ) -> ConflictResolution {
        let backend_is_watched = backend_state.is_watched.unwrap_or(false);

        if local_is_watched == backend_is_watched {
            ConflictResolution::NoConflict
        } else {
            debug!(
                "Using local watch_status={} (always-local strategy)",
                local_is_watched
            );
            ConflictResolution::UseLocal
        }
    }

    fn strategy_name(&self) -> &'static str {
        "Always-Local"
    }
}

/// Helper to fetch backend playback state
pub async fn fetch_backend_state(
    backend: &dyn MediaBackend,
    media_item_id: &str,
) -> Result<PlaybackProgress> {
    // Try to get playback progress from backend
    // Note: Not all backends expose this, so we handle errors gracefully
    match backend.get_playback_progress(media_item_id).await {
        Ok(progress) => Ok(progress),
        Err(e) => {
            // If backend doesn't support fetching progress, assume no conflict
            debug!(
                "Could not fetch backend state for {}: {}. Assuming no conflict.",
                media_item_id, e
            );
            Ok(PlaybackProgress {
                position: None,
                is_watched: None,
                last_updated_at: None,
            })
        }
    }
}

/// Conflict resolution context
pub struct ConflictResolverContext {
    resolver: Box<dyn ConflictResolver>,
}

impl ConflictResolverContext {
    /// Create a new context with Local-Progressive strategy (default)
    pub fn new_local_progressive() -> Self {
        Self {
            resolver: Box::new(LocalProgressiveStrategy::new()),
        }
    }

    /// Create a new context with Last-Write-Wins strategy
    pub fn new_last_write_wins() -> Self {
        Self {
            resolver: Box::new(LastWriteWinsStrategy::new()),
        }
    }

    /// Create a new context with Always-Local strategy
    pub fn new_always_local() -> Self {
        Self {
            resolver: Box::new(AlwaysLocalStrategy::new()),
        }
    }

    /// Create with a custom resolver
    pub fn new_custom(resolver: Box<dyn ConflictResolver>) -> Self {
        Self { resolver }
    }

    /// Resolve a conflict and determine if we should proceed with sync
    pub async fn should_sync(
        &self,
        change_type: &SyncChangeType,
        backend: &dyn MediaBackend,
        media_item_id: &str,
        local_position_ms: Option<i64>,
        _local_completed: Option<bool>,
    ) -> Result<bool> {
        // Fetch backend state
        let backend_state = fetch_backend_state(backend, media_item_id).await?;

        // Resolve based on change type
        let resolution = match change_type {
            SyncChangeType::ProgressUpdate => {
                if let Some(position) = local_position_ms {
                    self.resolver
                        .resolve_position_conflict(position, &backend_state)
                        .await
                } else {
                    warn!("ProgressUpdate without position_ms, defaulting to UseLocal");
                    ConflictResolution::UseLocal
                }
            }
            SyncChangeType::MarkWatched => {
                self.resolver
                    .resolve_watch_status_conflict(true, &backend_state)
                    .await
            }
            SyncChangeType::MarkUnwatched => {
                self.resolver
                    .resolve_watch_status_conflict(false, &backend_state)
                    .await
            }
        };

        // Log the resolution
        debug!(
            "Conflict resolution for {} ({}): {:?}",
            media_item_id,
            self.resolver.strategy_name(),
            resolution
        );

        // Return whether we should proceed with sync
        Ok(matches!(
            resolution,
            ConflictResolution::UseLocal | ConflictResolution::NoConflict
        ))
    }

    /// Get the strategy name for logging
    pub fn strategy_name(&self) -> &str {
        self.resolver.strategy_name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_local_progressive_position_higher() {
        let strategy = LocalProgressiveStrategy::new();
        let backend_state = PlaybackProgress {
            position: Some(Duration::from_millis(300_000)),
            is_watched: Some(false),
            last_updated_at: None,
        };

        let resolution = strategy
            .resolve_position_conflict(500_000, &backend_state)
            .await;
        assert_eq!(resolution, ConflictResolution::UseLocal);
    }

    #[tokio::test]
    async fn test_local_progressive_position_lower() {
        let strategy = LocalProgressiveStrategy::new();
        let backend_state = PlaybackProgress {
            position: Some(Duration::from_millis(500_000)),
            is_watched: Some(false),
            last_updated_at: None,
        };

        let resolution = strategy
            .resolve_position_conflict(300_000, &backend_state)
            .await;
        assert_eq!(resolution, ConflictResolution::UseBackend);
    }

    #[tokio::test]
    async fn test_local_progressive_position_equal() {
        let strategy = LocalProgressiveStrategy::new();
        let backend_state = PlaybackProgress {
            position: Some(Duration::from_millis(500_000)),
            is_watched: Some(false),
            last_updated_at: None,
        };

        let resolution = strategy
            .resolve_position_conflict(500_000, &backend_state)
            .await;
        assert_eq!(resolution, ConflictResolution::NoConflict);
    }

    #[tokio::test]
    async fn test_local_progressive_no_backend_position() {
        let strategy = LocalProgressiveStrategy::new();
        let backend_state = PlaybackProgress {
            position: None,
            is_watched: Some(false),
            last_updated_at: None,
        };

        let resolution = strategy
            .resolve_position_conflict(500_000, &backend_state)
            .await;
        assert_eq!(resolution, ConflictResolution::UseLocal);
    }

    #[tokio::test]
    async fn test_local_progressive_watch_status_conflict() {
        let strategy = LocalProgressiveStrategy::new();
        let backend_state = PlaybackProgress {
            position: None,
            is_watched: Some(false),
            last_updated_at: None,
        };

        // Local wants to mark as watched, backend shows unwatched
        let resolution = strategy
            .resolve_watch_status_conflict(true, &backend_state)
            .await;
        assert_eq!(resolution, ConflictResolution::UseLocal);
    }

    #[tokio::test]
    async fn test_local_progressive_watch_status_no_conflict() {
        let strategy = LocalProgressiveStrategy::new();
        let backend_state = PlaybackProgress {
            position: None,
            is_watched: Some(true),
            last_updated_at: None,
        };

        // Both local and backend agree: watched
        let resolution = strategy
            .resolve_watch_status_conflict(true, &backend_state)
            .await;
        assert_eq!(resolution, ConflictResolution::NoConflict);
    }

    #[tokio::test]
    async fn test_always_local_strategy() {
        let strategy = AlwaysLocalStrategy::new();
        let backend_state = PlaybackProgress {
            position: Some(Duration::from_millis(900_000)),
            is_watched: Some(true),
            last_updated_at: None,
        };

        // Even though backend is ahead, always use local
        let resolution = strategy
            .resolve_position_conflict(100_000, &backend_state)
            .await;
        assert_eq!(resolution, ConflictResolution::UseLocal);
    }
}
