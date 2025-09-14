use super::{BaseRepository, Repository};
use crate::db::entities::{
    PlaybackProgress, PlaybackProgressActiveModel, PlaybackProgressModel, playback_progress,
};
use anyhow::Result;
use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, Order, PaginatorTrait,
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
            id: Set(entity.id),
            media_id: Set(entity.media_id.clone()),
            user_id: Set(entity.user_id.clone()),
            position_ms: Set(entity.position_ms),
            duration_ms: Set(entity.duration_ms),
            watched: Set(entity.watched),
            view_count: Set(entity.view_count),
            last_watched_at: Set(entity.last_watched_at),
            updated_at: Set(chrono::Utc::now().naive_utc()),
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
            active_model.position_ms = Set(position_ms);
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
}
