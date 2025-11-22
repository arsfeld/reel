use crate::db::DatabaseConnection;
use crate::db::entities::{PlaybackSyncStatus, SyncChangeType};
use crate::db::repository::{PlaybackSyncRepository, PlaybackSyncRepositoryImpl};
use crate::models::SourceId;
use crate::services::core::backend::BackendService;
use relm4::{ComponentSender, Worker};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

/// Configuration for sync worker behavior
#[derive(Debug, Clone)]
pub struct SyncConfig {
    /// How often to poll the queue (default: 5 seconds)
    pub poll_interval: Duration,
    /// Maximum retry attempts before marking as permanently failed (default: 5)
    pub max_attempts: i32,
    /// Base delay for exponential backoff (default: 1 second)
    pub base_backoff: Duration,
    /// Maximum backoff delay (default: 60 seconds)
    pub max_backoff: Duration,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(5),
            max_attempts: 5,
            base_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(60),
        }
    }
}

#[derive(Debug, Clone)]
pub enum PlaybackSyncWorkerInput {
    /// Process the queue once
    ProcessQueue,
    /// Retry failed items that are eligible for retry
    RetryFailed,
    /// Pause sync operations
    PauseSync,
    /// Resume sync operations
    ResumeSync,
    /// Update configuration
    UpdateConfig(SyncConfig),
    /// Trigger immediate sync for a specific media item
    SyncImmediate {
        media_item_id: String,
        source_id: i32,
    },
}

#[derive(Debug, Clone)]
pub enum PlaybackSyncWorkerOutput {
    /// Sync queue processing started
    SyncStarted { pending_count: usize },
    /// Progress update during sync
    SyncProgress {
        synced: usize,
        failed: usize,
        remaining: usize,
    },
    /// Sync batch completed
    SyncCompleted {
        synced: usize,
        failed: usize,
        duration: Duration,
    },
    /// A single item failed to sync
    ItemSyncFailed {
        media_item_id: String,
        error: String,
        attempt_count: i32,
    },
    /// Sync was paused
    SyncPaused,
    /// Sync was resumed
    SyncResumed,
}

#[derive(Debug)]
pub struct PlaybackSyncWorker {
    db: Arc<DatabaseConnection>,
    config: SyncConfig,
    is_paused: bool,
    sync_handle: Option<relm4::JoinHandle<()>>,
    /// Track last sync attempt per source to avoid hammering failed sources
    last_attempt_times: HashMap<i32, Instant>,
}

impl PlaybackSyncWorker {
    fn new(db: Arc<DatabaseConnection>) -> Self {
        Self {
            db,
            config: SyncConfig::default(),
            is_paused: false,
            sync_handle: None,
            last_attempt_times: HashMap::new(),
        }
    }

    fn start_queue_processor(&mut self, sender: ComponentSender<Self>) {
        // Cancel existing processor if running
        if let Some(handle) = self.sync_handle.take() {
            handle.abort();
        }

        let _db = self.db.clone();
        let config = self.config.clone();

        // Spawn the queue processor
        self.sync_handle = Some(relm4::spawn(async move {
            loop {
                // Send process queue message
                sender.input(PlaybackSyncWorkerInput::ProcessQueue);

                // Wait for next poll interval
                sleep(config.poll_interval).await;
            }
        }));
    }

    fn stop_queue_processor(&mut self) {
        if let Some(handle) = self.sync_handle.take() {
            handle.abort();
        }
    }

