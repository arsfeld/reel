use crate::ui::shared::broker::{BROKER, BrokerMessage, DataMessage, SourceMessage};
use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use tracing::{info, warn};

use crate::backends::traits::MediaBackend;
use crate::db::connection::DatabaseConnection;
use crate::db::entities::SyncStatusModel;
use crate::db::repository::{
    Repository,
    source_repository::{SourceRepository, SourceRepositoryImpl},
    sync_repository::{SyncRepository, SyncRepositoryImpl},
};
use crate::models::{Library, MediaItem, Season, SourceId};
use crate::services::core::media::MediaService;

/// Pure functions for synchronization operations
pub struct SyncService;

impl SyncService {
    /// Sync all libraries for a source
    pub async fn sync_source(
        db: &DatabaseConnection,
        backend: &dyn MediaBackend,
        source_id: &SourceId,
    ) -> Result<SyncResult> {
        info!("Starting sync for source: {}", source_id);

        let mut result = SyncResult::default();

        // First, get all libraries to estimate total work
        let libraries = match backend.get_libraries().await {
            Ok(libs) => libs,
            Err(e) => {
                result
                    .errors
                    .push(format!("Failed to get libraries: {}", e));
                Self::update_sync_status(db, source_id, SyncStatus::Failed, None).await?;
                BROKER
                    .broadcast(BrokerMessage::Source(SourceMessage::SyncError {
                        source_id: source_id.to_string(),
                        error: e.to_string(),
                    }))
                    .await;
                BROKER
                    .broadcast(BrokerMessage::Data(DataMessage::LoadError {
                        source: source_id.to_string(),
                        error: e.to_string(),
                    }))
                    .await;
                return Err(e.context("Failed to sync source"));
            }
        };

        result.libraries_synced = libraries.len();

        // Estimate total items to sync (rough estimate based on library types)
        let mut estimated_total_items = 0;
        for library in &libraries {
            // Rough estimates per library type
            estimated_total_items += match library.library_type {
                crate::models::LibraryType::Movies => 100, // Estimate 100 movies per library
                crate::models::LibraryType::Shows => 500,  // Estimate shows + episodes
                _ => 50,
            };
        }

        // Track cumulative progress across entire sync
        let mut cumulative_items_synced = 0;

        // Notify sync started with estimated total
        BROKER
            .broadcast(BrokerMessage::Source(SourceMessage::SyncStarted {
                source_id: source_id.to_string(),
                total_items: Some(estimated_total_items),
            }))
            .await;

        // Mark sync as in progress
        Self::update_sync_status(db, source_id, SyncStatus::InProgress, None).await?;

        // Sync libraries
        for library in libraries {
            // Save library
            MediaService::save_library(db, library.clone(), source_id).await?;

            // Notify library sync started
            BROKER
                .broadcast(BrokerMessage::Source(SourceMessage::LibrarySyncStarted {
                    source_id: source_id.to_string(),
                    library_id: library.id.clone(),
                    library_name: library.title.clone(),
                }))
                .await;

            // Sync library content with cumulative progress tracking
            match Self::sync_library_with_progress(
                db,
                backend,
                source_id,
                &library,
                &mut cumulative_items_synced,
                estimated_total_items,
            )
            .await
            {
                Ok(items_count) => {
                    // Notify library sync completed
                    BROKER
                        .broadcast(BrokerMessage::Source(SourceMessage::LibrarySyncCompleted {
                            source_id: source_id.to_string(),
                            library_id: library.id.clone(),
                            library_name: library.title.clone(),
                            items_synced: items_count,
                        }))
                        .await;

                    result.items_synced += items_count;
                }
                Err(e) => {
                    warn!("Failed to sync library {}: {}", library.id, e);
                    // Also log the full error chain for debugging
                    let mut error_chain = vec![e.to_string()];
                    let mut source = e.source();
                    while let Some(err) = source {
                        error_chain.push(err.to_string());
                        source = err.source();
                    }
                    warn!(
                        "Error chain for library {}: {:?}",
                        library.title, error_chain
                    );
                    result
                        .errors
                        .push(format!("Library {}: {}", library.title, e));
                }
            }
        }

        // Mark sync as complete
        Self::update_sync_status(
            db,
            source_id,
            SyncStatus::Completed,
            Some(chrono::Utc::now().naive_utc()),
        )
        .await?;

        // Update the source's last_sync timestamp
        let source_repo = SourceRepositoryImpl::new(db.clone());
        source_repo.update_last_sync(source_id.as_str()).await?;

        // Notify sync completed
        BROKER
            .broadcast(BrokerMessage::Source(SourceMessage::SyncCompleted {
                source_id: source_id.to_string(),
                items_synced: result.items_synced,
            }))
            .await;
        BROKER
            .broadcast(BrokerMessage::Data(DataMessage::LoadComplete {
                source: source_id.to_string(),
            }))
            .await;

        info!(
            "Completed sync for source {}: {} libraries, {} items",
            source_id, result.libraries_synced, result.items_synced
        );

        Ok(result)
    }

