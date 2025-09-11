use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, trace};

use crate::backends::traits::MediaBackend;
use crate::events::{DatabaseEvent, EventBus, EventPayload, EventType};
use crate::models::{Library, MediaItem, Movie, Show};
use crate::services::data::DataService;
use crate::utils::image_loader::{ImageLoader, ImageSize};

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

/// Poster download request
#[derive(Debug, Clone)]
struct PosterDownloadRequest {
    url: String,
    size: ImageSize,
    media_id: String,
    media_type: String,
}

pub struct SyncManager {
    data_service: Arc<DataService>,
    sync_status: Arc<RwLock<HashMap<String, SyncStatus>>>,
    image_loader: Arc<ImageLoader>,
    poster_download_queue: Arc<Mutex<VecDeque<PosterDownloadRequest>>>,
    poster_download_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    poster_download_cancel: Arc<Mutex<CancellationToken>>,
    event_bus: Arc<EventBus>,
    max_queue_size: usize,
    // Throttle noisy logs when queue is full
    queue_full_log_last: Arc<Mutex<Option<std::time::Instant>>>,
}

impl SyncManager {
    pub fn new(data_service: Arc<DataService>, event_bus: Arc<EventBus>) -> Self {
        let manager = Self {
            data_service,
            sync_status: Arc::new(RwLock::new(HashMap::new())),
            image_loader: Arc::new(ImageLoader::new().expect("Failed to create ImageLoader")),
            poster_download_queue: Arc::new(Mutex::new(VecDeque::new())),
            poster_download_handle: Arc::new(Mutex::new(None)),
            poster_download_cancel: Arc::new(Mutex::new(CancellationToken::new())),
            event_bus: event_bus.clone(),
            max_queue_size: 100, // Limit queue to 100 items at a time
            queue_full_log_last: Arc::new(Mutex::new(None)),
        };

        // Start the background poster download processor
        manager.start_poster_processor();

        manager
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
                    current_item: "Initializing source".to_string(),
                },
            );
        }

        info!("Starting sync for backend: {}", backend_id);

        // Emit sync started event
        let _ = self
            .event_bus
            .publish(DatabaseEvent::new(
                EventType::SyncStarted,
                EventPayload::Sync {
                    source_id: backend_id.to_string(),
                    sync_type: "full".to_string(),
                    progress: Some(0.0),
                    items_synced: None,
                    error: None,
                },
            ))
            .await;

        // Validate that source exists before syncing - sources should be created during authentication
        if self.data_service.get_source(backend_id).await?.is_none() {
            let error_msg = format!(
                "Source {} does not exist in database - sources must be created during authentication, not during sync",
                backend_id
            );
            error!("{}", error_msg);
            errors.push(error_msg.clone());

            // Emit error event
            let _ = self
                .event_bus
                .publish(DatabaseEvent::new(
                    EventType::ErrorOccurred,
                    EventPayload::System {
                        message: error_msg.clone(),
                        details: Some(serde_json::json!({
                            "backend_id": backend_id,
                            "error": "Source not found during sync operation"
                        })),
                    },
                ))
                .await;

            return Err(anyhow::anyhow!("{}", error_msg));
        }

        // Fetch and cache home sections first
        match backend.get_home_sections().await {
            Ok(sections) => {
                info!("Found {} home sections", sections.len());

                // Cache home sections for this backend
                let home_sections_key = format!("{}:home_sections", backend_id);
                if let Err(e) = self
                    .data_service
                    .store_home_sections(&home_sections_key, &sections)
                    .await
                {
                    error!("Failed to cache home sections: {}", e);
                    errors.push(format!("Failed to cache home sections: {}", e));
                } else {
                    items_synced += sections.len();
                }
            }
            Err(e) => {
                info!(
                    "Backend doesn't provide home sections or failed to fetch: {}",
                    e
                );
                // This is not a critical error, continue with library sync
            }
        }

        // Fetch libraries
        match backend.get_libraries().await {
            Ok(libraries) => {
                info!("Found {} libraries", libraries.len());

                // Cache libraries
                for library in &libraries {
                    if let Err(e) = self.data_service.store_library(library, backend_id).await {
                        error!("Failed to cache library {}: {}", library.id, e);
                        errors.push(format!("Failed to cache library {}: {}", library.id, e));
                    } else {
                        items_synced += 1;
                    }
                }

                // Cache the library list
                let libraries_key = format!("{}:libraries", backend_id);
                if let Err(e) = self
                    .data_service
                    .store_library_list(&libraries_key, &libraries)
                    .await
                {
                    error!("Failed to cache library list: {}", e);
                    errors.push(format!("Failed to cache library list: {}", e));
                }

                // Sync content from each library
                // Reserve 80% for library sync, 20% for episode sync
                for (idx, library) in libraries.iter().enumerate() {
                    let progress = (idx as f32 / libraries.len() as f32) * 80.0;

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

                    // Emit sync progress event
                    let _ = self
                        .event_bus
                        .publish(DatabaseEvent::new(
                            EventType::SyncProgress,
                            EventPayload::Sync {
                                source_id: backend_id.to_string(),
                                sync_type: "full".to_string(),
                                progress: Some(progress),
                                items_synced: Some(items_synced),
                                error: None,
                            },
                        ))
                        .await;

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

        // Phase 2: Episode sync progress tracking for TV show libraries
        // Count total shows across all TV libraries for progress calculation
        match backend.get_libraries().await {
            Ok(libraries) => {
                let tv_libraries: Vec<_> = libraries
                    .iter()
                    .filter(|lib| matches!(lib.library_type, crate::models::LibraryType::Shows))
                    .collect();

                if !tv_libraries.is_empty() {
                    // Emit episode sync progress update
                    let episode_progress = 80.0 + 20.0; // Library sync (80%) + episode sync (20%) = 100%

                    // Update sync status for episode phase
                    {
                        let mut status = self.sync_status.write().await;
                        status.insert(
                            backend_id.to_string(),
                            SyncStatus::Syncing {
                                progress: episode_progress,
                                current_item: "Syncing TV show episodes...".to_string(),
                            },
                        );
                    }

                    // Emit episode sync progress event
                    let _ = self
                        .event_bus
                        .publish(DatabaseEvent::new(
                            EventType::SyncProgress,
                            EventPayload::Sync {
                                source_id: backend_id.to_string(),
                                sync_type: "episodes".to_string(),
                                progress: Some(episode_progress),
                                items_synced: Some(items_synced),
                                error: None,
                            },
                        ))
                        .await;
                }
            }
            Err(_) => {
                // If we can't get libraries, skip episode progress tracking
            }
        }

        let duration = start_time.elapsed();
        let success = errors.is_empty();

        // Update final sync status and emit corresponding event
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

                // Emit sync completed event
                let _ = self
                    .event_bus
                    .publish(DatabaseEvent::new(
                        EventType::SyncCompleted,
                        EventPayload::Sync {
                            source_id: backend_id.to_string(),
                            sync_type: "full".to_string(),
                            progress: Some(100.0),
                            items_synced: Some(items_synced),
                            error: None,
                        },
                    ))
                    .await;
            } else {
                let error_msg = errors.join(", ");
                status.insert(
                    backend_id.to_string(),
                    SyncStatus::Failed {
                        error: error_msg.clone(),
                        at: Utc::now(),
                    },
                );

                // Emit sync failed event
                let _ = self
                    .event_bus
                    .publish(DatabaseEvent::new(
                        EventType::SyncFailed,
                        EventPayload::Sync {
                            source_id: backend_id.to_string(),
                            sync_type: "full".to_string(),
                            progress: None,
                            items_synced: Some(items_synced),
                            error: Some(error_msg),
                        },
                    ))
                    .await;
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
        trace!(
            "Syncing {:?} items from library {}",
            library_type, library_id
        );

        // Get existing items to detect new ones
        let existing_items_key = format!("{}:library:{}:items", backend_id, library_id);
        let existing_items: Vec<MediaItem> = self
            .data_service
            .get_media(&existing_items_key)
            .await?
            .unwrap_or_default();

        let existing_ids: std::collections::HashSet<String> = existing_items
            .iter()
            .map(|item| item.id().to_string())
            .collect();

        // Use the generic get_library_items method
        let items = backend
            .get_library_items(library_id)
            .await
            .context("Failed to fetch library items")?;

        trace!("Found {} items to sync", items.len());

        // Collect IDs for batch event and detect new items
        let mut media_ids = Vec::new();
        let mut new_items = Vec::new();

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

            // Include library_id in the cache key to help with foreign key relationships
            let cache_key = format!("{}:{}:{}:{}", backend_id, library_id, item_type, item.id());
            media_ids.push(cache_key.clone());

            // Check if this is a new item
            if !existing_ids.contains(&item.id().to_string()) {
                new_items.push((item.clone(), cache_key.clone()));
            }

            // Use silent storage to avoid individual events during sync
            self.data_service
                .store_media_item_silent(&cache_key, item)
                .await
                .context(format!("Failed to cache {} {}", item_type, item.id()))?;
        }

        // Queue poster downloads for new items only
        if !new_items.is_empty() {
            trace!(
                "Queueing poster downloads for {} new items",
                new_items.len()
            );
            for (item, cache_key) in &new_items {
                self.queue_poster_download(item, cache_key).await;
            }
        }

        // Emit batch created event for all synced items
        if !media_ids.is_empty() {
            let _ = self
                .event_bus
                .publish(DatabaseEvent::new(
                    EventType::MediaBatchCreated,
                    EventPayload::MediaBatch {
                        ids: media_ids,
                        library_id: library_id.to_string(),
                        source_id: backend_id.to_string(),
                    },
                ))
                .await;
        }

        // Cache the item list for this library
        // Items list no longer needs to be cached - individual items are stored in the database

        // Also maintain backward compatibility by caching typed lists
        match library_type {
            crate::models::LibraryType::Movies => {
                let _movies: Vec<Movie> = items
                    .iter()
                    .filter_map(|item| match item {
                        MediaItem::Movie(m) => Some(m.clone()),
                        _ => None,
                    })
                    .collect();
                // Movie lists no longer need separate caching - individual items are stored in the database
            }
            crate::models::LibraryType::Shows => {
                let shows: Vec<Show> = items
                    .iter()
                    .filter_map(|item| match item {
                        MediaItem::Show(s) => Some(s.clone()),
                        _ => None,
                    })
                    .collect();
                // Show lists no longer need separate caching - individual items are stored in the database

                // Sync episodes for each show
                let mut episode_sync_errors = 0;
                for show in &shows {
                    if let Err(e) = self
                        .sync_show_episodes(backend_id, library_id, &show.id, backend.clone())
                        .await
                    {
                        trace!("Failed to sync episodes for show {}: {}", show.id, e);
                        episode_sync_errors += 1;
                    }
                }
                if episode_sync_errors > 0 {
                    debug!("Failed to sync episodes for {} shows", episode_sync_errors);
                }
            }
            _ => {}
        }

        // Log sync summary for this library
        if !new_items.is_empty() {
            info!(
                "Library sync: {} ({} items, {} new)",
                library_id,
                items.len(),
                new_items.len()
            );
        } else if items.len() > 0 {
            debug!(
                "Library sync: {} ({} items, no changes)",
                library_id,
                items.len()
            );
        }

        // Update library item count in database and emit event
        let item_count = items.len() as i32;
        // Update item count in database
        if let Err(e) = self
            .data_service
            .update_library_item_count(library_id, item_count)
            .await
        {
            error!(
                "Failed to update library item count for {}: {}",
                library_id, e
            );
        }
        // Emit library item count changed event
        let _ = self
            .event_bus
            .publish(DatabaseEvent::new(
                EventType::LibraryItemCountChanged,
                EventPayload::Library {
                    id: library_id.to_string(),
                    source_id: backend_id.to_string(),
                    item_count: Some(item_count),
                },
            ))
            .await;

        Ok(())
    }

    /// Get the current sync status for a backend
    pub async fn get_sync_status(&self, backend_id: &str) -> SyncStatus {
        let status = self.sync_status.read().await;
        status.get(backend_id).cloned().unwrap_or(SyncStatus::Idle)
    }

    /// Get cached libraries for a backend
    pub async fn get_cached_libraries(&self, backend_id: &str) -> Result<Vec<Library>> {
        // First try to get from memory cache (fast path)
        let libraries_key = format!("{}:libraries", backend_id);
        if let Some(libraries) = self.data_service.get_media(&libraries_key).await? {
            return Ok(libraries);
        }

        // Fallback to database when memory cache is empty (e.g., on startup)
        let db_libraries = self.data_service.get_libraries(backend_id).await?;
        let libraries: Vec<Library> = db_libraries
            .into_iter()
            .map(|lib| {
                use crate::models::LibraryType;
                Library {
                    id: lib.id,
                    title: lib.title,
                    library_type: match lib.library_type.as_str() {
                        "Movies" => LibraryType::Movies,
                        "Shows" => LibraryType::Shows,
                        "Music" => LibraryType::Music,
                        "Photos" => LibraryType::Photos,
                        _ => LibraryType::Mixed,
                    },
                    icon: lib.icon,
                }
            })
            .collect();

        Ok(libraries)
    }

    /// Get cached movies for a library
    pub async fn get_cached_movies(
        &self,
        backend_id: &str,
        library_id: &str,
    ) -> Result<Vec<Movie>> {
        let movies_key = format!("{}:library:{}:movies", backend_id, library_id);
        Ok(self
            .data_service
            .get_media(&movies_key)
            .await?
            .unwrap_or_default())
    }

    /// Get cached shows for a library
    pub async fn get_cached_shows(&self, backend_id: &str, library_id: &str) -> Result<Vec<Show>> {
        let shows_key = format!("{}:library:{}:shows", backend_id, library_id);
        Ok(self
            .data_service
            .get_media(&shows_key)
            .await?
            .unwrap_or_default())
    }

    /// Get cached items for a library (generic)
    pub async fn get_cached_items(
        &self,
        backend_id: &str,
        library_id: &str,
    ) -> Result<Vec<MediaItem>> {
        let items_key = format!("{}:library:{}:items", backend_id, library_id);
        Ok(self
            .data_service
            .get_media(&items_key)
            .await?
            .unwrap_or_default())
    }

    /// Get item count for a library
    pub async fn get_library_item_count(&self, backend_id: &str, library_id: &str) -> usize {
        self.get_cached_items(backend_id, library_id)
            .await
            .unwrap_or_default()
            .len()
    }

    /// Queue poster download for a single media item (can be called externally)
    pub async fn queue_media_poster(&self, media_item: &MediaItem, media_id: &str) {
        self.queue_poster_download(media_item, media_id).await;
    }

    /// Get current poster queue size
    pub async fn get_poster_queue_size(&self) -> usize {
        let queue = self.poster_download_queue.lock().await;
        queue.len()
    }

    /// Sync episodes for a show
    async fn sync_show_episodes(
        &self,
        backend_id: &str,
        library_id: &str,
        show_id: &str,
        backend: Arc<dyn MediaBackend>,
    ) -> Result<()> {
        trace!("Syncing episodes for show {}", show_id);

        // Get the cached show data which includes season information
        let show_key = format!("{}:{}:show:{}", backend_id, library_id, show_id);
        let show: Option<MediaItem> = self.data_service.get_media(&show_key).await?;

        let seasons = match show {
            Some(MediaItem::Show(s)) => {
                if s.seasons.is_empty() {
                    debug!(
                        "Show {} has no seasons listed, skipping episode sync",
                        show_id
                    );
                    return Ok(());
                }
                s.seasons
            }
            _ => {
                error!(
                    "Failed to get show data for {} - cannot sync episodes without season information",
                    show_id
                );
                return Ok(()); // Return Ok to allow sync to continue with other shows
            }
        };

        // Now sync episodes for each known season
        let mut total_episodes = 0;
        let mut failed_episodes = 0;

        for season in seasons {
            match backend.get_episodes(show_id, season.season_number).await {
                Ok(episodes) if !episodes.is_empty() => {
                    let episode_count = episodes.len();
                    total_episodes += episode_count;
                    trace!(
                        "Found {} episodes for show {} season {}",
                        episode_count, show_id, season.season_number
                    );

                    // Convert episodes to MediaItem::Episode and store
                    for episode in episodes {
                        let episode_item = MediaItem::Episode(episode.clone());
                        // Include library_id in the cache key for proper database storage
                        // Format: backend_id:library_id:episode:episode_id
                        let cache_key =
                            format!("{}:{}:episode:{}", backend_id, library_id, episode.id);

                        // Store the episode silently during sync
                        if let Err(e) = self
                            .data_service
                            .store_media_item_silent(&cache_key, &episode_item)
                            .await
                        {
                            trace!(
                                "Failed to cache episode {} s{}e{}: {}",
                                episode.id, season.season_number, episode.episode_number, e
                            );
                            failed_episodes += 1;
                            // Continue with other episodes instead of aborting the whole show
                            continue;
                        }

                        // Queue poster downloads for the episode
                        self.queue_poster_download(&episode_item, &cache_key).await;
                    }
                }
                Ok(_) => {
                    // Empty season
                    debug!(
                        "No episodes found for show {} season {}",
                        show_id, season.season_number
                    );
                }
                Err(e) => {
                    // Log error but continue with other seasons
                    debug!(
                        "Failed to get episodes for show {} season {}: {}",
                        show_id, season.season_number, e
                    );
                }
            }
        }

        if total_episodes > 0 {
            debug!(
                "Show {}: synced {} episodes ({} failed)",
                show_id,
                total_episodes - failed_episodes,
                failed_episodes
            );
        }

        Ok(())
    }

    /// Queue poster download for a media item
    async fn queue_poster_download(&self, item: &MediaItem, media_id: &str) {
        let mut queue = self.poster_download_queue.lock().await;

        // Skip if queue is full
        if queue.len() >= self.max_queue_size {
            // Rate-limit this log to avoid flooding
            let mut last_guard = self.queue_full_log_last.lock().await;
            let now = std::time::Instant::now();
            let should_log = match *last_guard {
                None => true,
                Some(last) => now.duration_since(last) > std::time::Duration::from_secs(2),
            };
            if should_log {
                debug!("Poster download queue is full, skipping more items...");
                *last_guard = Some(now);
            } else {
                trace!("Poster queue full; skipping {}", media_id);
            }
            return;
        }

        // Extract poster URLs for this specific item
        let poster_url = match item {
            MediaItem::Movie(m) => m.poster_url.as_ref(),
            MediaItem::Show(s) => s.poster_url.as_ref(),
            MediaItem::Episode(e) => e.show_poster_url.as_ref().or(e.thumbnail_url.as_ref()),
            MediaItem::MusicAlbum(a) => a.cover_url.as_ref(),
            MediaItem::MusicTrack(t) => t.cover_url.as_ref(),
            MediaItem::Photo(p) => p.thumbnail_url.as_ref(),
        };

        let media_type = match item {
            MediaItem::Movie(_) => "movie",
            MediaItem::Show(_) => "show",
            MediaItem::Episode(_) => "episode",
            MediaItem::MusicAlbum(_) => "album",
            MediaItem::MusicTrack(_) => "track",
            MediaItem::Photo(_) => "photo",
        };

        // Queue poster if available
        if let Some(url) = poster_url {
            // Check if not already cached
            if !self
                .image_loader
                .batch_check_cached(vec![(url.clone(), ImageSize::Small)])
                .await[0]
            {
                queue.push_back(PosterDownloadRequest {
                    url: url.clone(),
                    size: ImageSize::Small,
                    media_id: media_id.to_string(),
                    media_type: media_type.to_string(),
                });
            }
        }

        // Also queue backdrop URLs for movies and shows
        match item {
            MediaItem::Movie(m) => {
                if let Some(backdrop) = &m.backdrop_url
                    && !self
                        .image_loader
                        .batch_check_cached(vec![(backdrop.clone(), ImageSize::Medium)])
                        .await[0]
                {
                    queue.push_back(PosterDownloadRequest {
                        url: backdrop.clone(),
                        size: ImageSize::Medium,
                        media_id: media_id.to_string(),
                        media_type: "movie_backdrop".to_string(),
                    });
                }
            }
            MediaItem::Show(s) => {
                if let Some(backdrop) = &s.backdrop_url
                    && !self
                        .image_loader
                        .batch_check_cached(vec![(backdrop.clone(), ImageSize::Medium)])
                        .await[0]
                {
                    queue.push_back(PosterDownloadRequest {
                        url: backdrop.clone(),
                        size: ImageSize::Medium,
                        media_id: media_id.to_string(),
                        media_type: "show_backdrop".to_string(),
                    });
                }
            }
            _ => {}
        }
    }

    /// Start the background poster download processor
    fn start_poster_processor(&self) {
        let queue = self.poster_download_queue.clone();
        let image_loader = self.image_loader.clone();
        let event_bus = self.event_bus.clone();
        let cancel_token = CancellationToken::new();
        let cancel_clone = cancel_token.clone();

        // Store the cancellation token
        let poster_download_cancel = self.poster_download_cancel.clone();

        // Emit BackgroundTaskStarted event
        let event = DatabaseEvent::new(
            EventType::BackgroundTaskStarted,
            EventPayload::System {
                message: "Poster download processor started".to_string(),
                details: Some(serde_json::json!({
                    "task_type": "poster_download_processor",
                    "batch_size": 5
                })),
            },
        );

        if let Err(e) = futures::executor::block_on(event_bus.publish(event)) {
            tracing::warn!("Failed to publish BackgroundTaskStarted event: {}", e);
        }

        // Spawn the processor task
        let handle = tokio::spawn(async move {
            {
                let mut cancel_guard = poster_download_cancel.lock().await;
                *cancel_guard = cancel_clone;
            }

            info!("Started poster download processor");

            loop {
                // Check for cancellation
                if cancel_token.is_cancelled() {
                    info!("Poster download processor cancelled");
                    break;
                }

                // Process queue in batches
                const BATCH_SIZE: usize = 5;
                let mut batch = Vec::new();

                {
                    let mut queue_guard = queue.lock().await;
                    for _ in 0..BATCH_SIZE {
                        if let Some(request) = queue_guard.pop_front() {
                            batch.push(request);
                        } else {
                            break;
                        }
                    }
                }

                if batch.is_empty() {
                    // Queue is empty, wait a bit before checking again
                    tokio::select! {
                        _ = tokio::time::sleep(tokio::time::Duration::from_secs(5)) => {},
                        _ = cancel_token.cancelled() => {
                            info!("Poster download processor cancelled while idle");
                            break;
                        }
                    }
                    continue;
                }

                // Download the batch
                debug!("Processing batch of {} poster downloads", batch.len());

                let urls_to_download: Vec<(String, ImageSize)> = batch
                    .iter()
                    .map(|req| (req.url.clone(), req.size))
                    .collect();

                // Download images
                image_loader.warm_cache(urls_to_download).await;

                // Log completion for debugging
                for req in &batch {
                    debug!(
                        "Downloaded poster for {} ({})",
                        req.media_id, req.media_type
                    );
                }

                // Small delay between batches to keep priority low
                tokio::select! {
                    _ = tokio::time::sleep(tokio::time::Duration::from_millis(500)) => {},
                    _ = cancel_token.cancelled() => {
                        debug!("Poster download processor cancelled between batches");
                        break;
                    }
                }
            }

            info!("Poster download processor stopped");

            // Emit BackgroundTaskCompleted event
            let completed_event = DatabaseEvent::new(
                EventType::BackgroundTaskCompleted,
                EventPayload::System {
                    message: "Poster download processor completed".to_string(),
                    details: Some(serde_json::json!({
                        "task_type": "poster_download_processor",
                        "status": "completed"
                    })),
                },
            );

            if let Err(e) = event_bus.publish(completed_event).await {
                tracing::warn!("Failed to publish BackgroundTaskCompleted event: {}", e);
            }
        });

        // Store the handle
        let poster_download_handle = self.poster_download_handle.clone();
        tokio::spawn(async move {
            let mut handle_guard = poster_download_handle.lock().await;
            *handle_guard = Some(handle);
        });
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
