use anyhow::{Context, Result};
use tracing::{info, warn};

use crate::db::{
    connection::DatabaseConnection,
    entities::LibraryModel,
    repository::{
        LibraryRepository, LibraryRepositoryImpl, MediaRepository, MediaRepositoryImpl,
        PlaybackRepository, PlaybackRepositoryImpl, Repository,
    },
};
use crate::models::{Library, LibraryId, MediaItem, MediaItemId, MediaType, ShowId, SourceId};

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
            .find_by_source(source_id.as_ref())
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
            .find_by_id(library_id.as_ref())
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
        } else {
            repo.insert(entity).await?;
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
            repo.find_by_library_paginated(library_id.as_ref(), offset as u64, limit as u64)
                .await
                .context("Failed to get paginated media items from database")?
        } else {
            // For filtered queries, we still need to get all items and filter manually
            // TODO: Add media type filtering at database level for better performance
            let all_items = repo
                .find_by_library(library_id.as_ref())
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
            .find_by_id(item_id.as_ref())
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

        let existing = repo.find_by_id(&entity.id).await?;
        if existing.is_some() {
            let _result = repo.update(entity.clone()).await?;

            // Verify update for Shows
            if entity.media_type == "show"
                && let Ok(Some(updated)) = repo.find_by_id(&entity.id).await
            {
                if let Some(ref metadata) = updated.metadata {
                    // metadata is already a serde_json::Value
                    if let Some(seasons_value) = metadata.get("seasons") {
                        if serde_json::from_value::<Vec<crate::models::Season>>(
                            seasons_value.clone(),
                        )
                        .is_err()
                        {
                            warn!(
                                "Failed to parse seasons for Show '{}' after update: {:?}",
                                updated.title, seasons_value
                            );
                        }
                    } else {
                        warn!(
                            "No seasons field in metadata for Show '{}' after update",
                            updated.title
                        );
                    }
                } else {
                    warn!("No metadata for Show '{}' after update", updated.title);
                }
            }
        } else {
            repo.insert(entity.clone()).await?;

            // Verify insert for Shows
            if entity.media_type == "show"
                && let Ok(Some(inserted)) = repo.find_by_id(&entity.id).await
            {
                if let Some(ref metadata) = inserted.metadata {
                    // metadata is already a serde_json::Value
                    if let Some(seasons_value) = metadata.get("seasons") {
                        if serde_json::from_value::<Vec<crate::models::Season>>(
                            seasons_value.clone(),
                        )
                        .is_err()
                        {
                            warn!(
                                "Failed to parse seasons for Show '{}' after insert: {:?}",
                                inserted.title, seasons_value
                            );
                        }
                    } else {
                        warn!(
                            "No seasons field in metadata for Show '{}' after insert",
                            inserted.title
                        );
                    }
                } else {
                    warn!("No metadata for Show '{}' after insert", inserted.title);
                }
            }
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
                            play_queue_id: None,
                            play_queue_version: None,
                            play_queue_item_id: None,
                            source_id: None,
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
                            play_queue_id: None,
                            play_queue_version: None,
                            play_queue_item_id: None,
                            source_id: None,
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
            return Ok(());
        }

        // Process items without holding a transaction lock
        // This prevents blocking other database operations during sync
        for (_index, item) in items.iter().enumerate() {
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
                if let Some(lib_id) = library_id
                    && item.library_id != lib_id.to_string()
                {
                    return false;
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
        repo.delete_by_library(library_id.as_ref())
            .await
            .context("Failed to clear library")
    }

    /// Clear all data for a source
    pub async fn clear_source(db: &DatabaseConnection, source_id: &SourceId) -> Result<()> {
        // Use repositories without transactions for now - they handle their own transactions
        let media_repo = MediaRepositoryImpl::new(db.clone());
        media_repo.delete_by_source(source_id.as_ref()).await?;

        let lib_repo = LibraryRepositoryImpl::new(db.clone());
        lib_repo.delete_by_source(source_id.as_ref()).await?;

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
            repo.mark_watched(media_id.as_ref(), None).await?;
        } else {
            repo.upsert_progress(media_id.as_ref(), None, position_ms, duration_ms)
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
                    warn!("Failed to sync playback progress to backend: {}", e);
                }
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::Database;
    use crate::db::entities::{LibraryModel, MediaItemModel};
    use crate::db::repository::{LibraryRepositoryImpl, MediaRepositoryImpl, Repository};
    use crate::models::{Library, LibraryId, LibraryType, MediaType, Movie, SourceId};
    use crate::services::core::media::MediaService;
    use anyhow::Result;
    use chrono::Utc;
    use sea_orm::{ActiveModelTrait, Set};
    use std::time::Duration;
    use tempfile::TempDir;

    async fn setup_test_database() -> Result<Database> {
        use crate::db::entities::sources::ActiveModel as SourceActiveModel;

        // Create temporary directory for test database
        let temp_dir = TempDir::new()?;
        let db_path = temp_dir.path().join("test.db");

        // Leak the temp_dir to keep it alive for the test
        let _temp_dir = Box::leak(Box::new(temp_dir));

        // Create database connection
        let db = Database::connect(&db_path).await?;

        // Run migrations
        db.migrate().await?;

        // Create test source
        let source = SourceActiveModel {
            id: Set("test-source".to_string()),
            name: Set("Test Source".to_string()),
            source_type: Set("plex".to_string()),
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
        source.insert(db.get_connection().as_ref()).await?;

        Ok(db)
    }

    fn create_test_movie_model(id: &str, title: &str, library_id: &str) -> MediaItemModel {
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
            duration_ms: Some(7200000),
            rating: Some(8.0),
            genres: Some(JsonValue::from(vec!["Action".to_string()])),
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
        }
    }

    fn create_test_library(id: &str, title: &str, lib_type: &str) -> Library {
        Library {
            id: id.to_string(),
            title: title.to_string(),
            library_type: match lib_type {
                "movie" => LibraryType::Movies,
                "show" => LibraryType::Shows,
                "music" => LibraryType::Music,
                _ => LibraryType::Movies,
            },
            icon: Some("video-x-generic".to_string()),
            item_count: 0,
        }
    }

    #[tokio::test]
    async fn test_get_libraries() -> Result<()> {
        let db = setup_test_database().await?;
        let db_conn = db.get_connection();

        // Insert test libraries
        let lib_repo = LibraryRepositoryImpl::new(db_conn.clone());

        let lib1 = LibraryModel {
            id: "lib-1".to_string(),
            source_id: "test-source".to_string(),
            title: "Movies".to_string(),
            library_type: "movie".to_string(),
            icon: Some("video-x-generic".to_string()),
            item_count: 10,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };

        let lib2 = LibraryModel {
            id: "lib-2".to_string(),
            source_id: "test-source".to_string(),
            title: "TV Shows".to_string(),
            library_type: "show".to_string(),
            icon: Some("video-x-generic".to_string()),
            item_count: 5,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };

        lib_repo.insert(lib1).await?;
        lib_repo.insert(lib2).await?;

        // Test get_libraries
        let libraries = MediaService::get_libraries(&db_conn).await?;
        assert_eq!(libraries.len(), 2);
        assert!(libraries.iter().any(|l| l.title == "Movies"));
        assert!(libraries.iter().any(|l| l.title == "TV Shows"));

        Ok(())
    }

    #[tokio::test]
    async fn test_get_libraries_for_source() -> Result<()> {
        let db = setup_test_database().await?;
        let db_conn = db.get_connection();

        // Create another source
        use crate::db::entities::sources::ActiveModel as SourceActiveModel;
        let source2 = SourceActiveModel {
            id: Set("test-source-2".to_string()),
            name: Set("Test Source 2".to_string()),
            source_type: Set("jellyfin".to_string()),
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
        source2.insert(db_conn.as_ref()).await?;

        // Insert libraries for different sources
        let lib_repo = LibraryRepositoryImpl::new(db_conn.clone());

        let lib1 = LibraryModel {
            id: "lib-1".to_string(),
            source_id: "test-source".to_string(),
            title: "Source 1 Movies".to_string(),
            library_type: "movie".to_string(),
            icon: Some("video-x-generic".to_string()),
            item_count: 10,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };

        let lib2 = LibraryModel {
            id: "lib-2".to_string(),
            source_id: "test-source-2".to_string(),
            title: "Source 2 Movies".to_string(),
            library_type: "movie".to_string(),
            icon: Some("video-x-generic".to_string()),
            item_count: 5,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };

        lib_repo.insert(lib1).await?;
        lib_repo.insert(lib2).await?;

        // Test get_libraries_for_source
        let source_id = SourceId::from("test-source");
        let libraries = MediaService::get_libraries_for_source(&db_conn, &source_id).await?;
        assert_eq!(libraries.len(), 1);
        assert_eq!(libraries[0].title, "Source 1 Movies");

        let source_id_2 = SourceId::from("test-source-2");
        let libraries_2 = MediaService::get_libraries_for_source(&db_conn, &source_id_2).await?;
        assert_eq!(libraries_2.len(), 1);
        assert_eq!(libraries_2[0].title, "Source 2 Movies");

        Ok(())
    }

    #[tokio::test]
    async fn test_get_library() -> Result<()> {
        let db = setup_test_database().await?;
        let db_conn = db.get_connection();

        // Insert test library
        let lib_repo = LibraryRepositoryImpl::new(db_conn.clone());

        let lib = LibraryModel {
            id: "lib-1".to_string(),
            source_id: "test-source".to_string(),
            title: "Test Library".to_string(),
            library_type: "movie".to_string(),
            icon: Some("video-x-generic".to_string()),
            item_count: 10,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };

        lib_repo.insert(lib).await?;

        // Test get_library - existing
        let lib_id = LibraryId::from("lib-1");
        let library = MediaService::get_library(&db_conn, &lib_id).await?;
        assert!(library.is_some());
        let library = library.unwrap();
        assert_eq!(library.title, "Test Library");
        assert_eq!(library.item_count, 10);

        // Test get_library - non-existent
        let lib_id = LibraryId::from("non-existent");
        let library = MediaService::get_library(&db_conn, &lib_id).await?;
        assert!(library.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_save_library() -> Result<()> {
        let db = setup_test_database().await?;
        let db_conn = db.get_connection();

        // Create and save a library
        let library = create_test_library("new-lib", "New Library", "movie");
        let source_id = SourceId::from("test-source");

        MediaService::save_library(&db_conn, library.clone(), &source_id).await?;

        // Verify it was saved
        let lib_id = LibraryId::from("new-lib");
        let saved = MediaService::get_library(&db_conn, &lib_id).await?;
        assert!(saved.is_some());
        let saved = saved.unwrap();
        assert_eq!(saved.title, "New Library");

        // Test updating the library
        let mut updated_library = library;
        updated_library.title = "Updated Library".to_string();
        updated_library.item_count = 20;

        MediaService::save_library(&db_conn, updated_library, &source_id).await?;

        // Verify it was updated
        let saved = MediaService::get_library(&db_conn, &lib_id).await?;
        assert!(saved.is_some());
        let saved = saved.unwrap();
        assert_eq!(saved.title, "Updated Library");
        assert_eq!(saved.item_count, 20);

        Ok(())
    }

    #[tokio::test]
    async fn test_get_media_items() -> Result<()> {
        let db = setup_test_database().await?;
        let db_conn = db.get_connection();

        // Create library
        let lib_repo = LibraryRepositoryImpl::new(db_conn.clone());
        let lib = LibraryModel {
            id: "test-lib".to_string(),
            source_id: "test-source".to_string(),
            title: "Test Library".to_string(),
            library_type: "movie".to_string(),
            icon: Some("video-x-generic".to_string()),
            item_count: 0,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        lib_repo.insert(lib).await?;

        // Insert test media items
        let media_repo = MediaRepositoryImpl::new(db_conn.clone());

        let movie1 = create_test_movie_model("movie-1", "Movie One", "test-lib");

        let movie2 = create_test_movie_model("movie-2", "Movie Two", "test-lib");

        media_repo.insert(movie1).await?;
        media_repo.insert(movie2).await?;

        // Test get_media_items
        let library_id = LibraryId::from("test-lib");
        let items = MediaService::get_media_items(&db_conn, &library_id, None, 0, 100).await?;
        assert_eq!(items.len(), 2);
        assert!(items.iter().any(|i| i.title() == "Movie One"));
        assert!(items.iter().any(|i| i.title() == "Movie Two"));

        // Test with media type filter
        let items =
            MediaService::get_media_items(&db_conn, &library_id, Some(MediaType::Movie), 0, 100)
                .await?;
        assert_eq!(items.len(), 2);

        // Test with pagination
        let items = MediaService::get_media_items(&db_conn, &library_id, None, 0, 1).await?;
        assert_eq!(items.len(), 1);

        Ok(())
    }

    #[tokio::test]
    async fn test_search_media() -> Result<()> {
        let db = setup_test_database().await?;
        let db_conn = db.get_connection();

        // Create library and media items
        let lib_repo = LibraryRepositoryImpl::new(db_conn.clone());
        let lib = LibraryModel {
            id: "test-lib".to_string(),
            source_id: "test-source".to_string(),
            title: "Test Library".to_string(),
            library_type: "movie".to_string(),
            icon: Some("video-x-generic".to_string()),
            item_count: 0,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        lib_repo.insert(lib).await?;

        let media_repo = MediaRepositoryImpl::new(db_conn.clone());

        let movie1 = create_test_movie_model("movie-1", "The Matrix", "test-lib");

        let movie2 = create_test_movie_model("movie-2", "Inception", "test-lib");

        media_repo.insert(movie1).await?;
        media_repo.insert(movie2).await?;

        // Test search
        let results = MediaService::search_media(&db_conn, "matrix", None, None).await?;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title(), "The Matrix");

        let results = MediaService::search_media(&db_conn, "inception", None, None).await?;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title(), "Inception");

        // Test partial match
        let results = MediaService::search_media(&db_conn, "cep", None, None).await?;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title(), "Inception");

        Ok(())
    }

    #[tokio::test]
    async fn test_get_recently_added() -> Result<()> {
        let db = setup_test_database().await?;
        let db_conn = db.get_connection();

        // Create library and media items with different added_at times
        let lib_repo = LibraryRepositoryImpl::new(db_conn.clone());
        let lib = LibraryModel {
            id: "test-lib".to_string(),
            source_id: "test-source".to_string(),
            title: "Test Library".to_string(),
            library_type: "movie".to_string(),
            icon: Some("video-x-generic".to_string()),
            item_count: 0,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        lib_repo.insert(lib).await?;

        let media_repo = MediaRepositoryImpl::new(db_conn.clone());

        // Create movies with different added times
        for i in 0..5 {
            use sea_orm::JsonValue;
            let mut movie = create_test_movie_model(
                &format!("movie-{}", i),
                &format!("Movie {}", i),
                "test-lib",
            );
            // Override the added_at to test sorting
            movie.added_at = Some(Utc::now().naive_utc() - chrono::Duration::hours(i as i64));
            media_repo.insert(movie).await?;

            // Small delay to ensure ordering
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        // Test get_recently_added
        let recent = MediaService::get_recently_added(&db_conn, 3).await?;
        assert_eq!(recent.len(), 3);
        // Most recent should be first (Movie 0 has the most recent added_at)
        match &recent[0] {
            crate::models::MediaItem::Movie(movie) => assert_eq!(movie.title, "Movie 0"),
            _ => panic!("Expected Movie variant"),
        }
        match &recent[1] {
            crate::models::MediaItem::Movie(movie) => assert_eq!(movie.title, "Movie 1"),
            _ => panic!("Expected Movie variant"),
        }
        match &recent[2] {
            crate::models::MediaItem::Movie(movie) => assert_eq!(movie.title, "Movie 2"),
            _ => panic!("Expected Movie variant"),
        }

        Ok(())
    }
}
