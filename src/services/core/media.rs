use anyhow::{Context, Result};
use tracing::{debug, warn};

use crate::db::{
    connection::DatabaseConnection,
    entities::LibraryModel,
    repository::{
        LibraryRepository, LibraryRepositoryImpl, MediaRepository, MediaRepositoryImpl,
        PeopleRepository, PlaybackRepository, PlaybackRepositoryImpl, Repository,
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

        // Enrich with playback progress data (watch status, position, etc.)
        let enriched_items = Self::enrich_with_playback_progress(db, items).await?;

        // Convert to domain models
        enriched_items
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
        use crate::db::repository::PeopleRepositoryImpl;

        let repo = MediaRepositoryImpl::new(db.clone());
        let model = repo
            .find_by_id(item_id.as_ref())
            .await
            .context("Failed to get media item from database")?;

        match model {
            Some(m) => {
                // Enrich with playback progress data
                let mut enriched_models = Self::enrich_with_playback_progress(db, vec![m]).await?;
                let enriched_model = enriched_models.remove(0);

                // Load people (cast/crew) from people tables
                let people_repo = PeopleRepositoryImpl::new(db.clone());
                let people_with_relations =
                    people_repo.find_by_media_item(item_id.as_ref()).await?;

                // Separate cast and crew based on person_type
                let mut cast = Vec::new();
                let mut crew = Vec::new();

                for (person, media_person) in people_with_relations {
                    let person_obj = crate::models::Person {
                        id: person.id.clone(),
                        name: person.name.clone(),
                        role: media_person.role.clone(),
                        image_url: person.image_url.clone(),
                    };

                    match media_person.person_type.as_str() {
                        "actor" | "cast" => cast.push(person_obj),
                        "director" | "writer" | "producer" | "crew" => crew.push(person_obj),
                        _ => {} // Unknown type, skip
                    }
                }

                // Convert model to MediaItem and inject people data
                let mut item: MediaItem = enriched_model.try_into()?;
                match &mut item {
                    MediaItem::Movie(movie) => {
                        movie.cast = cast;
                        movie.crew = crew;
                    }
                    MediaItem::Show(show) => {
                        show.cast = cast;
                        // Shows don't have crew in the model currently
                    }
                    _ => {} // Episodes and other types don't have cast/crew
                }

                Ok(Some(item))
            }
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
        let mut entity = item.to_model(source_id.as_str(), Some(library_id.to_string()));

        // For episodes, check using the UNIQUE constraint (parent_id, season_number, episode_number)
        // This prevents UNIQUE constraint violations when episode IDs change but the natural key stays the same
        let existing = if entity.media_type == "episode" {
            if let (Some(parent_id), Some(season_num), Some(episode_num)) = (
                &entity.parent_id,
                entity.season_number,
                entity.episode_number,
            ) {
                // First check by natural key (parent_id, season, episode)
                match repo
                    .find_episode_by_parent_season_episode(parent_id, season_num, episode_num)
                    .await?
                {
                    Some(existing_episode) => {
                        // Found an existing episode with same parent/season/episode
                        // Update it and preserve its ID to maintain playback progress references
                        entity.id = existing_episode.id.clone();
                        Some(existing_episode)
                    }
                    None => {
                        // No episode with this parent/season/episode exists, check by ID
                        repo.find_by_id(&entity.id).await?
                    }
                }
            } else {
                // Episode missing required fields, fall back to ID check
                warn!(
                    "Episode missing required fields (parent_id, season_number, episode_number): {}",
                    entity.id
                );
                repo.find_by_id(&entity.id).await?
            }
        } else {
            // For non-episodes, use simple ID-based check
            repo.find_by_id(&entity.id).await?
        };

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

        // Note: Cast/crew are no longer saved during sync (task-388)
        // They are fetched via lazy-loading when user views detail pages
        // This ensures we always get complete metadata, not truncated preview data

        Ok(())
    }

    /// Batch save media items
    pub async fn save_media_items_batch(
        db: &DatabaseConnection,
        items: Vec<MediaItem>,
        library_id: &LibraryId,
        source_id: &SourceId,
    ) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        let repo = MediaRepositoryImpl::new(db.clone());
        let playback_repo = PlaybackRepositoryImpl::new(db.clone());

        // Collect playback progress updates for batch operation
        let mut progress_updates = Vec::new();

        // Process each media item
        for item in items.iter() {
            // Convert to entity using the mapper
            let mut entity = item.to_model(source_id.as_str(), Some(library_id.to_string()));

            // Handle episode natural key lookups (same logic as save_media_item)
            let existing = if entity.media_type == "episode" {
                if let (Some(parent_id), Some(season_num), Some(episode_num)) = (
                    &entity.parent_id,
                    entity.season_number,
                    entity.episode_number,
                ) {
                    match repo
                        .find_episode_by_parent_season_episode(parent_id, season_num, episode_num)
                        .await?
                    {
                        Some(existing_episode) => {
                            entity.id = existing_episode.id.clone();
                            Some(existing_episode)
                        }
                        None => repo.find_by_id(&entity.id).await?,
                    }
                } else {
                    warn!(
                        "Episode missing required fields (parent_id, season_number, episode_number): {}",
                        entity.id
                    );
                    repo.find_by_id(&entity.id).await?
                }
            } else {
                repo.find_by_id(&entity.id).await?
            };

            // Save or update media item
            if existing.is_some() {
                repo.update(entity.clone()).await?;
            } else {
                repo.insert(entity.clone()).await?;
            }

            // Collect playback progress data for batch upsert
            match item {
                MediaItem::Movie(movie) => {
                    if movie.watched || movie.view_count > 0 || movie.playback_position.is_some() {
                        let position_ms = movie
                            .playback_position
                            .map(|d| d.as_millis() as i64)
                            .unwrap_or(0);
                        let duration_ms = movie.duration.as_millis() as i64;

                        progress_updates.push((
                            movie.id.clone(),
                            None, // user_id
                            position_ms,
                            duration_ms,
                            movie.watched,
                            movie.view_count as i32,
                            movie.last_watched_at.map(|dt| dt.naive_utc()),
                        ));
                    }
                }
                MediaItem::Episode(episode) => {
                    if episode.watched
                        || episode.view_count > 0
                        || episode.playback_position.is_some()
                    {
                        let position_ms = episode
                            .playback_position
                            .map(|d| d.as_millis() as i64)
                            .unwrap_or(0);
                        let duration_ms = episode.duration.as_millis() as i64;

                        progress_updates.push((
                            episode.id.clone(),
                            None, // user_id
                            position_ms,
                            duration_ms,
                            episode.watched,
                            episode.view_count as i32,
                            episode.last_watched_at.map(|dt| dt.naive_utc()),
                        ));
                    }
                }
                _ => {
                    // Other media types don't have playback progress
                }
            }
        }

        // Perform batch upsert of playback progress
        if !progress_updates.is_empty() {
            let start = std::time::Instant::now();
            let count = progress_updates.len();

            playback_repo
                .batch_upsert_progress(progress_updates)
                .await?;

            let elapsed = start.elapsed();
            debug!(
                "Batch upsert of {} playback progress records completed in {:?}",
                count, elapsed
            );
        }

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

        // Enrich with playback progress data
        let enriched_models = Self::enrich_with_playback_progress(db, models).await?;

        enriched_models
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
        let mut models = Vec::new();
        for progress in progress_items.into_iter().take(limit as usize) {
            if let Some(model) = media_repo.find_by_id(&progress.media_id).await? {
                models.push(model);
            }
        }

        // Enrich with playback progress data
        let enriched_models = Self::enrich_with_playback_progress(db, models).await?;

        // Convert to MediaItem
        let mut items = Vec::new();
        for model in enriched_models {
            items.push(model.try_into()?);
        }

        Ok(items)
    }

    /// Enrich media item models with playback progress data from the database.
    /// This ensures that watch status comes from playback_progress table (source of truth)
    /// rather than from metadata JSON (backend cache that may be stale).
    async fn enrich_with_playback_progress(
        db: &DatabaseConnection,
        mut models: Vec<crate::db::entities::media_items::Model>,
    ) -> Result<Vec<crate::db::entities::media_items::Model>> {
        use std::collections::HashMap;

        if models.is_empty() {
            return Ok(models);
        }

        // Batch load playback progress for all media items
        let playback_repo = PlaybackRepositoryImpl::new(db.clone());
        let media_ids: Vec<String> = models.iter().map(|m| m.id.clone()).collect();

        // Build a map of media_id -> playback progress
        let mut progress_map: HashMap<String, crate::db::entities::PlaybackProgressModel> =
            HashMap::new();
        for media_id in media_ids {
            if let Some(progress) = playback_repo.find_by_media_id(&media_id).await? {
                progress_map.insert(media_id, progress);
            }
        }

        // Enrich each model's metadata with actual playback progress
        for model in &mut models {
            if let Some(progress) = progress_map.get(&model.id) {
                // Get existing metadata or create new
                let mut metadata = model
                    .metadata
                    .as_ref()
                    .and_then(|v| v.as_object().cloned())
                    .unwrap_or_default();

                // Override watch status with actual data from playback_progress table
                metadata.insert("watched".to_string(), serde_json::json!(progress.watched));
                metadata.insert(
                    "view_count".to_string(),
                    serde_json::json!(progress.view_count),
                );

                if let Some(last_watched) = progress.last_watched_at {
                    metadata.insert(
                        "last_watched_at".to_string(),
                        serde_json::json!(last_watched.and_utc().to_rfc3339()),
                    );
                }

                if progress.position_ms > 0 {
                    metadata.insert(
                        "playback_position_ms".to_string(),
                        serde_json::json!(progress.position_ms as u64),
                    );
                }

                // Update model with enriched metadata
                model.metadata = Some(serde_json::Value::Object(metadata));
            }
        }

        Ok(models)
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

        // Enrich with playback progress data (watch status, position, etc.)
        let enriched_models = Self::enrich_with_playback_progress(db, models).await?;

        // Convert models to MediaItem::Episode
        let mut episodes = Vec::new();
        for model in enriched_models {
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

    /// Helper function to retry backend sync with exponential backoff
    async fn retry_backend_sync<F, Fut>(
        operation_name: &str,
        media_id: &MediaItemId,
        max_retries: u32,
        operation: F,
    ) -> Result<()>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        let mut attempt = 0;
        let mut last_error = None;

        while attempt <= max_retries {
            match operation().await {
                Ok(()) => {
                    debug!(
                        "Successfully synced {} to backend for {} (attempt {}/{})",
                        operation_name,
                        media_id.as_ref(),
                        attempt + 1,
                        max_retries + 1
                    );
                    return Ok(());
                }
                Err(e) => {
                    last_error = Some(e);

                    if attempt < max_retries {
                        // Exponential backoff: 1s, 2s, 4s
                        let delay_ms = 1000 * (1 << attempt);
                        debug!(
                            "Failed to sync {} for {}, retrying in {}ms (attempt {}/{}): {}",
                            operation_name,
                            media_id.as_ref(),
                            delay_ms,
                            attempt + 1,
                            max_retries + 1,
                            last_error.as_ref().unwrap()
                        );
                        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                    }

                    attempt += 1;
                }
            }
        }

        let err = last_error.unwrap();
        warn!(
            "Failed to sync {} to backend for {} after {} attempts: {}",
            operation_name,
            media_id.as_ref(),
            max_retries + 1,
            err
        );
        Err(err)
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
            let watched_clone = watched;

            // Spawn a detached task to sync with backend (with retry logic)
            tokio::spawn(async move {
                use crate::services::core::backend::BackendService;
                use std::time::Duration;

                let operation_name = if watched_clone {
                    "watched status"
                } else {
                    "playback progress"
                };

                // Retry up to 2 times (3 attempts total) with exponential backoff
                let _result = Self::retry_backend_sync(
                    operation_name,
                    &media_id_clone,
                    2, // max_retries
                    || async {
                        if watched_clone {
                            BackendService::mark_watched(&db_clone, &source_id, &media_id_clone)
                                .await
                        } else {
                            let position = Duration::from_millis(position_ms_clone as u64);
                            let duration = Duration::from_millis(duration_ms_clone as u64);
                            BackendService::update_playback_progress(
                                &db_clone,
                                &source_id,
                                &media_id_clone,
                                position,
                                duration,
                            )
                            .await
                        }
                    },
                )
                .await;

                // Result is already logged by retry_backend_sync
            });
        }

        Ok(())
    }

    /// Mark a media item as watched
    pub async fn mark_watched(db: &DatabaseConnection, media_id: &MediaItemId) -> Result<()> {
        let repo = PlaybackRepositoryImpl::new(db.clone());

        // Mark as watched in database (None for user_id for single-user system)
        repo.mark_watched(media_id.as_ref(), None).await?;

        // Look up the media item to get its source_id
        let media_repo = MediaRepositoryImpl::new(db.clone());
        if let Ok(Some(media_item)) = media_repo.find_by_id(media_id.as_ref()).await {
            let source_id = media_item.source_id.clone();
            let media_id_clone = media_id.clone();
            let db_clone = db.clone();

            debug!(
                "Spawning backend sync task for media_id: {} with source_id: {}",
                media_id.as_ref(),
                source_id
            );

            // Sync to backend with retry logic
            tokio::spawn(async move {
                use crate::services::core::backend::BackendService;

                debug!(
                    "Backend sync task running for media_id: {}",
                    media_id_clone.as_ref()
                );

                // Retry up to 2 times (3 attempts total) with exponential backoff
                let _result = Self::retry_backend_sync(
                    "watched status",
                    &media_id_clone,
                    2, // max_retries
                    || async {
                        BackendService::mark_watched(&db_clone, &source_id, &media_id_clone).await
                    },
                )
                .await;

                // Result is already logged by retry_backend_sync
            });
        } else {
            warn!(
                "Could not find media item {} in database to sync watch status",
                media_id.as_ref()
            );
        }

        Ok(())
    }

    /// Mark a media item as unwatched
    pub async fn mark_unwatched(db: &DatabaseConnection, media_id: &MediaItemId) -> Result<()> {
        let repo = PlaybackRepositoryImpl::new(db.clone());

        // Mark as unwatched in database (None for user_id for single-user system)
        repo.mark_unwatched(media_id.as_ref(), None).await?;

        // Look up the media item to get its source_id
        let media_repo = MediaRepositoryImpl::new(db.clone());
        if let Ok(Some(media_item)) = media_repo.find_by_id(media_id.as_ref()).await {
            let source_id = media_item.source_id.clone();
            let media_id_clone = media_id.clone();
            let db_clone = db.clone();

            debug!(
                "Spawning backend sync task for media_id: {} with source_id: {}",
                media_id.as_ref(),
                source_id
            );

            // Sync to backend with retry logic
            tokio::spawn(async move {
                use crate::services::core::backend::BackendService;

                debug!(
                    "Backend sync task running for media_id: {}",
                    media_id_clone.as_ref()
                );

                // Retry up to 2 times (3 attempts total) with exponential backoff
                let _result = Self::retry_backend_sync(
                    "unwatched status",
                    &media_id_clone,
                    2, // max_retries
                    || async {
                        BackendService::mark_unwatched(&db_clone, &source_id, &media_id_clone).await
                    },
                )
                .await;

                // Result is already logged by retry_backend_sync
            });
        } else {
            warn!(
                "Could not find media item {} in database to sync watch status",
                media_id.as_ref()
            );
        }

        Ok(())
    }

    /// Mark all episodes in a show as watched
    pub async fn mark_show_watched(db: &DatabaseConnection, show_id: &ShowId) -> Result<()> {
        // Get all episodes for the show
        let episodes = Self::get_episodes_for_show(db, show_id, None).await?;

        tracing::info!(
            "mark_show_watched: show_id={}, found {} episodes",
            show_id.as_ref(),
            episodes.len()
        );

        // Mark each episode as watched
        for item in episodes {
            if let MediaItem::Episode(episode) = item {
                let media_id = MediaItemId::new(&episode.id);
                tracing::info!(
                    "Marking episode as watched: id={}, title={}",
                    episode.id,
                    episode.title
                );
                Self::mark_watched(db, &media_id).await?;
            }
        }

        Ok(())
    }

    /// Mark all episodes in a show as unwatched
    pub async fn mark_show_unwatched(db: &DatabaseConnection, show_id: &ShowId) -> Result<()> {
        // Get all episodes for the show
        let episodes = Self::get_episodes_for_show(db, show_id, None).await?;

        // Mark each episode as unwatched
        for item in episodes {
            if let MediaItem::Episode(episode) = item {
                let media_id = MediaItemId::new(episode.id);
                Self::mark_unwatched(db, &media_id).await?;
            }
        }

        Ok(())
    }

    /// Mark all episodes in a season as watched
    pub async fn mark_season_watched(
        db: &DatabaseConnection,
        show_id: &ShowId,
        season_number: u32,
    ) -> Result<()> {
        // Get all episodes for the season
        let episodes = Self::get_episodes_for_show(db, show_id, Some(season_number)).await?;

        tracing::info!(
            "mark_season_watched: show_id={}, season={}, found {} episodes",
            show_id.as_ref(),
            season_number,
            episodes.len()
        );

        // Mark each episode as watched
        for item in episodes {
            if let MediaItem::Episode(episode) = item {
                let media_id = MediaItemId::new(&episode.id);
                tracing::info!(
                    "Marking episode as watched: id={}, title={}",
                    episode.id,
                    episode.title
                );
                Self::mark_watched(db, &media_id).await?;
            }
        }

        Ok(())
    }

    /// Mark all episodes in a season as unwatched
    pub async fn mark_season_unwatched(
        db: &DatabaseConnection,
        show_id: &ShowId,
        season_number: u32,
    ) -> Result<()> {
        // Get all episodes for the season
        let episodes = Self::get_episodes_for_show(db, show_id, Some(season_number)).await?;

        // Mark each episode as unwatched
        for item in episodes {
            if let MediaItem::Episode(episode) = item {
                let media_id = MediaItemId::new(episode.id);
                Self::mark_unwatched(db, &media_id).await?;
            }
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
            auth_status: Set("authenticated".to_string()),
            last_auth_check: Set(None),
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
            intro_marker_start_ms: None,
            intro_marker_end_ms: None,
            credits_marker_start_ms: None,
            credits_marker_end_ms: None,
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
            auth_status: Set("authenticated".to_string()),
            last_auth_check: Set(None),
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

    fn create_test_episode_model(
        id: &str,
        parent_id: &str,
        season: i32,
        episode: i32,
        title: &str,
        library_id: &str,
    ) -> MediaItemModel {
        use sea_orm::JsonValue;
        MediaItemModel {
            id: id.to_string(),
            source_id: "test-source".to_string(),
            library_id: library_id.to_string(),
            parent_id: Some(parent_id.to_string()),
            media_type: "episode".to_string(),
            title: title.to_string(),
            sort_title: Some(title.to_lowercase()),
            year: Some(2024),
            duration_ms: Some(2700000), // 45 minutes
            rating: Some(8.5),
            genres: Some(JsonValue::from(vec!["Drama".to_string()])),
            poster_url: Some(format!("https://example.com/{}.jpg", id)),
            backdrop_url: None,
            overview: Some(format!("Episode summary for {}", title)),
            season_number: Some(season),
            episode_number: Some(episode),
            added_at: Some(Utc::now().naive_utc()),
            updated_at: Utc::now().naive_utc(),
            metadata: Some(serde_json::json!({
                "show_title": "Test Show",
                "show_poster_url": "https://example.com/show.jpg",
                "air_date": "2024-01-01",
                "watched": false,
                "view_count": 0
            })),
            intro_marker_start_ms: None,
            intro_marker_end_ms: None,
            credits_marker_start_ms: None,
            credits_marker_end_ms: None,
        }
    }

    fn create_test_episode_domain(
        id: &str,
        parent_id: &str,
        season: u32,
        episode: u32,
        title: &str,
    ) -> crate::models::Episode {
        crate::models::Episode {
            id: id.to_string(),
            backend_id: "test-source".to_string(),
            show_id: Some(parent_id.to_string()),
            title: title.to_string(),
            season_number: season,
            episode_number: episode,
            duration: Duration::from_secs(2700), // 45 minutes
            thumbnail_url: Some(format!("https://example.com/{}.jpg", id)),
            overview: Some(format!("Episode summary for {}", title)),
            air_date: Some(chrono::Utc::now()),
            watched: false,
            view_count: 0,
            last_watched_at: None,
            playback_position: None,
            show_title: Some("Test Show".to_string()),
            show_poster_url: Some("https://example.com/show.jpg".to_string()),
            intro_marker: None,
            credits_marker: None,
        }
    }

    #[tokio::test]
    async fn test_save_episode_insert_new() -> Result<()> {
        let db = setup_test_database().await?;
        let db_conn = db.get_connection();

        // Create library
        let lib_repo = LibraryRepositoryImpl::new(db_conn.clone());
        let lib = LibraryModel {
            id: "test-lib".to_string(),
            source_id: "test-source".to_string(),
            title: "Test Library".to_string(),
            library_type: "show".to_string(),
            icon: Some("video-x-generic".to_string()),
            item_count: 0,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        lib_repo.insert(lib).await?;

        // Create and save an episode
        let episode = create_test_episode_domain("ep-1", "show-1", 1, 1, "Pilot");
        let library_id = LibraryId::from("test-lib");
        let source_id = SourceId::from("test-source");

        MediaService::save_media_item(
            &db_conn,
            MediaItem::Episode(episode.clone()),
            &library_id,
            &source_id,
        )
        .await?;

        // Verify it was saved
        let media_repo = MediaRepositoryImpl::new(db_conn.clone());
        let saved = media_repo.find_by_id("ep-1").await?;
        assert!(saved.is_some());
        let saved = saved.unwrap();
        assert_eq!(saved.title, "Pilot");
        assert_eq!(saved.season_number, Some(1));
        assert_eq!(saved.episode_number, Some(1));
        assert_eq!(saved.parent_id, Some("show-1".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_save_episode_update_same_id() -> Result<()> {
        let db = setup_test_database().await?;
        let db_conn = db.get_connection();

        // Create library
        let lib_repo = LibraryRepositoryImpl::new(db_conn.clone());
        let lib = LibraryModel {
            id: "test-lib".to_string(),
            source_id: "test-source".to_string(),
            title: "Test Library".to_string(),
            library_type: "show".to_string(),
            icon: Some("video-x-generic".to_string()),
            item_count: 0,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        lib_repo.insert(lib).await?;

        // Insert initial episode
        let episode = create_test_episode_domain("ep-1", "show-1", 1, 1, "Pilot");
        let library_id = LibraryId::from("test-lib");
        let source_id = SourceId::from("test-source");

        MediaService::save_media_item(
            &db_conn,
            MediaItem::Episode(episode.clone()),
            &library_id,
            &source_id,
        )
        .await?;

        // Update the same episode with new title
        let mut updated_episode = episode;
        updated_episode.title = "Pilot - Updated".to_string();
        updated_episode.overview = Some("Updated overview".to_string());

        MediaService::save_media_item(
            &db_conn,
            MediaItem::Episode(updated_episode),
            &library_id,
            &source_id,
        )
        .await?;

        // Verify it was updated
        let media_repo = MediaRepositoryImpl::new(db_conn.clone());
        let saved = media_repo.find_by_id("ep-1").await?;
        assert!(saved.is_some());
        let saved = saved.unwrap();
        assert_eq!(saved.title, "Pilot - Updated");
        assert_eq!(saved.overview, Some("Updated overview".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_save_episode_update_different_id_same_natural_key() -> Result<()> {
        // This is the critical test case: episode ID changes but parent_id/season/episode stays the same
        // Without our fix, this would cause a UNIQUE constraint violation
        let db = setup_test_database().await?;
        let db_conn = db.get_connection();

        // Create library
        let lib_repo = LibraryRepositoryImpl::new(db_conn.clone());
        let lib = LibraryModel {
            id: "test-lib".to_string(),
            source_id: "test-source".to_string(),
            title: "Test Library".to_string(),
            library_type: "show".to_string(),
            icon: Some("video-x-generic".to_string()),
            item_count: 0,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        lib_repo.insert(lib).await?;

        // Insert initial episode with ID "ep-old"
        let episode = create_test_episode_domain("ep-old", "show-1", 1, 1, "Pilot");
        let library_id = LibraryId::from("test-lib");
        let source_id = SourceId::from("test-source");

        MediaService::save_media_item(
            &db_conn,
            MediaItem::Episode(episode.clone()),
            &library_id,
            &source_id,
        )
        .await?;

        // Verify initial save
        let media_repo = MediaRepositoryImpl::new(db_conn.clone());
        let initial = media_repo.find_by_id("ep-old").await?;
        assert!(initial.is_some());
        assert_eq!(initial.unwrap().title, "Pilot");

        // Now save an episode with DIFFERENT ID but SAME parent_id/season/episode
        // This simulates a backend regenerating IDs
        let new_episode = create_test_episode_domain("ep-new", "show-1", 1, 1, "Pilot - Updated");

        // This should NOT fail with UNIQUE constraint violation
        // Our fix should detect the existing episode by natural key and update it
        MediaService::save_media_item(
            &db_conn,
            MediaItem::Episode(new_episode),
            &library_id,
            &source_id,
        )
        .await?;

        // Verify the episode was updated (using the OLD ID)
        let saved = media_repo.find_by_id("ep-old").await?;
        assert!(saved.is_some());
        let saved = saved.unwrap();
        assert_eq!(saved.title, "Pilot - Updated");
        assert_eq!(saved.id, "ep-old"); // ID should be preserved
        assert_eq!(saved.parent_id, Some("show-1".to_string()));
        assert_eq!(saved.season_number, Some(1));
        assert_eq!(saved.episode_number, Some(1));

        // Verify the NEW ID doesn't exist as a separate record
        let new_id_result = media_repo.find_by_id("ep-new").await?;
        assert!(new_id_result.is_none());

        // Verify there's only ONE episode with this parent/season/episode combination
        let episodes = media_repo
            .find_episode_by_parent_season_episode("show-1", 1, 1)
            .await?;
        assert!(episodes.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_save_multiple_episodes_same_show() -> Result<()> {
        let db = setup_test_database().await?;
        let db_conn = db.get_connection();

        // Create library
        let lib_repo = LibraryRepositoryImpl::new(db_conn.clone());
        let lib = LibraryModel {
            id: "test-lib".to_string(),
            source_id: "test-source".to_string(),
            title: "Test Library".to_string(),
            library_type: "show".to_string(),
            icon: Some("video-x-generic".to_string()),
            item_count: 0,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        lib_repo.insert(lib).await?;

        let library_id = LibraryId::from("test-lib");
        let source_id = SourceId::from("test-source");

        // Save multiple episodes for the same show
        for ep_num in 1..=5 {
            let episode = create_test_episode_domain(
                &format!("ep-{}", ep_num),
                "show-1",
                1,
                ep_num,
                &format!("Episode {}", ep_num),
            );
            MediaService::save_media_item(
                &db_conn,
                MediaItem::Episode(episode),
                &library_id,
                &source_id,
            )
            .await?;
        }

        // Verify all episodes were saved
        let media_repo = MediaRepositoryImpl::new(db_conn.clone());
        let episodes = media_repo.find_episodes_by_show("show-1").await?;
        assert_eq!(episodes.len(), 5);

        // Verify each episode has correct season/episode numbers
        for ep_num in 1..=5 {
            let episode = media_repo
                .find_episode_by_parent_season_episode("show-1", 1, ep_num as i32)
                .await?;
            assert!(episode.is_some());
            let episode = episode.unwrap();
            assert_eq!(episode.episode_number, Some(ep_num as i32));
            assert_eq!(episode.title, format!("Episode {}", ep_num));
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_save_episode_batch_with_id_changes() -> Result<()> {
        // Test batch save scenario where some episodes have ID changes
        let db = setup_test_database().await?;
        let db_conn = db.get_connection();

        // Create library
        let lib_repo = LibraryRepositoryImpl::new(db_conn.clone());
        let lib = LibraryModel {
            id: "test-lib".to_string(),
            source_id: "test-source".to_string(),
            title: "Test Library".to_string(),
            library_type: "show".to_string(),
            icon: Some("video-x-generic".to_string()),
            item_count: 0,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        lib_repo.insert(lib).await?;

        let library_id = LibraryId::from("test-lib");
        let source_id = SourceId::from("test-source");

        // First sync: Save episodes with IDs ep-1, ep-2, ep-3
        for ep_num in 1..=3 {
            let episode = create_test_episode_domain(
                &format!("ep-{}", ep_num),
                "show-1",
                1,
                ep_num,
                &format!("Episode {}", ep_num),
            );
            MediaService::save_media_item(
                &db_conn,
                MediaItem::Episode(episode),
                &library_id,
                &source_id,
            )
            .await?;
        }

        // Second sync: Re-sync with NEW IDs (simulating backend ID regeneration)
        // ep-1 -> ep-new-1, ep-2 -> ep-new-2, ep-3 -> ep-new-3
        for ep_num in 1..=3 {
            let episode = create_test_episode_domain(
                &format!("ep-new-{}", ep_num),
                "show-1",
                1,
                ep_num,
                &format!("Episode {} - Resync", ep_num),
            );
            MediaService::save_media_item(
                &db_conn,
                MediaItem::Episode(episode),
                &library_id,
                &source_id,
            )
            .await?;
        }

        // Verify we still have exactly 3 episodes (not 6)
        let media_repo = MediaRepositoryImpl::new(db_conn.clone());
        let episodes = media_repo.find_episodes_by_show("show-1").await?;
        assert_eq!(episodes.len(), 3);

        // Verify episodes have updated titles and old IDs
        for ep_num in 1..=3 {
            let episode = media_repo.find_by_id(&format!("ep-{}", ep_num)).await?;
            assert!(episode.is_some());
            let episode = episode.unwrap();
            assert_eq!(episode.title, format!("Episode {} - Resync", ep_num));

            // Verify new IDs don't exist
            let new_id_result = media_repo.find_by_id(&format!("ep-new-{}", ep_num)).await?;
            assert!(new_id_result.is_none());
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_episode_missing_required_fields() -> Result<()> {
        let db = setup_test_database().await?;
        let db_conn = db.get_connection();

        // Create library
        let lib_repo = LibraryRepositoryImpl::new(db_conn.clone());
        let lib = LibraryModel {
            id: "test-lib".to_string(),
            source_id: "test-source".to_string(),
            title: "Test Library".to_string(),
            library_type: "show".to_string(),
            icon: Some("video-x-generic".to_string()),
            item_count: 0,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        lib_repo.insert(lib).await?;

        // Create an episode with missing parent_id
        let mut episode = create_test_episode_domain("ep-1", "show-1", 1, 1, "Pilot");
        episode.show_id = None; // Remove parent_id

        let library_id = LibraryId::from("test-lib");
        let source_id = SourceId::from("test-source");

        // This should still work (falls back to ID-based check)
        MediaService::save_media_item(
            &db_conn,
            MediaItem::Episode(episode),
            &library_id,
            &source_id,
        )
        .await?;

        // Verify it was saved
        let media_repo = MediaRepositoryImpl::new(db_conn.clone());
        let saved = media_repo.find_by_id("ep-1").await?;
        assert!(saved.is_some());

        Ok(())
    }
}
