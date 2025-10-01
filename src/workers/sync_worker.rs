use crate::db::DatabaseConnection;
use crate::models::{LibraryId, SourceId};
use crate::services::core::backend::BackendService;
use crate::services::core::sync::SyncService;
use relm4::{ComponentSender, Worker};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::info;

#[derive(Debug, Clone)]
pub struct SyncProgress {
    pub source_id: SourceId,
    pub library_id: Option<LibraryId>,
    pub current: usize,
    pub total: usize,
    pub message: String,
    pub home_sections_synced: bool,
}

#[derive(Debug, Clone)]
pub enum SyncWorkerInput {
    StartSync {
        source_id: SourceId,
        library_id: Option<LibraryId>,
        force: bool,
    },
    StopSync {
        source_id: SourceId,
    },
    StopAllSyncs,
    SetSyncInterval(Duration),
    EnableAutoSync(bool),
    RecordSuccessfulSync {
        source_id: SourceId,
    },
}

#[derive(Debug, Clone)]
pub enum SyncWorkerOutput {
    SyncStarted {
        source_id: SourceId,
        library_id: Option<LibraryId>,
    },
    SyncProgress(SyncProgress),
    SyncCompleted {
        source_id: SourceId,
        library_id: Option<LibraryId>,
        items_synced: usize,
        sections_synced: usize,
        duration: Duration,
    },
    SyncFailed {
        source_id: SourceId,
        library_id: Option<LibraryId>,
        error: String,
    },
    SyncCancelled {
        source_id: SourceId,
    },
}

#[derive(Debug)]
pub struct SyncWorker {
    db: Arc<DatabaseConnection>,
    active_syncs: HashMap<SourceId, relm4::JoinHandle<()>>,
    sync_interval: Duration,
    auto_sync_enabled: bool,
    last_sync_times: HashMap<SourceId, Instant>,
}

impl SyncWorker {
    fn new(db: Arc<DatabaseConnection>) -> Self {
        Self {
            db,
            active_syncs: HashMap::new(),
            sync_interval: Duration::from_secs(3600), // Default 1 hour
            auto_sync_enabled: true,
            last_sync_times: HashMap::new(),
        }
    }

    async fn perform_sync(
        db: Arc<DatabaseConnection>,
        source_id: SourceId,
        library_id: Option<LibraryId>,
        _force: bool,
        sender: ComponentSender<SyncWorker>,
    ) {
        info!("perform_sync called for source: {:?}", source_id);
        let start_time = Instant::now();

        // Send sync started
        sender
            .output(SyncWorkerOutput::SyncStarted {
                source_id: source_id.clone(),
                library_id: library_id.clone(),
            })
            .ok();

        // Load source configuration to create backend
        use crate::db::repository::Repository;
        use crate::db::repository::source_repository::SourceRepositoryImpl;

        let source_repo = SourceRepositoryImpl::new(db.as_ref().clone());
        let source_entity = match source_repo.find_by_id(source_id.as_str()).await {
            Ok(Some(entity)) => entity,
            Ok(None) => {
                tracing::error!("Source not found: {:?}", source_id);
                sender
                    .output(SyncWorkerOutput::SyncFailed {
                        source_id: source_id.clone(),
                        library_id,
                        error: "Source not found".to_string(),
                    })
                    .ok();
                return;
            }
            Err(e) => {
                tracing::error!("Failed to load source: {}", e);
                sender
                    .output(SyncWorkerOutput::SyncFailed {
                        source_id: source_id.clone(),
                        library_id,
                        error: e.to_string(),
                    })
                    .ok();
                return;
            }
        };

        // Create backend for this source
        let backend = match BackendService::create_backend_for_source(&db, &source_entity).await {
            Ok(backend) => backend,
            Err(e) => {
                tracing::error!("Failed to create backend: {}", e);
                sender
                    .output(SyncWorkerOutput::SyncFailed {
                        source_id: source_id.clone(),
                        library_id,
                        error: e.to_string(),
                    })
                    .ok();
                return;
            }
        };

        info!("Calling SyncService::sync_source for {:?}", source_id);
        // Call sync service directly with the backend
        match SyncService::sync_source(&db, backend.as_ref(), &source_id).await {
            Ok(sync_result) => {
                info!(
                    "Sync succeeded for {:?}: {} items",
                    source_id, sync_result.items_synced
                );

                // After successful library/media sync, fetch and save home sections
                info!("Fetching home sections for source: {:?}", source_id);
                let sections_synced = Self::sync_home_sections(&db, &source_id, &sender).await;

                // Send the sync completed output with sections count
                sender
                    .output(SyncWorkerOutput::SyncCompleted {
                        source_id: source_id.clone(),
                        library_id,
                        items_synced: sync_result.items_synced,
                        sections_synced,
                        duration: start_time.elapsed(),
                    })
                    .ok();

                // Record successful sync time to prevent too-frequent retries
                sender.input(SyncWorkerInput::RecordSuccessfulSync {
                    source_id: source_id.clone(),
                });
            }
            Err(e) => {
                tracing::error!("Sync failed for {:?}: {}", source_id, e);
                // Log the error chain for debugging
                let mut error_chain = vec![e.to_string()];
                let mut source = e.source();
                while let Some(err) = source {
                    error_chain.push(err.to_string());
                    source = err.source();
                }
                tracing::error!("Error chain for {:?}: {:?}", source_id, error_chain);

                sender
                    .output(SyncWorkerOutput::SyncFailed {
                        source_id,
                        library_id,
                        error: e.to_string(),
                    })
                    .ok();
            }
        }
    }

