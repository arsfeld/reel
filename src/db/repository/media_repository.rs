use super::{BaseRepository, Repository};
use crate::db::entities::{MediaItem, MediaItemActiveModel, MediaItemModel, media_items};
use crate::events::{
    event_bus::EventBus,
    types::{DatabaseEvent, EventPayload, EventType},
};
use anyhow::Result;
use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, Order, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, Set,
};
use std::sync::Arc;

/// Repository trait for MediaItem entities
#[async_trait]
pub trait MediaRepository: Repository<MediaItemModel> {
    /// Find media items by library
    async fn find_by_library(&self, library_id: &str) -> Result<Vec<MediaItemModel>>;

    /// Find media items by source
    async fn find_by_source(&self, source_id: &str) -> Result<Vec<MediaItemModel>>;

    /// Find media items by type
    async fn find_by_type(&self, media_type: &str) -> Result<Vec<MediaItemModel>>;

    /// Search media items by title
    async fn search(&self, query: &str) -> Result<Vec<MediaItemModel>>;

    /// Find recently added items
    async fn find_recently_added(&self, limit: usize) -> Result<Vec<MediaItemModel>>;

    /// Find items by genre
    async fn find_by_genre(&self, genre: &str) -> Result<Vec<MediaItemModel>>;

    /// Bulk insert media items
    async fn bulk_insert(&self, items: Vec<MediaItemModel>) -> Result<()>;

    /// Update metadata for a media item
    async fn update_metadata(&self, id: &str, metadata: serde_json::Value) -> Result<()>;

    /// Find episodes for a show
    async fn find_episodes_by_show(&self, show_id: &str) -> Result<Vec<MediaItemModel>>;

    /// Find episodes for a specific season of a show
    async fn find_episodes_by_season(
        &self,
        show_id: &str,
        season_number: i32,
    ) -> Result<Vec<MediaItemModel>>;
}

#[derive(Debug)]
pub struct MediaRepositoryImpl {
    base: BaseRepository,
}

impl MediaRepositoryImpl {
    pub fn new(db: Arc<DatabaseConnection>, event_bus: Arc<EventBus>) -> Self {
        Self {
            base: BaseRepository::new(db, event_bus),
        }
    }

    // Insert without emitting events (used for bulk/silent paths)
    pub async fn insert_silent(&self, entity: MediaItemModel) -> anyhow::Result<MediaItemModel> {
        use sea_orm::ActiveModelTrait;
        let active_model: MediaItemActiveModel = MediaItemActiveModel {
            id: Set(entity.id),
            library_id: Set(entity.library_id),
            source_id: Set(entity.source_id),
            media_type: Set(entity.media_type),
            title: Set(entity.title),
            sort_title: Set(entity.sort_title),
            year: Set(entity.year),
            duration_ms: Set(entity.duration_ms),
            rating: Set(entity.rating),
            poster_url: Set(entity.poster_url),
            backdrop_url: Set(entity.backdrop_url),
            overview: Set(entity.overview),
            genres: Set(entity.genres),
            added_at: Set(entity.added_at),
            updated_at: Set(chrono::Utc::now().naive_utc()),
            metadata: Set(entity.metadata),
            parent_id: Set(entity.parent_id),
            season_number: Set(entity.season_number),
            episode_number: Set(entity.episode_number),
        };

        let result = active_model.insert(self.base.db.as_ref()).await?;
        Ok(result)
    }

    // Update without emitting events (used for bulk/silent paths)
    pub async fn update_silent(&self, entity: MediaItemModel) -> anyhow::Result<MediaItemModel> {
        use sea_orm::ActiveModelTrait;
        let mut active_model: MediaItemActiveModel = entity.into();
        active_model.updated_at = Set(chrono::Utc::now().naive_utc());
        let result = active_model.update(self.base.db.as_ref()).await?;
        Ok(result)
    }

    // Lookup an episode by its unique tuple (parent_id, season_number, episode_number)
    pub async fn find_episode_by_parent_season_episode(
        &self,
        parent_id: &str,
        season_number: i32,
        episode_number: i32,
    ) -> anyhow::Result<Option<MediaItemModel>> {
        use crate::db::entities::media_items;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
        let result = MediaItem::find()
            .filter(media_items::Column::MediaType.eq("episode"))
            .filter(media_items::Column::ParentId.eq(parent_id))
            .filter(media_items::Column::SeasonNumber.eq(season_number))
            .filter(media_items::Column::EpisodeNumber.eq(episode_number))
            .one(self.base.db.as_ref())
            .await?;
        Ok(result)
    }
}

#[async_trait]
impl Repository<MediaItemModel> for MediaRepositoryImpl {
    type Entity = MediaItem;

    async fn find_by_id(&self, id: &str) -> Result<Option<MediaItemModel>> {
        Ok(MediaItem::find_by_id(id).one(self.base.db.as_ref()).await?)
    }

