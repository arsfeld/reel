use anyhow::{Result, Context};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error};

use crate::backends::traits::MediaBackend;
use crate::models::{Library, Movie, Show};
use crate::services::cache::CacheManager;

#[derive(Debug, Clone)]
pub enum SyncType {
    Full,           // Full sync of all data
    Incremental,    // Only changes since last sync
    Library(String), // Specific library
    Media(String),   // Specific media item
}

#[derive(Debug, Clone)]
pub enum SyncStatus {
    Idle,
    Syncing { progress: f32, current_item: String },
    Completed { at: DateTime<Utc>, items_synced: usize },
    Failed { error: String, at: DateTime<Utc> },
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
}

impl SyncManager {
    pub fn new(cache: Arc<CacheManager>) -> Self {
        Self {
            cache,
            sync_status: Arc::new(RwLock::new(HashMap::new())),
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
                if let Err(e) = self.cache.set_media(&libraries_key, "library_list", &libraries).await {
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
                    
                    // Sync library content based on type
                    match &library.library_type {
                        crate::models::LibraryType::Movies => {
                            if let Err(e) = self.sync_movies(backend_id, &library.id, backend.clone()).await {
                                error!("Failed to sync movies from library {}: {}", library.id, e);
                                errors.push(format!("Failed to sync movies: {}", e));
                            } else {
                                items_synced += 1;
                            }
                        }
                        crate::models::LibraryType::Shows => {
                            if let Err(e) = self.sync_shows(backend_id, &library.id, backend.clone()).await {
                                error!("Failed to sync shows from library {}: {}", library.id, e);
                                errors.push(format!("Failed to sync shows: {}", e));
                            } else {
                                items_synced += 1;
                            }
                        }
                        _ => {
                            debug!("Skipping library {} of type {:?}", library.id, library.library_type);
                        }
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
    
    /// Sync movies from a library
    async fn sync_movies(
        &self,
        backend_id: &str,
        library_id: &str,
        backend: Arc<dyn MediaBackend>,
    ) -> Result<()> {
        info!("Syncing movies from library {}", library_id);
        
        let movies = backend.get_movies(library_id).await
            .context("Failed to fetch movies")?;
        
        info!("Found {} movies to sync", movies.len());
        
        // Cache each movie
        for movie in &movies {
            let cache_key = format!("{}:movie:{}", backend_id, movie.id);
            self.cache.set_media(&cache_key, "movie", movie).await
                .context(format!("Failed to cache movie {}", movie.id))?;
        }
        
        // Cache the movie list for this library
        let movies_key = format!("{}:library:{}:movies", backend_id, library_id);
        self.cache.set_media(&movies_key, "movie_list", &movies).await
            .context("Failed to cache movie list")?;
        
        Ok(())
    }
    
    /// Sync TV shows from a library
    async fn sync_shows(
        &self,
        backend_id: &str,
        library_id: &str,
        backend: Arc<dyn MediaBackend>,
    ) -> Result<()> {
        info!("Syncing shows from library {}", library_id);
        
        let shows = backend.get_shows(library_id).await
            .context("Failed to fetch shows")?;
        
        info!("Found {} shows to sync", shows.len());
        
        // Cache each show
        for show in &shows {
            let cache_key = format!("{}:show:{}", backend_id, show.id);
            self.cache.set_media(&cache_key, "show", show).await
                .context(format!("Failed to cache show {}", show.id))?;
        }
        
        // Cache the show list for this library
        let shows_key = format!("{}:library:{}:shows", backend_id, library_id);
        self.cache.set_media(&shows_key, "show_list", &shows).await
            .context("Failed to cache show list")?;
        
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
        Ok(self.cache.get_media(&libraries_key).await?.unwrap_or_default())
    }
    
    /// Get cached movies for a library
    pub async fn get_cached_movies(&self, backend_id: &str, library_id: &str) -> Result<Vec<Movie>> {
        let movies_key = format!("{}:library:{}:movies", backend_id, library_id);
        Ok(self.cache.get_media(&movies_key).await?.unwrap_or_default())
    }
    
    /// Get cached shows for a library
    pub async fn get_cached_shows(&self, backend_id: &str, library_id: &str) -> Result<Vec<Show>> {
        let shows_key = format!("{}:library:{}:shows", backend_id, library_id);
        Ok(self.cache.get_media(&shows_key).await?.unwrap_or_default())
    }
}