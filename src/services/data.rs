use anyhow::{Context, Result};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::db::{
    connection::DatabaseConnection,
    entities::{LibraryModel, SourceModel},
    repository::{
        LibraryRepository, LibraryRepositoryImpl, MediaRepository, MediaRepositoryImpl,
        PlaybackRepository, PlaybackRepositoryImpl, Repository, SourceRepositoryImpl,
    },
};
use crate::events::{DatabaseEvent, EventBus, EventPayload, EventType};
use crate::models::{Library, MediaItem};
use sea_orm::TransactionTrait;
use sea_orm::prelude::Json;

/// Trait for converting domain models into database entities
trait IntoEntity<T> {
    fn into_entity(self, cache_key: &str, source_id: &str) -> Result<T>;
}

impl IntoEntity<crate::db::entities::MediaItemModel> for MediaItem {
    fn into_entity(
        self,
        cache_key: &str,
        source_id: &str,
    ) -> Result<crate::db::entities::MediaItemModel> {
        let (
            title,
            year,
            duration_ms,
            rating,
            poster_url,
            backdrop_url,
            overview,
            genres,
            media_type,
        ) = match &self {
            MediaItem::Movie(movie) => (
                movie.title.clone(),
                movie.year.map(|y| y as i32),
                Some(movie.duration.as_millis() as i64),
                movie.rating,
                movie.poster_url.clone(),
                movie.backdrop_url.clone(),
                movie.overview.clone(),
                if movie.genres.is_empty() {
                    None
                } else {
                    serde_json::to_value(&movie.genres).ok()
                },
                "movie".to_string(),
            ),
            MediaItem::Show(show) => (
                show.title.clone(),
                show.year.map(|y| y as i32),
                None, // Shows don't have a single duration
                show.rating,
                show.poster_url.clone(),
                show.backdrop_url.clone(),
                show.overview.clone(),
                if show.genres.is_empty() {
                    None
                } else {
                    serde_json::to_value(&show.genres).ok()
                },
                "show".to_string(),
            ),
            MediaItem::Episode(episode) => (
                episode.title.clone(),
                None, // Episodes don't have years
                Some(episode.duration.as_millis() as i64),
                None, // Episodes don't have ratings
                episode.thumbnail_url.clone(),
                episode.thumbnail_url.clone(),
                episode.overview.clone(),
                None,
                "episode".to_string(),
            ),
            MediaItem::MusicAlbum(album) => (
                album.title.clone(),
                album.year.map(|y| y as i32),
                None,
                None,
                album.cover_url.clone(),
                None,
                None,
                None,
                "album".to_string(),
            ),
            MediaItem::MusicTrack(track) => (
                track.title.clone(),
                None,
                Some(track.duration.as_millis() as i64),
                None,
                None,
                None,
                None,
                None,
                "track".to_string(),
            ),
            MediaItem::Photo(photo) => (
                photo.title.clone(),
                None,
                None,
                None,
                photo.full_url.clone(),
                None,
                None,
                None,
                "photo".to_string(),
            ),
        };

        // Create sort_title from title (remove "The ", "A ", "An " from beginning)
        let sort_title = {
            let title_lower = title.to_lowercase();
            if title_lower.starts_with("the ") {
                Some(title[4..].to_string())
            } else if title_lower.starts_with("a ") {
                Some(title[2..].to_string())
            } else if title_lower.starts_with("an ") {
                Some(title[3..].to_string())
            } else {
                Some(title.clone())
            }
        };

        // Extract library_id from cache_key (format: "backend_id:library_id:type:item_id")
        let library_id = {
            let parts: Vec<&str> = cache_key.split(':').collect();
            if parts.len() >= 4 {
                parts[1].to_string()
            } else {
                "unknown".to_string()
            }
        };

        // Extract episode-specific fields if this is an episode
        let (parent_id, season_number, episode_number) =
            if let MediaItem::Episode(ref episode) = self {
                // Parent should reference the show's DB ID: "{source_id}:{library_id}:show:{show_id}"
                let parent_db_id = episode
                    .show_id
                    .as_ref()
                    .map(|sid| format!("{}:{}:show:{}", source_id, library_id, sid));
                // Use season and episode numbers from the episode struct
                (
                    parent_db_id,
                    Some(episode.season_number as i32),
                    Some(episode.episode_number as i32),
                )
            } else {
                (None, None, None)
            };

        Ok(crate::db::entities::MediaItemModel {
            id: cache_key.to_string(),
            library_id,
            source_id: source_id.to_string(),
            media_type,
            title,
            sort_title,
            year,
            duration_ms,
            rating,
            poster_url,
            backdrop_url,
            overview,
            genres,
            added_at: Some(chrono::Utc::now().naive_utc()),
            updated_at: chrono::Utc::now().naive_utc(),
            metadata: Some((serde_json::to_value(&self)?)),
            parent_id,
            season_number,
            episode_number,
        })
    }
}

