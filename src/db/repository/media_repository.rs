use super::{BaseRepository, Repository};
use crate::db::entities::{MediaItem, MediaItemActiveModel, MediaItemModel, media_items};
use crate::ui::shared::broker::{BROKER, BrokerMessage, DataMessage};
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

    /// Find media items by library and type
    async fn find_by_library_and_type(
        &self,
        library_id: &str,
        media_type: &str,
    ) -> Result<Vec<MediaItemModel>>;

    /// Find media items by source
    async fn find_by_source(&self, source_id: &str) -> Result<Vec<MediaItemModel>>;

    /// Find media items by source and type
    async fn find_by_source_and_type(
        &self,
        source_id: &str,
        media_type: &str,
    ) -> Result<Vec<MediaItemModel>>;

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

    /// Find media item by source and original backend item ID
    /// This searches for items where the ID ends with the backend item ID
    async fn find_by_source_and_backend_id(
        &self,
        source_id: &str,
        backend_item_id: &str,
    ) -> Result<Option<MediaItemModel>>;

    /// Delete all media items for a library
    async fn delete_by_library(&self, library_id: &str) -> Result<()>;

    /// Delete all media items for a source
    async fn delete_by_source(&self, source_id: &str) -> Result<()>;

    /// Get all episodes for a show in playback order
    async fn find_episode_playlist(&self, show_id: &str) -> Result<Vec<MediaItemModel>>;

    /// Find the next episode after the given one
    async fn find_next_episode(
        &self,
        show_id: &str,
        season: i32,
        episode: i32,
    ) -> Result<Option<MediaItemModel>>;

    /// Find the previous episode before the given one
    async fn find_previous_episode(
        &self,
        show_id: &str,
        season: i32,
        episode: i32,
    ) -> Result<Option<MediaItemModel>>;

    /// Find next unwatched episode in show
    async fn find_next_unwatched_episode(
        &self,
        show_id: &str,
        after_season: i32,
        after_episode: i32,
    ) -> Result<Option<MediaItemModel>>;

    /// Update show's watched_episode_count based on actual episode watch status
    async fn update_show_watched_count(&self, show_id: &str) -> Result<()>;
}

#[derive(Debug)]
pub struct MediaRepositoryImpl {
    base: BaseRepository,
}

impl MediaRepositoryImpl {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self {
            base: BaseRepository::new(db),
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
            intro_marker_start_ms: Set(entity.intro_marker_start_ms),
            intro_marker_end_ms: Set(entity.intro_marker_end_ms),
            credits_marker_start_ms: Set(entity.credits_marker_start_ms),
            credits_marker_end_ms: Set(entity.credits_marker_end_ms),
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
            intro_marker_start_ms: Set(entity.intro_marker_start_ms),
            intro_marker_end_ms: Set(entity.intro_marker_end_ms),
            credits_marker_start_ms: Set(entity.credits_marker_start_ms),
            credits_marker_end_ms: Set(entity.credits_marker_end_ms),
        };

        let result = active_model.insert(self.base.db.as_ref()).await?;

        // Broadcast media updated event
        BROKER
            .broadcast(BrokerMessage::Data(DataMessage::MediaUpdated {
                media_id: result.id.clone(),
            }))
            .await;

