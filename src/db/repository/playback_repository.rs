use super::{BaseRepository, Repository};
use crate::db::entities::{
    PlaybackProgress, PlaybackProgressActiveModel, PlaybackProgressModel, playback_progress,
};
use anyhow::Result;
use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, NotSet, Order, PaginatorTrait,
    QueryFilter, QueryOrder, Set,
};
use std::sync::Arc;

/// Repository trait for PlaybackProgress entities
#[async_trait]
pub trait PlaybackRepository: Repository<PlaybackProgressModel> {
    /// Find playback progress for a specific media item
    async fn find_by_media_id(&self, media_id: &str) -> Result<Option<PlaybackProgressModel>>;

    /// Find playback progress for a specific user and media
    async fn find_by_media_and_user(
        &self,
        media_id: &str,
        user_id: &str,
    ) -> Result<Option<PlaybackProgressModel>>;

    /// Find all watched items
    async fn find_watched(&self, user_id: Option<&str>) -> Result<Vec<PlaybackProgressModel>>;

    /// Find items in progress (started but not finished)
    async fn find_in_progress(&self, user_id: Option<&str>) -> Result<Vec<PlaybackProgressModel>>;

    /// Update or create playback progress
    async fn upsert_progress(
        &self,
        media_id: &str,
        user_id: Option<&str>,
        position_ms: i64,
        duration_ms: i64,
    ) -> Result<PlaybackProgressModel>;

    /// Mark an item as watched
    async fn mark_watched(&self, media_id: &str, user_id: Option<&str>) -> Result<()>;

    /// Mark an item as unwatched
    async fn mark_unwatched(&self, media_id: &str, user_id: Option<&str>) -> Result<()>;

    /// Get recently watched items
    async fn find_recently_watched(
        &self,
        user_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<PlaybackProgressModel>>;

    /// Clean up old progress entries
    async fn cleanup_old_entries(&self, days: i64) -> Result<u64>;

    /// Save PlayQueue state for a media item
    async fn save_playqueue_state(
        &self,
        media_id: &str,
        user_id: Option<&str>,
        play_queue_id: i64,
        play_queue_version: i32,
        play_queue_item_id: i64,
        source_id: i32,
    ) -> Result<()>;

    /// Get PlayQueue state for a media item
    async fn get_playqueue_state(
        &self,
        media_id: &str,
        user_id: Option<&str>,
    ) -> Result<Option<(i64, i32, i64, i32)>>;

    /// Clear PlayQueue state for a media item
    async fn clear_playqueue_state(&self, media_id: &str, user_id: Option<&str>) -> Result<()>;

    /// Find progress by PlayQueue ID
    async fn find_by_playqueue_id(
        &self,
        play_queue_id: i64,
        source_id: i32,
    ) -> Result<Option<PlaybackProgressModel>>;
}

#[derive(Debug)]
pub struct PlaybackRepositoryImpl {
    base: BaseRepository,
}

impl PlaybackRepositoryImpl {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self {
            base: BaseRepository::new(db),
        }
    }
}

#[async_trait]
impl Repository<PlaybackProgressModel> for PlaybackRepositoryImpl {
    type Entity = PlaybackProgress;

    async fn find_by_id(&self, id: &str) -> Result<Option<PlaybackProgressModel>> {
        let id_parsed = id.parse::<i32>().unwrap_or(0);
        Ok(PlaybackProgress::find_by_id(id_parsed)
            .one(self.base.db.as_ref())
            .await?)
    }

    async fn find_all(&self) -> Result<Vec<PlaybackProgressModel>> {
        Ok(PlaybackProgress::find().all(self.base.db.as_ref()).await?)
    }

    async fn insert(&self, entity: PlaybackProgressModel) -> Result<PlaybackProgressModel> {
        let active_model = PlaybackProgressActiveModel {
            id: NotSet, // Let database auto-generate the ID
            media_id: Set(entity.media_id.clone()),
            user_id: Set(entity.user_id.clone()),
            position_ms: Set(entity.position_ms),
            duration_ms: Set(entity.duration_ms),
            watched: Set(entity.watched),
            view_count: Set(entity.view_count),
            last_watched_at: Set(entity.last_watched_at),
            updated_at: Set(chrono::Utc::now().naive_utc()),
            play_queue_id: Set(entity.play_queue_id),
            play_queue_version: Set(entity.play_queue_version),
            play_queue_item_id: Set(entity.play_queue_item_id),
            source_id: Set(entity.source_id),
        };

        Ok(active_model.insert(self.base.db.as_ref()).await?)
    }

    async fn update(&self, entity: PlaybackProgressModel) -> Result<PlaybackProgressModel> {
        let mut active_model: PlaybackProgressActiveModel = entity.clone().into();
        active_model.updated_at = Set(chrono::Utc::now().naive_utc());

        Ok(active_model.update(self.base.db.as_ref()).await?)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let id_parsed = id.parse::<i32>().unwrap_or(0);
        PlaybackProgress::delete_by_id(id_parsed)
            .exec(self.base.db.as_ref())
            .await?;
        Ok(())
    }