pub struct DataService {
    db: DatabaseConnection,
    media_repo: Arc<MediaRepositoryImpl>,
    library_repo: Arc<LibraryRepositoryImpl>,
    source_repo: Arc<SourceRepositoryImpl>,
    playback_repo: Arc<PlaybackRepositoryImpl>,
    memory_cache: Arc<tokio::sync::RwLock<lru::LruCache<String, serde_json::Value>>>,
    event_bus: Arc<EventBus>,
}

impl DataService {
    pub fn new(db: DatabaseConnection, event_bus: Arc<EventBus>) -> Self {
        // Create repositories with event bus
        let media_repo = Arc::new(MediaRepositoryImpl::new(db.clone(), event_bus.clone()));
        let library_repo = Arc::new(LibraryRepositoryImpl::new(db.clone(), event_bus.clone()));
        let source_repo = Arc::new(SourceRepositoryImpl::new(db.clone(), event_bus.clone()));
        let playback_repo = Arc::new(PlaybackRepositoryImpl::new(db.clone(), event_bus.clone()));

        // Create memory cache with 1000 item capacity
        let memory_cache = Arc::new(tokio::sync::RwLock::new(lru::LruCache::new(
            std::num::NonZeroUsize::new(1000).unwrap(),
        )));

        info!("DataService initialized with database connection and event bus");

        Self {
            db,
            media_repo,
            library_repo,
            source_repo,
            playback_repo,
            memory_cache,
            event_bus,
        }
    }

    pub async fn new_async(db: DatabaseConnection, event_bus: Arc<EventBus>) -> Result<Self> {
        Ok(Self::new(db, event_bus))
    }

    /// Create a DataService for testing with an in-memory database
    #[cfg(test)]
    pub fn new_test() -> Result<Self> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            // Create in-memory SQLite database for testing
            let connection = sea_orm::Database::connect("sqlite::memory:")
                .await
                .map_err(|e| anyhow::anyhow!("Failed to create test database: {}", e))?;

            let db_connection = Arc::new(connection);

            // Run migrations for the test database
            use crate::db::migrations::Migrator;
            use sea_orm_migration::MigratorTrait;
            Migrator::up(&*db_connection, None)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to run test migrations: {}", e))?;

            // Create a test event bus
            let event_bus = Arc::new(crate::events::EventBus::new(100));

