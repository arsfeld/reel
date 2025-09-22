use anyhow::{Context, Result};

use crate::db::connection::DatabaseConnection;
use crate::db::entities::PlaybackProgressModel;
use crate::db::repository::{PlaybackRepository, PlaybackRepositoryImpl};
use crate::models::MediaItemId;

/// Pure functions for playback operations
pub struct PlaybackService;

impl PlaybackService {
    /// Get playback progress for a media item
    pub async fn get_progress(
        db: &DatabaseConnection,
        user_id: &str,
        item_id: &MediaItemId,
    ) -> Result<Option<PlaybackProgressModel>> {
        let repo = PlaybackRepositoryImpl::new(db.clone());
        repo.find_by_media_and_user(&item_id.to_string(), user_id)
            .await
            .context("Failed to get playback progress")
    }

    /// Get PlayQueue state for a media item
    pub async fn get_playqueue_state(
        db: &DatabaseConnection,
        user_id: &str,
        item_id: &MediaItemId,
    ) -> Result<Option<(i64, i32, i64, i32)>> {
        let repo = PlaybackRepositoryImpl::new(db.clone());
        repo.get_playqueue_state(&item_id.to_string(), Some(user_id))
            .await
            .context("Failed to get PlayQueue state")
    }

    /// Load PlayQueue state from a specific PlayQueue ID
    pub async fn load_playqueue_by_id(
        db: &DatabaseConnection,
        play_queue_id: i64,
        source_id: i32,
    ) -> Result<Option<PlaybackProgressModel>> {
        let repo = PlaybackRepositoryImpl::new(db.clone());
        repo.find_by_playqueue_id(play_queue_id, source_id)
            .await
            .context("Failed to load PlayQueue by ID")
    }
}