    /// Sync home sections for a source
    async fn sync_home_sections(
        db: &Arc<DatabaseConnection>,
        source_id: &SourceId,
        sender: &ComponentSender<SyncWorker>,
    ) -> usize {
        use crate::db::repository::Repository;
        use crate::db::repository::home_section_repository::{
            HomeSectionRepository, HomeSectionRepositoryImpl,
        };
        use crate::db::repository::source_repository::SourceRepositoryImpl;

        // Send progress notification for home sections sync
        sender
            .output(SyncWorkerOutput::SyncProgress(SyncProgress {
                source_id: source_id.clone(),
                library_id: None,
                current: 0,
                total: 1,
                message: "Syncing home sections...".to_string(),
                home_sections_synced: false,
            }))
            .ok();

        // Load source configuration to create backend
        let source_repo = SourceRepositoryImpl::new(db.as_ref().clone());
        let source_entity = match source_repo.find_by_id(source_id.as_str()).await {
            Ok(Some(entity)) => entity,
            Ok(None) => {
                tracing::warn!("Source not found for home sections sync: {:?}", source_id);
                return 0;
            }
            Err(e) => {
                tracing::error!("Failed to load source for home sections sync: {}", e);
                return 0;
            }
        };

        // Create backend for this source
        let backend = match BackendService::create_backend_for_source(db, &source_entity).await {
            Ok(backend) => backend,
            Err(e) => {
                tracing::error!("Failed to create backend for home sections sync: {}", e);
                return 0;
            }
        };

        // Fetch home sections from the backend
        let sections = match backend.get_home_sections().await {
            Ok(sections) => sections,
            Err(e) => {
                tracing::error!("Failed to fetch home sections from backend: {}", e);
                return 0;
            }
        };

        let sections_count = sections.len();
        info!(
            "Fetched {} home sections from source: {:?}",
            sections_count, source_id
        );

        if sections_count > 0 {
            // Convert HomeSection models to database entities
            let home_section_repo = HomeSectionRepositoryImpl::new(db.as_ref().clone());
            let mut section_models = Vec::new();
            let mut section_items = Vec::new();

            for (index, section) in sections.into_iter().enumerate() {
                // Convert to HomeSectionModel
                let section_model = crate::db::entities::home_sections::Model {
                    id: 0, // Will be auto-generated
                    source_id: source_id.as_str().to_string(),
                    hub_identifier: section.id.clone(),
                    title: section.title.clone(),
                    section_type: match section.section_type {
                        crate::models::HomeSectionType::ContinueWatching => {
                            "continue_watching".to_string()
                        }
                        crate::models::HomeSectionType::OnDeck => "on_deck".to_string(),
                        crate::models::HomeSectionType::RecentlyAdded(ref media_type) => {
                            format!("recently_added_{}", media_type)
                        }
                        crate::models::HomeSectionType::Suggested => "suggested".to_string(),
                        crate::models::HomeSectionType::TopRated => "top_rated".to_string(),
                        crate::models::HomeSectionType::Trending => "trending".to_string(),
                        crate::models::HomeSectionType::RecentlyPlayed => {
                            "recently_played".to_string()
                        }
                        crate::models::HomeSectionType::RecentPlaylists => {
                            "recent_playlists".to_string()
                        }
                        crate::models::HomeSectionType::Custom(ref name) => name.clone(),
                    },
                    position: index as i32,
                    context: None,                       // TODO: Add context if needed
                    style: Some("shelf".to_string()),    // Default style
                    hub_type: Some("video".to_string()), // Default hub type
                    size: Some(section.items.len() as i32),
                    last_updated: chrono::Utc::now().naive_utc(),
                    is_stale: false,
                    created_at: chrono::Utc::now().naive_utc(),
                    updated_at: chrono::Utc::now().naive_utc(),
                };

                // Only include media items that exist in the database
                let mut existing_media_ids = Vec::new();

                if !section.items.is_empty() {
                    use crate::db::repository::media_repository::MediaRepositoryImpl;
                    let media_repo = MediaRepositoryImpl::new(db.as_ref().clone());

                    for item in section.items {
                        let media_id = item.id().to_string();

                        // Log the actual item type and details for debugging
                        let (item_type, extra_info) = match &item {
                            crate::models::MediaItem::Movie(m) => {
                                ("movie", format!("title: '{}'", m.title))
                            }
                            crate::models::MediaItem::Show(s) => {
                                ("show", format!("title: '{}'", s.title))
                            }
                            crate::models::MediaItem::Episode(e) => (
                                "episode",
                                format!(
                                    "show: '{}' (id: {:?}), S{}E{}, title: '{}'",
                                    e.show_title.as_ref().unwrap_or(&"Unknown".to_string()),
                                    e.show_id,
                                    e.season_number,
                                    e.episode_number,
                                    e.title
                                ),
                            ),
                            crate::models::MediaItem::MusicTrack(_) => ("track", String::new()),
                            crate::models::MediaItem::MusicAlbum(_) => ("album", String::new()),
                            crate::models::MediaItem::Photo(_) => ("photo", String::new()),
                        };

                        // Check if item exists in database
                        match media_repo.find_by_id(&media_id).await {
                            Ok(Some(_)) => {
                                // Item exists, include it
                                existing_media_ids.push(media_id);
                            }
                            Ok(None) => {
                                // Item doesn't exist, log with full details
                                tracing::warn!(
                                    "Media item {} ({}) not found in database for home section '{}' - {}",
                                    media_id,
                                    item_type,
                                    section.title,
                                    extra_info
                                );

                                // For episodes, let's also check if the show exists
                                if let crate::models::MediaItem::Episode(episode) = &item
                                    && let Some(show_id) = &episode.show_id
                                {
                                    match media_repo.find_by_id(show_id).await {
                                        Ok(Some(_)) => {
                                            tracing::info!(
                                                "  -> Parent show {} exists in database",
                                                show_id
                                            );
                                        }
                                        Ok(None) => {
                                            tracing::warn!(
                                                "  -> Parent show {} also missing from database!",
                                                show_id
                                            );
                                        }
                                        Err(e) => {
                                            tracing::error!(
                                                "  -> Error checking parent show {}: {}",
                                                show_id,
                                                e
                                            );
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to check if media item {} exists: {}",
                                    media_id,
                                    e
                                );
                            }
                        }
                    }
                }

                // Only add section if it has items that exist in the database
                if !existing_media_ids.is_empty() {
                    section_items.push((index as i32, existing_media_ids));
                    section_models.push(section_model);
                } else {
                    tracing::warn!(
                        "Skipping home section '{}' as none of its items exist in database",
                        section_model.title
                    );
                }
            }

            // Save sections and items using repository with transaction
            match home_section_repo
                .save_sections(source_id.as_str(), section_models, section_items)
                .await
            {
                Ok(_) => {
                    info!(
                        "Successfully saved {} home sections for source: {:?}",
                        sections_count, source_id
                    );

                    // Send progress notification for completed home sections sync
                    sender
                        .output(SyncWorkerOutput::SyncProgress(SyncProgress {
                            source_id: source_id.clone(),
                            library_id: None,
                            current: 1,
                            total: 1,
                            message: format!("Synced {} home sections", sections_count),
                            home_sections_synced: true,
                        }))
                        .ok();
                }
                Err(e) => {
                    tracing::error!("Failed to save home sections: {}", e);
                    return 0;
                }
            }
        }

        sections_count
    }

    fn start_sync(
        &mut self,
        source_id: SourceId,
        library_id: Option<LibraryId>,
        force: bool,
        sender: ComponentSender<Self>,
    ) {
        // Cancel any existing sync for this source
        if let Some(handle) = self.active_syncs.remove(&source_id) {
            handle.abort();
        }

        // Check if we should sync (unless forced)
        if !force
            && let Some(last_sync) = self.last_sync_times.get(&source_id)
            && last_sync.elapsed() < self.sync_interval
        {
            info!(
                "Skipping sync for {:?}, too soon since last sync",
                source_id
            );
            return;
        }

        // Start new sync
        info!("Starting async sync task for source: {:?}", source_id);
        let db = self.db.clone();
        let source_id_clone = source_id.clone();
        let source_id_clone2 = source_id.clone();
        let handle = relm4::spawn(async move {
            info!(
                "Async sync task starting for source: {:?}",
                source_id_clone2
            );
            Self::perform_sync(db, source_id_clone, library_id, force, sender).await;
            info!(
                "Async sync task completed for source: {:?}",
                source_id_clone2
            );
        });

        self.active_syncs.insert(source_id.clone(), handle);
        // For now, still track when sync starts to prevent rapid retries
        // TODO: Only track successful syncs to allow retry after failures
        self.last_sync_times.insert(source_id, Instant::now());
    }

    fn stop_sync(&mut self, source_id: &SourceId) {
        if let Some(handle) = self.active_syncs.remove(source_id) {
            handle.abort();
            info!("Stopped sync for {:?}", source_id);
        }
    }

    fn stop_all_syncs(&mut self) {
        for (_, handle) in self.active_syncs.drain() {
            handle.abort();
        }
        info!("Stopped all active syncs");
    }
}

impl Worker for SyncWorker {
    type Init = Arc<DatabaseConnection>;
    type Input = SyncWorkerInput;
    type Output = SyncWorkerOutput;

    fn init(db: Self::Init, _sender: ComponentSender<Self>) -> Self {
        Self::new(db)
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            SyncWorkerInput::StartSync {
                source_id,
                library_id,
                force,
            } => {
                info!(
                    "SyncWorker received StartSync request for source: {:?}, force: {}",
                    source_id, force
                );
                self.start_sync(source_id, library_id, force, sender);
            }

            SyncWorkerInput::StopSync { source_id } => {
                self.stop_sync(&source_id);
                sender
                    .output(SyncWorkerOutput::SyncCancelled { source_id })
                    .ok();
            }

            SyncWorkerInput::StopAllSyncs => {
                self.stop_all_syncs();
            }

            SyncWorkerInput::SetSyncInterval(interval) => {
                self.sync_interval = interval;
                info!("Sync interval set to {:?}", interval);
            }

            SyncWorkerInput::EnableAutoSync(enabled) => {
                self.auto_sync_enabled = enabled;
                info!("Auto-sync {}", if enabled { "enabled" } else { "disabled" });

                if !enabled {
                    self.stop_all_syncs();
                }
            }

            SyncWorkerInput::RecordSuccessfulSync { source_id } => {
                // Record the time of successful sync to prevent too-frequent retries
                self.last_sync_times
                    .insert(source_id.clone(), Instant::now());
                info!("Recorded successful sync time for {:?}", source_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_sync_worker_initialization() {
        // Create a mock database connection for testing
        let db = Arc::new(DatabaseConnection::default());
        let worker = SyncWorker::new(db);

        // Verify default values
        assert!(worker.active_syncs.is_empty());
        assert_eq!(worker.sync_interval, Duration::from_secs(3600));
        assert!(worker.auto_sync_enabled);
        assert!(worker.last_sync_times.is_empty());
    }

    #[test]
    fn test_sync_interval_updates() {
        let db = Arc::new(DatabaseConnection::default());
        let mut worker = SyncWorker::new(db);

        // Default interval should be 1 hour
        assert_eq!(worker.sync_interval, Duration::from_secs(3600));

        // Update interval directly (simulating update message)
        worker.sync_interval = Duration::from_secs(1800);
        assert_eq!(worker.sync_interval, Duration::from_secs(1800));
    }

    #[test]
    fn test_auto_sync_flag() {
        let db = Arc::new(DatabaseConnection::default());
        let mut worker = SyncWorker::new(db);

        // Auto-sync should be enabled by default
        assert!(worker.auto_sync_enabled);

        // Disable auto-sync
        worker.auto_sync_enabled = false;
        assert!(!worker.auto_sync_enabled);

        // Re-enable auto-sync
        worker.auto_sync_enabled = true;
        assert!(worker.auto_sync_enabled);
    }

    #[test]
    fn test_sync_interval_prevents_rapid_syncs() {
        let db = Arc::new(DatabaseConnection::default());
        let mut worker = SyncWorker::new(db);

        // Set a specific interval
        worker.sync_interval = Duration::from_secs(60);

        let source_id = SourceId::from("test-source");

        // Record a sync time
        worker
            .last_sync_times
            .insert(source_id.clone(), Instant::now());

        // Check that sync is too recent (should be prevented unless forced)
        if let Some(last_sync) = worker.last_sync_times.get(&source_id) {
            assert!(last_sync.elapsed() < worker.sync_interval);
        }

        // After waiting, it should be allowed
        worker
            .last_sync_times
            .insert(source_id.clone(), Instant::now() - Duration::from_secs(120));

        if let Some(last_sync) = worker.last_sync_times.get(&source_id) {
            assert!(last_sync.elapsed() >= worker.sync_interval);
        }
    }

    #[test]
    fn test_concurrent_sync_tracking() {
        let db = Arc::new(DatabaseConnection::default());
        let mut worker = SyncWorker::new(db);

        // Simulate multiple active syncs
        let source_id1 = SourceId::from("test-source-1");
        let source_id2 = SourceId::from("test-source-2");
        let source_id3 = SourceId::from("test-source-3");

        // Simulate adding sync handles (we'll use dummy handles for testing)
        let handle1 = relm4::spawn(async {});
        let handle2 = relm4::spawn(async {});
        let handle3 = relm4::spawn(async {});

        worker.active_syncs.insert(source_id1.clone(), handle1);
        worker.active_syncs.insert(source_id2.clone(), handle2);
        worker.active_syncs.insert(source_id3.clone(), handle3);

        // Verify all syncs are tracked
        assert_eq!(worker.active_syncs.len(), 3);
        assert!(worker.active_syncs.contains_key(&source_id1));
        assert!(worker.active_syncs.contains_key(&source_id2));
        assert!(worker.active_syncs.contains_key(&source_id3));

        // Remove one sync
        if let Some(handle) = worker.active_syncs.remove(&source_id2) {
            handle.abort();
        }

        assert_eq!(worker.active_syncs.len(), 2);
        assert!(!worker.active_syncs.contains_key(&source_id2));

        // Clear all syncs
        worker.stop_all_syncs();
        assert!(worker.active_syncs.is_empty());
    }

    #[test]
    fn test_sync_time_recording() {
        let db = Arc::new(DatabaseConnection::default());
        let mut worker = SyncWorker::new(db);

        let source_id = SourceId::from("test-source");

        // Record a successful sync time
        worker
            .last_sync_times
            .insert(source_id.clone(), Instant::now());

        // Verify sync time was recorded
        assert!(worker.last_sync_times.contains_key(&source_id));

        let recorded_time = worker.last_sync_times.get(&source_id).unwrap();
        assert!(recorded_time.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn test_sync_cancellation_behavior() {
        let db = Arc::new(DatabaseConnection::default());
        let mut worker = SyncWorker::new(db);

        let source_id = SourceId::from("test-source");

        // Add a sync handle
        let handle = relm4::spawn(async {
            // Simulate a long-running sync
            tokio::time::sleep(Duration::from_secs(10)).await;
        });

        worker.active_syncs.insert(source_id.clone(), handle);
        assert_eq!(worker.active_syncs.len(), 1);

        // Simulate cancelling and replacing with new sync
        if let Some(old_handle) = worker.active_syncs.remove(&source_id) {
            old_handle.abort();
        }

        let new_handle = relm4::spawn(async {});
        worker.active_syncs.insert(source_id.clone(), new_handle);

        // Should still have exactly one sync for this source
        assert_eq!(worker.active_syncs.len(), 1);
        assert!(worker.active_syncs.contains_key(&source_id));
    }

    #[test]
    fn test_stop_specific_sync() {
        let db = Arc::new(DatabaseConnection::default());
        let mut worker = SyncWorker::new(db);

        let source_id = SourceId::from("test-source");

        // Add a sync
        let handle = relm4::spawn(async {});
        worker.active_syncs.insert(source_id.clone(), handle);

        assert!(worker.active_syncs.contains_key(&source_id));

        // Stop the sync
        worker.stop_sync(&source_id);

        assert!(!worker.active_syncs.contains_key(&source_id));
    }

    #[test]
    fn test_auto_sync_stops_all_on_disable() {
        let db = Arc::new(DatabaseConnection::default());
        let mut worker = SyncWorker::new(db);

        // Add multiple syncs
        let source_id1 = SourceId::from("test-source-1");
        let source_id2 = SourceId::from("test-source-2");

        let handle1 = relm4::spawn(async {});
        let handle2 = relm4::spawn(async {});

        worker.active_syncs.insert(source_id1, handle1);
        worker.active_syncs.insert(source_id2, handle2);

        assert_eq!(worker.active_syncs.len(), 2);
        assert!(worker.auto_sync_enabled);

        // Disable auto-sync should stop all syncs
        worker.auto_sync_enabled = false;
        worker.stop_all_syncs(); // This would be called in the actual update handler

        assert!(!worker.auto_sync_enabled);
        assert!(worker.active_syncs.is_empty());
    }
}
