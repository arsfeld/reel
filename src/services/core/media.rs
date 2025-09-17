use anyhow::{Context, Result};
use sea_orm::ConnectionTrait;
use tracing::{debug, error, info, warn};
// Import the mapper extension trait for MediaItem::to_model()
use crate::mapper::media_item_mapper;

use crate::db::{
    connection::DatabaseConnection,
    entities::{LibraryModel, MediaItemModel},
    repository::{
        LibraryRepository, LibraryRepositoryImpl, MediaRepository, MediaRepositoryImpl,
        PlaybackRepository, PlaybackRepositoryImpl, Repository, SourceRepositoryImpl,
        source_repository::SourceRepository,
    },
};
use crate::models::{Library, LibraryId, MediaItem, MediaItemId, MediaType, ShowId, SourceId};
use crate::services::cache_keys::CacheKey;

/// Pure functions for media operations
/// No state, no Arc<Self>, just functions that operate on data
pub struct MediaService;

impl MediaService {
    /// Get all libraries (optionally filtered by source)
    pub async fn get_libraries(db: &DatabaseConnection) -> Result<Vec<Library>> {
        let repo = LibraryRepositoryImpl::new(db.clone());
        let models = repo
            .find_all()
            .await
            .context("Failed to get libraries from database")?;

        models
            .into_iter()
            .map(|m| m.try_into())
            .collect::<Result<Vec<Library>, _>>()
            .context("Failed to convert library models")
    }

    /// Get all libraries for a specific source
    pub async fn get_libraries_for_source(
        db: &DatabaseConnection,
        source_id: &SourceId,
    ) -> Result<Vec<Library>> {
        let repo = LibraryRepositoryImpl::new(db.clone());
        let models = repo
            .find_by_source(&source_id.to_string())
            .await
            .context("Failed to get libraries from database")?;

        models
            .into_iter()
            .map(|m| m.try_into())
            .collect::<Result<Vec<Library>, _>>()
            .context("Failed to convert library models")
    }

    /// Get a specific library by ID
    pub async fn get_library(
        db: &DatabaseConnection,
        library_id: &LibraryId,
    ) -> Result<Option<Library>> {
        let repo = LibraryRepositoryImpl::new(db.clone());
        let model = repo
            .find_by_id(&library_id.to_string())
            .await
            .context("Failed to get library from database")?;

        match model {
            Some(m) => Ok(Some(m.try_into()?)),
            None => Ok(None),
        }
    }

    /// Save or update a library
    pub async fn save_library(
        db: &DatabaseConnection,
        library: Library,
        source_id: &SourceId,
    ) -> Result<()> {
        let repo = LibraryRepositoryImpl::new(db.clone());

        // Convert to entity
        let entity = LibraryModel {
            id: library.id.clone(),
            source_id: source_id.to_string(),
            title: library.title.clone(),
            library_type: format!("{:?}", library.library_type).to_lowercase(),
            icon: library.icon.clone(),
            item_count: library.item_count,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };

        if repo.find_by_id(&library.id).await?.is_some() {
            repo.update(entity).await?;
            debug!("Updated library: {}", library.id);
        } else {
            repo.insert(entity).await?;
            debug!("Created library: {}", library.id);
        }

        Ok(())
    }