    async fn count(&self) -> Result<u64> {
        Ok(PlaybackProgress::find()
            .count(self.base.db.as_ref())
            .await?)
    }
}

#[async_trait]
impl PlaybackRepository for PlaybackRepositoryImpl {
    async fn find_by_media_id(&self, media_id: &str) -> Result<Option<PlaybackProgressModel>> {
        Ok(PlaybackProgress::find()
            .filter(playback_progress::Column::MediaId.eq(media_id))
            .one(self.base.db.as_ref())
            .await?)
    }

    async fn find_by_media_and_user(
        &self,
        media_id: &str,
        user_id: &str,
    ) -> Result<Option<PlaybackProgressModel>> {
        Ok(PlaybackProgress::find()
            .filter(playback_progress::Column::MediaId.eq(media_id))
            .filter(playback_progress::Column::UserId.eq(user_id))
            .one(self.base.db.as_ref())
            .await?)
    }

    async fn find_watched(&self, user_id: Option<&str>) -> Result<Vec<PlaybackProgressModel>> {
        let mut query =
            PlaybackProgress::find().filter(playback_progress::Column::Watched.eq(true));

        if let Some(uid) = user_id {
            query = query.filter(playback_progress::Column::UserId.eq(uid));
        }

        Ok(query.all(self.base.db.as_ref()).await?)
    }

    async fn find_in_progress(&self, user_id: Option<&str>) -> Result<Vec<PlaybackProgressModel>> {
        let mut query = PlaybackProgress::find()
            .filter(playback_progress::Column::Watched.eq(false))
            .filter(playback_progress::Column::PositionMs.gt(0));

        if let Some(uid) = user_id {
            query = query.filter(playback_progress::Column::UserId.eq(uid));
        }

        Ok(query.all(self.base.db.as_ref()).await?)
    }

    async fn upsert_progress(
        &self,
        media_id: &str,
        user_id: Option<&str>,
        position_ms: i64,
        duration_ms: i64,
    ) -> Result<PlaybackProgressModel> {
        // Check if progress exists
        let existing = if let Some(uid) = user_id {
            self.find_by_media_and_user(media_id, uid).await?
        } else {
            self.find_by_media_id(media_id).await?
        };

        let now = chrono::Utc::now().naive_utc();

        if let Some(progress) = existing {
            // Update existing progress
            let mut active_model: PlaybackProgressActiveModel = progress.clone().into();

            // Prevent accidental position resets during loading/errors
            // Only reject position updates if:
            // 1. New position is very small (< 5 seconds) AND
            // 2. Existing position is significant (> 5 seconds) AND
            // 3. Item is not being marked as watched (which legitimately resets to 0)
            let threshold_ms = 5000; // 5 seconds
            let is_suspicious_reset = position_ms < threshold_ms
                && progress.position_ms > threshold_ms
                && !(position_ms as f32 / duration_ms as f32 > 0.9);

            if is_suspicious_reset {
                tracing::warn!(
                    "Suspicious position reset detected for media_id={}: attempted to set position to {}ms from {}ms (duration={}ms). Keeping existing position.",
                    media_id,
                    position_ms,
                    progress.position_ms,
                    duration_ms
                );
            } else {
                active_model.position_ms = Set(position_ms);
            }

            active_model.duration_ms = Set(duration_ms);
            active_model.last_watched_at = Set(Some(now));
            active_model.updated_at = Set(now);

            // Auto-mark as watched if near completion (>90%)
            if position_ms as f32 / duration_ms as f32 > 0.9 {
                active_model.watched = Set(true);
                active_model.view_count = Set(progress.view_count + 1);
            }

            Ok(active_model.update(self.base.db.as_ref()).await?)
        } else {
            // Create new progress
            let active_model = PlaybackProgressActiveModel {
                id: sea_orm::NotSet,
                media_id: Set(media_id.to_string()),
                user_id: Set(user_id.map(|s| s.to_string())),
                position_ms: Set(position_ms),
                duration_ms: Set(duration_ms),
                watched: Set(false),
                view_count: Set(0),
                last_watched_at: Set(Some(now)),
                updated_at: Set(now),
                play_queue_id: Set(None),
                play_queue_version: Set(None),
                play_queue_item_id: Set(None),
                source_id: Set(None),
            };

            Ok(active_model.insert(self.base.db.as_ref()).await?)
        }
    }

    async fn mark_watched(&self, media_id: &str, user_id: Option<&str>) -> Result<()> {
        let progress = if let Some(uid) = user_id {
            self.find_by_media_and_user(media_id, uid).await?
        } else {
            self.find_by_media_id(media_id).await?
        };

        if let Some(p) = progress {
            let mut active_model: PlaybackProgressActiveModel = p.clone().into();
            active_model.watched = Set(true);
            active_model.view_count = Set(p.view_count + 1);
            active_model.last_watched_at = Set(Some(chrono::Utc::now().naive_utc()));
            active_model.updated_at = Set(chrono::Utc::now().naive_utc());
            active_model.update(self.base.db.as_ref()).await?;
        }

        Ok(())
    }

