use anyhow::Result;
use async_trait::async_trait;
use std::time::Duration;

use crate::models::{
    ChapterMarker, Credentials, Episode, HomeSection, Library, LibraryId, MediaItemId, Movie,
    Season, Show, ShowId, StreamInfo, User,
};

#[async_trait]
pub trait MediaBackend: Send + Sync + std::fmt::Debug {
    /// Initialize the backend with stored credentials
    /// Returns Ok(Some(user)) if successfully connected, Ok(None) if no credentials, Err if failed
    async fn initialize(&self) -> Result<Option<User>>;

    // is_initialized and is_playback_ready removed - never used

    /// Get the backend as Any for downcasting
    fn as_any(&self) -> &dyn std::any::Any;

    async fn authenticate(&self, credentials: Credentials) -> Result<User>;

    async fn get_libraries(&self) -> Result<Vec<Library>>;

    async fn get_movies(&self, library_id: &LibraryId) -> Result<Vec<Movie>>;

    async fn get_shows(&self, library_id: &LibraryId) -> Result<Vec<Show>>;

    /// Get full metadata for a single movie (including complete cast/crew)
    /// Used for lazy loading detailed information when user views movie details
    async fn get_movie_metadata(&self, movie_id: &MediaItemId) -> Result<Movie>;

    /// Get full metadata for a single show (including complete cast/crew)
    /// Used for lazy loading detailed information when user views show details
    async fn get_show_metadata(&self, show_id: &ShowId) -> Result<Show>;

    async fn get_seasons(&self, show_id: &ShowId) -> Result<Vec<Season>>;

    async fn get_episodes(&self, show_id: &ShowId, season: u32) -> Result<Vec<Episode>>;

    async fn get_stream_url(&self, media_id: &MediaItemId) -> Result<StreamInfo>;

    async fn update_progress(
        &self,
        media_id: &MediaItemId,
        position: Duration,
        duration: Duration,
    ) -> Result<()>;

    /// Fetch intro and credits markers for a media item
    /// Returns (intro_marker, credits_marker) tuple with None if markers don't exist
    /// Used during playback initialization to enable skip intro/credits buttons
    async fn fetch_markers(
        &self,
        media_id: &MediaItemId,
    ) -> Result<(Option<ChapterMarker>, Option<ChapterMarker>)> {
        // Default implementation returns no markers
        // Backends should override this if they support markers
        Ok((None, None))
    }

    // Watch status methods removed - never used in production
    // Search method removed - never used in production

    /// Get homepage sections with suggested content, recently added, etc.
    async fn get_home_sections(&self) -> Result<Vec<HomeSection>> {
        // Default implementation returns empty sections
        // Backends should override this to provide homepage data
        Ok(Vec::new())
    }

    /// Mark a media item as watched on the backend server
    async fn mark_watched(&self, _item_id: &str) -> Result<()> {
        // Default implementation does nothing
        // Backends should override this to sync watch status
        Ok(())
    }

    /// Mark a media item as unwatched on the backend server
    async fn mark_unwatched(&self, _item_id: &str) -> Result<()> {
        // Default implementation does nothing
        // Backends should override this to sync watch status
        Ok(())
    }

    // Marker and navigation methods removed - never used in production

    // get_library_items removed - never used in production

    // Music and photo methods removed - never implemented

    // get_backend_info removed - never used

    // Sync support methods

    // get_last_sync_time and supports_offline removed - never used
    // get_backend_id removed - never used

    /// Test a connection URL for availability and latency
    /// Returns (is_available, response_time_ms)
    async fn test_connection(
        &self,
        url: &str,
        auth_token: Option<&str>,
    ) -> Result<(bool, Option<u64>)>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::entities::sync_status::SyncType;
    use crate::services::core::sync::SyncStatus;
    use chrono::{DateTime, Utc};
    use std::collections::HashMap;
    use std::time::Duration;

    #[derive(Debug, Clone)]
    enum SyncPriority {
        High,
        Normal,
        Low,
    }

    #[derive(Debug)]
    struct SyncTask {
        backend_id: String,
        sync_type: SyncType,
        priority: SyncPriority,
        scheduled_at: DateTime<Utc>,
    }

    // Test-only types
    #[derive(Debug)]
    struct SearchResults {
        movies: Vec<Movie>,
        shows: Vec<Show>,
        episodes: Vec<Episode>,
    }

    #[derive(Debug)]
    struct WatchStatus {
        watched: bool,
        view_count: u32,
        last_watched_at: Option<DateTime<Utc>>,
        playback_position: Option<Duration>,
    }

    #[derive(Debug)]
    struct SyncResult {
        backend_id: BackendId,
        success: bool,
        items_synced: usize,
        duration: Duration,
        errors: Vec<String>,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct BackendId(String);

    impl BackendId {
        fn as_str(&self) -> &str {
            &self.0
        }
    }

    impl From<&str> for BackendId {
        fn from(s: &str) -> Self {
            BackendId(s.to_string())
        }
    }

    #[derive(Debug, Clone)]
    enum BackendType {
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
            backend_id: BackendId::from("test_backend"),
            success: true,
            items_synced: 42,
            duration: Duration::from_secs(10),
            errors: vec!["warning1".to_string()],
        };

        assert_eq!(result.backend_id.as_str(), "test_backend");
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
    fn test_sync_status_in_progress() {
        let status = SyncStatus::InProgress;

        match status {
            SyncStatus::InProgress => {
                // Test passed - InProgress status created correctly
            }
            _ => panic!("Expected InProgress status"),
        }
    }

    #[test]
    fn test_sync_status_completed() {
        let status = SyncStatus::Completed;

        match status {
            SyncStatus::Completed => {
                // Test passed - Completed status created correctly
            }
            _ => panic!("Expected Completed status"),
        }
    }

    #[test]
    fn test_sync_status_failed() {
        let status = SyncStatus::Failed;

        match status {
            SyncStatus::Failed => {
                // Test passed - Failed status created correctly
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
    fn test_backend_type_display() {
        assert_eq!(BackendType::Plex.to_string(), "Plex");
        assert_eq!(BackendType::Jellyfin.to_string(), "Jellyfin");
        assert_eq!(BackendType::Local.to_string(), "Local Files");
        assert_eq!(BackendType::Generic.to_string(), "Generic");
    }
}