    async fn process_queue(
        db: Arc<DatabaseConnection>,
        config: SyncConfig,
        sender: ComponentSender<PlaybackSyncWorker>,
    ) {
        let start_time = Instant::now();
        let repo = PlaybackSyncRepositoryImpl::new(db.as_ref().clone());

        // Get all pending items
        let pending_items = match repo.get_pending().await {
            Ok(items) => items,
            Err(e) => {
                error!("Failed to fetch pending sync queue items: {}", e);
                return;
            }
        };

        if pending_items.is_empty() {
            debug!("No pending items in sync queue");
            return;
        }

        info!("Processing {} pending sync items", pending_items.len());
        sender
            .output(PlaybackSyncWorkerOutput::SyncStarted {
                pending_count: pending_items.len(),
            })
            .ok();

        // Group items by source for batching
        let mut items_by_source: HashMap<i32, Vec<_>> = HashMap::new();
        for item in pending_items {
            items_by_source
                .entry(item.source_id)
                .or_default()
                .push(item);
        }

        let mut total_synced = 0;
        let mut total_failed = 0;
        let total_items: usize = items_by_source.values().map(|v| v.len()).sum();

        // Process each source's items
        for (source_id, items) in &items_by_source {
            debug!("Processing {} items for source {}", items.len(), source_id);

            // Deduplicate items for the same media (keep most recent)
            let deduplicated = Self::deduplicate_items(items.clone());
            debug!(
                "After deduplication: {} items for source {}",
                deduplicated.len(),
                source_id
            );

            // Process each item
            for item in deduplicated {
                match repo.mark_syncing(item.id).await {
                    Ok(_) => {
                        debug!("Marked item {} as syncing", item.id);
                    }
                    Err(e) => {
                        error!("Failed to mark item {} as syncing: {}", item.id, e);
                        continue;
                    }
                }

                // Sync the item
                match Self::sync_item(&db, &item).await {
                    Ok(_) => {
                        debug!("Successfully synced item {}", item.id);
                        if let Err(e) = repo.mark_synced(item.id).await {
                            error!("Failed to mark item {} as synced: {}", item.id, e);
                        }
                        total_synced += 1;
                    }
                    Err(e) => {
                        error!("Failed to sync item {}: {}", item.id, e);
                        let error_msg = e.to_string();
                        if let Err(e) = repo.mark_failed(item.id, &error_msg).await {
                            error!("Failed to mark item {} as failed: {}", item.id, e);
                        }
                        total_failed += 1;

                        sender
                            .output(PlaybackSyncWorkerOutput::ItemSyncFailed {
                                media_item_id: item.media_item_id.clone(),
                                error: error_msg,
                                attempt_count: item.attempt_count + 1,
                            })
                            .ok();
                    }
                }

                // Emit progress
                let remaining = total_items - total_synced - total_failed;

                sender
                    .output(PlaybackSyncWorkerOutput::SyncProgress {
                        synced: total_synced,
                        failed: total_failed,
                        remaining,
                    })
                    .ok();
            }
        }

        let duration = start_time.elapsed();
        info!(
            "Sync queue processing completed: {} synced, {} failed in {:?}",
            total_synced, total_failed, duration
        );

        sender
            .output(PlaybackSyncWorkerOutput::SyncCompleted {
                synced: total_synced,
                failed: total_failed,
                duration,
            })
            .ok();
    }

    /// Deduplicate items for the same media item, keeping only the most recent
    fn deduplicate_items(
        items: Vec<crate::db::entities::PlaybackSyncQueueModel>,
    ) -> Vec<crate::db::entities::PlaybackSyncQueueModel> {
        let mut latest_items: HashMap<String, crate::db::entities::PlaybackSyncQueueModel> =
            HashMap::new();

        for item in items {
            let key = item.media_item_id.clone();
            match latest_items.get(&key) {
                Some(existing) if existing.created_at > item.created_at => {
                    // Keep existing (it's newer)
                }
                _ => {
                    // Replace with this item (it's newer or first)
                    latest_items.insert(key, item);
                }
            }
        }

        latest_items.into_values().collect()
    }

