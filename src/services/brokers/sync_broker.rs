use tracing::debug;

use crate::models::SourceId;
use crate::services::core::{SyncProgress, SyncStatus};

/// Messages for sync-related updates
#[derive(Debug, Clone)]
pub enum SyncMessage {
    /// Sync started for a source
    SyncStarted { source_id: SourceId },
    /// Sync progress update
    ProgressUpdate {
        source_id: SourceId,
        progress: SyncProgress,
    },
    /// Sync completed successfully
    SyncCompleted {
        source_id: SourceId,
        libraries_synced: usize,
        items_synced: usize,
    },
    /// Sync failed
    SyncFailed { source_id: SourceId, error: String },
    /// Sync cancelled
    SyncCancelled { source_id: SourceId },
    /// Sync status query response
    StatusUpdate {
        source_id: SourceId,
        status: SyncStatus,
    },
}

// For now, we'll use these message types directly in components
// Components can create their own relm4::Sender/Receiver channels as needed

/// Convenience functions for logging sync operations
pub fn log_sync_started(source_id: SourceId) {
    debug!("Sync started: source={}", source_id);
}

pub fn log_sync_progress(source_id: SourceId, progress: &SyncProgress) {
    debug!(
        "Sync progress: source={}, progress={:?}",
        source_id, progress
    );
}

pub fn log_sync_completed(source_id: SourceId, libraries_synced: usize, items_synced: usize) {
    debug!(
        "Sync completed: source={}, libraries={}, items={}",
        source_id, libraries_synced, items_synced
    );
}

pub fn log_sync_failed(source_id: SourceId, error: &str) {
    debug!("Sync failed: source={}, error={}", source_id, error);
}

pub fn log_sync_cancelled(source_id: SourceId) {
    debug!("Sync cancelled: source={}", source_id);
}

pub fn log_sync_status(source_id: SourceId, status: &SyncStatus) {
    debug!("Sync status: source={}, status={:?}", source_id, status);
}
