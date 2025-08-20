use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::time::Duration;

use crate::models::{
    Credentials, Episode, Library, Movie, Show, StreamInfo, User, MediaItem, MusicAlbum, MusicTrack, Photo,
    HomeSection,
};

#[async_trait]
pub trait MediaBackend: Send + Sync + std::fmt::Debug {
    /// Initialize the backend with stored credentials
    /// Returns Ok(Some(user)) if successfully connected, Ok(None) if no credentials, Err if failed
    async fn initialize(&self) -> Result<Option<User>>;
    
    /// Check if the backend is initialized and ready to use
    async fn is_initialized(&self) -> bool;
    
    /// Get the backend as Any for downcasting
    fn as_any(&self) -> &dyn std::any::Any;
    
    async fn authenticate(&self, credentials: Credentials) -> Result<User>;
    
    async fn get_libraries(&self) -> Result<Vec<Library>>;
    
    async fn get_movies(&self, library_id: &str) -> Result<Vec<Movie>>;
    
    async fn get_shows(&self, library_id: &str) -> Result<Vec<Show>>;
    
    async fn get_episodes(&self, show_id: &str, season: u32) -> Result<Vec<Episode>>;
    
    async fn get_stream_url(&self, media_id: &str) -> Result<StreamInfo>;
    
    async fn update_progress(&self, media_id: &str, position: Duration) -> Result<()>;
    
    async fn mark_watched(&self, media_id: &str) -> Result<()>;
    
    async fn mark_unwatched(&self, media_id: &str) -> Result<()>;
    
    async fn get_watch_status(&self, media_id: &str) -> Result<WatchStatus>;
    
    async fn search(&self, query: &str) -> Result<SearchResults>;
    
    /// Get homepage sections with suggested content, recently added, etc.
    async fn get_home_sections(&self) -> Result<Vec<HomeSection>> {
        // Default implementation returns empty sections
        // Backends should override this to provide homepage data
        Ok(Vec::new())
    }
    
    // Generic media item fetching for all library types
    async fn get_library_items(&self, library_id: &str) -> Result<Vec<MediaItem>> {
        // Default implementation that only handles movies and shows
        // Backends can override this to support all types
        let library = self.get_libraries().await?
            .into_iter()
            .find(|l| l.id == library_id)
            .ok_or_else(|| anyhow::anyhow!("Library not found"))?;
        
        use crate::models::LibraryType;
        match library.library_type {
            LibraryType::Movies => {
                let movies = self.get_movies(library_id).await?;
                Ok(movies.into_iter().map(MediaItem::Movie).collect())
            }
            LibraryType::Shows => {
                let shows = self.get_shows(library_id).await?;
                Ok(shows.into_iter().map(MediaItem::Show).collect())
            }
            LibraryType::Music => {
                // Backend should override this method to support music
                Ok(Vec::new())
            }
            LibraryType::Photos => {
                // Backend should override this method to support photos
                Ok(Vec::new())
            }
            LibraryType::Mixed => {
                // Backend should override this method to support mixed content
                Ok(Vec::new())
            }
        }
    }
    
    // Optional: Get music albums for music libraries
    async fn get_music_albums(&self, _library_id: &str) -> Result<Vec<MusicAlbum>> {
        Ok(Vec::new())
    }
    
    // Optional: Get music tracks for an album
    async fn get_music_tracks(&self, _album_id: &str) -> Result<Vec<MusicTrack>> {
        Ok(Vec::new())
    }
    
    // Optional: Get photos for photo libraries
    async fn get_photos(&self, _library_id: &str) -> Result<Vec<Photo>> {
        Ok(Vec::new())
    }
    
    // Backend information
    async fn get_backend_info(&self) -> BackendInfo {
        BackendInfo {
            name: self.get_backend_id().await,
            display_name: self.get_backend_id().await,
            backend_type: BackendType::Generic,
            server_name: None,
            server_version: None,
            connection_type: ConnectionType::Unknown,
            is_local: false,
            is_relay: false,
        }
    }
    
    // Sync support methods
    async fn get_backend_id(&self) -> String;
    
    async fn get_last_sync_time(&self) -> Option<DateTime<Utc>>;
    
    async fn supports_offline(&self) -> bool;
}

#[derive(Debug, Clone)]
pub struct SearchResults {
    pub movies: Vec<Movie>,
    pub shows: Vec<Show>,
    pub episodes: Vec<Episode>,
}

#[derive(Debug, Clone)]
pub struct WatchStatus {
    pub watched: bool,
    pub view_count: u32,
    pub last_watched_at: Option<DateTime<Utc>>,
    pub playback_position: Option<Duration>,
}

#[derive(Debug, Clone)]
pub struct SyncResult {
    pub backend_id: String,
    pub success: bool,
    pub items_synced: usize,
    pub duration: Duration,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum SyncType {
    Full,           // Full sync of all data
    Incremental,    // Only changes since last sync
    Library(String), // Specific library
    Media(String),   // Specific media item
}

#[derive(Debug, Clone)]
pub enum SyncPriority {
    High,
    Normal,
    Low,
}

#[derive(Debug, Clone)]
pub enum SyncStatus {
    Idle,
    Syncing { progress: f32, current_item: String },
    Completed { at: DateTime<Utc>, items_synced: usize },
    Failed { error: String, at: DateTime<Utc> },
}

#[derive(Debug, Clone)]
pub struct SyncTask {
    pub backend_id: String,
    pub sync_type: SyncType,
    pub priority: SyncPriority,
    pub scheduled_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct BackendOfflineInfo {
    pub total_items: usize,
    pub size_mb: u64,
    pub last_sync: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct OfflineStatus {
    pub total_size_mb: u64,
    pub used_size_mb: u64,
    pub items_count: usize,
    pub backends: std::collections::HashMap<String, BackendOfflineInfo>,
}

#[derive(Debug, Clone)]
pub enum BackendType {
    Plex,
    Jellyfin,
    Local,
    Generic,
}

impl std::fmt::Display for BackendType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendType::Plex => write!(f, "Plex"),
            BackendType::Jellyfin => write!(f, "Jellyfin"),
            BackendType::Local => write!(f, "Local Files"),
            BackendType::Generic => write!(f, "Generic"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionType {
    Local,
    Remote,
    Relay,
    Offline,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct BackendInfo {
    pub name: String,
    pub display_name: String,
    pub backend_type: BackendType,
    pub server_name: Option<String>,
    pub server_version: Option<String>,
    pub connection_type: ConnectionType,
    pub is_local: bool,
    pub is_relay: bool,
}