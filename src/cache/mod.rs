pub mod chunk_downloader;
pub mod chunk_manager;
pub mod chunk_store;
pub mod config;
pub mod file_cache;
pub mod metadata;
pub mod proxy;
pub mod state_computer;
pub mod state_types;
pub mod stats;
pub mod storage;

#[cfg(test)]
mod integration_tests;

pub use chunk_downloader::{ChunkDownloader, RetryConfig};
pub use chunk_manager::{ChunkManager, Priority};
pub use chunk_store::ChunkStore;
pub use config::FileCacheConfig;
pub use file_cache::{FileCache, FileCacheHandle};
pub use state_computer::StateComputer;
pub use state_types::{DownloadState, DownloadStateInfo};
pub use stats::CurrentCacheStats;
