use crate::db::DatabaseConnection;
use crate::models::{LibraryId, SourceId};
use crate::services::core::backend::BackendService;
use relm4::prelude::*;
use relm4::{ComponentSender, Worker, WorkerHandle};
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

        info!("Calling BackendService::sync_source for {:?}", source_id);
        // Call actual sync service using stateless BackendService
        match BackendService::sync_source(&db, &source_id).await {
            Ok(sync_result) => {
                info!(
                    "Sync succeeded for {:?}: {} items",
                    source_id, sync_result.items_synced
                );

                // Send the sync completed output
                sender
                    .output(SyncWorkerOutput::SyncCompleted {
                        source_id: source_id.clone(),
                        library_id,
                        items_synced: sync_result.items_synced,
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

// Helper function to create a sync worker instance
pub fn create_sync_worker(db: Arc<DatabaseConnection>) -> WorkerHandle<SyncWorker> {
    SyncWorker::builder().detach_worker(db)
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
