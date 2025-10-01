use crate::ui::shared::broker::BROKER;
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
use crate::models::{Library, MediaItem, SourceId};
use crate::services::core::media::MediaService;

/// Pure functions for synchronization operations
pub struct SyncService;

impl SyncService {
    /// Estimate total items for sync progress tracking
    async fn estimate_total_items(_backend: &dyn MediaBackend) -> Result<Option<i32>> {
        // For now, we can't estimate total items without fetching all libraries
        // This would be too expensive, so we'll track progress per batch instead
        Ok(None)
    }
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
                    .notify_sync_error(source_id.to_string(), e.to_string())
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
            .notify_sync_started(source_id.to_string(), Some(estimated_total_items))
            .await;

        // Mark sync as in progress
        Self::update_sync_status(db, source_id, SyncStatus::InProgress, None).await?;

        // Sync libraries
        for library in libraries {
            // Save library
            MediaService::save_library(db, library.clone(), source_id).await?;

            // Notify library sync started
            BROKER
                .notify_library_sync_started(
                    source_id.to_string(),
                    library.id.clone(),
                    library.title.clone(),
                )
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
                        .notify_library_sync_completed(
                            source_id.to_string(),
                            library.id.clone(),
                            library.title.clone(),
                            items_count,
                        )
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
            .notify_sync_completed(source_id.to_string(), result.items_synced)
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
                .notify_sync_progress(
                    source_id.to_string(),
                    *cumulative_items_synced,
                    estimated_total,
                )
                .await;
        }

        // Sync episodes for TV shows
        if matches!(library.library_type, crate::models::LibraryType::Shows) {
            info!(
                "Starting episode sync for TV shows in library {}",
                library.title
            );
            let shows =
                MediaService::get_media_items(db, &library.id.clone().into(), None, 0, 1000)
                    .await?;
            info!(
                "Found {} shows in library to sync episodes for",
                shows.len()
            );

            for show in shows {
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
                                    db,
                                    MediaItem::Show(show_data.clone()),
                                    &library.id.clone().into(),
                                    source_id,
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
                            }
                        }
                    }
                    match Self::sync_show_episodes_with_progress(
                        db,
                        backend,
                        source_id,
                        &library.id.clone().into(),
                        &show_data.id.clone().into(),
                        &show_data.title,
                        cumulative_items_synced,
                        estimated_total,
                    )
                    .await
                    {
                        Ok(episodes_count) => {
                            items_synced += episodes_count;
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
        let mut dummy_progress = 0;
        Self::sync_show_episodes_with_progress(
            db,
            backend,
            source_id,
            library_id,
            show_id,
            "Show",
            &mut dummy_progress,
            0,
        )
        .await
    }

    /// Sync episodes for a TV show with cumulative progress tracking
    pub async fn sync_show_episodes_with_progress(
        db: &DatabaseConnection,
        backend: &dyn MediaBackend,
        source_id: &SourceId,
        library_id: &crate::models::LibraryId,
        show_id: &crate::models::ShowId,
        show_title: &str,
        cumulative_items_synced: &mut usize,
        estimated_total: usize,
    ) -> Result<usize> {
        // Fetch seasons directly from the backend API
        let seasons = match backend.get_seasons(show_id).await {
            Ok(seasons) => seasons,
            Err(e) => {
                warn!("Failed to get seasons for show {}: {}", show_id.as_str(), e);
                return Ok(0);
            }
        };

        let mut total_episodes_synced = 0;

        // Iterate through all seasons
        for season in &seasons {
            info!(
                "Fetching episodes for show {} season {}",
                show_id.as_str(),
                season.season_number
            );
            match backend.get_episodes(show_id, season.season_number).await {
                Ok(episodes) => {
                    // Check for season_number = 0 issues
                    for episode in &episodes {
                        if episode.season_number == 0 {
                            warn!(
                                "Episode '{}' (S{}E{}) has season_number=0 for show {}",
                                episode.title,
                                episode.season_number,
                                episode.episode_number,
                                show_id.as_str()
                            );
                        }
                    }

                    let episodes_media: Vec<MediaItem> =
                        episodes.into_iter().map(MediaItem::Episode).collect();

                    let episode_count = episodes_media.len();
                    if episode_count > 0 {
                        MediaService::save_media_items_batch(
                            db,
                            episodes_media,
                            library_id,
                            source_id,
                        )
                        .await?;

                        total_episodes_synced += episode_count;
                        *cumulative_items_synced += episode_count;

                        // Notify cumulative progress with current item info
                        BROKER
                            .notify_sync_progress(
                                source_id.to_string(),
                                *cumulative_items_synced,
                                estimated_total,
                            )
                            .await;
                    }
                }
                Err(e) => {
                    warn!(
                        "Failed to sync episodes for show {} season {}: {}",
                        show_id.as_str(),
                        season.season_number,
                        e
                    );
                }
            }
        }

        Ok(total_episodes_synced)
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
