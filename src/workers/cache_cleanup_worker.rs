use crate::cache::config::FileCacheConfig;
use crate::db::DatabaseConnection;
use crate::db::repository::cache_repository::CacheRepository;
use crate::ui::shared::broker::{BROKER, BrokerMessage, CacheMessage};
use anyhow::Result;
use relm4::{ComponentSender, Worker};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Configuration for cache cleanup operations
#[derive(Debug, Clone)]
pub struct CleanupConfig {
    /// Interval between cleanup runs (default: 1 hour)
    pub cleanup_interval_secs: u64,
    /// Maximum age for cache entries in days (default: 30 days)
    pub max_age_days: i64,
    /// Threshold percentage (0-100) for proactive cleanup (default: 80%)
    pub proactive_threshold_percent: u8,
}

impl Default for CleanupConfig {
    fn default() -> Self {
        Self {
            cleanup_interval_secs: 3600, // 1 hour
            max_age_days: 30,
            proactive_threshold_percent: 80,
        }
    }
}

/// Statistics about a cleanup operation
#[derive(Debug, Clone)]
pub struct CleanupStats {
    pub entries_removed: u64,
    pub space_freed_bytes: i64,
    pub duration_ms: u128,
    pub cleanup_type: CleanupType,
}

#[derive(Debug, Clone)]
pub enum CleanupType {
    TimeBased,
    ProactiveLRU,
    Manual,
}

/// Input messages for the CacheCleanupWorker
#[derive(Debug)]
pub enum CacheCleanupInput {
    /// Start the periodic cleanup timer
    Start,
    /// Stop the periodic cleanup timer
    Stop,
    /// Trigger immediate cleanup
    TriggerCleanup,
    /// Update cleanup configuration
    UpdateConfig(CleanupConfig),
}

/// Output messages from the CacheCleanupWorker
#[derive(Debug, Clone)]
pub enum CacheCleanupOutput {
    /// Cleanup started
    CleanupStarted,
    /// Cleanup completed with statistics
    CleanupCompleted(CleanupStats),
    /// Cleanup failed with error
    CleanupFailed { error: String },
}

/// Worker component for proactive cache cleanup
#[derive(Debug)]
pub struct CacheCleanupWorker {
    db: Arc<DatabaseConnection>,
    cache_config: FileCacheConfig,
    cleanup_config: CleanupConfig,
    is_running: bool,
}

impl CacheCleanupWorker {
    fn new(
        db: Arc<DatabaseConnection>,
        cache_config: FileCacheConfig,
        cleanup_config: CleanupConfig,
    ) -> Self {
        Self {
            db,
            cache_config,
            cleanup_config,
            is_running: false,
        }
    }

    /// Perform time-based cleanup: remove entries older than TTL
    async fn cleanup_old_entries(&self) -> Result<CleanupStats> {
        use crate::db::repository::cache_repository::CacheRepositoryImpl;

        let start = std::time::Instant::now();
        let repo = CacheRepositoryImpl::new(Arc::clone(&self.db));

        info!(
            "Starting time-based cleanup: removing entries older than {} days",
            self.cleanup_config.max_age_days
        );

        let removed_count = repo
            .delete_old_entries(self.cleanup_config.max_age_days)
            .await?;

        // TODO: Calculate actual space freed - for now estimate based on average
        let space_freed = (removed_count as i64) * 100 * 1024 * 1024; // Estimate 100MB per entry

        let duration = start.elapsed();
        info!(
            "Time-based cleanup completed: {} entries removed, ~{} MB freed, took {:?}",
            removed_count,
            space_freed / (1024 * 1024),
            duration
        );

        Ok(CleanupStats {
            entries_removed: removed_count,
            space_freed_bytes: space_freed,
            duration_ms: duration.as_millis(),
            cleanup_type: CleanupType::TimeBased,
        })
    }

