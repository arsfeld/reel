use super::{BaseRepository, Repository};
use crate::db::entities::{
    PlaybackSyncQueue, PlaybackSyncQueueActiveModel, PlaybackSyncQueueModel, PlaybackSyncStatus,
    SyncChangeType, playback_sync_queue,
};
use anyhow::Result;
use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, NotSet, Order, PaginatorTrait,
    QueryFilter, QueryOrder, Set,
};
use std::sync::Arc;

/// Repository trait for PlaybackSyncQueue entities
#[async_trait]
pub trait PlaybackSyncRepository: Repository<PlaybackSyncQueueModel> {
    /// Enqueue a new sync change
    async fn enqueue_change(
        &self,
        media_item_id: &str,
        source_id: i32,
        user_id: Option<&str>,
        change_type: SyncChangeType,
        position_ms: Option<i64>,
        completed: Option<bool>,
    ) -> Result<PlaybackSyncQueueModel>;

    /// Get all pending changes ordered by creation time
    async fn get_pending(&self) -> Result<Vec<PlaybackSyncQueueModel>>;

    /// Get pending changes for a specific source
    async fn get_pending_by_source(&self, source_id: i32) -> Result<Vec<PlaybackSyncQueueModel>>;

    /// Mark a change as syncing
    async fn mark_syncing(&self, id: i32) -> Result<()>;

    /// Mark a change as successfully synced
    async fn mark_synced(&self, id: i32) -> Result<()>;

    /// Mark a change as failed with error message
    async fn mark_failed(&self, id: i32, error_message: &str) -> Result<()>;

    /// Get failed changes that can be retried
    async fn get_failed_retryable(&self, max_attempts: i32) -> Result<Vec<PlaybackSyncQueueModel>>;

    /// Delete synced items older than a certain age
    async fn cleanup_synced(&self, days: i64) -> Result<u64>;

    /// Get the count of pending items by source
    async fn count_pending_by_source(&self, source_id: i32) -> Result<u64>;

    /// Get the count of failed items
    async fn count_failed(&self) -> Result<u64>;

    /// Delete a sync queue item by ID
    async fn delete_by_id(&self, id: i32) -> Result<()>;

    /// Get all changes for a specific media item (for deduplication)
    async fn get_by_media_item(
        &self,
        media_item_id: &str,
        source_id: i32,
    ) -> Result<Vec<PlaybackSyncQueueModel>>;

    /// Cancel pending changes for a media item (e.g., when a newer change supersedes)
    async fn cancel_pending_for_media(&self, media_item_id: &str, source_id: i32) -> Result<u64>;
}

#[derive(Debug)]
pub struct PlaybackSyncRepositoryImpl {
    base: BaseRepository,
}

impl PlaybackSyncRepositoryImpl {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self {
            base: BaseRepository::new(db),
        }
    }
}

#[async_trait]
impl Repository<PlaybackSyncQueueModel> for PlaybackSyncRepositoryImpl {
    type Entity = PlaybackSyncQueue;

    async fn find_by_id(&self, id: &str) -> Result<Option<PlaybackSyncQueueModel>> {
        let id_parsed = id.parse::<i32>().unwrap_or(0);
        Ok(PlaybackSyncQueue::find_by_id(id_parsed)
            .one(self.base.db.as_ref())
            .await?)
    }

    async fn find_all(&self) -> Result<Vec<PlaybackSyncQueueModel>> {
        Ok(PlaybackSyncQueue::find().all(self.base.db.as_ref()).await?)
    }

    async fn insert(&self, entity: PlaybackSyncQueueModel) -> Result<PlaybackSyncQueueModel> {
        let active_model = PlaybackSyncQueueActiveModel {
            id: NotSet,
            media_item_id: Set(entity.media_item_id.clone()),
            source_id: Set(entity.source_id),
            user_id: Set(entity.user_id.clone()),
            change_type: Set(entity.change_type.clone()),
            position_ms: Set(entity.position_ms),
            completed: Set(entity.completed),
            created_at: Set(chrono::Utc::now().naive_utc()),
            last_attempt_at: Set(None),
            attempt_count: Set(0),
            error_message: Set(None),
            status: Set(PlaybackSyncStatus::Pending.to_string()),
        };

        Ok(active_model.insert(self.base.db.as_ref()).await?)
    }

    async fn update(&self, entity: PlaybackSyncQueueModel) -> Result<PlaybackSyncQueueModel> {
        let active_model: PlaybackSyncQueueActiveModel = entity.into();
        Ok(active_model.update(self.base.db.as_ref()).await?)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let id_parsed = id.parse::<i32>().unwrap_or(0);
        PlaybackSyncQueue::delete_by_id(id_parsed)
            .exec(self.base.db.as_ref())
            .await?;
        Ok(())
    }

    async fn count(&self) -> Result<u64> {
        Ok(PlaybackSyncQueue::find()
            .count(self.base.db.as_ref())
            .await?)
    }
}

#[async_trait]
impl PlaybackSyncRepository for PlaybackSyncRepositoryImpl {
    async fn enqueue_change(
        &self,
        media_item_id: &str,
        source_id: i32,
        user_id: Option<&str>,
        change_type: SyncChangeType,
        position_ms: Option<i64>,
        completed: Option<bool>,
    ) -> Result<PlaybackSyncQueueModel> {
        let active_model = PlaybackSyncQueueActiveModel {
            id: NotSet,
            media_item_id: Set(media_item_id.to_string()),
            source_id: Set(source_id),
            user_id: Set(user_id.map(|s| s.to_string())),
            change_type: Set(change_type.to_string()),
            position_ms: Set(position_ms),
            completed: Set(completed),
            created_at: Set(chrono::Utc::now().naive_utc()),
            last_attempt_at: Set(None),
            attempt_count: Set(0),
            error_message: Set(None),
            status: Set(PlaybackSyncStatus::Pending.to_string()),
        };

        Ok(active_model.insert(self.base.db.as_ref()).await?)
    }