    /// Get media items for a library with filtering options
    pub async fn get_media_items(
        db: &DatabaseConnection,
        library_id: &LibraryId,
        media_type: Option<MediaType>,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<MediaItem>> {
        let repo = MediaRepositoryImpl::new(db.clone());

        // Use paginated query if no media type filter is needed
        let items = if media_type.is_none() {
            // Use efficient pagination at database level
            repo.find_by_library_paginated(&library_id.to_string(), offset as u64, limit as u64)
                .await
                .context("Failed to get paginated media items from database")?
        } else {
            // For filtered queries, we still need to get all items and filter manually
            // TODO: Add media type filtering at database level for better performance
            let all_items = repo
                .find_by_library(&library_id.to_string())
                .await
                .context("Failed to get media items from database")?;

            all_items
                .into_iter()
                .filter(|item| match &media_type {
                    Some(MediaType::Movie) => item.media_type == "movie",
                    Some(MediaType::Show) => item.media_type == "show",
                    Some(MediaType::Music) => {
                        item.media_type == "album" || item.media_type == "track"
                    }
                    Some(MediaType::Photo) => item.media_type == "photo",
                    None => true,
                })
                .skip(offset as usize)
                .take(limit as usize)
                .collect()
        };

        // Convert to domain models
        items
            .into_iter()
            .map(|model| model.try_into())
            .collect::<Result<Vec<MediaItem>, _>>()
            .context("Failed to convert media items")
    }

    /// Get a specific media item
    pub async fn get_media_item(
        db: &DatabaseConnection,
        item_id: &MediaItemId,
    ) -> Result<Option<MediaItem>> {
        let repo = MediaRepositoryImpl::new(db.clone());
        let model = repo
            .find_by_id(&item_id.to_string())
            .await
            .context("Failed to get media item from database")?;

        match model {
            Some(m) => Ok(Some(m.try_into()?)),
            None => Ok(None),
        }
    }

    /// Get detailed information about a media item (same as get_media_item but returns Result<MediaItem> for command pattern)
    pub async fn get_item_details(
        db: &DatabaseConnection,
        item_id: &MediaItemId,
    ) -> Result<MediaItem> {
        let item = Self::get_media_item(db, item_id).await?;
        item.ok_or_else(|| anyhow::anyhow!("Media item not found: {}", item_id))
    }

    /// Save or update a media item
    pub async fn save_media_item(
        db: &DatabaseConnection,
        item: MediaItem,
        library_id: &LibraryId,
        source_id: &SourceId,
    ) -> Result<()> {
        let repo = MediaRepositoryImpl::new(db.clone());
        let playback_repo = PlaybackRepositoryImpl::new(db.clone());

        // Convert to entity using the new mapper
        let entity = item.to_model(source_id.as_str(), Some(library_id.to_string()));

        if repo.find_by_id(&entity.id).await?.is_some() {
            repo.update(entity).await?;
            debug!("Updated media item: {}", item.id());
        } else {
            repo.insert(entity).await?;
            debug!("Created media item: {}", item.id());
        }

        // Save playback progress if the item has been watched
        match &item {
            MediaItem::Movie(movie) => {
                if movie.watched || movie.view_count > 0 || movie.playback_position.is_some() {
                    let position_ms = movie
                        .playback_position
                        .map(|d| d.as_millis() as i64)
                        .unwrap_or(0);
                    let duration_ms = movie.duration.as_millis() as i64;

                    // Check if we already have playback progress for this item
                    if let Some(mut existing) = playback_repo.find_by_media_id(&movie.id).await? {
                        // Update existing record with latest data from backend
                        existing.watched = movie.watched;
                        existing.view_count = movie.view_count as i32;
                        if position_ms > 0 {
                            existing.position_ms = position_ms;
                        }
                        existing.duration_ms = duration_ms;
                        existing.last_watched_at = movie.last_watched_at.map(|dt| dt.naive_utc());
                        playback_repo.update(existing).await?;
                    } else {
                        // Create new playback progress
                        let progress = crate::db::entities::PlaybackProgressModel {
                            id: 0, // Will be auto-generated
                            media_id: movie.id.clone(),
                            user_id: None,
                            position_ms,
                            duration_ms,
                            watched: movie.watched,
                            view_count: movie.view_count as i32,
                            last_watched_at: movie.last_watched_at.map(|dt| dt.naive_utc()),
                            updated_at: chrono::Utc::now().naive_utc(),
                        };
                        playback_repo.insert(progress).await?;
                    }
                }
            }
            MediaItem::Episode(episode) => {
                if episode.watched || episode.view_count > 0 || episode.playback_position.is_some()
                {
                    let position_ms = episode
                        .playback_position
                        .map(|d| d.as_millis() as i64)
                        .unwrap_or(0);
                    let duration_ms = episode.duration.as_millis() as i64;

                    // Check if we already have playback progress for this item
                    if let Some(mut existing) = playback_repo.find_by_media_id(&episode.id).await? {
                        // Update existing record with latest data from backend
                        existing.watched = episode.watched;
                        existing.view_count = episode.view_count as i32;
                        if position_ms > 0 {
                            existing.position_ms = position_ms;
                        }
                        existing.duration_ms = duration_ms;
                        existing.last_watched_at = episode.last_watched_at.map(|dt| dt.naive_utc());
                        playback_repo.update(existing).await?;
                    } else {
                        // Create new playback progress
                        let progress = crate::db::entities::PlaybackProgressModel {
                            id: 0, // Will be auto-generated
                            media_id: episode.id.clone(),
                            user_id: None,
                            position_ms,
                            duration_ms,
                            watched: episode.watched,
                            view_count: episode.view_count as i32,
                            last_watched_at: episode.last_watched_at.map(|dt| dt.naive_utc()),
                            updated_at: chrono::Utc::now().naive_utc(),
                        };
                        playback_repo.insert(progress).await?;
                    }
                }
            }
            _ => {
                // Other media types don't have watched status
            }
        }

        Ok(())
    }

    /// Batch save media items
    pub async fn save_media_items_batch(
        db: &DatabaseConnection,
        items: Vec<MediaItem>,
        library_id: &LibraryId,
        source_id: &SourceId,
    ) -> Result<()> {
        info!(
            "Saving batch of {} media items to library {} for source {}",
            items.len(),
            library_id,
            source_id
        );

        if items.is_empty() {
            debug!("No items to save, returning early");
            return Ok(());
        }

        // Process items without holding a transaction lock
        // This prevents blocking other database operations during sync
        for (index, item) in items.iter().enumerate() {
            debug!(
                "Saving item {}/{}: {} ({})",
                index + 1,
                items.len(),
                item.id(),
                match item {
                    MediaItem::Movie(m) => &m.title,
                    MediaItem::Show(s) => &s.title,
                    MediaItem::Episode(e) => &e.title,
                    MediaItem::MusicAlbum(a) => &a.title,
                    MediaItem::MusicTrack(t) => &t.title,
                    MediaItem::Photo(p) => &p.title,
                }
            );

            Self::save_media_item(db, item.clone(), library_id, source_id).await?;
        }

        info!("Successfully saved {} media items", items.len());
        Ok(())
    }

    /// Search media items
    pub async fn search_media(
        db: &DatabaseConnection,
        query: &str,
        library_id: Option<&LibraryId>,
        media_type: Option<MediaType>,
    ) -> Result<Vec<MediaItem>> {
        let repo = MediaRepositoryImpl::new(db.clone());

        // TODO: Add library-specific search to repository
        // For now, search all and filter if needed
        let all_results = repo
            .search(query)
            .await
            .context("Failed to search media items")?;

        let filtered = all_results
            .into_iter()
            .filter(|item| {
                // Filter by library if specified
                if let Some(lib_id) = library_id {
                    if item.library_id != lib_id.to_string() {
                        return false;
                    }
                }
                // Filter by media type if specified
                if let Some(media_type) = &media_type {
                    let matches = match media_type {
                        MediaType::Movie => item.media_type == "movie",
                        MediaType::Show => item.media_type == "show",
                        MediaType::Music => {
                            item.media_type == "album" || item.media_type == "track"
                        }
                        MediaType::Photo => item.media_type == "photo",
                    };
                    if !matches {
                        return false;
                    }
                }
                true
            })
            .collect::<Vec<_>>();

        // Convert to MediaItem
        filtered
            .into_iter()
            .map(|model| model.try_into())
            .collect::<Result<Vec<MediaItem>, _>>()
            .context("Failed to convert search results")
    }

    /// Get recently added media
    pub async fn get_recently_added(db: &DatabaseConnection, limit: u32) -> Result<Vec<MediaItem>> {
        let repo = MediaRepositoryImpl::new(db.clone());
        let models = repo
            .find_recently_added(limit as usize)
            .await
            .context("Failed to get recently added media")?;

        models
            .into_iter()
            .map(|model| model.try_into())
            .collect::<Result<Vec<MediaItem>, _>>()
            .context("Failed to convert media items")
    }

    /// Get trending items (currently uses recently added as a proxy)
    pub async fn get_trending(db: &DatabaseConnection, limit: u32) -> Result<Vec<MediaItem>> {
        // TODO: Implement proper trending algorithm based on watch history
        // For now, use recently added items as trending
        Self::get_recently_added(db, limit).await
    }

    /// Get playback progress for a media item
    pub async fn get_playback_progress(
        db: &DatabaseConnection,
        media_id: &str,
    ) -> Result<Option<crate::db::entities::PlaybackProgressModel>> {
        let playback_repo = PlaybackRepositoryImpl::new(db.clone());
        playback_repo
            .find_by_media_id(media_id)
            .await
            .context("Failed to get playback progress")
    }

    /// Get playback progress for multiple media items in batch
    pub async fn get_playback_progress_batch(
        db: &DatabaseConnection,
        media_ids: &[String],
    ) -> Result<std::collections::HashMap<String, crate::db::entities::PlaybackProgressModel>> {
        use crate::db::entities::{PlaybackProgress, playback_progress};
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

        let progress_records = PlaybackProgress::find()
            .filter(playback_progress::Column::MediaId.is_in(media_ids.to_vec()))
            .all(db.as_ref())
            .await
            .context("Failed to fetch playback progress batch")?;

        let mut progress_map = std::collections::HashMap::new();
        for record in progress_records {
            progress_map.insert(record.media_id.clone(), record);
        }

        Ok(progress_map)
    }

    /// Get continue watching items
    pub async fn get_continue_watching(
        db: &DatabaseConnection,
        limit: u32,
    ) -> Result<Vec<MediaItem>> {
        let playback_repo = PlaybackRepositoryImpl::new(db.clone());
        let media_repo = MediaRepositoryImpl::new(db.clone());

        // Get items with progress
        let progress_items = playback_repo
            .find_in_progress(None) // Use None for single-user system
            .await
            .context("Failed to get in-progress items")?;

        // Fetch the full media items
        let mut items = Vec::new();
        for progress in progress_items.into_iter().take(limit as usize) {
            if let Some(model) = media_repo.find_by_id(&progress.media_id).await? {
                items.push(model.try_into()?);
            }
        }

        Ok(items)
    }

    /// Get episodes for a show, optionally filtered by season
    pub async fn get_episodes_for_show(
        db: &DatabaseConnection,
        show_id: &ShowId,
        season_number: Option<u32>,
    ) -> Result<Vec<MediaItem>> {
        let repo = MediaRepositoryImpl::new(db.clone());

        let models = if let Some(season) = season_number {
            // Get episodes for specific season
            repo.find_episodes_by_season(show_id.as_str(), season as i32)
                .await
                .context("Failed to get episodes from database")?
        } else {
            // Get all episodes for the show
            repo.find_episodes_by_show(show_id.as_str())
                .await
                .context("Failed to get episodes from database")?
        };

        // Convert models to MediaItem::Episode
        let mut episodes = Vec::new();
        for model in models {
            match MediaItem::try_from(model) {
                Ok(item) => episodes.push(item),
                Err(e) => {
                    warn!("Failed to convert episode model: {}", e);
                }
            }
        }

        Ok(episodes)
    }

    /// Clear all media for a library
    pub async fn clear_library(db: &DatabaseConnection, library_id: &LibraryId) -> Result<()> {
        let repo = MediaRepositoryImpl::new(db.clone());
        repo.delete_by_library(&library_id.to_string())
            .await
            .context("Failed to clear library")
    }

    /// Clear all data for a source
    pub async fn clear_source(db: &DatabaseConnection, source_id: &SourceId) -> Result<()> {
        // Use repositories without transactions for now - they handle their own transactions
        let media_repo = MediaRepositoryImpl::new(db.clone());
        media_repo.delete_by_source(&source_id.to_string()).await?;

        let lib_repo = LibraryRepositoryImpl::new(db.clone());
        lib_repo.delete_by_source(&source_id.to_string()).await?;

        Ok(())
    }

    /// Update playback progress for a media item
    pub async fn update_playback_progress(
        db: &DatabaseConnection,
        media_id: &MediaItemId,
        position_ms: i64,
        duration_ms: i64,
        watched: bool,
    ) -> Result<()> {
        let repo = PlaybackRepositoryImpl::new(db.clone());

        // Use None for user_id for now (single-user system)
        if watched {
            repo.mark_watched(&media_id.to_string(), None).await?;
        } else {
            repo.upsert_progress(&media_id.to_string(), None, position_ms, duration_ms)
                .await?;
        }

        // Also sync progress to the backend server in a fire-and-forget manner
        // Parse source ID from media ID (format: "source_id:item_id")
        let media_id_str = media_id.to_string();
        if let Some(colon_pos) = media_id_str.find(':') {
            let source_id = media_id_str[..colon_pos].to_string();
            let media_id_clone = media_id.clone();
            let db_clone = db.clone();
            let position_ms_clone = position_ms;
            let duration_ms_clone = duration_ms;

            // Spawn a detached task to sync with backend
            tokio::spawn(async move {
                use crate::services::core::backend::BackendService;
                use std::time::Duration;

                let position = Duration::from_millis(position_ms_clone as u64);
                let duration = Duration::from_millis(duration_ms_clone as u64);

                if let Err(e) = BackendService::update_playback_progress(
                    &db_clone,
                    &source_id,
                    &media_id_clone,
                    position,
                    duration,
                )
                .await
                {
                    debug!("Failed to sync playback progress to backend: {}", e);
                }
            });
        }

        Ok(())
    }
}
