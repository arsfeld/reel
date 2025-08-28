use anyhow::{Result, anyhow};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use super::traits::{MediaBackend, SearchResults};
use crate::models::{
    AuthProvider, Credentials, Episode, Library, Movie, Show, Source, StreamInfo, User,
};
use crate::services::{AuthManager, DataService};

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

    /// Create from AuthProvider and Source
    pub fn from_auth(
        _provider: AuthProvider,
        source: Source,
        _auth_manager: Arc<AuthManager>,
        _cache: Option<Arc<DataService>>,
    ) -> Result<Self> {
        // Extract path from source
        let path = match source.source_type {
            crate::models::SourceType::LocalFolder { path } => path,
            _ => return Err(anyhow!("Invalid source type for LocalBackend")),
        };

        let backend = Self {
            media_directories: Arc::new(RwLock::new(vec![path])),
            backend_id: source.id,
            last_scan_time: Arc::new(RwLock::new(None)),
        };

        Ok(backend)
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
            resolution: crate::models::Resolution {
                width: 0,
                height: 0,
            },
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AuthProvider, ConnectionInfo, Source, SourceType};

    #[test]
    fn test_new() {
        let backend = LocalBackend::new();
        assert_eq!(backend.backend_id, "local_media");
    }

    #[test]
    fn test_with_id() {
        let backend = LocalBackend::with_id("custom_local".to_string());
        assert_eq!(backend.backend_id, "custom_local");
    }

    #[tokio::test]
    async fn test_add_directory() {
        let backend = LocalBackend::new();
        let path = PathBuf::from("/test/media");

        assert!(backend.media_directories.read().await.is_empty());

        backend.add_directory(path.clone()).await.unwrap();

        let dirs = backend.media_directories.read().await;
        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0], path);
    }

    #[tokio::test]
    async fn test_add_directory_duplicate() {
        let backend = LocalBackend::new();
        let path = PathBuf::from("/test/media");

        backend.add_directory(path.clone()).await.unwrap();
        backend.add_directory(path.clone()).await.unwrap();

        let dirs = backend.media_directories.read().await;
        assert_eq!(dirs.len(), 1); // Should not add duplicate
    }

    #[tokio::test]
    async fn test_add_multiple_directories() {
        let backend = LocalBackend::new();
        let path1 = PathBuf::from("/test/media1");
        let path2 = PathBuf::from("/test/media2");
        let path3 = PathBuf::from("/test/media3");

        backend.add_directory(path1.clone()).await.unwrap();
        backend.add_directory(path2.clone()).await.unwrap();
        backend.add_directory(path3.clone()).await.unwrap();

        let dirs = backend.media_directories.read().await;
        assert_eq!(dirs.len(), 3);
        assert!(dirs.contains(&path1));
        assert!(dirs.contains(&path2));
        assert!(dirs.contains(&path3));
    }

    #[test]
    fn test_from_auth_invalid_source_type() {
        let auth_provider = AuthProvider::PlexAccount {
            id: "plex".to_string(),
            username: "user".to_string(),
            email: "user@example.com".to_string(),
            token: "token123".to_string(),
            refresh_token: None,
            token_expiry: None,
        };

        let source = Source::new(
            "source1".to_string(),
            "Test Source".to_string(),
            SourceType::PlexServer {
                machine_id: "abc123".to_string(),
                owned: true,
            },
            Some("plex".to_string()),
        );

        let auth_manager = Arc::new(AuthManager::new());
        let result = LocalBackend::from_auth(auth_provider, source, auth_manager, None);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid source type")
        );
    }

    #[test]
    fn test_from_auth_valid() {
        let auth_provider = AuthProvider::LocalFiles {
            id: "local".to_string(),
        };

        let path = PathBuf::from("/home/user/media");
        let source = Source::new(
            "local_source".to_string(),
            "Local Media".to_string(),
            SourceType::LocalFolder { path: path.clone() },
            Some("local".to_string()),
        );

        let auth_manager = Arc::new(AuthManager::new());
        let result = LocalBackend::from_auth(auth_provider, source, auth_manager, None);

        assert!(result.is_ok());
        let backend = result.unwrap();
        assert_eq!(backend.backend_id, "local_source");
    }

    #[tokio::test]
    async fn test_from_auth_sets_directory() {
        let auth_provider = AuthProvider::LocalFiles {
            id: "local".to_string(),
        };

        let path = PathBuf::from("/home/user/videos");
        let source = Source::new(
            "local_videos".to_string(),
            "My Videos".to_string(),
            SourceType::LocalFolder { path: path.clone() },
            Some("local".to_string()),
        );

        let auth_manager = Arc::new(AuthManager::new());
        let backend = LocalBackend::from_auth(auth_provider, source, auth_manager, None).unwrap();

        let dirs = backend.media_directories.read().await;
        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0], path);
    }

    #[tokio::test]
    async fn test_initialize_no_directories() {
        let backend = LocalBackend::new();
        let user = backend.initialize().await.unwrap();
        assert!(user.is_none());
    }

    #[tokio::test]
    async fn test_initialize_with_directories() {
        let backend = LocalBackend::new();
        backend.add_directory(PathBuf::from("/test")).await.unwrap();

        let user = backend.initialize().await.unwrap();
        assert!(user.is_some());

        let user = user.unwrap();
        assert_eq!(user.id, "local");
        assert_eq!(user.username, "Local Media");
        assert!(user.email.is_none());
        assert!(user.avatar_url.is_none());
    }

    #[tokio::test]
    async fn test_is_initialized_false() {
        let backend = LocalBackend::new();
        assert!(!backend.is_initialized().await);
    }

    #[tokio::test]
    async fn test_is_initialized_true() {
        let backend = LocalBackend::new();
        backend
            .add_directory(PathBuf::from("/media"))
            .await
            .unwrap();
        assert!(backend.is_initialized().await);
    }

    #[tokio::test]
    async fn test_authenticate() {
        let backend = LocalBackend::new();

        let user = backend
            .authenticate(Credentials::Token {
                token: "ignored".to_string(),
            })
            .await
            .unwrap();

        assert_eq!(user.id, "local");
        assert_eq!(user.username, "Local User");
        assert!(user.email.is_none());
        assert!(user.avatar_url.is_none());
    }

    #[tokio::test]
    async fn test_get_stream_url() {
        let backend = LocalBackend::new();
        let media_id = "/home/user/movie.mp4";

        let stream_info = backend.get_stream_url(media_id).await.unwrap();

        assert_eq!(stream_info.url, "file:///home/user/movie.mp4");
        assert!(stream_info.direct_play);
        assert!(stream_info.video_codec.is_empty());
        assert!(stream_info.audio_codec.is_empty());
        assert!(stream_info.container.is_empty());
        assert_eq!(stream_info.bitrate, 0);
        assert_eq!(stream_info.resolution.width, 0);
        assert_eq!(stream_info.resolution.height, 0);
        assert!(stream_info.quality_options.is_empty());
    }

    #[tokio::test]
    async fn test_get_backend_id() {
        let backend = LocalBackend::with_id("my_local_backend".to_string());
        let id = backend.get_backend_id().await;
        assert_eq!(id, "my_local_backend");
    }

    #[tokio::test]
    async fn test_get_last_sync_time_none() {
        let backend = LocalBackend::new();
        let time = backend.get_last_sync_time().await;
        assert!(time.is_none());
    }

    #[tokio::test]
    async fn test_get_last_sync_time_with_value() {
        let backend = LocalBackend::new();
        let now = Utc::now();

        *backend.last_scan_time.write().await = Some(now);

        let time = backend.get_last_sync_time().await;
        assert!(time.is_some());
        assert_eq!(time.unwrap(), now);
    }

    #[tokio::test]
    async fn test_supports_offline() {
        let backend = LocalBackend::new();
        assert!(backend.supports_offline().await);
    }

    #[test]
    fn test_debug_impl() {
        let backend = LocalBackend::new();
        let debug_str = format!("{:?}", backend);

        assert!(debug_str.contains("LocalBackend"));
        assert!(debug_str.contains("media_directories"));
        assert!(debug_str.contains("backend_id"));
        assert!(debug_str.contains("last_scan_time"));
    }

    #[tokio::test]
    async fn test_get_libraries_todo() {
        let backend = LocalBackend::new();
        let result = std::panic::catch_unwind(|| {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async { backend.get_libraries().await })
        });
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_movies_todo() {
        let backend = LocalBackend::new();
        let result = std::panic::catch_unwind(|| {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async { backend.get_movies("lib1").await })
        });
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_shows_todo() {
        let backend = LocalBackend::new();
        let result = std::panic::catch_unwind(|| {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async { backend.get_shows("lib1").await })
        });
        assert!(result.is_err());
    }
}
