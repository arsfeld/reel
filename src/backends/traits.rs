use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::time::Duration;

use crate::models::{
    ChapterMarker, Credentials, Episode, HomeSection, Library, MediaItem, Movie, MusicAlbum,
    MusicTrack, Photo, Show, StreamInfo, User,
};

#[async_trait]
pub trait MediaBackend: Send + Sync + std::fmt::Debug {
    /// Initialize the backend with stored credentials
    /// Returns Ok(Some(user)) if successfully connected, Ok(None) if no credentials, Err if failed
    async fn initialize(&self) -> Result<Option<User>>;

    /// Check if the backend is initialized and ready to use
    async fn is_initialized(&self) -> bool;

    /// Check if the backend has credentials and can attempt playback
    /// This is faster than full initialization and enables streaming without full API connection
    async fn is_playback_ready(&self) -> bool {
        // Default implementation checks if initialized, but backends should override
        // to check credentials without requiring full connection test
        self.is_initialized().await
    }

    /// Get the backend as Any for downcasting
    fn as_any(&self) -> &dyn std::any::Any;

    async fn authenticate(&self, credentials: Credentials) -> Result<User>;

    async fn get_libraries(&self) -> Result<Vec<Library>>;

    async fn get_movies(&self, library_id: &str) -> Result<Vec<Movie>>;

    async fn get_shows(&self, library_id: &str) -> Result<Vec<Show>>;

    async fn get_episodes(&self, show_id: &str, season: u32) -> Result<Vec<Episode>>;

    async fn get_stream_url(&self, media_id: &str) -> Result<StreamInfo>;

    async fn update_progress(
        &self,
        media_id: &str,
        position: Duration,
        duration: Duration,
    ) -> Result<()>;

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

    /// Fetch intro and credits markers for an episode
    async fn fetch_episode_markers(
        &self,
        _episode_id: &str,
    ) -> Result<(Option<ChapterMarker>, Option<ChapterMarker>)> {
        // Default implementation returns no markers
        // Only Plex backend currently implements this
        Ok((None, None))
    }

    /// Fetch intro and credits markers for any media (movie or episode)
    async fn fetch_media_markers(
        &self,
        _media_id: &str,
    ) -> Result<(Option<ChapterMarker>, Option<ChapterMarker>)> {
        // Default implementation returns no markers
        // Backends should override this to provide marker functionality
        Ok((None, None))
    }

    /// Find the next episode after the given episode
    async fn find_next_episode(&self, _current_episode: &Episode) -> Result<Option<Episode>> {
        // Default implementation returns None
        // Backends should override this to provide next episode functionality
        Ok(None)
    }