    /// Perform proactive LRU cleanup: remove least-accessed entries when approaching limit
    async fn cleanup_lru_entries(&self) -> Result<CleanupStats> {
        use crate::db::repository::cache_repository::CacheRepositoryImpl;

        let start = std::time::Instant::now();
        let repo = CacheRepositoryImpl::new(Arc::clone(&self.db));

        // Get disk space info and calculate dynamic limit
        let disk_info = self.cache_config.get_disk_space_info()?;
        let dynamic_limit = self.cache_config.calculate_dynamic_cache_limit(&disk_info);

        // Get current cache statistics
        let stats = repo.get_cache_statistics().await?;
        if stats.is_none() {
            debug!("No cache statistics available, skipping LRU cleanup");
            return Ok(CleanupStats {
                entries_removed: 0,
                space_freed_bytes: 0,
                duration_ms: start.elapsed().as_millis(),
                cleanup_type: CleanupType::ProactiveLRU,
            });
        }

        let stats = stats.unwrap();
        let current_size = stats.total_size;
        let threshold_bytes = dynamic_limit.cleanup_threshold_bytes as i64;

        debug!(
            "LRU cleanup check: current={} MB, threshold={} MB, effective_limit={} MB",
            current_size / (1024 * 1024),
            threshold_bytes / (1024 * 1024),
            dynamic_limit.effective_limit_bytes / (1024 * 1024)
        );

        // Only cleanup if we're above the threshold
        if current_size < threshold_bytes {
            debug!(
                "Cache size below threshold, skipping LRU cleanup ({} < {})",
                current_size, threshold_bytes
            );
            return Ok(CleanupStats {
                entries_removed: 0,
                space_freed_bytes: 0,
                duration_ms: start.elapsed().as_millis(),
                cleanup_type: CleanupType::ProactiveLRU,
            });
        }

        info!(
            "Starting proactive LRU cleanup: current size {} MB exceeds threshold {} MB",
            current_size / (1024 * 1024),
            threshold_bytes / (1024 * 1024)
        );

        // Calculate how many entries to remove (aim for 70% of limit)
        let target_size = (dynamic_limit.effective_limit_bytes as f64 * 0.70) as i64;
        let bytes_to_free = current_size - target_size;

        if bytes_to_free <= 0 {
            debug!("No cleanup needed");
            return Ok(CleanupStats {
                entries_removed: 0,
                space_freed_bytes: 0,
                duration_ms: start.elapsed().as_millis(),
                cleanup_type: CleanupType::ProactiveLRU,
            });
        }

        // Estimate number of entries to remove (assume average 100MB per entry)
        let avg_entry_size = 100 * 1024 * 1024; // 100MB
        let entries_to_remove = (bytes_to_free / avg_entry_size).max(1) as usize;

        info!(
            "Need to free {} MB, targeting removal of {} entries",
            bytes_to_free / (1024 * 1024),
            entries_to_remove
        );

        // Get LRU entries (least recently accessed)
        let entries_to_delete = repo.get_entries_for_cleanup(entries_to_remove).await?;
        let mut total_freed = 0i64;
        let mut removed_count = 0u64;

        for entry in entries_to_delete {
            let entry_id = entry.id;
            let entry_size = entry.downloaded_bytes;

            debug!(
                "Deleting cache entry {} (size: {} MB)",
                entry_id,
                entry_size / (1024 * 1024)
            );

            // Delete the entry (this will cascade to chunks and headers)
            if let Err(e) = repo.delete_cache_entry(entry_id).await {
                warn!("Failed to delete cache entry {}: {}", entry_id, e);
                continue;
            }

            total_freed += entry_size;
            removed_count += 1;

            // Check if we've freed enough space
            if total_freed >= bytes_to_free {
                break;
            }
        }

        // Update cache statistics
        let new_size = current_size - total_freed;
        let new_count = stats.file_count - (removed_count as i32);
        repo.update_cache_size(new_size, new_count).await?;

        let duration = start.elapsed();
        info!(
            "LRU cleanup completed: {} entries removed, {} MB freed, took {:?}",
            removed_count,
            total_freed / (1024 * 1024),
            duration
        );

        Ok(CleanupStats {
            entries_removed: removed_count,
            space_freed_bytes: total_freed,
            duration_ms: duration.as_millis(),
            cleanup_type: CleanupType::ProactiveLRU,
        })
    }