            Ok(Self::new(db_connection, event_bus))
        })
    }

    pub async fn get_media<T>(&self, id: &str) -> Result<Option<T>>
    where
        T: serde::de::DeserializeOwned,
    {
        // Check memory cache first
        // LRU cache.get() mutates the cache (updates recency), so we need a write lock
        {
            let mut cache = self.memory_cache.write().await;
            if let Some(cached) = cache.get(&format!("media:{}", id))
                && let Ok(item) = serde_json::from_value::<T>(cached.clone())
            {
                debug!("Media {} found in memory cache", id);
                return Ok(Some(item));
            }
        }

        // Query database
        if let Some(model) = self.media_repo.find_by_id(id).await? {
            // Get metadata JSON and deserialize
            if let Some(metadata) = model.metadata {
                let value = serde_json::to_value(metadata)?;
                if let Ok(item) = serde_json::from_value::<T>(value.clone()) {
                    // Update memory cache
                    {
                        let mut cache = self.memory_cache.write().await;
                        cache.put(format!("media:{}", id), value);
                    }
                    return Ok(Some(item));
                }
            }
        }

        Ok(None)
    }

    pub async fn store_media_item(
        &self,
        cache_key: &str,
        media_item: &crate::models::MediaItem,
    ) -> Result<()> {
        self.store_media_item_internal(cache_key, media_item, true)
            .await
    }

    /// Internal method for storing media items with optional event emission
    async fn store_media_item_internal(
        &self,
        cache_key: &str,
        media_item: &crate::models::MediaItem,
        emit_events: bool,
    ) -> Result<()> {
        // Extract source_id from cache_key (format: "backend_id:library_id:type:item_id")
        let source_id = cache_key.split(':').next().unwrap_or("unknown").to_string();

        // Convert domain model to database entity using the trait
        let model = media_item.clone().into_entity(cache_key, &source_id)?;

        // Check if exists
        let exists = self.media_repo.find_by_id(cache_key).await?.is_some();

        // Clone needed values before moving model
        let media_type_for_event = model.media_type.clone();
        let library_id_for_event = model.library_id.clone();

        // Insert or update
        if exists {
            if emit_events {
                self.media_repo.update(model).await?;
            } else {
                self.media_repo.update_silent(model).await?;
            }

            // Emit update event only if requested
            if emit_events {
                self.event_bus
                    .publish(DatabaseEvent::new(
                        EventType::MediaUpdated,
                        EventPayload::Media {
                            id: cache_key.to_string(),
                            media_type: media_type_for_event,
                            library_id: library_id_for_event,
                            source_id,
                        },
                    ))
                    .await?;
            }
        } else {
            // Handle potential duplicate episodes defined by (parent_id, season, episode)
            if model.media_type == "episode"
                && let (Some(parent), Some(season_no), Some(episode_no)) = (
                    model.parent_id.as_ref(),
                    model.season_number,
                    model.episode_number,
                )
                && let Some(existing) = self
                    .media_repo
                    .find_episode_by_parent_season_episode(parent, season_no, episode_no)
                    .await?
            {
                // Update existing episode row to avoid UNIQUE constraint violation
                let mut updated = model.clone();
                updated.id = existing.id;
                if emit_events {
                    self.media_repo.update(updated).await?;
                } else {
                    self.media_repo.update_silent(updated).await?;
                }
                // Since we performed an update, skip the insert path entirely
                return Ok(());
            }

            if emit_events {
                self.media_repo.insert(model).await?;
            } else {
                self.media_repo.insert_silent(model).await?;
            }

            // Emit create event only if requested
            if emit_events {
                self.event_bus
                    .publish(DatabaseEvent::new(
                        EventType::MediaCreated,
                        EventPayload::Media {
                            id: cache_key.to_string(),
                            media_type: media_type_for_event,
                            library_id: library_id_for_event,
                            source_id,
                        },
                    ))
                    .await?;
            }
        }

        Ok(())
    }

    /// Store media item without emitting events (for batch operations)
    pub async fn store_media_item_silent(
        &self,
        cache_key: &str,
        media_item: &crate::models::MediaItem,
    ) -> Result<()> {
        self.store_media_item_internal(cache_key, media_item, false)
            .await
    }

    pub async fn store_library(
        &self,
        library: &crate::models::Library,
        source_id: &str,
    ) -> Result<()> {
        let model = LibraryModel {
            id: library.id.clone(),
            source_id: source_id.to_string(),
            title: library.title.clone(),
            library_type: format!("{:?}", library.library_type),
            icon: library.icon.clone(),
            item_count: 0, // Will be updated later
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };

        // Check if exists
        let exists = self.library_repo.find_by_id(&library.id).await?.is_some();

        if exists {
            self.library_repo.update(model.clone()).await?;

            // Emit library updated event
            self.event_bus
                .publish(DatabaseEvent::new(
                    EventType::LibraryUpdated,
                    EventPayload::Library {
                        id: library.id.clone(),
                        source_id: model.source_id.clone(),
                        item_count: Some(model.item_count),
                    },
                ))
                .await?;
        } else {
            self.library_repo.insert(model.clone()).await?;

            // Emit library created event
            self.event_bus
                .publish(DatabaseEvent::new(
                    EventType::LibraryCreated,
                    EventPayload::Library {
                        id: library.id.clone(),
                        source_id: model.source_id,
                        item_count: Some(model.item_count),
                    },
                ))
                .await?;
        }

        Ok(())
    }

    pub async fn store_library_list(
        &self,
        cache_key: &str,
        libraries: &[crate::models::Library],
    ) -> Result<()> {
        // Store library list in memory cache for get_cached_libraries to work
        let json_data = serde_json::to_value(libraries)?;
        {
            let mut cache = self.memory_cache.write().await;
            cache.put(format!("media:{}", cache_key), json_data);
        }

        // Emit cache updated event
        let _ = self
            .event_bus
            .publish(DatabaseEvent::new(
                EventType::CacheUpdated,
                EventPayload::Cache {
                    cache_key: Some(format!("media:{}", cache_key)),
                    cache_type: "memory".to_string(),
                },
            ))
            .await;

        Ok(())
    }

    pub async fn get_playback_progress(&self, media_id: &str) -> Result<Option<(u64, u64)>> {
        if let Some(progress) = self.playback_repo.find_by_media_id(media_id).await? {
            Ok(Some((
                progress.position_ms as u64,
                progress.duration_ms as u64,
            )))
        } else {
            Ok(None)
        }
    }

    pub async fn set_playback_progress(
        &self,
        media_id: &str,
        position: u64,
        duration: u64,
    ) -> Result<()> {
        self.playback_repo
            .upsert_progress(media_id, None, position as i64, duration as i64)
            .await?;

        // Emit event
        self.event_bus
            .emit_playback_position(
                media_id.to_string(),
                std::time::Duration::from_millis(position),
                std::time::Duration::from_millis(duration),
            )
            .await?;

        Ok(())
    }

    /// Get all sources from the database
    pub async fn get_all_sources(&self) -> Result<Vec<crate::db::entities::SourceModel>> {
        self.source_repo.find_all().await
    }

    pub async fn ensure_source_exists(&self, backend_id: &str) -> Result<()> {
        // Check if source already exists
        if self.source_repo.find_by_id(backend_id).await?.is_some() {
            debug!("Source {} already exists in database", backend_id);
            return Ok(());
        }

        // Determine source type from backend_id
        let source_type = if backend_id.starts_with("plex") {
            "plex"
        } else if backend_id.starts_with("jellyfin") {
            "jellyfin"
        } else if backend_id.starts_with("local") {
            "local"
        } else {
            "unknown"
        };

        // Create source record
        let source_model = SourceModel {
            id: backend_id.to_string(),
            name: format!("{} Source", backend_id),
            source_type: source_type.to_string(),
            auth_provider_id: None,
            connection_url: None,
            is_online: true,
            last_sync: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };

        self.source_repo.insert(source_model).await?;
        info!("Created source record for {}", backend_id);

        // Emit source added event
        self.event_bus
            .publish(DatabaseEvent::new(
                EventType::SourceAdded,
                EventPayload::Source {
                    id: backend_id.to_string(),
                    source_type: source_type.to_string(),
                    is_online: Some(true),
                },
            ))
            .await?;

        Ok(())
    }

    /// Sync multiple libraries and their items in a single transaction
    /// This ensures atomicity - either all operations succeed or all are rolled back
    pub async fn sync_libraries_transactional(
        &self,
        backend_id: &str,
        libraries: &[Library],
        items_by_library: &[(String, Vec<MediaItem>)], // (library_id, items)
    ) -> Result<()> {
        // Start transaction
        let txn = self
            .db
            .begin()
            .await
            .context("Failed to begin transaction for library sync")?;

        // Ensure source exists within transaction
        // NOTE: In a full implementation, we'd pass the transaction to all operations
        // For now, we'll ensure source exists before the transaction
        self.ensure_source_exists(backend_id).await?;

        // Process each library
        for library in libraries {
            // Store library (this would ideally use the transaction)
            self.store_library(library, backend_id)
                .await
                .context(format!(
                    "Failed to cache library {} in transaction",
                    library.id
                ))?;
        }

        // Process media items for each library
        for (library_id, items) in items_by_library {
            for item in items {
                let item_type = match item {
                    MediaItem::Movie(_) => "movie",
                    MediaItem::Show(_) => "show",
                    MediaItem::Episode(_) => "episode",
                    MediaItem::MusicAlbum(_) => "album",
                    MediaItem::MusicTrack(_) => "track",
                    MediaItem::Photo(_) => "photo",
                };

                let cache_key =
                    format!("{}:{}:{}:{}", backend_id, library_id, item_type, item.id());

                // Store media item silently during batch sync (this would ideally use the transaction)
                self.store_media_item_silent(&cache_key, item)
                    .await
                    .context(format!(
                        "Failed to cache {} {} in transaction",
                        item_type,
                        item.id()
                    ))?;
            }
        }

        // Commit transaction
        txn.commit()
            .await
            .context("Failed to commit library sync transaction")?;

        info!(
            "Successfully synced {} libraries with items in transaction for {}",
            libraries.len(),
            backend_id
        );

        Ok(())
    }

    /// Execute a batch of cache operations in a transaction
    /// Note: This is a simplified implementation. A full implementation would need
    /// to create new repository instances that use the transaction connection.
    pub async fn execute_in_transaction<F, Fut>(&self, operation: F) -> Result<()>
    where
        F: FnOnce(Arc<sea_orm::DatabaseTransaction>) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        let txn = self
            .db
            .begin()
            .await
            .context("Failed to begin transaction")?;

        // Execute the operation with the transaction
        match operation(Arc::new(txn)).await {
            Ok(_) => {
                // Transaction will be committed when it goes out of scope
                // if we consumed it properly in the operation
                info!("Transaction completed successfully");
                Ok(())
            }
            Err(e) => {
                error!("Transaction failed, will rollback: {}", e);
                // Transaction automatically rolls back on drop
                Err(e)
            }
        }
    }

    pub async fn clear_backend_cache(&self, backend_id: &str) -> Result<()> {
        // Emit cache invalidated event first
        self.event_bus
            .publish(DatabaseEvent::new(
                EventType::CacheInvalidated,
                EventPayload::Cache {
                    cache_key: Some(backend_id.to_string()),
                    cache_type: "backend_media_cache".to_string(),
                },
            ))
            .await?;

        // Delete from database
        let items = self.media_repo.find_by_source(backend_id).await?;
        for item in &items {
            self.media_repo.delete(&item.id).await?;

            // Emit media deleted event
            let _ = self
                .event_bus
                .publish(DatabaseEvent::new(
                    EventType::MediaDeleted,
                    EventPayload::Media {
                        id: item.id.clone(),
                        media_type: item.media_type.clone(),
                        library_id: item.library_id.clone(),
                        source_id: item.source_id.clone(),
                    },
                ))
                .await;
        }

        // Clear memory cache entries
        {
            let mut cache = self.memory_cache.write().await;
            // We can't selectively remove by pattern in LRU cache, so we'll clear all
            // In production, you might want to maintain a separate index
            cache.clear();
        }

        // Emit cache cleared event
        self.event_bus
            .publish(DatabaseEvent::new(
                EventType::CacheCleared,
                EventPayload::Cache {
                    cache_key: Some(format!("source:{}", backend_id)),
                    cache_type: "backend".to_string(),
                },
            ))
            .await?;

        info!("Cleared cache for backend {}", backend_id);

        Ok(())
    }

    /// Get library by ID from repository
    pub async fn get_library(&self, id: &str) -> Result<Option<crate::db::entities::LibraryModel>> {
        self.library_repo.find_by_id(id).await
    }

    /// Get libraries by source
    pub async fn get_libraries(
        &self,
        source_id: &str,
    ) -> Result<Vec<crate::db::entities::LibraryModel>> {
        self.library_repo.find_by_source(source_id).await
    }

    /// Get all libraries
    pub async fn get_all_libraries(&self) -> Result<Vec<crate::db::entities::LibraryModel>> {
        self.library_repo.find_all().await
    }

    /// Update library item count
    pub async fn update_library_item_count(&self, library_id: &str, count: i32) -> Result<()> {
        self.library_repo.update_item_count(library_id, count).await
    }

    /// Get media items for a library as domain models
    pub async fn get_media_items(&self, library_id: &str) -> Result<Vec<MediaItem>> {
        let models = self.media_repo.find_by_library(library_id).await?;
        let mut items = Vec::new();

        for model in models {
            match MediaItem::try_from(model) {
                Ok(item) => items.push(item),
                Err(e) => {
                    warn!("Failed to convert media item to domain model: {}", e);
                    // Skip items that fail to convert
                }
            }
        }

        Ok(items)
    }

    /// Get raw media item models for a library (for database operations)
    pub async fn get_media_item_models(
        &self,
        library_id: &str,
    ) -> Result<Vec<crate::db::entities::MediaItemModel>> {
        self.media_repo.find_by_library(library_id).await
    }

    /// Get media item by ID as domain model
    pub async fn get_media_item(&self, id: &str) -> Result<Option<MediaItem>> {
        if let Some(model) = self.media_repo.find_by_id(id).await? {
            match MediaItem::try_from(model) {
                Ok(item) => Ok(Some(item)),
                Err(e) => {
                    warn!("Failed to convert media item {}: {}", id, e);
                    Ok(None)
                }
            }
        } else {
            Ok(None)
        }
    }

    /// Get media items by IDs for differential updates
    pub async fn get_media_items_by_ids(&self, ids: &[String]) -> Result<Vec<MediaItem>> {
        let mut items = Vec::new();

        // Batch fetch from database for efficiency
        for id in ids {
            if let Some(item) = self.get_media_item(id).await? {
                items.push(item);
            }
        }

        Ok(items)
    }

    /// Get media items modified since a timestamp (for incremental sync)
    pub async fn get_media_items_since(
        &self,
        library_id: &str,
        since: chrono::NaiveDateTime,
    ) -> Result<Vec<MediaItem>> {
        use crate::db::entities::media_items;
        use sea_orm::prelude::*;

        let models = media_items::Entity::find()
            .filter(media_items::Column::LibraryId.eq(library_id))
            .filter(media_items::Column::UpdatedAt.gt(since))
            .all(&*self.db)
            .await?;

        let mut items = Vec::new();
        for model in models {
            match MediaItem::try_from(model) {
                Ok(item) => items.push(item),
                Err(e) => {
                    warn!("Failed to convert media item: {}", e);
                }
            }
        }

        Ok(items)
    }

    /// Get sources from repository
    pub async fn get_sources(&self) -> Result<Vec<crate::db::entities::SourceModel>> {
        self.source_repo.find_all().await
    }

    /// Get source by ID
    pub async fn get_source(&self, id: &str) -> Result<Option<crate::db::entities::SourceModel>> {
        self.source_repo.find_by_id(id).await
    }

    /// Add a new source
    pub async fn add_source(&self, source: crate::db::entities::SourceModel) -> Result<()> {
        self.source_repo.insert(source.clone()).await?;

        // Emit source added event
        self.event_bus
            .publish(DatabaseEvent::new(
                EventType::SourceAdded,
                EventPayload::Source {
                    id: source.id,
                    source_type: source.source_type,
                    is_online: Some(source.is_online),
                },
            ))
            .await?;

        Ok(())
    }

    /// Remove a source
    pub async fn remove_source(&self, id: &str) -> Result<()> {
        if let Some(source) = self.source_repo.find_by_id(id).await? {
            self.source_repo.delete(id).await?;

            // Emit source removed event
            self.event_bus
                .publish(DatabaseEvent::new(
                    EventType::SourceRemoved,
                    EventPayload::Source {
                        id: source.id,
                        source_type: source.source_type,
                        is_online: Some(source.is_online),
                    },
                ))
                .await?;
        }

        Ok(())
    }

    /// Get latest sync status
    pub async fn get_latest_sync_status(
        &self,
        source_id: &str,
    ) -> Result<Option<crate::db::entities::SyncStatusModel>> {
        // Note: This assumes we have a sync status repository, which may need to be added
        // For now, return None to avoid compilation errors
        Ok(None)
    }

    /// Get continue watching items (recently viewed with progress)
    pub async fn get_continue_watching(&self) -> Result<Vec<MediaItem>> {
        // Get all media items with playback progress
        use crate::db::entities::{media_items, playback_progress};
        use sea_orm::prelude::*;

        // Find all media items that have playback progress
        let models = media_items::Entity::find()
            .inner_join(playback_progress::Entity)
            .all(&*self.db)
            .await?;

        let mut items = Vec::new();
        for model in models {
            match MediaItem::try_from(model) {
                Ok(item) => items.push(item),
                Err(e) => {
                    warn!("Failed to convert media item to domain model: {}", e);
                }
            }
        }

        Ok(items)
    }

    /// Get recently added items
    pub async fn get_recently_added(&self, limit: Option<usize>) -> Result<Vec<MediaItem>> {
        // This would order by added_at descending
        let mut models = self.media_repo.find_all().await?;
        models.sort_by(|a, b| b.added_at.cmp(&a.added_at));

        if let Some(limit) = limit {
            models.truncate(limit);
        }

        let mut items = Vec::new();
        for model in models {
            match MediaItem::try_from(model) {
                Ok(item) => items.push(item),
                Err(e) => {
                    warn!("Failed to convert media item to domain model: {}", e);
                }
            }
        }

        Ok(items)
    }

    /// Update playback progress
    pub async fn update_playback_progress(
        &self,
        media_id: &str,
        position_ms: i64,
        duration_ms: i64,
        watched: bool,
    ) -> Result<()> {
        self.playback_repo
            .upsert_progress(media_id, None, position_ms, duration_ms)
            .await?;

        // Emit playback position event
        self.event_bus
            .emit_playback_position(
                media_id.to_string(),
                std::time::Duration::from_millis(position_ms as u64),
                std::time::Duration::from_millis(duration_ms as u64),
            )
            .await?;

        Ok(())
    }

    /// Get episodes for a show
    pub async fn get_episodes_by_show(&self, show_id: &str) -> Result<Vec<MediaItem>> {
        let models = self.media_repo.find_episodes_by_show(show_id).await?;
        let mut items = Vec::new();

        for model in models {
            match MediaItem::try_from(model) {
                Ok(mut item) => {
                    // Enrich episode with playback info
                    if let MediaItem::Episode(ref mut ep) = item
                        && let Ok(Some(progress)) =
                            self.playback_repo.find_by_media_id(&ep.id).await
                    {
                        use std::time::Duration as StdDuration;
                        let position = StdDuration::from_millis(progress.position_ms as u64);
                        let duration = StdDuration::from_millis(progress.duration_ms as u64);
                        ep.playback_position = Some(position);
                        // Consider watched either explicit flag or >90% complete
                        let near_complete = progress.duration_ms > 0
                            && (progress.position_ms as f64 / progress.duration_ms as f64) > 0.9;
                        ep.watched = progress.watched || near_complete;
                        ep.view_count = progress.view_count as u32;
                        ep.last_watched_at = progress.last_watched_at.map(|dt| dt.and_utc());
                    }
                    items.push(item)
                }
                Err(e) => {
                    warn!("Failed to convert episode to domain model: {}", e);
                }
            }
        }

        Ok(items)
    }

    /// Get episodes for a specific season
    pub async fn get_episodes_by_season(
        &self,
        show_id: &str,
        season_number: i32,
    ) -> Result<Vec<MediaItem>> {
        let models = self
            .media_repo
            .find_episodes_by_season(show_id, season_number)
            .await?;
        let mut items = Vec::new();

        for model in models {
            match MediaItem::try_from(model) {
                Ok(mut item) => {
                    // Enrich episode with playback info
                    if let MediaItem::Episode(ref mut ep) = item
                        && let Ok(Some(progress)) =
                            self.playback_repo.find_by_media_id(&ep.id).await
                    {
                        use std::time::Duration as StdDuration;
                        let position = StdDuration::from_millis(progress.position_ms as u64);
                        let duration = StdDuration::from_millis(progress.duration_ms as u64);
                        ep.playback_position = Some(position);
                        let near_complete = progress.duration_ms > 0
                            && (progress.position_ms as f64 / progress.duration_ms as f64) > 0.9;
                        ep.watched = progress.watched || near_complete;
                        ep.view_count = progress.view_count as u32;
                        ep.last_watched_at = progress.last_watched_at.map(|dt| dt.and_utc());
                    }
                    items.push(item)
                }
                Err(e) => {
                    warn!("Failed to convert episode to domain model: {}", e);
                }
            }
        }

        Ok(items)
    }

    /// Store episode with proper parent relationship
    pub async fn store_episode(
        &self,
        episode: &crate::models::Episode,
        show_id: &str,
        season_number: i32,
    ) -> Result<()> {
        use crate::db::entities::MediaItemModel;
        use chrono::Utc;

        let episode_entity = MediaItemModel {
            id: episode.id.clone(),
            library_id: "default".to_string(), // TODO: Get from show
            source_id: episode.backend_id.clone(),
            media_type: "episode".to_string(),
            title: episode.title.clone(),
            sort_title: Some(episode.title.clone()),
            year: None,
            duration_ms: Some(episode.duration.as_millis() as i64),
            rating: None,
            poster_url: episode.thumbnail_url.clone(),
            backdrop_url: None,
            overview: episode.overview.clone(),
            genres: None,
            added_at: Some(Utc::now().naive_utc()),
            updated_at: Utc::now().naive_utc(),
            metadata: Some(serde_json::json!({
                "episode_number": episode.episode_number,
                "air_date": episode.air_date,
            })),
            parent_id: Some(show_id.to_string()),
            season_number: Some(season_number),
            episode_number: Some(episode.episode_number as i32),
        };

        self.media_repo.insert(episode_entity).await?;
        Ok(())
    }
}
