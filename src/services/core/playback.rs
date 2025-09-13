use anyhow::{Context, Result};
use std::time::Duration;
use tracing::{debug, info};

use crate::db::connection::DatabaseConnection;
use crate::db::entities::PlaybackProgressModel;
use crate::db::repository::{PlaybackRepository, PlaybackRepositoryImpl, Repository};
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
        let repo = PlaybackRepositoryImpl::new_without_events(db.clone());
        repo.find_by_media_and_user(&item_id.to_string(), user_id)
            .await
            .context("Failed to get playback progress")
    }

    /// Update playback progress
    pub async fn update_progress(
        db: &DatabaseConnection,
        user_id: &str,
        item_id: &MediaItemId,
        position: Duration,
        duration: Duration,
    ) -> Result<()> {
        let repo = PlaybackRepositoryImpl::new_without_events(db.clone());

        let progress_pct = if duration.as_secs() > 0 {
            (position.as_secs() as f64 / duration.as_secs() as f64 * 100.0) as i32
        } else {
            0
        };

        // Determine if watched (>90% complete)
        let is_watched = progress_pct >= 90;

        let entity = PlaybackProgressModel {
            id: 0, // Will be set by database
            user_id: Some(user_id.to_string()),
            media_id: item_id.to_string(),
            position_ms: position.as_millis() as i64,
            duration_ms: duration.as_millis() as i64,
            watched: is_watched,
            view_count: if is_watched { 1 } else { 0 },
            last_watched_at: Some(chrono::Utc::now().naive_utc()),
            updated_at: chrono::Utc::now().naive_utc(),
        };

        if let Some(mut existing) = repo
            .find_by_media_and_user(&item_id.to_string(), user_id)
            .await?
        {
            let was_watched = existing.watched;
            existing.position_ms = entity.position_ms;
            existing.duration_ms = entity.duration_ms;
            existing.watched = entity.watched;
            existing.last_watched_at = entity.last_watched_at;
            existing.updated_at = entity.updated_at;
            if entity.watched && !was_watched {
                existing.view_count += 1;
            }
            repo.update(existing).await?;
            debug!("Updated playback progress for item: {}", item_id);
        } else {
            repo.insert(entity).await?;
            debug!("Created playback progress for item: {}", item_id);
        }

        Ok(())
    }

    /// Mark item as watched
    pub async fn mark_watched(
        db: &DatabaseConnection,
        user_id: &str,
        item_id: &MediaItemId,
        duration: Duration,
    ) -> Result<()> {
        Self::update_progress(db, user_id, item_id, duration, duration).await
    }

    /// Mark item as unwatched
    pub async fn mark_unwatched(
        db: &DatabaseConnection,
        user_id: &str,
        item_id: &MediaItemId,
    ) -> Result<()> {
        let repo = PlaybackRepositoryImpl::new_without_events(db.clone());
        if let Some(mut progress) = repo
            .find_by_media_and_user(&item_id.to_string(), user_id)
            .await?
        {
            progress.position_ms = 0;
            progress.watched = false;
            progress.updated_at = chrono::Utc::now().naive_utc();
            repo.update(progress).await?;
            debug!("Marked item as unwatched: {}", item_id);
        }

        Ok(())
    }

    /// Get all watched items for a user
    pub async fn get_watched_items(
        db: &DatabaseConnection,
        user_id: &str,
        limit: Option<u64>,
    ) -> Result<Vec<String>> {
        let repo = PlaybackRepositoryImpl::new_without_events(db.clone());
        let progress_items = repo.find_watched(Some(user_id)).await?;

        Ok(progress_items
            .into_iter()
            .take(limit.unwrap_or(100) as usize)
            .map(|p| p.media_id)
            .collect())
    }

    /// Get continue watching items
    pub async fn get_continue_watching(
        db: &DatabaseConnection,
        user_id: &str,
        limit: u64,
    ) -> Result<Vec<PlaybackProgressModel>> {
        let repo = PlaybackRepositoryImpl::new_without_events(db.clone());
        let items = repo
            .find_in_progress(Some(user_id))
            .await
            .context("Failed to get continue watching items")?;
        Ok(items.into_iter().take(limit as usize).collect())
    }

    /// Clean up old playback records
    pub async fn cleanup_old_progress(db: &DatabaseConnection, days_old: i64) -> Result<usize> {
        let repo = PlaybackRepositoryImpl::new_without_events(db.clone());
        let cutoff = chrono::Utc::now().naive_utc() - chrono::Duration::days(days_old);

        // Use cleanup_old_entries method
        let deleted_count = repo.cleanup_old_entries(days_old).await?;

        info!("Cleaned up {} old playback records", deleted_count);
        Ok(deleted_count as usize)
    }

    /// Calculate total watch time for a user
    pub async fn get_total_watch_time(db: &DatabaseConnection, user_id: &str) -> Result<Duration> {
        let repo = PlaybackRepositoryImpl::new_without_events(db.clone());
        let all_progress = repo.find_watched(Some(user_id)).await?;

        let total_ms: i64 = all_progress
            .into_iter()
            .filter(|p| p.watched)
            .map(|p| p.position_ms)
            .sum();

        Ok(Duration::from_millis(total_ms as u64))
    }
}