    async fn find_all(&self) -> Result<Vec<MediaItemModel>> {
        Ok(MediaItem::find().all(self.base.db.as_ref()).await?)
    }

    async fn insert(&self, entity: MediaItemModel) -> Result<MediaItemModel> {
        let active_model = MediaItemActiveModel {
            id: Set(entity.id.clone()),
            library_id: Set(entity.library_id.clone()),
            source_id: Set(entity.source_id.clone()),
            media_type: Set(entity.media_type.clone()),
            title: Set(entity.title.clone()),
            sort_title: Set(entity.sort_title.clone()),
            year: Set(entity.year),
            duration_ms: Set(entity.duration_ms),
            rating: Set(entity.rating),
            poster_url: Set(entity.poster_url.clone()),
            backdrop_url: Set(entity.backdrop_url.clone()),
            overview: Set(entity.overview.clone()),
            genres: Set(entity.genres.clone()),
            added_at: Set(entity.added_at),
            updated_at: Set(chrono::Utc::now().naive_utc()),
            metadata: Set(entity.metadata.clone()),
            parent_id: Set(entity.parent_id.clone()),
            season_number: Set(entity.season_number),
            episode_number: Set(entity.episode_number),
        };

        let result = active_model.insert(self.base.db.as_ref()).await?;

        // Emit MediaCreated event
        let event = DatabaseEvent::new(
            EventType::MediaCreated,
            EventPayload::Media {
                id: result.id.clone(),
                media_type: result.media_type.clone(),
                library_id: result.library_id.clone(),
                source_id: result.source_id.clone(),
            },
        );

        if let Err(e) = self.base.event_bus.publish(event).await {
            tracing::warn!("Failed to publish MediaCreated event: {}", e);
        }

        Ok(result)
    }

    async fn update(&self, entity: MediaItemModel) -> Result<MediaItemModel> {
        let mut active_model: MediaItemActiveModel = entity.clone().into();
        active_model.updated_at = Set(chrono::Utc::now().naive_utc());

        let result = active_model.update(self.base.db.as_ref()).await?;

        // Emit MediaUpdated event
        let event = DatabaseEvent::new(
            EventType::MediaUpdated,
            EventPayload::Media {
                id: result.id.clone(),
                media_type: result.media_type.clone(),
                library_id: result.library_id.clone(),
                source_id: result.source_id.clone(),
            },
        );

        if let Err(e) = self.base.event_bus.publish(event).await {
            tracing::warn!("Failed to publish MediaUpdated event: {}", e);
        }

        Ok(result)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        // First, get the entity details for the event before deleting
        let entity = self.find_by_id(id).await?;

        MediaItem::delete_by_id(id)
            .exec(self.base.db.as_ref())
            .await?;

        // Emit MediaDeleted event if entity existed
        if let Some(media) = entity {
            let event = DatabaseEvent::new(
                EventType::MediaDeleted,
                EventPayload::Media {
                    id: media.id.clone(),
                    media_type: media.media_type.clone(),
                    library_id: media.library_id.clone(),
                    source_id: media.source_id.clone(),
                },
            );

            if let Err(e) = self.base.event_bus.publish(event).await {
                tracing::warn!("Failed to publish MediaDeleted event: {}", e);
            }
        }

        Ok(())
    }

    async fn count(&self) -> Result<u64> {
        Ok(MediaItem::find().count(self.base.db.as_ref()).await?)
    }
}

#[async_trait]
impl MediaRepository for MediaRepositoryImpl {
    async fn find_by_library(&self, library_id: &str) -> Result<Vec<MediaItemModel>> {
        let start = std::time::Instant::now();
        tracing::info!(
            "[PERF] MediaRepository::find_by_library: Starting query for library {}",
            library_id
        );

        let result = MediaItem::find()
            .filter(media_items::Column::LibraryId.eq(library_id))
            .order_by(media_items::Column::SortTitle, Order::Asc)
            .all(self.base.db.as_ref())
            .await?;

        let elapsed = start.elapsed();
        tracing::warn!(
            "[PERF] MediaRepository::find_by_library: Query completed in {:?} ({} items)",
            elapsed,
            result.len()
        );

        Ok(result)
    }

    async fn find_by_source(&self, source_id: &str) -> Result<Vec<MediaItemModel>> {
        Ok(MediaItem::find()
            .filter(media_items::Column::SourceId.eq(source_id))
            .all(self.base.db.as_ref())
            .await?)
    }

    async fn find_by_type(&self, media_type: &str) -> Result<Vec<MediaItemModel>> {
        Ok(MediaItem::find()
            .filter(media_items::Column::MediaType.eq(media_type))
            .all(self.base.db.as_ref())
            .await?)
    }

    async fn search(&self, query: &str) -> Result<Vec<MediaItemModel>> {
        let search_pattern = format!("%{}%", query);
        Ok(MediaItem::find()
            .filter(media_items::Column::Title.like(&search_pattern))
            .order_by(media_items::Column::SortTitle, Order::Asc)
            .limit(100)
            .all(self.base.db.as_ref())
            .await?)
    }

