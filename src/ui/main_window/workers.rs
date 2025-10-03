use relm4::prelude::*;
use std::sync::Arc;
use tokio::runtime::Runtime;

use crate::db::connection::DatabaseConnection;
use crate::models::SourceId;
use crate::workers::{
    ConnectionMonitor, ConnectionMonitorInput, ConnectionMonitorOutput, SearchWorker,
    SearchWorkerInput, SearchWorkerOutput, SyncWorker, SyncWorkerInput, SyncWorkerOutput,
    cache_cleanup_worker::{
        CacheCleanupInput, CacheCleanupOutput, CacheCleanupWorker, CleanupConfig,
    },
    config_manager::{ConfigManager, ConfigManagerOutput},
};

use super::{ConnectionStatus, MainWindowInput};

/// Container for all worker controllers
pub struct Workers {
    pub config_manager: relm4::WorkerController<ConfigManager>,
    pub connection_monitor: relm4::WorkerController<ConnectionMonitor>,
    pub sync_worker: relm4::WorkerController<SyncWorker>,
    pub search_worker: relm4::WorkerController<SearchWorker>,
    pub cache_cleanup_worker: relm4::WorkerController<CacheCleanupWorker>,
}

/// Initialize all background workers
pub fn initialize_workers(
    db: DatabaseConnection,
    runtime: Arc<Runtime>,
    sender: &AsyncComponentSender<super::MainWindow>,
) -> Workers {
    // Initialize the ConfigManager with file watcher
    tracing::info!("Initializing ConfigManager with file watcher...");
    let config_manager =
        ConfigManager::builder()
            .detach_worker(())
            .forward(sender.input_sender(), |output| {
                match output {
                    ConfigManagerOutput::ConfigLoaded(config) => {
                        tracing::info!("Config reloaded from disk via file watcher");
                        // Update the global config service
                        let config_service = crate::services::config_service::config_service();
                        relm4::spawn_local(async move {
                            let _ = config_service.update_config((*config).clone()).await;
                        });
                        MainWindowInput::ConfigUpdated
                    }
                    ConfigManagerOutput::ConfigUpdated(_) => {
                        tracing::info!("Config updated programmatically");
                        MainWindowInput::ConfigUpdated
                    }
                    ConfigManagerOutput::Error(err) => {
                        tracing::error!("Config manager error: {}", err);
                        MainWindowInput::ConfigUpdated
                    }
                }
            });
    tracing::info!("ConfigManager with file watcher initialized successfully");

    // Initialize the ConnectionMonitor worker
    let runtime_handle = runtime.handle().clone();
    let connection_monitor = ConnectionMonitor::builder()
        .detach_worker((db.clone(), runtime_handle.clone()))
        .forward(sender.input_sender(), |output| {
            tracing::info!("📡 ConnectionMonitor output: {:?}", output);
            match output {
                ConnectionMonitorOutput::ConnectionChanged {
                    source_id,
                    new_url,
                    connection_type,
                } => {
                    tracing::info!(
                        "📡 ConnectionChanged: {} -> {} ({:?})",
                        source_id,
                        new_url,
                        connection_type
                    );
                    MainWindowInput::ConnectionStatusChanged {
                        source_id,
                        status: ConnectionStatus::Connected {
                            url: new_url,
                            connection_type,
                        },
                    }
                }
                ConnectionMonitorOutput::ConnectionLost { source_id } => {
                    tracing::info!("📡 ConnectionLost: {}", source_id);
                    MainWindowInput::ConnectionStatusChanged {
                        source_id,
                        status: ConnectionStatus::Disconnected,
                    }
                }
                ConnectionMonitorOutput::ConnectionRestored {
                    source_id,
                    url,
                    connection_type,
                } => {
                    tracing::info!(
                        "📡 ConnectionRestored: {} -> {} ({:?})",
                        source_id,
                        url,
                        connection_type
                    );
                    MainWindowInput::ConnectionStatusChanged {
                        source_id,
                        status: ConnectionStatus::Connected {
                            url,
                            connection_type,
                        },
                    }
                }
            }
        });

    // Start monitoring connections periodically
    ConnectionMonitor::start_monitoring(
        connection_monitor.sender().clone(),
        runtime_handle.clone(),
    );

    // Trigger an immediate initial check to populate connection types
    connection_monitor.emit(ConnectionMonitorInput::CheckAllSources);

    // Initialize the SyncWorker
    let sync_worker = SyncWorker::builder()
        .detach_worker(Arc::new(db.clone()))
        .forward(sender.input_sender(), move |output| match output {
            SyncWorkerOutput::SyncStarted { source_id, .. } => {
                tracing::info!("Sync started for source: {:?}", source_id);
                MainWindowInput::ShowToast("Syncing source...".to_string())
            }
            SyncWorkerOutput::SyncProgress(progress) => {
                tracing::debug!(
                    "Sync progress for {:?}: {}/{}",
                    progress.source_id,
                    progress.current,
                    progress.total
                );
                // Could be used to update UI progress indicators
                MainWindowInput::ShowToast(progress.message)
            }
            SyncWorkerOutput::SyncCompleted {
                source_id,
                items_synced,
                sections_synced,
                ..
            } => {
                tracing::info!(
                    "Sync completed for {:?}: {} items, {} sections",
                    source_id,
                    items_synced,
                    sections_synced
                );

                // Broadcast sync completion to all subscribed components
                let source_id_str = source_id.to_string();
                relm4::spawn(async move {
                    use crate::ui::shared::broker::{BROKER, BrokerMessage, SourceMessage};
                    BROKER
                        .broadcast(BrokerMessage::Source(SourceMessage::SyncCompleted {
                            source_id: source_id_str,
                            items_synced,
                        }))
                        .await;
                });

                // Note: We no longer navigate to home after sync completes.
                // The home page will be updated through data change events
                // without forcing navigation away from the user's current page.

                // Trigger search index refresh after sync
                MainWindowInput::Navigate("refresh_search_index".to_string())
            }
            SyncWorkerOutput::SyncFailed {
                source_id, error, ..
            } => {
                tracing::error!("Sync failed for {:?}: {}", source_id, error);
                MainWindowInput::ShowToast(format!("Sync failed: {}", error))
            }
            SyncWorkerOutput::SyncCancelled { source_id } => {
                tracing::info!("Sync cancelled for {:?}", source_id);
                MainWindowInput::ShowToast("Sync cancelled".to_string())
            }
        });

    // Initialize the SearchWorker
    let search_worker = SearchWorker::builder().detach_worker(db.clone()).forward(
        sender.input_sender(),
        |output| match output {
            SearchWorkerOutput::SearchResults {
                query,
                results,
                total_hits,
            } => {
                tracing::info!(
                    "Search for '{}' returned {} results ({} total hits)",
                    query,
                    results.len(),
                    total_hits
                );
                MainWindowInput::SearchResultsReceived {
                    query,
                    results,
                    total_hits,
                }
            }
            SearchWorkerOutput::IndexingComplete { documents_indexed } => {
                tracing::info!("Search index completed: {} documents", documents_indexed);
                MainWindowInput::ShowToast(format!(
                    "Indexed {} items for search",
                    documents_indexed
                ))
            }
            SearchWorkerOutput::Error(error) => {
                tracing::error!("Search error: {}", error);
                MainWindowInput::ShowToast(format!("Search error: {}", error))
            }
            _ => {
                // Handle other search worker outputs
                MainWindowInput::ShowToast("Search operation completed".to_string())
            }
        },
    );

    // Initialize the CacheCleanupWorker
    // Use default cache config for now
    // TODO: Update this to use config from ConfigService once we support config updates
    let cache_config = crate::cache::config::FileCacheConfig::default();
    let cleanup_config = CleanupConfig::default();
    let cache_cleanup_worker = CacheCleanupWorker::builder()
        .detach_worker((Arc::new(db.clone()), cache_config, cleanup_config))
        .forward(sender.input_sender(), |output| match output {
            CacheCleanupOutput::CleanupStarted => {
                tracing::info!("Cache cleanup started");
                MainWindowInput::ShowToast("Cache cleanup started...".to_string())
            }
            CacheCleanupOutput::CleanupCompleted(stats) => {
                tracing::info!(
                    "Cache cleanup completed: {} entries removed, {} MB freed",
                    stats.entries_removed,
                    stats.space_freed_bytes / (1024 * 1024)
                );
                MainWindowInput::ShowToast(format!(
                    "Cache cleanup: {} entries removed, {} MB freed",
                    stats.entries_removed,
                    stats.space_freed_bytes / (1024 * 1024)
                ))
            }
            CacheCleanupOutput::CleanupFailed { error } => {
                tracing::error!("Cache cleanup failed: {}", error);
                MainWindowInput::ShowToast(format!("Cache cleanup failed: {}", error))
            }
        });

    // Start the cache cleanup worker
    cache_cleanup_worker.emit(CacheCleanupInput::Start);

    Workers {
        config_manager,
        connection_monitor,
        sync_worker,
        search_worker,
        cache_cleanup_worker,
    }
}
