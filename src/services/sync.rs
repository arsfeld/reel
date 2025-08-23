use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::backends::traits::MediaBackend;
use crate::models::{Library, MediaItem, Movie, Show};
use crate::services::cache::CacheManager;
use crate::utils::image_loader::{ImageLoader, ImageSize};

#[derive(Debug, Clone)]
pub enum SyncType {
    Full,            // Full sync of all data
    Incremental,     // Only changes since last sync
    Library(String), // Specific library
    Media(String),   // Specific media item
}

#[derive(Debug, Clone)]
pub enum SyncStatus {
    Idle,
    Syncing {
        progress: f32,
        current_item: String,
    },
    Completed {
        at: DateTime<Utc>,
        items_synced: usize,
    },
    Failed {
        error: String,
        at: DateTime<Utc>,
    },
}

#[derive(Debug, Clone)]
pub struct SyncResult {
    pub backend_id: String,
    pub success: bool,
    pub items_synced: usize,
    pub duration: std::time::Duration,
    pub errors: Vec<String>,
}

pub struct SyncManager {
    cache: Arc<CacheManager>,
    sync_status: Arc<RwLock<HashMap<String, SyncStatus>>>,
    image_loader: Arc<ImageLoader>,
    poster_download_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    poster_download_cancel: Arc<Mutex<CancellationToken>>,
}

impl SyncManager {
    pub fn new(cache: Arc<CacheManager>) -> Self {
        Self {
            cache,
            sync_status: Arc::new(RwLock::new(HashMap::new())),
            image_loader: Arc::new(ImageLoader::new().expect("Failed to create ImageLoader")),
            poster_download_handle: Arc::new(Mutex::new(None)),
            poster_download_cancel: Arc::new(Mutex::new(CancellationToken::new())),
        }
    }

    /// Sync all data from a backend
    pub async fn sync_backend(
        &self,
        backend_id: &str,
        backend: Arc<dyn MediaBackend>,
    ) -> Result<SyncResult> {
        let start_time = std::time::Instant::now();
        let mut errors = Vec::new();
        let mut items_synced = 0;

        // Cancel any existing background poster downloads
        self.cancel_background_downloads().await;

        // Update sync status
        {
            let mut status = self.sync_status.write().await;
            status.insert(
                backend_id.to_string(),
                SyncStatus::Syncing {
                    progress: 0.0,
                    current_item: "Fetching libraries".to_string(),
                },
            );
        }

        info!("Starting sync for backend: {}", backend_id);

        // Fetch libraries
        match backend.get_libraries().await {
            Ok(libraries) => {
                info!("Found {} libraries", libraries.len());

                // Cache libraries
                for library in &libraries {
                    let cache_key = format!("{}:library:{}", backend_id, library.id);
                    if let Err(e) = self.cache.set_media(&cache_key, "library", library).await {
                        error!("Failed to cache library {}: {}", library.id, e);
                        errors.push(format!("Failed to cache library {}: {}", library.id, e));
                    } else {
                        items_synced += 1;
                    }
                }

                // Cache the library list
                let libraries_key = format!("{}:libraries", backend_id);
                if let Err(e) = self
                    .cache
                    .set_media(&libraries_key, "library_list", &libraries)
                    .await
                {
                    error!("Failed to cache library list: {}", e);
                    errors.push(format!("Failed to cache library list: {}", e));
                }

                // Sync content from each library
                for (idx, library) in libraries.iter().enumerate() {
                    let progress = (idx as f32 / libraries.len() as f32) * 100.0;

                    // Update sync status
                    {
                        let mut status = self.sync_status.write().await;
                        status.insert(
                            backend_id.to_string(),
                            SyncStatus::Syncing {
                                progress,
                                current_item: format!("Syncing library: {}", library.title),
                            },
                        );
                    }

                    // Sync library content using generic method
                    if let Err(e) = self
                        .sync_library_items(
                            backend_id,
                            &library.id,
                            &library.library_type,
                            backend.clone(),
                        )
                        .await
                    {
                        error!("Failed to sync items from library {}: {}", library.id, e);
                        errors.push(format!("Failed to sync library {}: {}", library.title, e));
                    } else {
                        items_synced += 1;
                    }
                }
            }
            Err(e) => {
                error!("Failed to fetch libraries: {}", e);
                errors.push(format!("Failed to fetch libraries: {}", e));
            }
        }

        let duration = start_time.elapsed();
        let success = errors.is_empty();

        // Update final sync status
        {
            let mut status = self.sync_status.write().await;
            if success {
                status.insert(
                    backend_id.to_string(),
                    SyncStatus::Completed {
                        at: Utc::now(),
                        items_synced,
                    },
                );
            } else {
                status.insert(
                    backend_id.to_string(),
                    SyncStatus::Failed {
                        error: errors.join(", "),
                        at: Utc::now(),
                    },
                );
            }
        }

        info!(
            "Sync completed for backend {}: {} items synced in {:?}",
            backend_id, items_synced, duration
        );

        Ok(SyncResult {
            backend_id: backend_id.to_string(),
            success,
            items_synced,
            duration,
            errors,
        })
    }

