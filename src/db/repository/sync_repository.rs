use super::{BaseRepository, Repository};
use crate::db::entities::{SyncStatus, SyncStatusActiveModel, SyncStatusModel, sync_status};
use anyhow::Result;
use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, Order, PaginatorTrait,
    QueryFilter, QueryOrder, Set,
};
use std::sync::Arc;

/// Repository trait for SyncStatus entities
#[async_trait]

pub trait SyncRepository: Repository<SyncStatusModel> {
    /// Find sync status by source
    async fn find_by_source(&self, source_id: &str) -> Result<Vec<SyncStatusModel>>;

    /// Find the latest sync for a source
    async fn find_latest_for_source(&self, source_id: &str) -> Result<Option<SyncStatusModel>>;

    /// Find running syncs
    async fn find_running(&self) -> Result<Vec<SyncStatusModel>>;

    /// Start a new sync
    async fn start_sync(
        &self,
        source_id: &str,
        sync_type: &str,
        total_items: Option<i32>,
    ) -> Result<SyncStatusModel>;

    /// Complete a sync
    async fn complete_sync(&self, sync_id: i32, items_synced: i32) -> Result<()>;

    /// Fail a sync
    async fn fail_sync(&self, sync_id: i32, error_message: &str) -> Result<()>;

    /// Clean up old sync records
    async fn cleanup_old_records(&self, keep_count: usize) -> Result<u64>;
}

#[cfg(test)]
mod sync_repository_tests;

pub struct SyncRepositoryImpl {
    base: BaseRepository,
}

impl SyncRepositoryImpl {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self {
            base: BaseRepository::new(db),
        }
    }
}

#[async_trait]
impl Repository<SyncStatusModel> for SyncRepositoryImpl {
    type Entity = SyncStatus;

    async fn find_by_id(&self, id: &str) -> Result<Option<SyncStatusModel>> {
        let id_parsed = id.parse::<i32>().unwrap_or(0);
        Ok(SyncStatus::find_by_id(id_parsed)
            .one(self.base.db.as_ref())
            .await?)
    }

    async fn find_all(&self) -> Result<Vec<SyncStatusModel>> {
        Ok(SyncStatus::find().all(self.base.db.as_ref()).await?)
    }

    async fn insert(&self, entity: SyncStatusModel) -> Result<SyncStatusModel> {
        let active_model = SyncStatusActiveModel {
            id: if entity.id == 0 {
                sea_orm::NotSet
            } else {
                Set(entity.id)
            },
            source_id: Set(entity.source_id.clone()),
            sync_type: Set(entity.sync_type.clone()),
            status: Set(entity.status.clone()),
            started_at: Set(entity.started_at),
            completed_at: Set(entity.completed_at),
            items_synced: Set(entity.items_synced),
            total_items: Set(entity.total_items),
            error_message: Set(entity.error_message.clone()),
        };

        Ok(active_model.insert(self.base.db.as_ref()).await?)
    }

    async fn update(&self, entity: SyncStatusModel) -> Result<SyncStatusModel> {
        let active_model = SyncStatusActiveModel {
            id: Set(entity.id),
            source_id: Set(entity.source_id.clone()),
            sync_type: Set(entity.sync_type.clone()),
            status: Set(entity.status.clone()),
            started_at: Set(entity.started_at),
            completed_at: Set(entity.completed_at),
            items_synced: Set(entity.items_synced),
            total_items: Set(entity.total_items),
            error_message: Set(entity.error_message.clone()),
        };
        Ok(active_model.update(self.base.db.as_ref()).await?)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let id_parsed = id.parse::<i32>().unwrap_or(0);
        SyncStatus::delete_by_id(id_parsed)
            .exec(self.base.db.as_ref())
            .await?;
        Ok(())
    }

    async fn count(&self) -> Result<u64> {
        Ok(SyncStatus::find().count(self.base.db.as_ref()).await?)
    }
}

#[async_trait]
impl SyncRepository for SyncRepositoryImpl {
    async fn find_by_source(&self, source_id: &str) -> Result<Vec<SyncStatusModel>> {
        Ok(SyncStatus::find()
            .filter(sync_status::Column::SourceId.eq(source_id))
            .order_by(sync_status::Column::StartedAt, Order::Desc)
            .all(self.base.db.as_ref())
            .await?)
    }

    async fn find_latest_for_source(&self, source_id: &str) -> Result<Option<SyncStatusModel>> {
        Ok(SyncStatus::find()
            .filter(sync_status::Column::SourceId.eq(source_id))
            .order_by(sync_status::Column::StartedAt, Order::Desc)
            .one(self.base.db.as_ref())
            .await?)
    }

    async fn find_running(&self) -> Result<Vec<SyncStatusModel>> {
        Ok(SyncStatus::find()
            .filter(sync_status::Column::Status.eq("running"))
            .all(self.base.db.as_ref())
            .await?)
    }

    async fn start_sync(
        &self,
        source_id: &str,
        sync_type: &str,
        total_items: Option<i32>,
    ) -> Result<SyncStatusModel> {
        let now = chrono::Utc::now().naive_utc();

        let active_model = SyncStatusActiveModel {
            id: sea_orm::NotSet,
            source_id: Set(source_id.to_string()),
            sync_type: Set(sync_type.to_string()),
            status: Set("running".to_string()),
            started_at: Set(Some(now)),
            completed_at: Set(None),
            items_synced: Set(0),
            total_items: Set(total_items),
            error_message: Set(None),
        };

        Ok(active_model.insert(self.base.db.as_ref()).await?)
    }

    async fn complete_sync(&self, sync_id: i32, items_synced: i32) -> Result<()> {
        if let Some(sync) = SyncStatus::find_by_id(sync_id)
            .one(self.base.db.as_ref())
            .await?
        {
            let mut active_model: SyncStatusActiveModel = sync.into();
            active_model.status = Set("completed".to_string());
            active_model.completed_at = Set(Some(chrono::Utc::now().naive_utc()));
            active_model.items_synced = Set(items_synced);
            active_model.update(self.base.db.as_ref()).await?;
        }

        Ok(())
    }

    async fn fail_sync(&self, sync_id: i32, error_message: &str) -> Result<()> {
        if let Some(sync) = SyncStatus::find_by_id(sync_id)
            .one(self.base.db.as_ref())
            .await?
        {
            let mut active_model: SyncStatusActiveModel = sync.into();
            active_model.status = Set("failed".to_string());
            active_model.completed_at = Set(Some(chrono::Utc::now().naive_utc()));
            active_model.error_message = Set(Some(error_message.to_string()));
            active_model.update(self.base.db.as_ref()).await?;
        }

        Ok(())
    }

    async fn cleanup_old_records(&self, keep_count: usize) -> Result<u64> {
        // For each source, keep only the latest N records
        // Get all sync records first, then extract unique source IDs
        let all_syncs = SyncStatus::find().all(self.base.db.as_ref()).await?;

        let mut sources: Vec<String> = all_syncs.iter().map(|s| s.source_id.clone()).collect();
        sources.sort();
        sources.dedup();

        let mut total_deleted = 0u64;

        for source_id in sources {
            // Get all sync records for this source, ordered by date
            let syncs = self.find_by_source(&source_id).await?;

            if syncs.len() > keep_count {
                // Delete the older ones
                let to_delete = &syncs[keep_count..];
                for sync in to_delete {
                    SyncStatus::delete_by_id(sync.id)
                        .exec(self.base.db.as_ref())
                        .await?;
                    total_deleted += 1;
                }
            }
        }

        Ok(total_deleted)
    }
}
