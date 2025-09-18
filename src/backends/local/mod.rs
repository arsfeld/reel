use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use super::traits::MediaBackend;
use crate::models::{
    BackendId, Credentials, Episode, Library, LibraryId, MediaItemId, Movie, Season, Show, ShowId,
    StreamInfo, User,
};
// Stateful services removed during Relm4 migration
// use crate::services::{AuthManager, DataService};

#[allow(dead_code)] // Placeholder for future local files support
#[derive(Debug)]
pub struct LocalBackend {
    media_directories: Arc<RwLock<Vec<PathBuf>>>,
    backend_id: String,
    last_scan_time: Arc<RwLock<Option<DateTime<Utc>>>>,
}

impl LocalBackend {
    // from_auth method removed - never used
    // Local backend is mostly unimplemented placeholder
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

    async fn get_movies(&self, _library_id: &LibraryId) -> Result<Vec<Movie>> {
        // TODO: Scan local directory for movies
        todo!("Local movie scanning not yet implemented")
    }

    async fn get_shows(&self, _library_id: &LibraryId) -> Result<Vec<Show>> {
        // TODO: Scan local directory for TV shows
        todo!("Local show scanning not yet implemented")
    }

    async fn get_seasons(&self, _show_id: &ShowId) -> Result<Vec<Season>> {
        // TODO: Scan local directory for seasons
        todo!("Local season scanning not yet implemented")
    }

    async fn get_episodes(&self, _show_id: &ShowId, _season: u32) -> Result<Vec<Episode>> {
        // TODO: Scan local directory for episodes
        todo!("Local episode scanning not yet implemented")
    }

    async fn get_stream_url(&self, media_id: &MediaItemId) -> Result<StreamInfo> {
        // For local files, the stream URL is just the file path
        Ok(StreamInfo {
            url: format!("file://{}", media_id),
            direct_play: true,
            video_codec: String::new(), // Will be detected by GStreamer
            audio_codec: String::new(),
            container: String::new(),
            bitrate: 0,
            resolution: crate::models::Resolution {
                width: 0,
                height: 0,
            },
            quality_options: vec![], // Local files don't need quality options
        })
    }

    async fn update_progress(
        &self,
        _media_id: &MediaItemId,
        _position: Duration,
        _duration: Duration,
    ) -> Result<()> {
        // TODO: Store progress locally
        todo!("Local progress tracking not yet implemented")
    }

    // Removed unused methods: mark_watched, mark_unwatched, get_watch_status, search

    async fn get_backend_id(&self) -> BackendId {
        BackendId::new(&self.backend_id)
    }

    // Removed unused methods: get_last_sync_time, supports_offline
}