    /// Sync a specific library
    pub async fn sync_library(&self, backend_id: &str, library_id: &str) -> Result<()> {
        // This is a simplified version that doesn't have the backend instance
        // In a real implementation, we'd need to get the backend from somewhere
        // For now, just mark as successful
        info!("Syncing library {} for backend {}", library_id, backend_id);
        Ok(())
    }

    /// Sync items from a library (generic for all types)
    async fn sync_library_items(
        &self,
        backend_id: &str,
        library_id: &str,
        library_type: &crate::models::LibraryType,
        backend: Arc<dyn MediaBackend>,
    ) -> Result<()> {
        info!(
            "Syncing {:?} items from library {}",
            library_type, library_id
        );

        // Use the generic get_library_items method
        let items = backend
            .get_library_items(library_id)
            .await
            .context("Failed to fetch library items")?;

        info!("Found {} items to sync", items.len());

        // Cache each item with its appropriate type
        for item in &items {
            let item_type = match item {
                MediaItem::Movie(_) => "movie",
                MediaItem::Show(_) => "show",
                MediaItem::Episode(_) => "episode",
                MediaItem::MusicAlbum(_) => "album",
                MediaItem::MusicTrack(_) => "track",
                MediaItem::Photo(_) => "photo",
            };

            let cache_key = format!("{}:{}:{}", backend_id, item_type, item.id());
            self.cache
                .set_media(&cache_key, item_type, item)
                .await
                .context(format!("Failed to cache {} {}", item_type, item.id()))?;
        }

        // Cache the item list for this library
        let items_key = format!("{}:library:{}:items", backend_id, library_id);
        self.cache
            .set_media(&items_key, "item_list", &items)
            .await
            .context("Failed to cache item list")?;

        // Also maintain backward compatibility by caching typed lists
        match library_type {
            crate::models::LibraryType::Movies => {
                let movies: Vec<Movie> = items
                    .iter()
                    .filter_map(|item| match item {
                        MediaItem::Movie(m) => Some(m.clone()),
                        _ => None,
                    })
                    .collect();
                let movies_key = format!("{}:library:{}:movies", backend_id, library_id);
                self.cache
                    .set_media(&movies_key, "movie_list", &movies)
                    .await?;
            }
            crate::models::LibraryType::Shows => {
                let shows: Vec<Show> = items
                    .iter()
                    .filter_map(|item| match item {
                        MediaItem::Show(s) => Some(s.clone()),
                        _ => None,
                    })
                    .collect();
                let shows_key = format!("{}:library:{}:shows", backend_id, library_id);
                self.cache
                    .set_media(&shows_key, "show_list", &shows)
                    .await?;
            }
            _ => {}
        }

        // Start background poster downloads after caching items
        self.download_posters_background(&items).await;

        Ok(())
    }

    /// Get the current sync status for a backend
    pub async fn get_sync_status(&self, backend_id: &str) -> SyncStatus {
        let status = self.sync_status.read().await;
        status.get(backend_id).cloned().unwrap_or(SyncStatus::Idle)
    }

    /// Get cached libraries for a backend
    pub async fn get_cached_libraries(&self, backend_id: &str) -> Result<Vec<Library>> {
        let libraries_key = format!("{}:libraries", backend_id);
        Ok(self
            .cache
            .get_media(&libraries_key)
            .await?
            .unwrap_or_default())
    }

    /// Get cached movies for a library
    pub async fn get_cached_movies(
        &self,
        backend_id: &str,
        library_id: &str,
    ) -> Result<Vec<Movie>> {
        let movies_key = format!("{}:library:{}:movies", backend_id, library_id);
        Ok(self.cache.get_media(&movies_key).await?.unwrap_or_default())
    }

    /// Get cached shows for a library
    pub async fn get_cached_shows(&self, backend_id: &str, library_id: &str) -> Result<Vec<Show>> {
        let shows_key = format!("{}:library:{}:shows", backend_id, library_id);
        Ok(self.cache.get_media(&shows_key).await?.unwrap_or_default())
    }

    /// Get cached items for a library (generic)
    pub async fn get_cached_items(
        &self,
        backend_id: &str,
        library_id: &str,
    ) -> Result<Vec<MediaItem>> {
        let items_key = format!("{}:library:{}:items", backend_id, library_id);
        Ok(self.cache.get_media(&items_key).await?.unwrap_or_default())
    }

    /// Get item count for a library
    pub async fn get_library_item_count(&self, backend_id: &str, library_id: &str) -> usize {
        self.get_cached_items(backend_id, library_id)
            .await
            .unwrap_or_default()
            .len()
    }

