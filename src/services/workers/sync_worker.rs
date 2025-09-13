use anyhow::Result;
use relm4::{ComponentSender, Worker};
use std::sync::Arc;
use tracing::{debug, error, info};

use crate::backends::traits::MediaBackend;
use crate::db::connection::DatabaseConnection;
use crate::models::SourceId;
use crate::services::core::{SyncProgress, SyncService};

/// Messages that can be sent to the SyncWorker
#[derive(Debug, Clone)]
pub enum SyncWorkerInput {
    /// Start syncing a source
    StartSync {
        source_id: SourceId,
        backend: Arc<dyn MediaBackend>,
    },
    /// Cancel current sync
    CancelSync,
    /// Get current sync progress
    GetProgress,
}

/// Messages sent from the SyncWorker
#[derive(Debug, Clone)]
pub enum SyncWorkerOutput {
    /// Sync started
    SyncStarted(SourceId),
    /// Sync progress update
    Progress(SourceId, SyncProgress),
    /// Sync completed successfully
    SyncCompleted(SourceId, usize, usize), // libraries, items
    /// Sync failed
    SyncFailed(SourceId, String),
    /// Sync cancelled
    SyncCancelled(SourceId),
}

/// Worker for background synchronization
pub struct SyncWorker {
    db: DatabaseConnection,
    current_sync: Option<SourceId>,
    cancel_requested: bool,
}

impl SyncWorker {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            current_sync: None,
            cancel_requested: false,
        }
    }

    async fn perform_sync(
        &mut self,
        source_id: SourceId,
        backend: Arc<dyn MediaBackend>,
        sender: ComponentSender<Self>,
    ) {
        info!("Starting sync for source: {}", source_id);
        self.current_sync = Some(source_id.clone());
        self.cancel_requested = false;

        // Notify sync started
        let _ = sender.output(SyncWorkerOutput::SyncStarted(source_id.clone()));

        // Perform the sync
        match SyncService::sync_source(&self.db, backend.as_ref(), &source_id).await {
            Ok(result) => {
                if self.cancel_requested {
                    let _ = sender.output(SyncWorkerOutput::SyncCancelled(source_id));
                } else {
                    let _ = sender.output(SyncWorkerOutput::SyncCompleted(
                        source_id,
                        result.libraries_synced,
                        result.items_synced,
                    ));
                }
            }
            Err(e) => {
                error!("Sync failed for source {}: {}", source_id, e);
                let _ = sender.output(SyncWorkerOutput::SyncFailed(source_id, e.to_string()));
            }
        }

        self.current_sync = None;
    }

    async fn get_progress(&self, sender: ComponentSender<Self>) {
        if let Some(ref source_id) = self.current_sync {
            match SyncService::get_sync_progress(&self.db, source_id).await {
                Ok(progress) => {
                    let _ = sender.output(SyncWorkerOutput::Progress(source_id.clone(), progress));
                }
                Err(e) => {
                    error!("Failed to get sync progress: {}", e);
                }
            }
        }
    }
}

impl Worker for SyncWorker {
    type Init = DatabaseConnection;
    type Input = SyncWorkerInput;
    type Output = SyncWorkerOutput;

    fn init(db: Self::Init, _sender: ComponentSender<Self>) -> Self {
        Self::new(db)
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            SyncWorkerInput::StartSync { source_id, backend } => {
                if self.current_sync.is_some() {
                    error!("Sync already in progress, ignoring new request");
                    return;
                }

                let mut worker = self.clone();
                relm4::spawn(async move {
                    worker.perform_sync(source_id, backend, sender).await;
                });
            }
            SyncWorkerInput::CancelSync => {
                if self.current_sync.is_some() {
                    info!("Sync cancellation requested");
                    self.cancel_requested = true;
                }
            }
            SyncWorkerInput::GetProgress => {
                let worker = self.clone();
                relm4::spawn(async move {
                    worker.get_progress(sender).await;
                });
            }
        }
    }
}

// Manual Clone implementation to work around non-Clone DatabaseConnection
impl Clone for SyncWorker {
    fn clone(&self) -> Self {
        Self {
            db: self.db.clone(),
            current_sync: self.current_sync.clone(),
            cancel_requested: self.cancel_requested,
        }
    }
}
