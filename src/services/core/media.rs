use anyhow::{Context, Result};
use sea_orm::{ConnectionTrait, TransactionTrait};
use tracing::{debug, error, info, warn};

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
            item_count: 0, // Will be updated during sync
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

        // Convert to entity
        let entity = convert_media_item_to_entity(item.clone(), library_id, source_id)?;

        if repo.find_by_id(&entity.id).await?.is_some() {
            repo.update(entity).await?;
            debug!("Updated media item: {}", item.id());
        } else {
            repo.insert(entity).await?;
            debug!("Created media item: {}", item.id());
        }

        Ok(())
    }

    /// Batch save media items in a transaction
    pub async fn save_media_items_batch(
        db: &DatabaseConnection,
        items: Vec<MediaItem>,
        library_id: &LibraryId,
        source_id: &SourceId,
    ) -> Result<()> {
        let txn = db.begin().await?;

        for item in items {
            // Use the db connection directly for now
            // TODO: Properly implement transaction support
            Self::save_media_item(db, item, library_id, source_id).await?;
        }

        txn.commit().await?;
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

        Ok(())
    }
}

/// Helper function to convert MediaItem to entity
fn convert_media_item_to_entity(
    item: MediaItem,
    library_id: &LibraryId,
    source_id: &SourceId,
) -> Result<MediaItemModel> {
    let (title, year, duration_ms, rating, poster_url, backdrop_url, overview, genres, media_type) =
        match &item {
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
                None,
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
                None,
                Some(episode.duration.as_millis() as i64),
                None,
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
                photo.thumbnail_url.clone(),
                photo.full_url.clone(),
                None, // Photos don't have description
                None,
                "photo".to_string(),
            ),
        };

    // Extract parent show ID for episodes
    let parent_id = match &item {
        MediaItem::Episode(episode) => episode.show_id.clone(),
        _ => None,
    };

    // Extract season and episode numbers
    let (season_number, episode_number) = match &item {
        MediaItem::Episode(episode) => (
            Some(episode.season_number as i32),
            Some(episode.episode_number as i32),
        ),
        _ => (None, None),
    };

    Ok(MediaItemModel {
        id: item.id().to_string(),
        source_id: source_id.to_string(),
        library_id: library_id.to_string(),
        title: title.clone(),
        year,
        media_type,
        duration_ms,
        rating,
        poster_url,
        backdrop_url,
        overview,
        genres,
        parent_id,
        season_number,
        episode_number,
        sort_title: Some(title),
        added_at: Some(chrono::Utc::now().naive_utc()),
        updated_at: chrono::Utc::now().naive_utc(),
        metadata: serde_json::to_value(&item).ok(),
    })
}