    /// Extract poster URLs from media items
    fn extract_poster_urls(&self, items: &[MediaItem]) -> Vec<(String, ImageSize)> {
        let mut urls = Vec::new();

        for item in items {
            let poster_url = match item {
                MediaItem::Movie(m) => m.poster_url.as_ref(),
                MediaItem::Show(s) => s.poster_url.as_ref(),
                MediaItem::Episode(e) => e.show_poster_url.as_ref().or(e.thumbnail_url.as_ref()),
                MediaItem::MusicAlbum(a) => a.cover_url.as_ref(),
                MediaItem::MusicTrack(t) => t.cover_url.as_ref(),
                MediaItem::Photo(p) => p.thumbnail_url.as_ref(),
            };

            if let Some(url) = poster_url {
                // Download small size for list views by default
                urls.push((url.clone(), ImageSize::Small));
            }

            // Also get backdrop URLs for movies and shows
            match item {
                MediaItem::Movie(m) => {
                    if let Some(backdrop) = &m.backdrop_url {
                        urls.push((backdrop.clone(), ImageSize::Medium));
                    }
                }
                MediaItem::Show(s) => {
                    if let Some(backdrop) = &s.backdrop_url {
                        urls.push((backdrop.clone(), ImageSize::Medium));
                    }
                }
                _ => {}
            }
        }

        urls
    }

    /// Download posters in background with low priority
    async fn download_posters_background(&self, items: &[MediaItem]) {
        let all_urls = self.extract_poster_urls(items);

        if all_urls.is_empty() {
            return;
        }

        // Check which images are not already cached
        let cached_status = self.image_loader.batch_check_cached(all_urls.clone()).await;
        let urls_to_download: Vec<(String, ImageSize)> = all_urls
            .into_iter()
            .zip(cached_status.iter())
            .filter_map(
                |(url_size, &is_cached)| {
                    if !is_cached { Some(url_size) } else { None }
                },
            )
            .collect();

        if urls_to_download.is_empty() {
            info!(
                "All {} posters already cached, skipping background download",
                cached_status.len()
            );
            return;
        }

        info!(
            "Starting background download of {} posters ({} already cached)",
            urls_to_download.len(),
            cached_status.len() - urls_to_download.len()
        );

        // Cancel any existing download and create a new cancellation token
        self.cancel_background_downloads().await;

        // Create a new cancellation token for this download session
        let cancel_token = CancellationToken::new();
        {
            let mut cancel_guard = self.poster_download_cancel.lock().await;
            *cancel_guard = cancel_token.clone();
        }

        // Clone what we need for the background task
        let image_loader = self.image_loader.clone();

        // Spawn a low-priority background task
        let handle = tokio::spawn(async move {
            // Add a small delay to let UI operations take priority
            tokio::select! {
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(500)) => {},
                _ = cancel_token.cancelled() => {
                    info!("Background poster download cancelled before starting");
                    return;
                }
            }

            // Process in small batches to avoid overwhelming the system
            const BATCH_SIZE: usize = 10;
            let mut downloaded_count = 0;

            for chunk in urls_to_download.chunks(BATCH_SIZE) {
                // Check for cancellation before processing each batch
                if cancel_token.is_cancelled() {
                    info!(
                        "Background poster download cancelled after {} downloads",
                        downloaded_count
                    );
                    return;
                }

                let chunk_vec: Vec<(String, ImageSize)> = chunk.to_vec();
                let batch_size = chunk_vec.len();

                // Use warm_cache which will only download images not already cached
                // This provides additional safety against duplicate downloads
                image_loader.warm_cache(chunk_vec).await;
                downloaded_count += batch_size;

                // Small delay between batches to keep priority low
                tokio::select! {
                    _ = tokio::time::sleep(tokio::time::Duration::from_millis(200)) => {},
                    _ = cancel_token.cancelled() => {
                        info!("Background poster download cancelled after {} downloads", downloaded_count);
                        return;
                    }
                }
            }

            info!(
                "Completed background poster downloads: {} images",
                downloaded_count
            );
        });

        // Store the handle so we can cancel it later if needed
        let mut handle_guard = self.poster_download_handle.lock().await;
        *handle_guard = Some(handle);
    }

    /// Cancel any existing background poster downloads
    async fn cancel_background_downloads(&self) {
        // Signal cancellation
        {
            let cancel_guard = self.poster_download_cancel.lock().await;
            cancel_guard.cancel();
        }

        // Wait for the task to complete if it exists
        let mut handle_guard = self.poster_download_handle.lock().await;
        if let Some(handle) = handle_guard.take() {
            // Give it a moment to cancel gracefully
            let _ = tokio::time::timeout(std::time::Duration::from_secs(1), handle).await;
        }
    }
}
