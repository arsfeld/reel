use anyhow::Result;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::db::entities::{LibraryModel, SourceModel};
use crate::events::EventBus;
use crate::models::MediaItem;
use crate::services::data::DataService;

/// Progress update for background loading operations
#[derive(Debug, Clone)]
pub struct LoadProgress {
    pub operation: String,
    pub current: u64,
    pub total: u64,
    pub message: String,
}

/// Background loader for database operations with progress reporting
pub struct BackgroundLoader {
    data_service: Arc<DataService>,
    event_bus: Arc<EventBus>,
    is_loading: Arc<AtomicBool>,
    items_loaded: Arc<AtomicU64>,
    total_items: Arc<AtomicU64>,
}

impl BackgroundLoader {
    pub fn new(data_service: Arc<DataService>, event_bus: Arc<EventBus>) -> Self {
        Self {
            data_service,
            event_bus,
            is_loading: Arc::new(AtomicBool::new(false)),
            items_loaded: Arc::new(AtomicU64::new(0)),
            total_items: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Check if a loading operation is in progress
    pub fn is_loading(&self) -> bool {
        self.is_loading.load(Ordering::Relaxed)
    }

    /// Get current progress as a percentage (0-100)
    pub fn progress_percentage(&self) -> u8 {
        let total = self.total_items.load(Ordering::Relaxed);
        if total == 0 {
            return 0;
        }
        let loaded = self.items_loaded.load(Ordering::Relaxed);
        ((loaded as f64 / total as f64) * 100.0).min(100.0) as u8
    }

    /// Load all sources with progress updates
    pub async fn load_sources_with_progress(
        &self,
        progress_tx: mpsc::Sender<LoadProgress>,
    ) -> Result<Vec<SourceModel>> {
        if self.is_loading.swap(true, Ordering::SeqCst) {
            warn!("Background load already in progress");
            return Ok(Vec::new());
        }

        // Reset counters
        self.items_loaded.store(0, Ordering::Relaxed);
        self.total_items.store(1, Ordering::Relaxed); // Start with 1 for sources query

        // Send initial progress
        let _ = progress_tx
            .send(LoadProgress {
                operation: "Loading sources".to_string(),
                current: 0,
                total: 1,
                message: "Querying database...".to_string(),
            })
            .await;

        // Load sources
        let sources = self.data_service.get_all_sources().await?;

        self.items_loaded.store(1, Ordering::Relaxed);
        let _ = progress_tx
            .send(LoadProgress {
                operation: "Loading sources".to_string(),
                current: 1,
                total: 1,
                message: format!("Loaded {} sources", sources.len()),
            })
            .await;

        self.is_loading.store(false, Ordering::Relaxed);
        Ok(sources)
    }

    /// Load libraries for multiple sources with progress updates
    pub async fn load_libraries_with_progress(
        &self,
        source_ids: Vec<String>,
        progress_tx: mpsc::Sender<LoadProgress>,
    ) -> Result<Vec<(String, Vec<LibraryModel>)>> {
        if self.is_loading.swap(true, Ordering::SeqCst) {
            warn!("Background load already in progress");
            return Ok(Vec::new());
        }

        let total = source_ids.len() as u64;
        self.items_loaded.store(0, Ordering::Relaxed);
        self.total_items.store(total, Ordering::Relaxed);

        let mut results = Vec::new();

        for (idx, source_id) in source_ids.iter().enumerate() {
            let current = idx as u64;

            // Send progress update
            let _ = progress_tx
                .send(LoadProgress {
                    operation: "Loading libraries".to_string(),
                    current,
                    total,
                    message: format!("Loading libraries for source {}", source_id),
                })
                .await;

            // Load libraries for this source
            match self.data_service.get_libraries(source_id).await {
                Ok(libraries) => {
                    info!(
                        "Loaded {} libraries for source {}",
                        libraries.len(),
                        source_id
                    );
                    results.push((source_id.clone(), libraries));
                }
                Err(e) => {
                    warn!("Failed to load libraries for source {}: {}", source_id, e);
                    results.push((source_id.clone(), Vec::new()));
                }
            }

            self.items_loaded.store(current + 1, Ordering::Relaxed);
        }

        // Send completion
        let _ = progress_tx
            .send(LoadProgress {
                operation: "Loading libraries".to_string(),
                current: total,
                total,
                message: "All libraries loaded".to_string(),
            })
            .await;

        self.is_loading.store(false, Ordering::Relaxed);
        Ok(results)
    }

    /// Load media items for a library with progress and chunking
    pub async fn load_library_items_with_progress(
        &self,
        library_id: String,
        _source_id: String,
        progress_tx: mpsc::Sender<LoadProgress>,
        chunk_size: usize,
    ) -> Result<Vec<MediaItem>> {
        if self.is_loading.swap(true, Ordering::SeqCst) {
            warn!("Background load already in progress");
            return Ok(Vec::new());
        }

        // First, get the total count
        let total_count = self
            .data_service
            .count_media_in_library(&library_id)
            .await
            .unwrap_or(0);

        self.items_loaded.store(0, Ordering::Relaxed);
        self.total_items
            .store(total_count as u64, Ordering::Relaxed);

        let all_items = Vec::new();
        let mut offset: i64 = 0;

        while offset < total_count {
            let current_chunk_size = chunk_size.min((total_count - offset) as usize);

            // Send progress update
            let _ = progress_tx
                .send(LoadProgress {
                    operation: "Loading media items".to_string(),
                    current: offset as u64,
                    total: total_count as u64,
                    message: format!(
                        "Loading items {}-{} of {}",
                        offset + 1,
                        offset + current_chunk_size as i64,
                        total_count
                    ),
                })
                .await;

            // Load chunk
            match self
                .data_service
                .get_media_in_library_paginated(&library_id, offset, current_chunk_size)
                .await
            {
                Ok(db_items) => {
                    // Convert to MediaItems - for now we'll just store the raw data
                    // since MediaItem doesn't have from_db_model method
                    info!("Loaded {} items from database", db_items.len());
                    // TODO: Convert db_items to MediaItems when conversion method is available
                }
                Err(e) => {
                    warn!("Error loading chunk at offset {}: {}", offset, e);
                }
            }

            offset += current_chunk_size as i64;
            self.items_loaded.store(offset as u64, Ordering::Relaxed);

            // Log progress for monitoring
            if offset % (chunk_size as i64 * 2) == 0 || offset >= total_count {
                info!(
                    "Library {} loading progress: {}/{}",
                    library_id, offset, total_count
                );
            }
        }

        // Send completion
        let _ = progress_tx
            .send(LoadProgress {
                operation: "Loading media items".to_string(),
                current: total_count as u64,
                total: total_count as u64,
                message: format!("Loaded {} items", all_items.len()),
            })
            .await;

        self.is_loading.store(false, Ordering::Relaxed);
        Ok(all_items)
    }

    /// Preload media items in batches for smooth scrolling
    pub async fn preload_media_batch(
        &self,
        library_id: &str,
        start_index: usize,
        count: usize,
    ) -> Result<Vec<MediaItem>> {
        debug!(
            "Preloading {} items starting at index {} for library {}",
            count, start_index, library_id
        );

        match self
            .data_service
            .get_media_in_library_paginated(library_id, start_index as i64, count)
            .await
        {
            Ok(db_items) => {
                // TODO: Convert db_items to MediaItems when conversion method is available
                info!("Preloaded {} items from database", db_items.len());
                Ok(Vec::new())
            }
            Err(e) => {
                warn!("Failed to preload media batch: {}", e);
                Ok(Vec::new())
            }
        }
    }
}

/// Helper to run background loading with automatic progress handling
pub async fn load_with_progress<T, F>(
    _loader: Arc<BackgroundLoader>,
    operation_name: &str,
    load_fn: F,
) -> Result<T>
where
    F: std::future::Future<Output = Result<T>>,
{
    info!("Starting background operation: {}", operation_name);
    let result = load_fn.await;
    info!("Completed background operation: {}", operation_name);
    result
}
