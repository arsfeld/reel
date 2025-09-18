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
}
