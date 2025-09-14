use crate::platforms::relm4::components::shared::broker::{BROKER, DataMessage, SourceMessage};
use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use sea_orm::TransactionTrait;
use tracing::{debug, info, warn};

use crate::backends::traits::MediaBackend;
use crate::db::connection::DatabaseConnection;
use crate::db::entities::{LibraryModel, SyncStatusModel};
use crate::db::repository::{
    Repository,
    sync_repository::{SyncRepository, SyncRepositoryImpl},
};
use crate::models::{Library, MediaItem, SourceId};
use crate::services::core::media::MediaService;

/// Pure functions for synchronization operations
pub struct SyncService;

impl SyncService {
    /// Estimate total items for sync progress tracking
    async fn estimate_total_items(backend: &dyn MediaBackend) -> Result<Option<i32>> {
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

        // Try to get total items count for better progress tracking
        let total_items = Self::estimate_total_items(backend).await.ok().flatten();

        // Notify sync started with total items if available
        BROKER
            .notify_sync_started(source_id.to_string(), total_items.map(|v| v as usize))
            .await;

        // Mark sync as in progress
        Self::update_sync_status(db, source_id, SyncStatus::InProgress, None).await?;

        // Sync libraries
        match backend.get_libraries().await {
            Ok(libraries) => {
                result.libraries_synced = libraries.len();

                for library in libraries {
                    // Save library
                    MediaService::save_library(db, library.clone(), source_id).await?;

                    // Sync library content
                    match Self::sync_library(db, backend, source_id, &library).await {
                        Ok(items_count) => {
                            result.items_synced += items_count;
                        }
                        Err(e) => {
                            warn!("Failed to sync library {}: {}", library.id, e);
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

                // Notify sync completed
                BROKER
                    .notify_sync_completed(source_id.to_string(), result.items_synced)
                    .await;
            }
            Err(e) => {
                result
                    .errors
                    .push(format!("Failed to get libraries: {}", e));
                Self::update_sync_status(db, source_id, SyncStatus::Failed, None).await?;

                // Notify sync error
                BROKER
                    .notify_sync_error(source_id.to_string(), e.to_string())
                    .await;

                return Err(e.context("Failed to sync source"));
            }
        }

        info!(
            "Completed sync for source {}: {} libraries, {} items",
            source_id, result.libraries_synced, result.items_synced
        );

        Ok(result)
    }

    /// Sync a single library
    pub async fn sync_library(
        db: &DatabaseConnection,
        backend: &dyn MediaBackend,
        source_id: &SourceId,
        library: &Library,
    ) -> Result<usize> {
        debug!("Syncing library: {} ({})", library.title, library.id);

        let mut items_synced = 0;

        // Fetch items based on library type
        let items = match &library.library_type {
            crate::models::LibraryType::Movies => backend
                .get_movies(&crate::models::LibraryId::new(library.id.clone()))
                .await?
                .into_iter()
                .map(MediaItem::Movie)
                .collect(),
            crate::models::LibraryType::Shows => backend
                .get_shows(&crate::models::LibraryId::new(library.id.clone()))
                .await?
                .into_iter()
                .map(MediaItem::Show)
                .collect(),
            crate::models::LibraryType::Music => {
                // For music, we might need both albums and tracks
                let mut music_items = Vec::new();
                if let Ok(albums) = backend
                    .get_music_albums(&crate::models::LibraryId::new(library.id.clone()))
                    .await
                {
                    music_items.extend(albums.into_iter().map(MediaItem::MusicAlbum));
                }
                // TODO: get_music_tracks takes album_id, not library_id
                // We need to iterate through albums first, then get tracks for each
                // For now, skip tracks
                music_items
            }
            crate::models::LibraryType::Photos | crate::models::LibraryType::Mixed => {
                warn!("Library type {:?} not yet supported", library.library_type);
                Vec::new()
            }
        };

        // Save items in batches
        let batch_size = 100;
        let total_items = items.len();
        for (index, chunk) in items.chunks(batch_size).enumerate() {
            MediaService::save_media_items_batch(
                db,
                chunk.to_vec(),
                &library.id.clone().into(),
                source_id,
            )
            .await?;
            items_synced += chunk.len();

            // Notify progress
            let current = std::cmp::min((index + 1) * batch_size, total_items);
            BROKER
                .notify_sync_progress(source_id.to_string(), current, total_items)
                .await;
        }

        // Sync episodes for TV shows
        if matches!(library.library_type, crate::models::LibraryType::Shows) {
            let shows =
                MediaService::get_media_items(db, &library.id.clone().into(), None, 0, 1000)
                    .await?;
            for show in shows {
                if let MediaItem::Show(show_data) = show {
                    match Self::sync_show_episodes(
                        db,
                        backend,
                        source_id,
                        &library.id.clone().into(),
                        &show_data.id.clone().into(),
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

        debug!(
            "Synced {} items for library {}",
            items_synced, library.title
        );
        Ok(items_synced)
    }

    /// Sync episodes for a TV show
    pub async fn sync_show_episodes(
        db: &DatabaseConnection,
        backend: &dyn MediaBackend,
        source_id: &SourceId,
        library_id: &crate::models::LibraryId,
        show_id: &crate::models::ShowId,
    ) -> Result<usize> {
        // TODO: This should properly iterate through all seasons
        // For now, just get season 1 as a placeholder
        let episodes = backend
            .get_episodes(show_id, 1)
            .await?
            .into_iter()
            .map(MediaItem::Episode)
            .collect::<Vec<_>>();

        let count = episodes.len();
        MediaService::save_media_items_batch(db, episodes, library_id, source_id).await?;

        Ok(count)
    }

    /// Get sync status for a source
    pub async fn get_sync_status(
        db: &DatabaseConnection,
        source_id: &SourceId,
    ) -> Result<Option<SyncStatusModel>> {
        let repo = SyncRepositoryImpl::new(db.clone());
        repo.find_latest_for_source(&source_id.to_string())
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
        let repo = SyncRepositoryImpl::new(db.clone());

        let entity = SyncStatusModel {
            id: 0, // Will be auto-generated
            source_id: source_id.to_string(),
            sync_type: "full".to_string(),
            status: status.to_string(),
            started_at: last_sync,
            completed_at: if status == SyncStatus::Completed {
                Some(chrono::Utc::now().naive_utc())
            } else {
                None
            },
            items_synced: 0,
            total_items: None,
            error_message: None,
        };

        // Since id is auto-generated, we need to find by source_id instead
        // For now, just insert a new record each time
        // TODO: Implement proper update logic based on source_id
        repo.insert(entity).await?;

        Ok(())
    }

    /// Calculate sync progress
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