    async fn find_recently_added(&self, limit: usize) -> Result<Vec<MediaItemModel>> {
        Ok(MediaItem::find()
            .order_by(media_items::Column::AddedAt, Order::Desc)
            .limit(limit as u64)
            .all(self.base.db.as_ref())
            .await?)
    }

    async fn find_by_genre(&self, genre: &str) -> Result<Vec<MediaItemModel>> {
        // This requires JSON contains query which might be database-specific
        // For SQLite, we'd need to use JSON functions
        let search_pattern = format!("%\"{}%", genre);
        Ok(MediaItem::find()
            .filter(media_items::Column::Genres.like(&search_pattern))
            .all(self.base.db.as_ref())
            .await?)
    }

    async fn bulk_insert(&self, items: Vec<MediaItemModel>) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        // Collect item IDs for event emission
        let item_ids: Vec<String> = items.iter().map(|item| item.id.clone()).collect();
        let library_id = items
            .first()
            .map(|item| item.library_id.clone())
            .unwrap_or_default();
        let source_id = items
            .first()
            .map(|item| item.source_id.clone())
            .unwrap_or_default();

        let active_models: Vec<MediaItemActiveModel> = items
            .into_iter()
            .map(|item| MediaItemActiveModel {
                id: Set(item.id),
                library_id: Set(item.library_id),
                source_id: Set(item.source_id),
                media_type: Set(item.media_type),
                title: Set(item.title),
                sort_title: Set(item.sort_title),
                year: Set(item.year),
                duration_ms: Set(item.duration_ms),
                rating: Set(item.rating),
                poster_url: Set(item.poster_url),
                backdrop_url: Set(item.backdrop_url),
                overview: Set(item.overview),
                genres: Set(item.genres),
                added_at: Set(item.added_at),
                updated_at: Set(chrono::Utc::now().naive_utc()),
                metadata: Set(item.metadata),
                parent_id: Set(item.parent_id),
                season_number: Set(item.season_number),
                episode_number: Set(item.episode_number),
            })
            .collect();

        MediaItem::insert_many(active_models)
            .exec(self.base.db.as_ref())
            .await?;

        // Emit MediaBatchCreated event
        let event = DatabaseEvent::new(
            EventType::MediaBatchCreated,
            EventPayload::MediaBatch {
                ids: item_ids,
                library_id,
                source_id,
            },
        );

        if let Err(e) = self.base.event_bus.publish(event).await {
            tracing::warn!("Failed to publish MediaBatchCreated event: {}", e);
        }

        Ok(())
    }

    async fn update_metadata(&self, id: &str, metadata: serde_json::Value) -> Result<()> {
        if let Some(item) = self.find_by_id(id).await? {
            let mut active_model: MediaItemActiveModel = item.into();
            active_model.metadata = Set(Some(metadata));
            active_model.updated_at = Set(chrono::Utc::now().naive_utc());
            active_model.update(self.base.db.as_ref()).await?;
        }
        Ok(())
    }

    async fn find_episodes_by_show(&self, show_id: &str) -> Result<Vec<MediaItemModel>> {
        // Episodes are media items with parent_id matching the show
        Ok(MediaItem::find()
            .filter(media_items::Column::MediaType.eq("episode"))
            .filter(media_items::Column::ParentId.eq(show_id))
            .order_by(media_items::Column::SeasonNumber, Order::Asc)
            .order_by(media_items::Column::EpisodeNumber, Order::Asc)
            .all(self.base.db.as_ref())
            .await?)
    }

    async fn find_episodes_by_season(
        &self,
        show_id: &str,
        season_number: i32,
    ) -> Result<Vec<MediaItemModel>> {
        // Episodes for a specific season of a show
        Ok(MediaItem::find()
            .filter(media_items::Column::MediaType.eq("episode"))
            .filter(media_items::Column::ParentId.eq(show_id))
            .filter(media_items::Column::SeasonNumber.eq(season_number))
            .order_by(media_items::Column::EpisodeNumber, Order::Asc)
            .all(self.base.db.as_ref())
            .await?)
    }
}

impl MediaRepositoryImpl {
    pub async fn count_by_library(&self, library_id: &str) -> Result<i64> {
        use sea_orm::PaginatorTrait;

        let count = MediaItem::find()
            .filter(media_items::Column::LibraryId.eq(library_id))
            .count(self.base.db.as_ref())
            .await?;
        Ok(count as i64)
    }

    pub async fn find_by_library_paginated(
        &self,
        library_id: &str,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<MediaItemModel>> {
        use sea_orm::PaginatorTrait;

        Ok(MediaItem::find()
            .filter(media_items::Column::LibraryId.eq(library_id))
            .order_by(media_items::Column::SortTitle, Order::Asc)
            .paginate(self.base.db.as_ref(), limit)
            .fetch_page(offset / limit)
            .await?)
    }
}
