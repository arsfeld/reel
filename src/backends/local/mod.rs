use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use super::traits::{MediaBackend, SearchResults};
use crate::models::{Credentials, Episode, Library, Movie, Show, StreamInfo, User};

#[derive(Debug)]
pub struct LocalBackend {
    media_directories: Arc<RwLock<Vec<PathBuf>>>,
    backend_id: String,
    last_scan_time: Arc<RwLock<Option<DateTime<Utc>>>>,
}

impl LocalBackend {
    pub fn new() -> Self {
        Self {
            media_directories: Arc::new(RwLock::new(Vec::new())),
            backend_id: "local_media".to_string(),
            last_scan_time: Arc::new(RwLock::new(None)),
        }
    }

    pub fn with_id(id: String) -> Self {
        Self {
            media_directories: Arc::new(RwLock::new(Vec::new())),
            backend_id: id,
            last_scan_time: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn add_directory(&self, path: PathBuf) -> Result<()> {
        let mut dirs = self.media_directories.write().await;
        if !dirs.contains(&path) {
            dirs.push(path);
        }
        Ok(())
    }
}

#[async_trait]
impl MediaBackend for LocalBackend {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn initialize(&self) -> Result<Option<User>> {
        // Local backend doesn't need authentication
        // Check if we have configured directories
        let dirs = self.media_directories.read().await;
        if dirs.is_empty() {
            // No directories configured yet
            return Ok(None);
        }

        // Return a local user
        Ok(Some(User {
            id: "local".to_string(),
            username: "Local Media".to_string(),
            email: None,
            avatar_url: None,
        }))
    }

    async fn is_initialized(&self) -> bool {
        !self.media_directories.read().await.is_empty()
    }

    async fn authenticate(&self, _credentials: Credentials) -> Result<User> {
        // Local backend doesn't need authentication
        Ok(User {
            id: "local".to_string(),
            username: "Local User".to_string(),
            email: None,
            avatar_url: None,
        })
    }

    async fn get_libraries(&self) -> Result<Vec<Library>> {
        // TODO: Scan local directories for media
        todo!("Local library scanning not yet implemented")
    }

    async fn get_movies(&self, _library_id: &str) -> Result<Vec<Movie>> {
        // TODO: Scan local directory for movies
        todo!("Local movie scanning not yet implemented")
    }

    async fn get_shows(&self, _library_id: &str) -> Result<Vec<Show>> {
        // TODO: Scan local directory for TV shows
        todo!("Local show scanning not yet implemented")
    }

    async fn get_episodes(&self, _show_id: &str, _season: u32) -> Result<Vec<Episode>> {
        // TODO: Scan local directory for episodes
        todo!("Local episode scanning not yet implemented")
    }

    async fn get_stream_url(&self, media_id: &str) -> Result<StreamInfo> {
        // For local files, the stream URL is just the file path
        Ok(StreamInfo {
            url: format!("file://{}", media_id),
            direct_play: true,
            video_codec: String::new(), // Will be detected by GStreamer
            audio_codec: String::new(),
            container: String::new(),
            bitrate: 0,
            resolution: crate::models::Resolution::default(),
            quality_options: vec![], // Local files don't need quality options
        })
    }

    async fn update_progress(
        &self,
        _media_id: &str,
        _position: Duration,
        _duration: Duration,
    ) -> Result<()> {
        // TODO: Store progress locally
        todo!("Local progress tracking not yet implemented")
    }

    async fn mark_watched(&self, _media_id: &str) -> Result<()> {
        // TODO: Store watched status locally
        todo!("Local mark watched not yet implemented")
    }

    async fn mark_unwatched(&self, _media_id: &str) -> Result<()> {
        // TODO: Store unwatched status locally
        todo!("Local mark unwatched not yet implemented")
    }

    async fn get_watch_status(&self, _media_id: &str) -> Result<super::traits::WatchStatus> {
        // TODO: Get watch status from local storage
        todo!("Local get watch status not yet implemented")
    }

    async fn search(&self, _query: &str) -> Result<SearchResults> {
        // TODO: Search local files
        todo!("Local search not yet implemented")
    }

    async fn get_backend_id(&self) -> String {
        self.backend_id.clone()
    }

    async fn get_last_sync_time(&self) -> Option<DateTime<Utc>> {
        // For local backend, we return the last scan time
        *self.last_scan_time.read().await
    }

    async fn supports_offline(&self) -> bool {
        true // Local files are always available offline
    }
}
