/// Consolidated worker components for background tasks
pub mod connection_monitor;
pub mod image_loader;
pub mod search_worker;
pub mod sync_worker;

// Test modules
#[cfg(test)]
mod connection_monitor_tests;
#[cfg(test)]
mod search_worker_tests;

// Re-export commonly used types
pub use connection_monitor::{ConnectionMonitor, ConnectionMonitorInput, ConnectionMonitorOutput};
pub use image_loader::{ImageLoader, ImageLoaderInput, ImageLoaderOutput, ImageRequest, ImageSize};
pub use sync_worker::{SyncWorker, SyncWorkerInput, SyncWorkerOutput};
