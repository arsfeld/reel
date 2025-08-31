use super::{BaseRepository, Repository};
use crate::db::entities::{Source, SourceActiveModel, SourceModel, sources};
use crate::events::{
    event_bus::EventBus,
    types::{DatabaseEvent, EventPayload, EventType},
};
use anyhow::Result;
use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    Set,
};
use std::sync::Arc;

/// Repository trait for Source entities
#[async_trait]
pub trait SourceRepository: Repository<SourceModel> {
    /// Find sources by type
    async fn find_by_type(&self, source_type: &str) -> Result<Vec<SourceModel>>;

    /// Find online sources
    async fn find_online(&self) -> Result<Vec<SourceModel>>;

    /// Update source online status
    async fn update_online_status(&self, id: &str, is_online: bool) -> Result<()>;

    /// Update last sync time
    async fn update_last_sync(&self, id: &str) -> Result<()>;
}

#[derive(Debug)]
pub struct SourceRepositoryImpl {
    base: BaseRepository,
}

impl SourceRepositoryImpl {
    pub fn new(db: Arc<DatabaseConnection>, event_bus: Arc<EventBus>) -> Self {
        Self {
            base: BaseRepository::new(db, event_bus),
        }
    }
}

#[async_trait]
impl Repository<SourceModel> for SourceRepositoryImpl {
    type Entity = Source;

    async fn find_by_id(&self, id: &str) -> Result<Option<SourceModel>> {
        Ok(Source::find_by_id(id).one(self.base.db.as_ref()).await?)
    }

    async fn find_all(&self) -> Result<Vec<SourceModel>> {
        Ok(Source::find().all(self.base.db.as_ref()).await?)
    }

    async fn insert(&self, entity: SourceModel) -> Result<SourceModel> {
        let active_model = SourceActiveModel {
            id: Set(entity.id.clone()),
            name: Set(entity.name.clone()),
            source_type: Set(entity.source_type.clone()),
            auth_provider_id: Set(entity.auth_provider_id.clone()),
            connection_url: Set(entity.connection_url.clone()),
            is_online: Set(entity.is_online),
            last_sync: Set(entity.last_sync),
            created_at: Set(chrono::Utc::now().naive_utc()),
            updated_at: Set(chrono::Utc::now().naive_utc()),
        };

        let result = active_model.insert(self.base.db.as_ref()).await?;

        // Emit SourceAdded event
        let event = DatabaseEvent::new(
            EventType::SourceAdded,
            EventPayload::Source {
                id: result.id.clone(),
                source_type: result.source_type.clone(),
                is_online: Some(result.is_online),
            },
        );

        if let Err(e) = self.base.event_bus.publish(event).await {
            tracing::warn!("Failed to publish SourceAdded event: {}", e);
        }

        Ok(result)
    }

    async fn update(&self, entity: SourceModel) -> Result<SourceModel> {
        let mut active_model: SourceActiveModel = entity.clone().into();
        active_model.updated_at = Set(chrono::Utc::now().naive_utc());

        let result = active_model.update(self.base.db.as_ref()).await?;

        // Emit SourceUpdated event
        let event = DatabaseEvent::new(
            EventType::SourceUpdated,
            EventPayload::Source {
                id: result.id.clone(),
                source_type: result.source_type.clone(),
                is_online: Some(result.is_online),
            },
        );

        if let Err(e) = self.base.event_bus.publish(event).await {
            tracing::warn!("Failed to publish SourceUpdated event: {}", e);
        }

        Ok(result)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        // Get entity details before deleting
        let entity = self.find_by_id(id).await?;

        Source::delete_by_id(id).exec(self.base.db.as_ref()).await?;

        // Emit SourceRemoved event if entity existed
        if let Some(source) = entity {
            let event = DatabaseEvent::new(
                EventType::SourceRemoved,
                EventPayload::Source {
                    id: source.id.clone(),
                    source_type: source.source_type.clone(),
                    is_online: Some(source.is_online),
                },
            );

            if let Err(e) = self.base.event_bus.publish(event).await {
                tracing::warn!("Failed to publish SourceRemoved event: {}", e);
            }
        }

        Ok(())
    }

    async fn count(&self) -> Result<u64> {
        Ok(Source::find().count(self.base.db.as_ref()).await?)
    }
}

#[async_trait]
impl SourceRepository for SourceRepositoryImpl {
    async fn find_by_type(&self, source_type: &str) -> Result<Vec<SourceModel>> {
        Ok(Source::find()
            .filter(sources::Column::SourceType.eq(source_type))
            .all(self.base.db.as_ref())
            .await?)
    }

    async fn find_online(&self) -> Result<Vec<SourceModel>> {
        Ok(Source::find()
            .filter(sources::Column::IsOnline.eq(true))
            .all(self.base.db.as_ref())
            .await?)
    }

    async fn update_online_status(&self, id: &str, is_online: bool) -> Result<()> {
        if let Some(source) = self.find_by_id(id).await? {
            let mut active_model: SourceActiveModel = source.clone().into();
            active_model.is_online = Set(is_online);
            active_model.updated_at = Set(chrono::Utc::now().naive_utc());
            active_model.update(self.base.db.as_ref()).await?;

            // Emit SourceOnlineStatusChanged event
            let event = DatabaseEvent::new(
                EventType::SourceOnlineStatusChanged,
                EventPayload::Source {
                    id: source.id.clone(),
                    source_type: source.source_type.clone(),
                    is_online: Some(is_online),
                },
            );

            if let Err(e) = self.base.event_bus.publish(event).await {
                tracing::warn!("Failed to publish SourceOnlineStatusChanged event: {}", e);
            }
        }
        Ok(())
    }

    async fn update_last_sync(&self, id: &str) -> Result<()> {
        if let Some(source) = self.find_by_id(id).await? {
            let mut active_model: SourceActiveModel = source.into();
            active_model.last_sync = Set(Some(chrono::Utc::now().naive_utc()));
            active_model.updated_at = Set(chrono::Utc::now().naive_utc());
            active_model.update(self.base.db.as_ref()).await?;
        }
        Ok(())
    }
}