    /// Sync a single item to the backend
    async fn sync_item(
        db: &Arc<DatabaseConnection>,
        item: &crate::db::entities::PlaybackSyncQueueModel,
    ) -> anyhow::Result<()> {
        use crate::db::repository::Repository;
        use crate::db::repository::source_repository::SourceRepositoryImpl;

        // Load source
        let source_repo = SourceRepositoryImpl::new(db.as_ref().clone());
        let source = source_repo
            .find_by_id(&item.source_id.to_string())
            .await?
            .ok_or_else(|| anyhow::anyhow!("Source {} not found", item.source_id))?;

        // Create backend
        let backend = BackendService::create_backend_for_source(db, &source).await?;

        // Parse change type
        let change_type = item.get_change_type().map_err(|e| anyhow::anyhow!(e))?;

        // Execute the appropriate sync operation
        match change_type {
            SyncChangeType::ProgressUpdate => {
                use crate::models::MediaItemId;

                let position_ms = item
                    .position_ms
                    .ok_or_else(|| anyhow::anyhow!("Missing position_ms for progress update"))?;

                let media_id = MediaItemId::new(item.media_item_id.clone());
                let position = Duration::from_millis(position_ms as u64);
                // For duration, we'll use 0 as a placeholder since we don't have it in the queue
                let duration = Duration::from_secs(0);

                backend
                    .update_progress(&media_id, position, duration)
                    .await?;

                debug!(
                    "Updated playback progress for {} to {}ms",
                    item.media_item_id, position_ms
                );
            }
            SyncChangeType::MarkWatched => {
                backend.mark_watched(&item.media_item_id).await?;
                debug!("Marked {} as watched", item.media_item_id);
            }
            SyncChangeType::MarkUnwatched => {
                backend.mark_unwatched(&item.media_item_id).await?;
                debug!("Marked {} as unwatched", item.media_item_id);
            }
        }

        Ok(())
    }

    async fn retry_failed_items(
        db: Arc<DatabaseConnection>,
        config: SyncConfig,
        sender: ComponentSender<PlaybackSyncWorker>,
    ) {
        let repo = PlaybackSyncRepositoryImpl::new(db.as_ref().clone());

        // Get failed items that can be retried
        let failed_items = match repo.get_failed_retryable(config.max_attempts).await {
            Ok(items) => items,
            Err(e) => {
                error!("Failed to fetch retryable items: {}", e);
                return;
            }
        };

        if failed_items.is_empty() {
            debug!("No failed items eligible for retry");
            return;
        }

        info!("Retrying {} failed items", failed_items.len());

        for item in failed_items {
            // Calculate backoff delay
            let backoff_delay = Self::calculate_backoff(&config, item.attempt_count);

            // Check if enough time has passed since last attempt
            if let Some(last_attempt) = item.last_attempt_at {
                let elapsed = chrono::Utc::now().naive_utc() - last_attempt;
                let elapsed_duration = Duration::from_secs(elapsed.num_seconds() as u64);

                if elapsed_duration < backoff_delay {
                    debug!(
                        "Skipping item {} - backoff not elapsed ({:?} < {:?})",
                        item.id, elapsed_duration, backoff_delay
                    );
                    continue;
                }
            }

            // Mark as syncing
            if let Err(e) = repo.mark_syncing(item.id).await {
                error!("Failed to mark item {} as syncing: {}", item.id, e);
                continue;
            }

            // Retry sync
            match Self::sync_item(&db, &item).await {
                Ok(_) => {
                    info!("Successfully retried item {}", item.id);
                    if let Err(e) = repo.mark_synced(item.id).await {
                        error!("Failed to mark item {} as synced: {}", item.id, e);
                    }
                }
                Err(e) => {
                    warn!("Retry failed for item {}: {}", item.id, e);
                    let error_msg = e.to_string();
                    if let Err(e) = repo.mark_failed(item.id, &error_msg).await {
                        error!("Failed to mark item {} as failed: {}", item.id, e);
                    }

                    sender
                        .output(PlaybackSyncWorkerOutput::ItemSyncFailed {
                            media_item_id: item.media_item_id.clone(),
                            error: error_msg,
                            attempt_count: item.attempt_count + 1,
                        })
                        .ok();
                }
            }
        }
    }

    /// Calculate exponential backoff delay
    fn calculate_backoff(config: &SyncConfig, attempt_count: i32) -> Duration {
        let multiplier = 2_u32.pow(attempt_count.saturating_sub(1).max(0) as u32);
        let delay = config.base_backoff * multiplier;
        delay.min(config.max_backoff)
    }
}

