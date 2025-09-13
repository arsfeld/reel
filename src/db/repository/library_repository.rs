use super::{BaseRepository, Repository};
use crate::db::entities::{Library, LibraryActiveModel, LibraryModel, libraries};
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

/// Repository trait for Library entities
#[async_trait]

pub trait LibraryRepository: Repository<LibraryModel> {
    /// Find libraries by source
    async fn find_by_source(&self, source_id: &str) -> Result<Vec<LibraryModel>>;

    /// Find libraries by type
    async fn find_by_type(&self, library_type: &str) -> Result<Vec<LibraryModel>>;

    /// Update item count for a library
    async fn update_item_count(&self, id: &str, count: i32) -> Result<()>;

    /// Get total item count across all libraries
    async fn get_total_item_count(&self) -> Result<i64>;
}

#[derive(Debug)]
pub struct LibraryRepositoryImpl {
    base: BaseRepository,
}

impl LibraryRepositoryImpl {
    pub fn new(db: Arc<DatabaseConnection>, event_bus: Arc<EventBus>) -> Self {
        Self {
            base: BaseRepository::new(db, event_bus),
        }
    }
}

#[async_trait]
impl Repository<LibraryModel> for LibraryRepositoryImpl {
    type Entity = Library;

    async fn find_by_id(&self, id: &str) -> Result<Option<LibraryModel>> {
        Ok(Library::find_by_id(id).one(self.base.db.as_ref()).await?)
    }

    async fn find_all(&self) -> Result<Vec<LibraryModel>> {
        Ok(Library::find().all(self.base.db.as_ref()).await?)
    }

    async fn insert(&self, entity: LibraryModel) -> Result<LibraryModel> {
        let active_model = LibraryActiveModel {
            id: Set(entity.id.clone()),
            source_id: Set(entity.source_id.clone()),
            title: Set(entity.title.clone()),
            library_type: Set(entity.library_type.clone()),
            icon: Set(entity.icon.clone()),
            item_count: Set(entity.item_count),
            created_at: Set(chrono::Utc::now().naive_utc()),
            updated_at: Set(chrono::Utc::now().naive_utc()),
        };

        Ok(active_model.insert(self.base.db.as_ref()).await?)
    }

    async fn update(&self, entity: LibraryModel) -> Result<LibraryModel> {
        let mut active_model: LibraryActiveModel = entity.clone().into();
        active_model.updated_at = Set(chrono::Utc::now().naive_utc());

        Ok(active_model.update(self.base.db.as_ref()).await?)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        // Get entity details before deleting
        let entity = self.find_by_id(id).await?;

        Library::delete_by_id(id)
            .exec(self.base.db.as_ref())
            .await?;

        // Emit LibraryDeleted event if entity existed
        if let Some(library) = entity {
            let event = DatabaseEvent::new(
                EventType::LibraryDeleted,
                EventPayload::Library {
                    id: library.id.clone(),
                    source_id: library.source_id.clone(),
                    item_count: Some(library.item_count),
                },
            );

            if let Err(e) = self.base.event_bus.publish(event).await {
                tracing::warn!("Failed to publish LibraryDeleted event: {}", e);
            }
        }

        Ok(())
    }

    async fn count(&self) -> Result<u64> {
        Ok(Library::find().count(self.base.db.as_ref()).await?)
    }
}

#[async_trait]
impl LibraryRepository for LibraryRepositoryImpl {
    async fn find_by_source(&self, source_id: &str) -> Result<Vec<LibraryModel>> {
        Ok(Library::find()
            .filter(libraries::Column::SourceId.eq(source_id))
            .all(self.base.db.as_ref())
            .await?)
    }

    async fn find_by_type(&self, library_type: &str) -> Result<Vec<LibraryModel>> {
        Ok(Library::find()
            .filter(libraries::Column::LibraryType.eq(library_type))
            .all(self.base.db.as_ref())
            .await?)
    }

    async fn update_item_count(&self, id: &str, count: i32) -> Result<()> {
        if let Some(library) = self.find_by_id(id).await? {
            let mut active_model: LibraryActiveModel = library.into();
            active_model.item_count = Set(count);
            active_model.updated_at = Set(chrono::Utc::now().naive_utc());
            let updated_library = active_model.update(self.base.db.as_ref()).await?;

            // Emit LibraryItemCountChanged event
            let event = DatabaseEvent::new(
                EventType::LibraryItemCountChanged,
                EventPayload::Library {
                    id: updated_library.id.clone(),
                    source_id: updated_library.source_id.clone(),
                    item_count: Some(updated_library.item_count),
                },
            );

            if let Err(e) = self.base.event_bus.publish(event).await {
                tracing::warn!("Failed to publish LibraryItemCountChanged event: {}", e);
            }
        }
        Ok(())
    }

    async fn get_total_item_count(&self) -> Result<i64> {
        use sea_orm::QuerySelect;
        use sea_orm::sea_query::Expr;

        let result = Library::find()
            .select_only()
            .column_as(Expr::col(libraries::Column::ItemCount).sum(), "total")
            .into_tuple::<Option<i64>>()
            .one(self.base.db.as_ref())
            .await?;

        Ok(result.flatten().unwrap_or(0))
    }
}