    /// Perform full cleanup: time-based + LRU
    async fn perform_cleanup(&self, sender: &ComponentSender<Self>) -> Result<()> {
        info!("Starting cache cleanup cycle");

        // Send cleanup started message
        sender.output(CacheCleanupOutput::CleanupStarted).ok();

        // Broadcast cleanup started via MessageBroker
        BROKER
            .broadcast(BrokerMessage::Cache(CacheMessage::CleanupStarted))
            .await;

        // First, do time-based cleanup
        match self.cleanup_old_entries().await {
            Ok(stats) => {
                if stats.entries_removed > 0 {
                    sender
                        .output(CacheCleanupOutput::CleanupCompleted(stats.clone()))
                        .ok();

                    // Broadcast via MessageBroker
                    BROKER
                        .broadcast(BrokerMessage::Cache(CacheMessage::CleanupCompleted {
                            entries_removed: stats.entries_removed,
                            space_freed_mb: stats.space_freed_bytes / (1024 * 1024),
                            duration_ms: stats.duration_ms,
                            cleanup_type: format!("{:?}", stats.cleanup_type),
                        }))
                        .await;
                }
            }
            Err(e) => {
                warn!("Time-based cleanup failed: {}", e);
                let error_msg = format!("Time-based cleanup failed: {}", e);
                sender
                    .output(CacheCleanupOutput::CleanupFailed {
                        error: error_msg.clone(),
                    })
                    .ok();

                // Broadcast failure via MessageBroker
                BROKER
                    .broadcast(BrokerMessage::Cache(CacheMessage::CleanupFailed {
                        error: error_msg,
                    }))
                    .await;
            }
        }

        // Then, do proactive LRU cleanup
        match self.cleanup_lru_entries().await {
            Ok(stats) => {
                if stats.entries_removed > 0 {
                    sender
                        .output(CacheCleanupOutput::CleanupCompleted(stats.clone()))
                        .ok();

                    // Broadcast via MessageBroker
                    BROKER
                        .broadcast(BrokerMessage::Cache(CacheMessage::CleanupCompleted {
                            entries_removed: stats.entries_removed,
                            space_freed_mb: stats.space_freed_bytes / (1024 * 1024),
                            duration_ms: stats.duration_ms,
                            cleanup_type: format!("{:?}", stats.cleanup_type),
                        }))
                        .await;
                }
            }
            Err(e) => {
                warn!("LRU cleanup failed: {}", e);
                let error_msg = format!("LRU cleanup failed: {}", e);
                sender
                    .output(CacheCleanupOutput::CleanupFailed {
                        error: error_msg.clone(),
                    })
                    .ok();

                // Broadcast failure via MessageBroker
                BROKER
                    .broadcast(BrokerMessage::Cache(CacheMessage::CleanupFailed {
                        error: error_msg,
                    }))
                    .await;
            }
        }

        Ok(())
    }

    fn start_timer(&mut self) {
        self.is_running = true;
        info!(
            "Cache cleanup timer started with interval of {} seconds",
            self.cleanup_config.cleanup_interval_secs
        );
    }

    fn stop_timer(&mut self) {
        self.is_running = false;
        info!("Cache cleanup timer stopped");
    }
}

impl Worker for CacheCleanupWorker {
    type Init = (Arc<DatabaseConnection>, FileCacheConfig, CleanupConfig);
    type Input = CacheCleanupInput;
    type Output = CacheCleanupOutput;

    fn init(init: Self::Init, _sender: ComponentSender<Self>) -> Self {
        let (db, cache_config, cleanup_config) = init;
        info!("Initializing CacheCleanupWorker");
        Self::new(db, cache_config, cleanup_config)
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            CacheCleanupInput::Start => {
                info!("CacheCleanupWorker received Start command");
                self.start_timer();

                // Spawn a task to handle periodic cleanup
                if self.is_running {
                    let cleanup_sender = sender.clone();
                    relm4::spawn(async move {
                        loop {
                            tokio::time::sleep(Duration::from_secs(3600)).await;
                            cleanup_sender.input(CacheCleanupInput::TriggerCleanup);
                        }
                    });
                }
            }

            CacheCleanupInput::Stop => {
                info!("CacheCleanupWorker received Stop command");
                self.stop_timer();
            }

            CacheCleanupInput::TriggerCleanup => {
                info!("Manual cleanup triggered");
                let worker_clone = Self {
                    db: self.db.clone(),
                    cache_config: self.cache_config.clone(),
                    cleanup_config: self.cleanup_config.clone(),
                    is_running: false,
                };
                let cleanup_sender = sender.clone();

                relm4::spawn(async move {
                    if let Err(e) = worker_clone.perform_cleanup(&cleanup_sender).await {
                        warn!("Cleanup failed: {}", e);
                        cleanup_sender
                            .output(CacheCleanupOutput::CleanupFailed {
                                error: e.to_string(),
                            })
                            .ok();
                    }
                });
            }

            CacheCleanupInput::UpdateConfig(new_config) => {
                info!("Updating cleanup configuration: {:?}", new_config);
                self.cleanup_config = new_config;

                // Restart timer if running
                if self.is_running {
                    self.start_timer();
                }
            }
        }
    }
}