    // Generic media item fetching for all library types
    async fn get_library_items(&self, library_id: &str) -> Result<Vec<MediaItem>> {
        // Default implementation that only handles movies and shows
        // Backends can override this to support all types
        let library = self
            .get_libraries()
            .await?
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::HashMap;
    use std::time::Duration;

    #[test]
    fn test_search_results_creation() {
        let results = SearchResults {
            movies: Vec::new(),
            shows: Vec::new(),
            episodes: Vec::new(),
        };

        assert_eq!(results.movies.len(), 0);
        assert_eq!(results.shows.len(), 0);
        assert_eq!(results.episodes.len(), 0);
    }

    #[test]
    fn test_watch_status_default() {
        let status = WatchStatus {
            watched: false,
            view_count: 0,
            last_watched_at: None,
            playback_position: None,
        };

        assert!(!status.watched);
        assert_eq!(status.view_count, 0);
        assert!(status.last_watched_at.is_none());
        assert!(status.playback_position.is_none());
    }

    #[test]
    fn test_watch_status_with_data() {
        let now = Utc::now();
        let position = Duration::from_secs(1234);

        let status = WatchStatus {
            watched: true,
            view_count: 3,
            last_watched_at: Some(now),
            playback_position: Some(position),
        };

        assert!(status.watched);
        assert_eq!(status.view_count, 3);
        assert_eq!(status.last_watched_at, Some(now));
        assert_eq!(status.playback_position, Some(position));
    }

    #[test]
    fn test_sync_result() {
        let result = SyncResult {
            backend_id: "test_backend".to_string(),
            success: true,
            items_synced: 42,
            duration: Duration::from_secs(10),
            errors: vec!["warning1".to_string()],
        };

        assert_eq!(result.backend_id, "test_backend");
        assert!(result.success);
        assert_eq!(result.items_synced, 42);
        assert_eq!(result.duration, Duration::from_secs(10));
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0], "warning1");
    }

    #[test]
    fn test_sync_type_variants() {
        let full = SyncType::Full;
        let incremental = SyncType::Incremental;
        let library = SyncType::Library("lib123".to_string());
        let media = SyncType::Media("media456".to_string());

        match full {
            SyncType::Full => assert!(true),
            _ => panic!("Expected Full variant"),
        }

        match incremental {
            SyncType::Incremental => assert!(true),
            _ => panic!("Expected Incremental variant"),
        }

        match library {
            SyncType::Library(id) => assert_eq!(id, "lib123"),
            _ => panic!("Expected Library variant"),
        }

        match media {
            SyncType::Media(id) => assert_eq!(id, "media456"),
            _ => panic!("Expected Media variant"),
        }
    }

    #[test]
    fn test_sync_priority() {
        let high = SyncPriority::High;
        let normal = SyncPriority::Normal;
        let low = SyncPriority::Low;

        match high {
            SyncPriority::High => assert!(true),
            _ => panic!("Expected High priority"),
        }

        match normal {
            SyncPriority::Normal => assert!(true),
            _ => panic!("Expected Normal priority"),
        }

        match low {
            SyncPriority::Low => assert!(true),
            _ => panic!("Expected Low priority"),
        }
    }

    #[test]
    fn test_sync_status_idle() {
        let status = SyncStatus::Idle;

        match status {
            SyncStatus::Idle => assert!(true),
            _ => panic!("Expected Idle status"),
        }
    }

    #[test]
    fn test_sync_status_syncing() {
        let status = SyncStatus::Syncing {
            progress: 0.75,
            current_item: "Movie XYZ".to_string(),
        };

        match status {
            SyncStatus::Syncing {
                progress,
                current_item,
            } => {
                assert_eq!(progress, 0.75);
                assert_eq!(current_item, "Movie XYZ");
            }
            _ => panic!("Expected Syncing status"),
        }
    }

    #[test]
    fn test_sync_status_completed() {
        let now = Utc::now();
        let status = SyncStatus::Completed {
            at: now,
            items_synced: 100,
        };

        match status {
            SyncStatus::Completed { at, items_synced } => {
                assert_eq!(at, now);
                assert_eq!(items_synced, 100);
            }
            _ => panic!("Expected Completed status"),
        }
    }

    #[test]
    fn test_sync_status_failed() {
        let now = Utc::now();
        let status = SyncStatus::Failed {
            error: "Network error".to_string(),
            at: now,
        };

        match status {
            SyncStatus::Failed { error, at } => {
                assert_eq!(error, "Network error");
                assert_eq!(at, now);
            }
            _ => panic!("Expected Failed status"),
        }
    }

    #[test]
    fn test_sync_task() {
        let now = Utc::now();
        let task = SyncTask {
            backend_id: "backend1".to_string(),
            sync_type: SyncType::Full,
            priority: SyncPriority::High,
            scheduled_at: now,
        };

        assert_eq!(task.backend_id, "backend1");
        assert!(matches!(task.sync_type, SyncType::Full));
        assert!(matches!(task.priority, SyncPriority::High));
        assert_eq!(task.scheduled_at, now);
    }

    #[test]
    fn test_backend_offline_info() {
        let now = Utc::now();
        let info = BackendOfflineInfo {
            total_items: 500,
            size_mb: 2048,
            last_sync: Some(now),
        };

        assert_eq!(info.total_items, 500);
        assert_eq!(info.size_mb, 2048);
        assert_eq!(info.last_sync, Some(now));
    }

    #[test]
    fn test_offline_status() {
        let mut backends = HashMap::new();
        backends.insert(
            "backend1".to_string(),
            BackendOfflineInfo {
                total_items: 100,
                size_mb: 512,
                last_sync: None,
            },
        );

        let status = OfflineStatus {
            total_size_mb: 1024,
            used_size_mb: 512,
            items_count: 100,
            backends,
        };

        assert_eq!(status.total_size_mb, 1024);
        assert_eq!(status.used_size_mb, 512);
        assert_eq!(status.items_count, 100);
        assert_eq!(status.backends.len(), 1);
        assert!(status.backends.contains_key("backend1"));
    }

    #[test]
    fn test_backend_type_display() {
        assert_eq!(BackendType::Plex.to_string(), "Plex");
        assert_eq!(BackendType::Jellyfin.to_string(), "Jellyfin");
        assert_eq!(BackendType::Local.to_string(), "Local Files");
        assert_eq!(BackendType::Generic.to_string(), "Generic");
    }

    #[test]
    fn test_connection_type_equality() {
        assert_eq!(ConnectionType::Local, ConnectionType::Local);
        assert_eq!(ConnectionType::Remote, ConnectionType::Remote);
        assert_eq!(ConnectionType::Relay, ConnectionType::Relay);
        assert_eq!(ConnectionType::Offline, ConnectionType::Offline);
        assert_eq!(ConnectionType::Unknown, ConnectionType::Unknown);

        assert_ne!(ConnectionType::Local, ConnectionType::Remote);
        assert_ne!(ConnectionType::Offline, ConnectionType::Unknown);
    }

    #[test]
    fn test_backend_info_creation() {
        let info = BackendInfo {
            name: "test_backend".to_string(),
            display_name: "Test Backend".to_string(),
            backend_type: BackendType::Plex,
            server_name: Some("My Plex Server".to_string()),
            server_version: Some("1.32.0".to_string()),
            connection_type: ConnectionType::Local,
            is_local: true,
            is_relay: false,
        };

        assert_eq!(info.name, "test_backend");
        assert_eq!(info.display_name, "Test Backend");
        assert!(matches!(info.backend_type, BackendType::Plex));
        assert_eq!(info.server_name, Some("My Plex Server".to_string()));
        assert_eq!(info.server_version, Some("1.32.0".to_string()));
        assert_eq!(info.connection_type, ConnectionType::Local);
        assert!(info.is_local);
        assert!(!info.is_relay);
    }

    #[test]
    fn test_backend_info_minimal() {
        let info = BackendInfo {
            name: "generic".to_string(),
            display_name: "Generic Backend".to_string(),
            backend_type: BackendType::Generic,
            server_name: None,
            server_version: None,
            connection_type: ConnectionType::Unknown,
            is_local: false,
            is_relay: false,
        };

        assert_eq!(info.name, "generic");
        assert_eq!(info.display_name, "Generic Backend");
        assert!(matches!(info.backend_type, BackendType::Generic));
        assert!(info.server_name.is_none());
        assert!(info.server_version.is_none());
        assert_eq!(info.connection_type, ConnectionType::Unknown);
        assert!(!info.is_local);
        assert!(!info.is_relay);
    }
}
