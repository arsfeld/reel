use anyhow::Result;
use relm4::{ComponentSender, Worker};
use std::collections::HashMap;
use tracing::{debug, error};

use crate::db::connection::DatabaseConnection;
use crate::models::{LibraryId, MediaItem};
use crate::services::core::MediaService;

/// Messages that can be sent to the SearchWorker
#[derive(Debug, Clone)]
pub enum SearchWorkerInput {
    /// Perform a search
    Search {
        query: String,
        library_id: Option<LibraryId>,
        limit: Option<usize>,
    },
    /// Build search index for a library
    BuildIndex(LibraryId),
    /// Clear search cache
    ClearCache,
}

/// Messages sent from the SearchWorker
#[derive(Debug, Clone)]
pub enum SearchWorkerOutput {
    /// Search results
    SearchResults {
        query: String,
        results: Vec<MediaItem>,
        total_count: usize,
    },
    /// Search failed
    SearchFailed { query: String, error: String },
    /// Index built
    IndexBuilt(LibraryId),
    /// Cache cleared
    CacheCleared,
}

/// Worker for search operations
pub struct SearchWorker {
    db: DatabaseConnection,
    search_cache: HashMap<String, Vec<MediaItem>>,
}

impl SearchWorker {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            search_cache: HashMap::new(),
        }
    }

    async fn perform_search(
        &mut self,
        query: String,
        library_id: Option<LibraryId>,
        limit: Option<usize>,
        sender: ComponentSender<Self>,
    ) {
        debug!("Searching for: {}", query);

        // Check cache first
        let cache_key = format!(
            "{}__{}",
            query,
            library_id.as_ref().map_or("all", |l| l.as_ref())
        );

        if let Some(cached) = self.search_cache.get(&cache_key) {
            debug!("Using cached search results for: {}", query);
            let results = if let Some(limit) = limit {
                cached.iter().take(limit).cloned().collect()
            } else {
                cached.clone()
            };

            let _ = sender.output(SearchWorkerOutput::SearchResults {
                query,
                results: results.clone(),
                total_count: cached.len(),
            });
            return;
        }

        // Perform database search
        match MediaService::search_media(&self.db, &query, library_id.as_ref(), None).await {
            Ok(mut results) => {
                // Cache the full results
                self.search_cache.insert(cache_key, results.clone());

                let total_count = results.len();

                // Apply limit if specified
                if let Some(limit) = limit {
                    results.truncate(limit);
                }

                let _ = sender.output(SearchWorkerOutput::SearchResults {
                    query,
                    results,
                    total_count,
                });
            }
            Err(e) => {
                error!("Search failed for '{}': {}", query, e);
                let _ = sender.output(SearchWorkerOutput::SearchFailed {
                    query,
                    error: e.to_string(),
                });
            }
        }
    }

    async fn build_index(&self, library_id: LibraryId, sender: ComponentSender<Self>) {
        debug!("Building search index for library: {}", library_id);

        // In a real implementation, this would build a full-text search index
        // For now, we just pre-load the library items to warm up any caches
        match MediaService::get_media_items(&self.db, &library_id, None, 0, 100).await {
            Ok(items) => {
                debug!("Indexed {} items for library {}", items.len(), library_id);
                let _ = sender.output(SearchWorkerOutput::IndexBuilt(library_id));
            }
            Err(e) => {
                error!("Failed to build index for library {}: {}", library_id, e);
            }
        }
    }

    fn clear_cache(&mut self, sender: ComponentSender<Self>) {
        debug!("Clearing search cache");
        self.search_cache.clear();
        let _ = sender.output(SearchWorkerOutput::CacheCleared);
    }
}

impl Worker for SearchWorker {
    type Init = DatabaseConnection;
    type Input = SearchWorkerInput;
    type Output = SearchWorkerOutput;

    fn init(db: Self::Init, _sender: ComponentSender<Self>) -> Self {
        Self::new(db)
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            SearchWorkerInput::Search {
                query,
                library_id,
                limit,
            } => {
                let mut worker = self.clone();
                relm4::spawn(async move {
                    worker
                        .perform_search(query, library_id, limit, sender)
                        .await;
                });
            }
            SearchWorkerInput::BuildIndex(library_id) => {
                let worker = self.clone();
                relm4::spawn(async move {
                    worker.build_index(library_id, sender).await;
                });
            }
            SearchWorkerInput::ClearCache => {
                self.clear_cache(sender);
            }
        }
    }
}

impl Clone for SearchWorker {
    fn clone(&self) -> Self {
        Self {
            db: self.db.clone(),
            search_cache: self.search_cache.clone(),
        }
    }
}