    async fn mark_unwatched(&self, media_id: &str, user_id: Option<&str>) -> Result<()> {
        let progress = if let Some(uid) = user_id {
            self.find_by_media_and_user(media_id, uid).await?
        } else {
            self.find_by_media_id(media_id).await?
        };

        if let Some(p) = progress {
            let mut active_model: PlaybackProgressActiveModel = p.into();
            active_model.watched = Set(false);
            active_model.position_ms = Set(0);
            active_model.updated_at = Set(chrono::Utc::now().naive_utc());
            active_model.update(self.base.db.as_ref()).await?;
        }

        Ok(())
    }

    async fn find_recently_watched(
        &self,
        user_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<PlaybackProgressModel>> {
        let mut query = PlaybackProgress::find();

        if let Some(uid) = user_id {
            query = query.filter(playback_progress::Column::UserId.eq(uid));
        }

        Ok(query
            .order_by(playback_progress::Column::LastWatchedAt, Order::Desc)
            .paginate(self.base.db.as_ref(), limit as u64)
            .fetch_page(0)
            .await?)
    }

    async fn cleanup_old_entries(&self, days: i64) -> Result<u64> {
        let cutoff_date = chrono::Utc::now().naive_utc() - chrono::Duration::days(days);

        let result = PlaybackProgress::delete_many()
            .filter(playback_progress::Column::UpdatedAt.lt(cutoff_date))
            .filter(playback_progress::Column::Watched.eq(false))
            .exec(self.base.db.as_ref())
            .await?;

        Ok(result.rows_affected)
    }

    async fn save_playqueue_state(
        &self,
        media_id: &str,
        user_id: Option<&str>,
        play_queue_id: i64,
        play_queue_version: i32,
        play_queue_item_id: i64,
        source_id: i32,
    ) -> Result<()> {
        // Find existing progress entry
        let existing = if let Some(uid) = user_id {
            self.find_by_media_and_user(media_id, uid).await?
        } else {
            self.find_by_media_id(media_id).await?
        };

        if let Some(progress) = existing {
            // Update existing progress with PlayQueue state
            let mut active_model: PlaybackProgressActiveModel = progress.into();
            active_model.play_queue_id = Set(Some(play_queue_id));
            active_model.play_queue_version = Set(Some(play_queue_version));
            active_model.play_queue_item_id = Set(Some(play_queue_item_id));
            active_model.source_id = Set(Some(source_id));
            active_model.updated_at = Set(chrono::Utc::now().naive_utc());
            active_model.update(self.base.db.as_ref()).await?;
        } else {
            // Create new progress entry with PlayQueue state
            let now = chrono::Utc::now().naive_utc();
            let active_model = PlaybackProgressActiveModel {
                id: NotSet,
                media_id: Set(media_id.to_string()),
                user_id: Set(user_id.map(|s| s.to_string())),
                position_ms: Set(0),
                duration_ms: Set(0),
                watched: Set(false),
                view_count: Set(0),
                last_watched_at: Set(None),
                updated_at: Set(now),
                play_queue_id: Set(Some(play_queue_id)),
                play_queue_version: Set(Some(play_queue_version)),
                play_queue_item_id: Set(Some(play_queue_item_id)),
                source_id: Set(Some(source_id)),
            };
            active_model.insert(self.base.db.as_ref()).await?;
        }

        Ok(())
    }

    async fn get_playqueue_state(
        &self,
        media_id: &str,
        user_id: Option<&str>,
    ) -> Result<Option<(i64, i32, i64, i32)>> {
        let progress = if let Some(uid) = user_id {
            self.find_by_media_and_user(media_id, uid).await?
        } else {
            self.find_by_media_id(media_id).await?
        };

        if let Some(p) = progress
            && let (Some(queue_id), Some(version), Some(item_id), Some(source)) = (
                p.play_queue_id,
                p.play_queue_version,
                p.play_queue_item_id,
                p.source_id,
            )
        {
            return Ok(Some((queue_id, version, item_id, source)));
        }

        Ok(None)
    }

    async fn clear_playqueue_state(&self, media_id: &str, user_id: Option<&str>) -> Result<()> {
        let progress = if let Some(uid) = user_id {
            self.find_by_media_and_user(media_id, uid).await?
        } else {
            self.find_by_media_id(media_id).await?
        };

        if let Some(p) = progress {
            let mut active_model: PlaybackProgressActiveModel = p.into();
            active_model.play_queue_id = Set(None);
            active_model.play_queue_version = Set(None);
            active_model.play_queue_item_id = Set(None);
            active_model.source_id = Set(None);
            active_model.updated_at = Set(chrono::Utc::now().naive_utc());
            active_model.update(self.base.db.as_ref()).await?;
        }

        Ok(())
    }

    async fn find_by_playqueue_id(
        &self,
        play_queue_id: i64,
        source_id: i32,
    ) -> Result<Option<PlaybackProgressModel>> {
        Ok(PlaybackProgress::find()
            .filter(playback_progress::Column::PlayQueueId.eq(play_queue_id))
            .filter(playback_progress::Column::SourceId.eq(source_id))
            .one(self.base.db.as_ref())
            .await?)
    }
}
