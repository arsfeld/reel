/// Consolidated worker components for background tasks
pub mod cache_cleanup_worker;
pub mod config_manager;
pub mod connection_monitor;
pub mod image_loader;
pub mod playback_sync_worker;
pub mod search_worker;
pub mod sync_worker;

// Test modules
#[cfg(test)]
mod connection_monitor_tests;
#[cfg(test)]
mod search_worker_tests;

// Re-export commonly used types
pub use cache_cleanup_worker::{
    CacheCleanupInput, CacheCleanupOutput, CacheCleanupWorker, CleanupConfig, CleanupStats,
    CleanupType,
};
pub use connection_monitor::{ConnectionMonitor, ConnectionMonitorInput, ConnectionMonitorOutput};
pub use image_loader::{ImageLoader, ImageLoaderInput, ImageLoaderOutput, ImageRequest, ImageSize};
pub use playback_sync_worker::{
    PlaybackSyncWorker, PlaybackSyncWorkerInput, PlaybackSyncWorkerOutput, SyncConfig,
};
pub use search_worker::{SearchWorker, SearchWorkerInput, SearchWorkerOutput};
pub use sync_worker::{SyncWorker, SyncWorkerInput, SyncWorkerOutput};