    /// Sync a single library (legacy method without progress tracking)
    pub async fn sync_library(
        db: &DatabaseConnection,
        backend: &dyn MediaBackend,
        source_id: &SourceId,
        library: &Library,
    ) -> Result<usize> {
        let mut dummy_progress = 0;
        Self::sync_library_with_progress(db, backend, source_id, library, &mut dummy_progress, 0)
            .await
    }

    /// Sync a single library with cumulative progress tracking
    pub async fn sync_library_with_progress(
        db: &DatabaseConnection,
        backend: &dyn MediaBackend,
        source_id: &SourceId,
        library: &Library,
        cumulative_items_synced: &mut usize,
        estimated_total: usize,
    ) -> Result<usize> {
        info!(
            "Syncing library: {} ({}) of type {:?}",
            library.title, library.id, library.library_type
        );

        let mut items_synced = 0;

        // Fetch items based on library type
        let items = match &library.library_type {
            crate::models::LibraryType::Movies => {
                info!("Fetching movies for library {}", library.title);
                let movies = backend
                    .get_movies(&crate::models::LibraryId::new(library.id.clone()))
                    .await?;
                info!("Found {} movies in library {}", movies.len(), library.title);
                movies.into_iter().map(MediaItem::Movie).collect()
            }
            crate::models::LibraryType::Shows => {
                info!("Fetching shows for library {}", library.title);
                let shows = backend
                    .get_shows(&crate::models::LibraryId::new(library.id.clone()))
                    .await?;
                info!("Found {} shows in library {}", shows.len(), library.title);

                // Log detailed information about each show's seasons
                for show in &shows {
                    if show.seasons.is_empty() {
                        warn!(
                            "  - Show '{}' has no seasons data from backend!",
                            show.title
                        );
                    }
                }

                shows.into_iter().map(MediaItem::Show).collect()
            }
            crate::models::LibraryType::Music => {
                // Music support not yet implemented
                // get_music_albums and get_music_tracks methods were removed as unused
                warn!("Music library sync not yet implemented");
                Vec::new()
            }
            crate::models::LibraryType::Photos | crate::models::LibraryType::Mixed => {
                warn!("Library type {:?} not yet supported", library.library_type);
                Vec::new()
            }
        };

        // Sync deletion: remove items that no longer exist on the backend
        Self::sync_deletions(db, &library.id, &library.library_type, &items).await?;

        // Save items in batches
        let batch_size = 100;
        for chunk in items.chunks(batch_size) {
            MediaService::save_media_items_batch(
                db,
                chunk.to_vec(),
                &library.id.clone().into(),
                source_id,
            )
            .await?;
            items_synced += chunk.len();
            *cumulative_items_synced += chunk.len();

            // Notify cumulative progress for the entire source
            BROKER
                .broadcast(BrokerMessage::Source(SourceMessage::SyncProgress {
                    source_id: source_id.to_string(),
                    library_id: Some(library.id.clone()),
                    current: *cumulative_items_synced,
                    total: estimated_total,
                }))
                .await;
            BROKER
                .broadcast(BrokerMessage::Data(DataMessage::SyncProgress {
                    source_id: source_id.to_string(),
                    current: *cumulative_items_synced,
                    total: estimated_total,
                }))
                .await;
        }

        // Sync episodes for TV shows
        if matches!(library.library_type, crate::models::LibraryType::Shows) {
            use futures::stream::{self, StreamExt};
            use std::sync::Arc;
            use tokio::sync::Mutex;

            info!(
                "Starting episode sync for TV shows in library {}",
                library.title
            );
            let shows = MediaService::get_media_items(
                db,
                &library.id.clone().into(),
                Some(crate::models::MediaType::Show),
                0,
                10000,
            )
            .await?;
            info!(
                "Found {} shows in library to sync episodes for",
                shows.len()
            );

            // Use Arc<Mutex> to safely share progress counter across concurrent tasks
            let progress_counter = Arc::new(Mutex::new(*cumulative_items_synced));
            let sync_counter = Arc::new(Mutex::new(items_synced));

            // Process shows with limited concurrency to avoid overwhelming backend/database
            // Concurrency of 5 means up to 5 shows can be processed simultaneously
            const CONCURRENT_SHOWS: usize = 5;

            stream::iter(shows)
                .for_each_concurrent(CONCURRENT_SHOWS, |show| {
                    let db = db.clone();
                    let backend = backend;
                    let source_id = source_id.clone();
                    let library_id = library.id.clone();
                    let progress_counter = Arc::clone(&progress_counter);
                    let sync_counter = Arc::clone(&sync_counter);

                    async move {
                        if let MediaItem::Show(mut show_data) = show {
                            // Check if show has seasons data, fetch if missing
                            let should_update_seasons =
                                show_data.seasons.is_empty() || show_data.total_episode_count == 0;

                            if should_update_seasons {
                                match backend.get_seasons(&show_data.id.clone().into()).await {
                                    Ok(seasons) => {
                                        show_data.seasons = seasons.clone();

                                        // Update episode count from seasons
                                        let total_episodes: u32 =
                                            seasons.iter().map(|s| s.episode_count).sum();
                                        if total_episodes > 0 {
                                            show_data.total_episode_count = total_episodes;
                                        }

                                        // Update the show in database with seasons
                                        if let Err(e) = MediaService::save_media_item(
                                            &db,
                                            MediaItem::Show(show_data.clone()),
                                            &library_id.clone().into(),
                                            &source_id,
                                        )
                                        .await
                                        {
                                            warn!(
                                                "Failed to update show {} with seasons: {}",
                                                show_data.title, e
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        warn!(
                                            "Failed to fetch seasons for show {}: {}",
                                            show_data.title, e
                                        );
                                        // Skip episode sync if we couldn't get seasons
                                        return;
                                    }
                                }
                            }

                            // Now sync episodes - pass the seasons we already have to avoid redundant API calls
                            let mut local_progress = progress_counter.lock().await;
                            match Self::sync_show_episodes_with_progress(
                                &db,
                                backend,
                                &source_id,
                                &library_id.clone().into(),
                                &show_data.id.clone().into(),
                                &show_data.title,
                                show_data.seasons.clone(),
                                &mut *local_progress,
                                estimated_total,
                            )
                            .await
                            {
                                Ok(episodes_count) => {
                                    let mut sync = sync_counter.lock().await;
                                    *sync += episodes_count;
                                }
                                Err(e) => {
                                    warn!(
                                        "Failed to sync episodes for show {}: {}",
                                        show_data.title, e
                                    );
                                }
                            }
                        }
                    }
                })
                .await;

            // Update local counters from Arc<Mutex> after parallel processing
            *cumulative_items_synced = *progress_counter.lock().await;
            items_synced = *sync_counter.lock().await;
        }

        // Update library item count in database
        use crate::db::repository::LibraryRepositoryImpl;
        let library_repo = LibraryRepositoryImpl::new(db.clone());
        if let Ok(Some(mut lib_entity)) = library_repo.find_by_id(&library.id).await {
            // Get actual count from media repository
            use crate::db::repository::MediaRepositoryImpl;
            let media_repo = MediaRepositoryImpl::new(db.clone());
            if let Ok(count) = media_repo.count_by_library(&library.id).await {
                info!(
                    "Updating library {} item count from {} to {}",
                    library.title, lib_entity.item_count, count
                );
                lib_entity.item_count = count as i32;
                lib_entity.updated_at = chrono::Utc::now().naive_utc();
                if let Err(e) = library_repo.update(lib_entity).await {
                    warn!("Failed to update library item count: {}", e);
                } else {
                    info!(
                        "Successfully updated library {} item count to {}",
                        library.title, count
                    );
                }
            } else {
                warn!("Failed to get count for library {}", library.id);
            }
        } else {
            warn!("Failed to find library {} in database", library.id);
        }

        Ok(items_synced)
    }

    /// Sync episodes for a TV show (legacy method)
    pub async fn sync_show_episodes(
        db: &DatabaseConnection,
        backend: &dyn MediaBackend,
        source_id: &SourceId,
        library_id: &crate::models::LibraryId,
        show_id: &crate::models::ShowId,
    ) -> Result<usize> {
        // Fetch seasons first
        let seasons = backend.get_seasons(show_id).await?;

        let mut dummy_progress = 0;
        Self::sync_show_episodes_with_progress(
            db,
            backend,
            source_id,
            library_id,
            show_id,
            "Show",
            seasons,
            &mut dummy_progress,
            0,
        )
        .await
    }

    /// Sync episodes for a TV show with cumulative progress tracking
    ///
    /// Optimized version that:
    /// 1. Accepts seasons as parameter to avoid redundant API calls
    /// 2. Fetches episodes for all seasons in parallel for better performance
    pub async fn sync_show_episodes_with_progress(
        db: &DatabaseConnection,
        backend: &dyn MediaBackend,
        source_id: &SourceId,
        library_id: &crate::models::LibraryId,
        show_id: &crate::models::ShowId,
        show_title: &str,
        seasons: Vec<Season>,
        cumulative_items_synced: &mut usize,
        estimated_total: usize,
    ) -> Result<usize> {
        use futures::stream::{FuturesUnordered, StreamExt};

        if seasons.is_empty() {
            info!(
                "No seasons found for show {}, skipping episode sync",
                show_title
            );
            return Ok(0);
        }

        info!(
            "Fetching episodes for show '{}' ({} seasons) in parallel",
            show_title,
            seasons.len()
        );

        // Create futures for fetching all seasons' episodes in parallel
        let mut fetch_futures = FuturesUnordered::new();
        for season in &seasons {
            let backend_ref = backend;
            let show_id_clone = show_id.clone();
            let season_number = season.season_number;

            fetch_futures.push(async move {
                (
                    season_number,
                    backend_ref
                        .get_episodes(&show_id_clone, season_number)
                        .await,
                )
            });
        }

        let mut total_episodes_synced = 0;

        // Process each season's episodes as they come in
        while let Some((season_number, result)) = fetch_futures.next().await {
            match result {
                Ok(episodes) => {
                    // Check for season_number = 0 issues
                    for episode in &episodes {
                        if episode.season_number == 0 {
                            warn!(
                                "Episode '{}' (S{}E{}) has season_number=0 for show {}",
                                episode.title,
                                episode.season_number,
                                episode.episode_number,
                                show_title
                            );
                        }
                    }

                    let episodes_media: Vec<MediaItem> =
                        episodes.into_iter().map(MediaItem::Episode).collect();

                    let episode_count = episodes_media.len();
                    if episode_count > 0 {
                        // Sync episode deletions for this season
                        Self::sync_episode_deletions(
                            db,
                            show_id.as_str(),
                            season_number as i32,
                            &episodes_media,
                        )
                        .await?;

                        MediaService::save_media_items_batch(
                            db,
                            episodes_media,
                            library_id,
                            source_id,
                        )
                        .await?;

                        total_episodes_synced += episode_count;
                        *cumulative_items_synced += episode_count;

                        info!(
                            "Synced {} episodes for show '{}' season {}",
                            episode_count, show_title, season_number
                        );

                        // Notify cumulative progress with current item info
                        BROKER
                            .broadcast(BrokerMessage::Source(SourceMessage::SyncProgress {
                                source_id: source_id.to_string(),
                                library_id: Some(library_id.as_str().to_string()),
                                current: *cumulative_items_synced,
                                total: estimated_total,
                            }))
                            .await;
                        BROKER
                            .broadcast(BrokerMessage::Data(DataMessage::SyncProgress {
                                source_id: source_id.to_string(),
                                current: *cumulative_items_synced,
                                total: estimated_total,
                            }))
                            .await;
                    }
                }
                Err(e) => {
                    warn!(
                        "Failed to sync episodes for show {} season {}: {}",
                        show_title, season_number, e
                    );
                }
            }
        }

        info!(
            "Completed episode sync for show '{}': {} total episodes",
            show_title, total_episodes_synced
        );

        Ok(total_episodes_synced)
    }

    /// Sync episode deletions: remove episodes that no longer exist on the backend
    async fn sync_episode_deletions(
        db: &DatabaseConnection,
        show_id: &str,
        season_number: i32,
        fetched_episodes: &[MediaItem],
    ) -> Result<()> {
        use crate::db::repository::{MediaRepository, MediaRepositoryImpl};
        use std::collections::HashSet;

        // Get all episode IDs from the backend for this season
        let backend_episode_ids: HashSet<String> = fetched_episodes
            .iter()
            .filter_map(|item| match item {
                MediaItem::Episode(e) => Some(e.id.to_string()),
                _ => None,
            })
            .collect();

        // Get all local episodes for this show and season
        let media_repo = MediaRepositoryImpl::new(db.clone());
        let local_episodes = media_repo
            .find_episodes_by_season(show_id, season_number)
            .await?;

        // Find episodes that are in local DB but not in backend
        let mut stale_episode_ids: Vec<String> = Vec::new();
        for local_episode in &local_episodes {
            if !backend_episode_ids.contains(&local_episode.id) {
                info!(
                    "Episode '{}' (ID: {}) no longer exists on backend for show {}, season {}, marking for deletion",
                    local_episode.title, local_episode.id, show_id, season_number
                );
                stale_episode_ids.push(local_episode.id.clone());
            }
        }

        // Delete stale episodes if any
        if !stale_episode_ids.is_empty() {
            info!(
                "Deleting {} stale episodes from show {} season {}",
                stale_episode_ids.len(),
                show_id,
                season_number
            );
            let deleted_count = media_repo.delete_by_ids(stale_episode_ids).await?;
            info!(
                "Successfully deleted {} episodes from show {} season {}",
                deleted_count, show_id, season_number
            );
        }

        Ok(())
    }

    /// Sync deletions: remove items that no longer exist on the backend
    async fn sync_deletions(
        db: &DatabaseConnection,
        library_id: &str,
        library_type: &crate::models::LibraryType,
        fetched_items: &[MediaItem],
    ) -> Result<()> {
        use crate::db::repository::{MediaRepository, MediaRepositoryImpl};
        use std::collections::HashSet;

        // Determine the media type to filter by
        let media_type = match library_type {
            crate::models::LibraryType::Movies => "movie",
            crate::models::LibraryType::Shows => "show",
            _ => {
                // Skip deletion sync for unsupported library types
                return Ok(());
            }
        };

        // Get all item IDs from the backend
        let backend_ids: HashSet<String> = fetched_items
            .iter()
            .map(|item| match item {
                MediaItem::Movie(m) => m.id.to_string(),
                MediaItem::Show(s) => s.id.to_string(),
                MediaItem::Episode(e) => e.id.to_string(),
                _ => String::new(),
            })
            .filter(|id| !id.is_empty())
            .collect();

        info!(
            "Backend has {} {} items for library {}",
            backend_ids.len(),
            media_type,
            library_id
        );

        // Get all local items for this library and type
        let media_repo = MediaRepositoryImpl::new(db.clone());
        let local_items = media_repo
            .find_by_library_and_type(library_id, media_type)
            .await?;

        info!(
            "Local database has {} {} items for library {}",
            local_items.len(),
            media_type,
            library_id
        );

        // Find items that are in local DB but not in backend
        let mut stale_ids: Vec<String> = Vec::new();
        for local_item in &local_items {
            if !backend_ids.contains(&local_item.id) {
                info!(
                    "Item '{}' (ID: {}) no longer exists on backend, marking for deletion",
                    local_item.title, local_item.id
                );
                stale_ids.push(local_item.id.clone());
            }
        }

        // Delete stale items if any
        if !stale_ids.is_empty() {
            info!(
                "Deleting {} stale {} items from library {}",
                stale_ids.len(),
                media_type,
                library_id
            );
            let deleted_count = media_repo.delete_by_ids(stale_ids).await?;
            info!(
                "Successfully deleted {} items from library {}",
                deleted_count, library_id
            );

            // Broadcast data updated event so UI refreshes
            BROKER
                .broadcast(BrokerMessage::Data(DataMessage::LoadComplete {
                    source: library_id.to_string(),
                }))
                .await;
        } else {
            info!("No stale items to delete for library {}", library_id);
        }

        Ok(())
    }

    /// Get sync status for a source
    #[allow(dead_code)]
    pub async fn get_sync_status(
        db: &DatabaseConnection,
        source_id: &SourceId,
    ) -> Result<Option<SyncStatusModel>> {
        let repo = SyncRepositoryImpl::new(db.clone());
        repo.find_latest_for_source(source_id.as_ref())
            .await
            .context("Failed to get sync status")
    }

    /// Update sync status
    pub async fn update_sync_status(
        db: &DatabaseConnection,
        source_id: &SourceId,
        status: SyncStatus,
        last_sync: Option<NaiveDateTime>,
    ) -> Result<()> {
        use crate::db::entities::sync_status;
        use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

        // Try to find existing sync status for this source
        let existing = sync_status::Entity::find()
            .filter(sync_status::Column::SourceId.eq(source_id.to_string()))
            .filter(sync_status::Column::SyncType.eq("full"))
            .one(db.as_ref())
            .await?;

        if let Some(existing_model) = existing {
            // Update existing record
            let mut active_model: sync_status::ActiveModel = existing_model.into();
            active_model.status = Set(status.to_string());
            if status == SyncStatus::Completed {
                active_model.completed_at = Set(Some(chrono::Utc::now().naive_utc()));
            }
            active_model.update(db.as_ref()).await?;
        } else {
            // Insert new record using start_sync which properly handles ID generation
            let repo = SyncRepositoryImpl::new(db.clone());
            let new_sync = repo.start_sync(source_id.as_ref(), "full", None).await?;

            // If we need to update the status immediately (e.g., to completed or failed)
            if status != SyncStatus::InProgress {
                let mut active_model: sync_status::ActiveModel = new_sync.into();
                active_model.status = Set(status.to_string());
                if status == SyncStatus::Completed {
                    active_model.completed_at =
                        Set(last_sync.or(Some(chrono::Utc::now().naive_utc())));
                }
                active_model.update(db.as_ref()).await?;
            }
        }

        Ok(())
    }

    /// Calculate sync progress
    #[allow(dead_code)]
    pub async fn get_sync_progress(
        db: &DatabaseConnection,
        source_id: &SourceId,
    ) -> Result<SyncProgress> {
        let status = Self::get_sync_status(db, source_id).await?;

        Ok(match status {
            Some(s) => SyncProgress {
                is_syncing: s.status == "running",
                items_synced: s.items_synced as usize,
                total_items: s.total_items.unwrap_or(0) as usize,
                percentage: if let Some(total) = s.total_items {
                    if total > 0 {
                        ((s.items_synced as f32 / total as f32) * 100.0) as u32
                    } else {
                        0
                    }
                } else {
                    0
                },
            },
            None => SyncProgress::default(),
        })
    }
}

#[derive(Debug, Default)]
pub struct SyncResult {
    pub libraries_synced: usize,
    pub items_synced: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum SyncStatus {
    Idle,
    InProgress,
    Completed,
    Failed,
}

impl ToString for SyncStatus {
    fn to_string(&self) -> String {
        match self {
            SyncStatus::Idle => "idle",
            SyncStatus::InProgress => "in_progress",
            SyncStatus::Completed => "completed",
            SyncStatus::Failed => "failed",
        }
        .to_string()
    }
}

#[derive(Debug, Default, Clone)]
pub struct SyncProgress {
    pub is_syncing: bool,
    pub items_synced: usize,
    pub total_items: usize,
    pub percentage: u32,
}