    async fn get_pending(&self) -> Result<Vec<PlaybackSyncQueueModel>> {
        Ok(PlaybackSyncQueue::find()
            .filter(playback_sync_queue::Column::Status.eq(PlaybackSyncStatus::Pending.to_string()))
            .order_by(playback_sync_queue::Column::CreatedAt, Order::Asc)
            .all(self.base.db.as_ref())
            .await?)
    }

    async fn get_pending_by_source(&self, source_id: i32) -> Result<Vec<PlaybackSyncQueueModel>> {
        Ok(PlaybackSyncQueue::find()
            .filter(playback_sync_queue::Column::Status.eq(PlaybackSyncStatus::Pending.to_string()))
            .filter(playback_sync_queue::Column::SourceId.eq(source_id))
            .order_by(playback_sync_queue::Column::CreatedAt, Order::Asc)
            .all(self.base.db.as_ref())
            .await?)
    }

    async fn mark_syncing(&self, id: i32) -> Result<()> {
        if let Some(item) = PlaybackSyncQueue::find_by_id(id)
            .one(self.base.db.as_ref())
            .await?
        {
            let mut active_model: PlaybackSyncQueueActiveModel = item.into();
            active_model.status = Set(PlaybackSyncStatus::Syncing.to_string());
            active_model.last_attempt_at = Set(Some(chrono::Utc::now().naive_utc()));
            active_model.attempt_count = Set(active_model.attempt_count.unwrap() + 1);
            active_model.update(self.base.db.as_ref()).await?;
        }
        Ok(())
    }

    async fn mark_synced(&self, id: i32) -> Result<()> {
        if let Some(item) = PlaybackSyncQueue::find_by_id(id)
            .one(self.base.db.as_ref())
            .await?
        {
            let mut active_model: PlaybackSyncQueueActiveModel = item.into();
            active_model.status = Set(PlaybackSyncStatus::Synced.to_string());
            active_model.error_message = Set(None);
            active_model.update(self.base.db.as_ref()).await?;
        }
        Ok(())
    }

    async fn mark_failed(&self, id: i32, error_message: &str) -> Result<()> {
        if let Some(item) = PlaybackSyncQueue::find_by_id(id)
            .one(self.base.db.as_ref())
            .await?
        {
            let mut active_model: PlaybackSyncQueueActiveModel = item.into();
            active_model.status = Set(PlaybackSyncStatus::Failed.to_string());
            active_model.error_message = Set(Some(error_message.to_string()));
            active_model.update(self.base.db.as_ref()).await?;
        }
        Ok(())
    }

    async fn get_failed_retryable(&self, max_attempts: i32) -> Result<Vec<PlaybackSyncQueueModel>> {
        Ok(PlaybackSyncQueue::find()
            .filter(playback_sync_queue::Column::Status.eq(PlaybackSyncStatus::Failed.to_string()))
            .filter(playback_sync_queue::Column::AttemptCount.lt(max_attempts))
            .order_by(playback_sync_queue::Column::LastAttemptAt, Order::Asc)
            .all(self.base.db.as_ref())
            .await?)
    }

    async fn cleanup_synced(&self, days: i64) -> Result<u64> {
        let cutoff_date = chrono::Utc::now().naive_utc() - chrono::Duration::days(days);

        let result = PlaybackSyncQueue::delete_many()
            .filter(playback_sync_queue::Column::Status.eq(PlaybackSyncStatus::Synced.to_string()))
            .filter(playback_sync_queue::Column::CreatedAt.lt(cutoff_date))
            .exec(self.base.db.as_ref())
            .await?;

        Ok(result.rows_affected)
    }

    async fn count_pending_by_source(&self, source_id: i32) -> Result<u64> {
        Ok(PlaybackSyncQueue::find()
            .filter(playback_sync_queue::Column::Status.eq(PlaybackSyncStatus::Pending.to_string()))
            .filter(playback_sync_queue::Column::SourceId.eq(source_id))
            .count(self.base.db.as_ref())
            .await?)
    }

    async fn count_failed(&self) -> Result<u64> {
        Ok(PlaybackSyncQueue::find()
            .filter(playback_sync_queue::Column::Status.eq(PlaybackSyncStatus::Failed.to_string()))
            .count(self.base.db.as_ref())
            .await?)
    }

    async fn delete_by_id(&self, id: i32) -> Result<()> {
        PlaybackSyncQueue::delete_by_id(id)
            .exec(self.base.db.as_ref())
            .await?;
        Ok(())
    }

    async fn get_by_media_item(
        &self,
        media_item_id: &str,
        source_id: i32,
    ) -> Result<Vec<PlaybackSyncQueueModel>> {
        Ok(PlaybackSyncQueue::find()
            .filter(playback_sync_queue::Column::MediaItemId.eq(media_item_id))
            .filter(playback_sync_queue::Column::SourceId.eq(source_id))
            .order_by(playback_sync_queue::Column::CreatedAt, Order::Asc)
            .all(self.base.db.as_ref())
            .await?)
    }

    async fn cancel_pending_for_media(&self, media_item_id: &str, source_id: i32) -> Result<u64> {
        let result = PlaybackSyncQueue::delete_many()
            .filter(playback_sync_queue::Column::MediaItemId.eq(media_item_id))
            .filter(playback_sync_queue::Column::SourceId.eq(source_id))
            .filter(playback_sync_queue::Column::Status.eq(PlaybackSyncStatus::Pending.to_string()))
            .exec(self.base.db.as_ref())
            .await?;

        Ok(result.rows_affected)
    }
}