impl Worker for PlaybackSyncWorker {
    type Init = Arc<DatabaseConnection>;
    type Input = PlaybackSyncWorkerInput;
    type Output = PlaybackSyncWorkerOutput;

    fn init(db: Self::Init, sender: ComponentSender<Self>) -> Self {
        let mut worker = Self::new(db);
        // Start the queue processor immediately
        worker.start_queue_processor(sender);
        worker
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            PlaybackSyncWorkerInput::ProcessQueue => {
                if self.is_paused {
                    debug!("Sync is paused, skipping queue processing");
                    return;
                }

                let db = self.db.clone();
                let config = self.config.clone();
                let sender = sender.clone();

                relm4::spawn(async move {
                    Self::process_queue(db, config, sender).await;
                });
            }

            PlaybackSyncWorkerInput::RetryFailed => {
                if self.is_paused {
                    debug!("Sync is paused, skipping retry");
                    return;
                }

                let db = self.db.clone();
                let config = self.config.clone();
                let sender = sender.clone();

                relm4::spawn(async move {
                    Self::retry_failed_items(db, config, sender).await;
                });
            }

            PlaybackSyncWorkerInput::PauseSync => {
                info!("Pausing playback sync worker");
                self.is_paused = true;
                self.stop_queue_processor();
                sender.output(PlaybackSyncWorkerOutput::SyncPaused).ok();
            }

            PlaybackSyncWorkerInput::ResumeSync => {
                info!("Resuming playback sync worker");
                self.is_paused = false;
                self.start_queue_processor(sender.clone());
                sender.output(PlaybackSyncWorkerOutput::SyncResumed).ok();
            }

            PlaybackSyncWorkerInput::UpdateConfig(config) => {
                info!("Updating sync worker configuration: {:?}", config);
                self.config = config.clone();

                // Restart processor with new poll interval
                if !self.is_paused {
                    self.start_queue_processor(sender);
                }
            }

            PlaybackSyncWorkerInput::SyncImmediate {
                media_item_id,
                source_id,
            } => {
                info!(
                    "Immediate sync requested for media {} on source {}",
                    media_item_id, source_id
                );

                let db = self.db.clone();
                let sender = sender.clone();

                relm4::spawn(async move {
                    let repo = PlaybackSyncRepositoryImpl::new(db.as_ref().clone());

                    // Get items for this media
                    match repo.get_by_media_item(&media_item_id, source_id).await {
                        Ok(items) => {
                            for item in items {
                                if item.status == PlaybackSyncStatus::Pending.to_string()
                                    || item.status == PlaybackSyncStatus::Failed.to_string()
                                {
                                    // Mark as syncing
                                    if let Err(e) = repo.mark_syncing(item.id).await {
                                        error!("Failed to mark item {} as syncing: {}", item.id, e);
                                        continue;
                                    }

                                    // Sync immediately
                                    match Self::sync_item(&db, &item).await {
                                        Ok(_) => {
                                            info!(
                                                "Successfully synced item {} immediately",
                                                item.id
                                            );
                                            if let Err(e) = repo.mark_synced(item.id).await {
                                                error!(
                                                    "Failed to mark item {} as synced: {}",
                                                    item.id, e
                                                );
                                            }
                                        }
                                        Err(e) => {
                                            error!(
                                                "Failed to sync item {} immediately: {}",
                                                item.id, e
                                            );
                                            let error_msg = e.to_string();
                                            if let Err(e) =
                                                repo.mark_failed(item.id, &error_msg).await
                                            {
                                                error!(
                                                    "Failed to mark item {} as failed: {}",
                                                    item.id, e
                                                );
                                            }

                                            sender
                                                .output(PlaybackSyncWorkerOutput::ItemSyncFailed {
                                                    media_item_id: item.media_item_id.clone(),
                                                    error: error_msg,
                                                    attempt_count: item.attempt_count + 1,
                                                })
                                                .ok();
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to get items for immediate sync: {}", e);
                        }
                    }
                });
            }
        }
    }
}

impl Drop for PlaybackSyncWorker {
    fn drop(&mut self) {
        // Stop the queue processor on drop
        self.stop_queue_processor();
    }
}
