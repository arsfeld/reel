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
        let start_time = Instant::now();

        // Send sync started
        sender
            .output(SyncWorkerOutput::SyncStarted {
                source_id: source_id.clone(),
                library_id: library_id.clone(),
            })
            .ok();

        // Call actual sync service using stateless BackendService
        match BackendService::sync_source(&db, &source_id).await {
            Ok(sync_result) => {
                sender
                    .output(SyncWorkerOutput::SyncCompleted {
                        source_id,
                        library_id,
                        items_synced: sync_result.items_synced,
                        duration: start_time.elapsed(),
                    })
                    .ok();
            }
            Err(e) => {
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
        if !force {
            if let Some(last_sync) = self.last_sync_times.get(&source_id) {
                if last_sync.elapsed() < self.sync_interval {
                    info!(
                        "Skipping sync for {:?}, too soon since last sync",
                        source_id
                    );
                    return;
                }
            }
        }

        // Start new sync
        let db = self.db.clone();
        let source_id_clone = source_id.clone();
        let handle = relm4::spawn(async move {
            Self::perform_sync(db, source_id_clone, library_id, force, sender).await;
        });

        self.active_syncs.insert(source_id.clone(), handle);
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
        }
    }
}

// Helper function to create a sync worker instance
pub fn create_sync_worker(db: Arc<DatabaseConnection>) -> WorkerHandle<SyncWorker> {
    SyncWorker::builder().detach_worker(db)
}