        Ok(result)
    }

    async fn update(&self, entity: MediaItemModel) -> Result<MediaItemModel> {
        // Manually create ActiveModel with all fields explicitly set to ensure proper update
        // The auto-generated From implementation doesn't properly mark all fields as Set
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
            intro_marker_start_ms: Set(entity.intro_marker_start_ms),
            intro_marker_end_ms: Set(entity.intro_marker_end_ms),
            credits_marker_start_ms: Set(entity.credits_marker_start_ms),
            credits_marker_end_ms: Set(entity.credits_marker_end_ms),
        };

        let result = active_model.update(self.base.db.as_ref()).await?;

        // Broadcast media updated event
        BROKER
            .broadcast(BrokerMessage::Data(DataMessage::MediaUpdated {
                media_id: result.id.clone(),
            }))
            .await;

        Ok(result)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        // First, get the entity details for the event before deleting
        let _entity = self.find_by_id(id).await?;

        MediaItem::delete_by_id(id)
            .exec(self.base.db.as_ref())
            .await?;

        // TODO: Broadcast via MessageBroker when needed
        // if entity.is_some() {
        //     BROKER.notify_media_updated(id.to_string()).await;
        // }

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

    async fn find_by_library_and_type(
        &self,
        library_id: &str,
        media_type: &str,
    ) -> Result<Vec<MediaItemModel>> {
        let start = std::time::Instant::now();
        tracing::info!(
            "[PERF] MediaRepository::find_by_library_and_type: Starting query for library {} type {}",
            library_id,
            media_type
        );

        let result = MediaItem::find()
            .filter(media_items::Column::LibraryId.eq(library_id))
            .filter(media_items::Column::MediaType.eq(media_type))
            .order_by(media_items::Column::SortTitle, Order::Asc)
            .all(self.base.db.as_ref())
            .await?;

        let elapsed = start.elapsed();
        tracing::warn!(
            "[PERF] MediaRepository::find_by_library_and_type: Query completed in {:?} ({} items)",
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

    async fn find_by_source_and_type(
        &self,
        source_id: &str,
        media_type: &str,
    ) -> Result<Vec<MediaItemModel>> {
        Ok(MediaItem::find()
            .filter(media_items::Column::SourceId.eq(source_id))
            .filter(media_items::Column::MediaType.eq(media_type))
            .order_by(media_items::Column::SortTitle, Order::Asc)
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

        // Clone items for event emission before consuming them
        let items_for_event = items.clone();

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
                intro_marker_start_ms: Set(item.intro_marker_start_ms),
                intro_marker_end_ms: Set(item.intro_marker_end_ms),
                credits_marker_start_ms: Set(item.credits_marker_start_ms),
                credits_marker_end_ms: Set(item.credits_marker_end_ms),
            })
            .collect();

        MediaItem::insert_many(active_models)
            .exec(self.base.db.as_ref())
            .await?;

        // Broadcast batch saved event
        BROKER
            .broadcast(BrokerMessage::Data(DataMessage::MediaBatchSaved {
                items: items_for_event,
            }))
            .await;

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

    async fn find_by_source_and_backend_id(
        &self,
        source_id: &str,
        backend_item_id: &str,
    ) -> Result<Option<MediaItemModel>> {
        // The ID in the database is formatted as: source_id:library_id:type:backend_item_id
        // We need to find items that match the pattern: source_id:*:*:backend_item_id
        let pattern = format!("{}:%:%:{}", source_id, backend_item_id);

        Ok(MediaItem::find()
            .filter(media_items::Column::SourceId.eq(source_id))
            .filter(media_items::Column::Id.like(&pattern))
            .one(self.base.db.as_ref())
            .await?)
    }

    async fn delete_by_library(&self, library_id: &str) -> Result<()> {
        use sea_orm::DeleteResult;

        let delete_result: DeleteResult = MediaItem::delete_many()
            .filter(media_items::Column::LibraryId.eq(library_id))
            .exec(self.base.db.as_ref())
            .await?;

        tracing::info!(
            "Deleted {} media items from library {}",
            delete_result.rows_affected,
            library_id
        );
        Ok(())
    }

    async fn delete_by_source(&self, source_id: &str) -> Result<()> {
        use sea_orm::DeleteResult;

        let delete_result: DeleteResult = MediaItem::delete_many()
            .filter(media_items::Column::SourceId.eq(source_id))
            .exec(self.base.db.as_ref())
            .await?;

        tracing::info!(
            "Deleted {} media items from source {}",
            delete_result.rows_affected,
            source_id
        );
        Ok(())
    }

    async fn find_episode_playlist(&self, show_id: &str) -> Result<Vec<MediaItemModel>> {
        Ok(MediaItem::find()
            .filter(media_items::Column::ParentId.eq(show_id))
            .filter(media_items::Column::MediaType.eq("episode"))
            .order_by_asc(media_items::Column::SeasonNumber)
            .order_by_asc(media_items::Column::EpisodeNumber)
            .all(self.base.db.as_ref())
            .await?)
    }

    async fn find_next_episode(
        &self,
        show_id: &str,
        season: i32,
        episode: i32,
    ) -> Result<Option<MediaItemModel>> {
        use sea_orm::Condition;

        Ok(MediaItem::find()
            .filter(media_items::Column::ParentId.eq(show_id))
            .filter(media_items::Column::MediaType.eq("episode"))
            .filter(
                Condition::any()
                    .add(
                        Condition::all()
                            .add(media_items::Column::SeasonNumber.eq(season))
                            .add(media_items::Column::EpisodeNumber.gt(episode)),
                    )
                    .add(media_items::Column::SeasonNumber.gt(season)),
            )
            .order_by_asc(media_items::Column::SeasonNumber)
            .order_by_asc(media_items::Column::EpisodeNumber)
            .one(self.base.db.as_ref())
            .await?)
    }

    async fn find_previous_episode(
        &self,
        show_id: &str,
        season: i32,
        episode: i32,
    ) -> Result<Option<MediaItemModel>> {
        use sea_orm::Condition;

        Ok(MediaItem::find()
            .filter(media_items::Column::ParentId.eq(show_id))
            .filter(media_items::Column::MediaType.eq("episode"))
            .filter(
                Condition::any()
                    .add(
                        Condition::all()
                            .add(media_items::Column::SeasonNumber.eq(season))
                            .add(media_items::Column::EpisodeNumber.lt(episode)),
                    )
                    .add(media_items::Column::SeasonNumber.lt(season)),
            )
            .order_by_desc(media_items::Column::SeasonNumber)
            .order_by_desc(media_items::Column::EpisodeNumber)
            .one(self.base.db.as_ref())
            .await?)
    }

    async fn find_next_unwatched_episode(
        &self,
        show_id: &str,
        after_season: i32,
        after_episode: i32,
    ) -> Result<Option<MediaItemModel>> {
        use crate::db::entities::playback_progress;
        use sea_orm::Condition;

        // First, try to get the next episode after the given one that is unwatched
        let result = MediaItem::find()
            .filter(media_items::Column::ParentId.eq(show_id))
            .filter(media_items::Column::MediaType.eq("episode"))
            .filter(
                Condition::any()
                    .add(
                        Condition::all()
                            .add(media_items::Column::SeasonNumber.eq(after_season))
                            .add(media_items::Column::EpisodeNumber.gt(after_episode)),
                    )
                    .add(media_items::Column::SeasonNumber.gt(after_season)),
            )
            .left_join(playback_progress::Entity)
            .filter(
                Condition::any()
                    .add(playback_progress::Column::Watched.eq(false))
                    .add(playback_progress::Column::Watched.is_null()),
            )
            .order_by_asc(media_items::Column::SeasonNumber)
            .order_by_asc(media_items::Column::EpisodeNumber)
            .one(self.base.db.as_ref())
            .await?;

        Ok(result)
    }

    async fn update_show_watched_count(&self, show_id: &str) -> Result<()> {
        use crate::db::entities::playback_progress;
        use sea_orm::{JoinType, QuerySelect, RelationTrait};

        // Get the show first to ensure it exists and is a show
        let show = match self.find_by_id(show_id).await? {
            Some(s) if s.media_type == "show" => s,
            _ => {
                tracing::debug!("Not updating watched count: {} is not a show", show_id);
                return Ok(());
            }
        };

        // Find all episodes for this show
        let all_episodes = MediaItem::find()
            .filter(media_items::Column::ParentId.eq(show_id))
            .filter(media_items::Column::MediaType.eq("episode"))
            .all(self.base.db.as_ref())
            .await?;

        let total_episode_count = all_episodes.len() as u32;

        if total_episode_count == 0 {
            tracing::debug!(
                "Show {} has no episodes, skipping watched count update",
                show_id
            );
            return Ok(());
        }

        // Count how many episodes are marked as watched in playback_progress
        let mut watched_count = 0u32;
        for episode in &all_episodes {
            let progress = playback_progress::Entity::find()
                .filter(playback_progress::Column::MediaId.eq(&episode.id))
                .filter(playback_progress::Column::Watched.eq(true))
                .one(self.base.db.as_ref())
                .await?;

            if progress.is_some() {
                watched_count += 1;
            }
        }

        // Get current metadata and update watched_episode_count
        let mut metadata = show
            .metadata
            .clone()
            .unwrap_or_else(|| serde_json::json!({}));

        // Update the counts in metadata
        if let Some(obj) = metadata.as_object_mut() {
            obj.insert(
                "watched_episode_count".to_string(),
                serde_json::json!(watched_count),
            );
            obj.insert(
                "total_episode_count".to_string(),
                serde_json::json!(total_episode_count),
            );
        }

        // Update the show in the database
        let mut active_model: MediaItemActiveModel = show.into();
        active_model.metadata = Set(Some(metadata));
        active_model.updated_at = Set(chrono::Utc::now().naive_utc());
        active_model.update(self.base.db.as_ref()).await?;

        tracing::debug!(
            "Updated show {} watched count: {}/{}",
            show_id,
            watched_count,
            total_episode_count
        );

        // Broadcast media updated event so UI refreshes
        BROKER
            .broadcast(BrokerMessage::Data(DataMessage::MediaUpdated {
                media_id: show_id.to_string(),
            }))
            .await;

        Ok(())
    }
}

impl MediaRepositoryImpl {
    pub async fn count_by_library(&self, library_id: &str) -> Result<i64> {
        use sea_orm::PaginatorTrait;

        // First get the library to check its type
        use crate::db::repository::LibraryRepositoryImpl;
        let library_repo = LibraryRepositoryImpl::new(self.base.db.clone());

        let library = library_repo.find_by_id(library_id).await?;

        let mut query = MediaItem::find().filter(media_items::Column::LibraryId.eq(library_id));

        // For TV show libraries, count shows (not episodes)
        // For movie libraries, count movies
        if let Some(lib) = library {
            match lib.library_type.as_str() {
                "shows" => {
                    query = query.filter(media_items::Column::MediaType.eq("show"));
                }
                "movies" => {
                    query = query.filter(media_items::Column::MediaType.eq("movie"));
                }
                _ => {
                    // For other library types, count all items
                }
            }
        }

        let count = query.count(self.base.db.as_ref()).await?;

        tracing::debug!("Counted {} items for library {}", count, library_id);
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

    /// Find media items by library ID and media type with pagination
    pub async fn find_by_library_and_type_paginated(
        &self,
        library_id: &str,
        media_type: &str,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<MediaItemModel>> {
        use sea_orm::PaginatorTrait;

        Ok(MediaItem::find()
            .filter(media_items::Column::LibraryId.eq(library_id))
            .filter(media_items::Column::MediaType.eq(media_type))
            .order_by(media_items::Column::SortTitle, Order::Asc)
            .paginate(self.base.db.as_ref(), limit)
            .fetch_page(offset / limit)
            .await?)
    }
}

#[cfg(test)]
mod tests {
    use crate::db::entities::MediaItemModel;
    use crate::db::repository::{MediaRepository, MediaRepositoryImpl, Repository};
    use anyhow::Result;
    use chrono::Utc;
    use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
    use std::sync::Arc;

    async fn setup_test_repository() -> Result<(Arc<DatabaseConnection>, Arc<MediaRepositoryImpl>)>
    {
        use crate::db::connection::Database;
        use crate::db::entities::libraries::ActiveModel as LibraryActiveModel;
        use crate::db::entities::sources::ActiveModel as SourceActiveModel;
        use tempfile::TempDir;

        // Create temporary directory for test database
        let temp_dir = TempDir::new()?;
        let db_path = temp_dir.path().join("test.db");

        // Need to leak the temp_dir to keep it alive for the test
        let _temp_dir = Box::leak(Box::new(temp_dir));

        // Create database connection
        let db = Database::connect(&db_path).await?;

        // Run migrations
        db.migrate().await?;

        let db_arc = db.get_connection();
        let repo = Arc::new(MediaRepositoryImpl::new(db_arc.clone()));

        // Create test source
        let source = SourceActiveModel {
            id: Set("test-source".to_string()),
            name: Set("Test Source".to_string()),
            source_type: Set("test".to_string()),
            auth_provider_id: Set(None),
            connection_url: Set(None),
            connections: Set(None),
            machine_id: Set(None),
            is_owned: Set(false),
            is_online: Set(true),
            last_sync: Set(None),
            last_connection_test: Set(None),
            connection_failure_count: Set(0),
            connection_quality: Set(None),
            created_at: Set(Utc::now().naive_utc()),
            updated_at: Set(Utc::now().naive_utc()),
        };
        source.insert(db_arc.as_ref()).await?;

        // Create test libraries
        let movie_library = LibraryActiveModel {
            id: Set("test-movie-lib".to_string()),
            source_id: Set("test-source".to_string()),
            title: Set("Movies".to_string()),
            library_type: Set("movie".to_string()),
            icon: Set(Some("video-x-generic".to_string())),
            item_count: Set(0),
            created_at: Set(Utc::now().naive_utc()),
            updated_at: Set(Utc::now().naive_utc()),
        };
        movie_library.insert(db_arc.as_ref()).await?;

        let show_library = LibraryActiveModel {
            id: Set("test-show-lib".to_string()),
            source_id: Set("test-source".to_string()),
            title: Set("TV Shows".to_string()),
            library_type: Set("show".to_string()),
            icon: Set(Some("video-x-generic".to_string())),
            item_count: Set(0),
            created_at: Set(Utc::now().naive_utc()),
            updated_at: Set(Utc::now().naive_utc()),
        };
        show_library.insert(db_arc.as_ref()).await?;

        Ok((db_arc, repo))
    }

    fn create_test_movie(id: &str, title: &str, library_id: &str) -> MediaItemModel {
        use sea_orm::JsonValue;
        MediaItemModel {
            id: id.to_string(),
            source_id: "test-source".to_string(),
            library_id: library_id.to_string(),
            parent_id: None,
            media_type: "movie".to_string(),
            title: title.to_string(),
            sort_title: Some(title.to_lowercase()),
            year: Some(2024),
            duration_ms: Some(7200000), // 2 hours in milliseconds
            rating: Some(8.5),
            genres: Some(JsonValue::from(vec![
                "Action".to_string(),
                "Adventure".to_string(),
            ])),
            poster_url: Some(format!("https://example.com/{}.jpg", id)),
            backdrop_url: Some(format!("https://example.com/{}_backdrop.jpg", id)),
            overview: Some(format!("Summary for {}", title)),
            season_number: None,
            episode_number: None,
            added_at: Some(Utc::now().naive_utc()),
            updated_at: Utc::now().naive_utc(),
            metadata: Some(serde_json::json!({
                "original_title": title,
                "tagline": format!("Tagline for {}", title),
                "studio": "Test Studios",
                "cast": [],
                "crew": [],
                "media_url": format!("https://example.com/{}.mp4", id),
                "container": "mp4",
                "video_codec": "h264",
                "audio_codec": "aac",
                "subtitles_available": ["en"]
            })),
            intro_marker_start_ms: None,
            intro_marker_end_ms: None,
            credits_marker_start_ms: None,
            credits_marker_end_ms: None,
        }
    }

    fn create_test_show(id: &str, title: &str) -> MediaItemModel {
        use sea_orm::JsonValue;
        MediaItemModel {
            id: id.to_string(),
            source_id: "test-source".to_string(),
            library_id: "test-show-lib".to_string(),
            parent_id: None,
            media_type: "show".to_string(),
            title: title.to_string(),
            sort_title: Some(title.to_lowercase()),
            year: Some(2024),
            duration_ms: None,
            rating: Some(8.0),
            genres: Some(JsonValue::from(vec!["Drama".to_string()])),
            poster_url: Some(format!("https://example.com/{}.jpg", id)),
            backdrop_url: Some(format!("https://example.com/{}_backdrop.jpg", id)),
            overview: Some(format!("Summary for show {}", title)),
            season_number: None,
            episode_number: None,
            added_at: Some(Utc::now().naive_utc()),
            updated_at: Utc::now().naive_utc(),
            metadata: Some(serde_json::json!({
                "original_title": title,
                "seasons": 3,
                "studio": "Test Network",
                "cast": [],
                "crew": []
            })),
            intro_marker_start_ms: None,
            intro_marker_end_ms: None,
            credits_marker_start_ms: None,
            credits_marker_end_ms: None,
        }
    }

    fn create_test_episode(
        id: &str,
        show_id: &str,
        title: &str,
        season: i32,
        episode: i32,
    ) -> MediaItemModel {
        MediaItemModel {
            id: id.to_string(),
            source_id: "test-source".to_string(),
            library_id: "test-show-lib".to_string(),
            parent_id: Some(show_id.to_string()),
            media_type: "episode".to_string(),
            title: title.to_string(),
            sort_title: Some(format!("s{:02}e{:02}", season, episode)),
            year: Some(2024),
            duration_ms: Some(2700000), // 45 minutes
            rating: Some(8.2),
            genres: None,
            poster_url: Some(format!("https://example.com/{}.jpg", id)),
            backdrop_url: None,
            overview: Some(format!("Episode {} summary", title)),
            season_number: Some(season),
            episode_number: Some(episode),
            added_at: Some(Utc::now().naive_utc()),
            updated_at: Utc::now().naive_utc(),
            metadata: Some(serde_json::json!({
                "original_title": title,
                "air_date": Utc::now().naive_utc().to_string(),
                "media_url": format!("https://example.com/{}.mp4", id),
                "container": "mp4",
                "video_codec": "h264",
                "audio_codec": "aac",
                "subtitles_available": ["en"]
            })),
            intro_marker_start_ms: None,
            intro_marker_end_ms: None,
            credits_marker_start_ms: None,
            credits_marker_end_ms: None,
        }
    }

    #[tokio::test]
    async fn test_media_repository_crud_operations() -> Result<()> {
        let (_db, repo) = setup_test_repository().await?;

        // Test insert
        let movie = create_test_movie("movie-1", "Test Movie", "test-movie-lib");
        repo.insert(movie.clone()).await?;

        // Test find_by_id
        let found = repo.find_by_id("movie-1").await?;
        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.title, "Test Movie");

        // Test update
        let mut updated_movie = found.clone();
        updated_movie.title = "Updated Movie".to_string();
        repo.update(updated_movie).await?;

        let found = repo.find_by_id("movie-1").await?;
        assert!(found.is_some());
        assert_eq!(found.unwrap().title, "Updated Movie");

        // Test delete
        repo.delete("movie-1").await?;
        let found = repo.find_by_id("movie-1").await?;
        assert!(found.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_find_by_library() -> Result<()> {
        let (_db, repo) = setup_test_repository().await?;

        // Insert test movies
        let movie1 = create_test_movie("movie-1", "Movie One", "test-movie-lib");
        let movie2 = create_test_movie("movie-2", "Movie Two", "test-movie-lib");
        let show = create_test_show("show-1", "Show One");

        repo.insert(movie1).await?;
        repo.insert(movie2).await?;
        repo.insert(show).await?;

        // Test find_by_library for movies
        let movies = repo.find_by_library("test-movie-lib").await?;
        assert_eq!(movies.len(), 2);
        assert!(movies.iter().all(|m| m.library_id == "test-movie-lib"));

        // Test find_by_library for shows
        let shows = repo.find_by_library("test-show-lib").await?;
        assert_eq!(shows.len(), 1);
        assert_eq!(shows[0].title, "Show One");

        Ok(())
    }

    #[tokio::test]
    async fn test_find_by_library_and_type() -> Result<()> {
        let (_db, repo) = setup_test_repository().await?;

        // Insert mixed content
        let movie = create_test_movie("movie-1", "Movie", "test-movie-lib");
        let show = create_test_show("show-1", "Show");
        let episode = create_test_episode("ep-1", "show-1", "Episode 1", 1, 1);

        repo.insert(movie).await?;
        repo.insert(show).await?;
        repo.insert(episode).await?;

        // Test finding movies
        let movies = repo
            .find_by_library_and_type("test-movie-lib", "movie")
            .await?;
        assert_eq!(movies.len(), 1);
        assert_eq!(movies[0].media_type, "movie");

        // Test finding shows
        let shows = repo
            .find_by_library_and_type("test-show-lib", "show")
            .await?;
        assert_eq!(shows.len(), 1);
        assert_eq!(shows[0].media_type, "show");

        // Test finding episodes
        let episodes = repo
            .find_by_library_and_type("test-show-lib", "episode")
            .await?;
        assert_eq!(episodes.len(), 1);
        assert_eq!(episodes[0].media_type, "episode");

        Ok(())
    }

    #[tokio::test]
    async fn test_search() -> Result<()> {
        let (_db, repo) = setup_test_repository().await?;

        // Insert test data
        let movie1 = create_test_movie("movie-1", "The Matrix", "test-movie-lib");
        let movie2 = create_test_movie("movie-2", "Inception", "test-movie-lib");
        let movie3 = create_test_movie("movie-3", "The Dark Knight", "test-movie-lib");

        repo.insert(movie1).await?;
        repo.insert(movie2).await?;
        repo.insert(movie3).await?;

        // Test search
        let results = repo.search("matrix").await?;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "The Matrix");

        let results = repo.search("the").await?;
        assert_eq!(results.len(), 2); // "The Matrix" and "The Dark Knight"

        let results = repo.search("inception").await?;
        assert_eq!(results.len(), 1);

        Ok(())
    }

    #[tokio::test]
    async fn test_find_recently_added() -> Result<()> {
        let (_db, repo) = setup_test_repository().await?;

        // Add movies with slight delays to ensure ordering
        let movie1 = create_test_movie("movie-1", "Old Movie", "test-movie-lib");
        repo.insert(movie1).await?;

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let movie2 = create_test_movie("movie-2", "Recent Movie", "test-movie-lib");
        repo.insert(movie2).await?;

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let movie3 = create_test_movie("movie-3", "Newest Movie", "test-movie-lib");
        repo.insert(movie3).await?;

        // Test find_recently_added
        let recent = repo.find_recently_added(2).await?;
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].title, "Newest Movie");
        assert_eq!(recent[1].title, "Recent Movie");

        Ok(())
    }

    #[tokio::test]
    async fn test_find_episodes_by_show() -> Result<()> {
        let (_db, repo) = setup_test_repository().await?;

        // Create show and episodes
        let show = create_test_show("show-1", "Test Show");
        repo.insert(show).await?;

        let ep1 = create_test_episode("ep-1", "show-1", "Pilot", 1, 1);
        let ep2 = create_test_episode("ep-2", "show-1", "Episode 2", 1, 2);
        let ep3 = create_test_episode("ep-3", "show-1", "Season Finale", 1, 10);

        repo.insert(ep1).await?;
        repo.insert(ep2).await?;
        repo.insert(ep3).await?;

        // Test find_episodes_by_show
        let episodes = repo.find_episodes_by_show("show-1").await?;
        assert_eq!(episodes.len(), 3);
        assert!(
            episodes
                .iter()
                .all(|e| e.parent_id == Some("show-1".to_string()))
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_find_episodes_by_season() -> Result<()> {
        let (_db, repo) = setup_test_repository().await?;

        // Create show and episodes across seasons
        let show = create_test_show("show-1", "Test Show");
        repo.insert(show).await?;

        let s1e1 = create_test_episode("s1e1", "show-1", "S1E1", 1, 1);
        let s1e2 = create_test_episode("s1e2", "show-1", "S1E2", 1, 2);
        let s2e1 = create_test_episode("s2e1", "show-1", "S2E1", 2, 1);

        repo.insert(s1e1).await?;
        repo.insert(s1e2).await?;
        repo.insert(s2e1).await?;

        // Test finding season 1 episodes
        let season1 = repo.find_episodes_by_season("show-1", 1).await?;
        assert_eq!(season1.len(), 2);
        assert!(season1.iter().all(|e| e.season_number == Some(1)));

        // Test finding season 2 episodes
        let season2 = repo.find_episodes_by_season("show-1", 2).await?;
        assert_eq!(season2.len(), 1);
        assert_eq!(season2[0].season_number, Some(2));

        Ok(())
    }

    #[tokio::test]
    async fn test_bulk_insert() -> Result<()> {
        let (_db, repo) = setup_test_repository().await?;

        // Create multiple movies
        let movies = vec![
            create_test_movie("bulk-1", "Bulk Movie 1", "test-movie-lib"),
            create_test_movie("bulk-2", "Bulk Movie 2", "test-movie-lib"),
            create_test_movie("bulk-3", "Bulk Movie 3", "test-movie-lib"),
        ];

        // Test bulk_insert
        repo.bulk_insert(movies).await?;

        // Verify all were inserted
        let found1 = repo.find_by_id("bulk-1").await?;
        let found2 = repo.find_by_id("bulk-2").await?;
        let found3 = repo.find_by_id("bulk-3").await?;

        assert!(found1.is_some());
        assert!(found2.is_some());
        assert!(found3.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_update_metadata() -> Result<()> {
        let (_db, repo) = setup_test_repository().await?;

        // Insert a movie
        let movie = create_test_movie("movie-1", "Test Movie", "test-movie-lib");
        repo.insert(movie).await?;

        // Update metadata
        let new_metadata = serde_json::json!({
            "custom_field": "custom_value",
            "rating_details": {
                "imdb": 8.5,
                "rotten_tomatoes": 92
            }
        });

        repo.update_metadata("movie-1", new_metadata.clone())
            .await?;

        // Verify metadata was updated
        let found = repo.find_by_id("movie-1").await?;
        assert!(found.is_some());
        assert_eq!(found.unwrap().metadata, Some(new_metadata));

        Ok(())
    }

    #[tokio::test]
    async fn test_find_next_and_previous_episode() -> Result<()> {
        let (_db, repo) = setup_test_repository().await?;

        // Create show and episodes
        let show = create_test_show("show-1", "Test Show");
        repo.insert(show).await?;

        let s1e1 = create_test_episode("s1e1", "show-1", "S1E1", 1, 1);
        let s1e2 = create_test_episode("s1e2", "show-1", "S1E2", 1, 2);
        let s1e3 = create_test_episode("s1e3", "show-1", "S1E3", 1, 3);
        let s2e1 = create_test_episode("s2e1", "show-1", "S2E1", 2, 1);

        repo.insert(s1e1).await?;
        repo.insert(s1e2).await?;
        repo.insert(s1e3).await?;
        repo.insert(s2e1).await?;

        // Test find_next_episode
        let next = repo.find_next_episode("show-1", 1, 2).await?;
        assert!(next.is_some());
        assert_eq!(next.unwrap().episode_number, Some(3));

        // Test next episode across seasons
        let next = repo.find_next_episode("show-1", 1, 3).await?;
        assert!(next.is_some());
        let next_ep = next.unwrap();
        assert_eq!(next_ep.season_number, Some(2));
        assert_eq!(next_ep.episode_number, Some(1));

        // Test find_previous_episode
        let prev = repo.find_previous_episode("show-1", 1, 2).await?;
        assert!(prev.is_some());
        let prev_ep = prev.unwrap();
        assert_eq!(prev_ep.episode_number, Some(1));

        // Test previous episode across seasons
        let prev = repo.find_previous_episode("show-1", 2, 1).await?;
        assert!(prev.is_some());
        let prev_ep = prev.unwrap();
        assert_eq!(prev_ep.season_number, Some(1));
        assert_eq!(prev_ep.episode_number, Some(3));

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_by_library() -> Result<()> {
        let (_db, repo) = setup_test_repository().await?;

        // Insert movies in different libraries
        let movie1 = create_test_movie("movie-1", "Movie 1", "test-movie-lib");
        let movie2 = create_test_movie("movie-2", "Movie 2", "test-movie-lib");
        let show = create_test_show("show-1", "Show 1");

        repo.insert(movie1).await?;
        repo.insert(movie2).await?;
        repo.insert(show).await?;

        // Delete all movies in movie library
        repo.delete_by_library("test-movie-lib").await?;

        // Verify movies are deleted but show remains
        let movies = repo.find_by_library("test-movie-lib").await?;
        assert_eq!(movies.len(), 0);

        let shows = repo.find_by_library("test-show-lib").await?;
        assert_eq!(shows.len(), 1);

        Ok(())
    }

    #[tokio::test]
    async fn test_find_by_genre() -> Result<()> {
        let (_db, repo) = setup_test_repository().await?;

        // Insert movies with different genres
        let mut action_movie = create_test_movie("movie-1", "Action Movie", "test-movie-lib");
        action_movie.genres = Some(sea_orm::JsonValue::from(vec![
            "Action".to_string(),
            "Thriller".to_string(),
        ]));

        let mut comedy_movie = create_test_movie("movie-2", "Comedy Movie", "test-movie-lib");
        comedy_movie.genres = Some(sea_orm::JsonValue::from(vec!["Comedy".to_string()]));

        let mut action_comedy = create_test_movie("movie-3", "Action Comedy", "test-movie-lib");
        action_comedy.genres = Some(sea_orm::JsonValue::from(vec![
            "Action".to_string(),
            "Comedy".to_string(),
        ]));

        repo.insert(action_movie).await?;
        repo.insert(comedy_movie).await?;
        repo.insert(action_comedy).await?;

        // Test finding by genre
        let action_movies = repo.find_by_genre("Action").await?;
        assert_eq!(action_movies.len(), 2);

        let comedy_movies = repo.find_by_genre("Comedy").await?;
        assert_eq!(comedy_movies.len(), 2);

        let thriller_movies = repo.find_by_genre("Thriller").await?;
        assert_eq!(thriller_movies.len(), 1);

        Ok(())
    }
}
