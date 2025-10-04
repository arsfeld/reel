#![cfg(test)]

use crate::db::connection::Database;
use anyhow::Result;
use sea_orm::DatabaseConnection as SeaOrmConnection;
use std::sync::Arc;
use tempfile::TempDir;

/// Test database wrapper that handles setup and teardown
pub struct TestDatabase {
    pub connection: Arc<SeaOrmConnection>,
    _temp_dir: TempDir,
}

impl TestDatabase {
    /// Create a new test database with migrations
    pub async fn new() -> Result<Self> {
        // Create temporary directory for test database
        let temp_dir = TempDir::new()?;
        let db_path = temp_dir.path().join("test.db");

        // Create database connection
        let db = Database::connect(&db_path).await?;

        // Run migrations
        db.migrate().await?;

        Ok(Self {
            connection: db.get_connection(),
            _temp_dir: temp_dir,
        })
    }

    /// Get a clone of the database connection
    pub fn connection(&self) -> Arc<SeaOrmConnection> {
        self.connection.clone()
    }
}

/// Helper function to create a test database
pub async fn create_test_db() -> Result<TestDatabase> {
    TestDatabase::new().await
}

/// Common test utilities
pub mod common {
    use std::future::Future;
    use std::time::Duration;
    use tokio::time::{sleep, timeout};

    /// Wait for an async condition to become true
    pub async fn wait_for_async<F, Fut>(mut condition: F, max_wait: Duration) -> bool
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = bool>,
    {
        let start = std::time::Instant::now();

        while start.elapsed() < max_wait {
            if condition().await {
                return true;
            }
            sleep(Duration::from_millis(10)).await;
        }

        false
    }

    /// Run a future with a timeout
    pub async fn timeout_future<T>(
        duration: Duration,
        future: impl Future<Output = T>,
    ) -> Result<T, tokio::time::error::Elapsed> {
        timeout(duration, future).await
    }
}

/// Mock backend factory for testing
pub mod mock_backend {
    use crate::backends::traits::MediaBackend;
    use crate::models::*;
    use anyhow::Result;
    use async_trait::async_trait;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicBool;
    use std::time::Duration;

    /// Simple in-memory mock backend for testing
    pub struct MockBackend {
        pub should_fail: AtomicBool,
        pub libraries: Mutex<Vec<Library>>,
        pub movies: Mutex<Vec<Movie>>,
        pub shows: Mutex<Vec<Show>>,
        pub episodes: Mutex<Vec<Episode>>,
    }

    impl MockBackend {
        pub fn new() -> Self {
            Self {
                should_fail: AtomicBool::new(false),
                libraries: Mutex::new(Vec::new()),
                movies: Mutex::new(Vec::new()),
                shows: Mutex::new(Vec::new()),
                episodes: Mutex::new(Vec::new()),
            }
        }

        pub fn set_should_fail(&self, should_fail: bool) {
            self.should_fail
                .store(should_fail, std::sync::atomic::Ordering::SeqCst);
        }

        pub fn add_library(&self, library: Library) {
            self.libraries.lock().unwrap().push(library);
        }

        pub fn add_movie(&self, movie: Movie) {
            self.movies.lock().unwrap().push(movie);
        }
    }

    impl std::fmt::Debug for MockBackend {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("MockBackend").finish()
        }
    }

    #[async_trait]
    impl MediaBackend for MockBackend {
        async fn initialize(&self) -> Result<Option<User>> {
            if self.should_fail.load(std::sync::atomic::Ordering::SeqCst) {
                anyhow::bail!("Mock initialization failure");
            }

            Ok(Some(User {
                id: "mock_user".to_string(),
                username: "Mock User".to_string(),
                email: Some("mock@example.com".to_string()),
                avatar_url: None,
            }))
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        async fn authenticate(&self, _credentials: Credentials) -> Result<User> {
            if self.should_fail.load(std::sync::atomic::Ordering::SeqCst) {
                anyhow::bail!("Mock authentication failure");
            }

            Ok(User {
                id: "mock_user".to_string(),
                username: "Mock User".to_string(),
                email: Some("mock@example.com".to_string()),
                avatar_url: None,
            })
        }

        async fn get_libraries(&self) -> Result<Vec<Library>> {
            if self.should_fail.load(std::sync::atomic::Ordering::SeqCst) {
                anyhow::bail!("Mock get_libraries failure");
            }

            Ok(self.libraries.lock().unwrap().clone())
        }

        async fn get_movies(&self, _library_id: &LibraryId) -> Result<Vec<Movie>> {
            if self.should_fail.load(std::sync::atomic::Ordering::SeqCst) {
                anyhow::bail!("Mock get_movies failure");
            }

            Ok(self.movies.lock().unwrap().clone())
        }

        async fn get_shows(&self, _library_id: &LibraryId) -> Result<Vec<Show>> {
            if self.should_fail.load(std::sync::atomic::Ordering::SeqCst) {
                anyhow::bail!("Mock get_shows failure");
            }

            Ok(self.shows.lock().unwrap().clone())
        }

        async fn get_seasons(&self, _show_id: &ShowId) -> Result<Vec<Season>> {
            if self.should_fail.load(std::sync::atomic::Ordering::SeqCst) {
                anyhow::bail!("Mock get_seasons failure");
            }

            Ok(vec![])
        }

        async fn get_episodes(&self, _show_id: &ShowId, _season: u32) -> Result<Vec<Episode>> {
            if self.should_fail.load(std::sync::atomic::Ordering::SeqCst) {
                anyhow::bail!("Mock get_episodes failure");
            }

            Ok(self.episodes.lock().unwrap().clone())
        }

        async fn get_movie_metadata(&self, movie_id: &MediaItemId) -> Result<Movie> {
            if self.should_fail.load(std::sync::atomic::Ordering::SeqCst) {
                anyhow::bail!("Mock get_movie_metadata failure");
            }

            // Return first movie that matches the ID
            self.movies
                .lock()
                .unwrap()
                .iter()
                .find(|m| m.id == movie_id.as_str())
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Movie not found"))
        }

        async fn get_show_metadata(&self, show_id: &ShowId) -> Result<Show> {
            if self.should_fail.load(std::sync::atomic::Ordering::SeqCst) {
                anyhow::bail!("Mock get_show_metadata failure");
            }

            // Return first show that matches the ID
            self.shows
                .lock()
                .unwrap()
                .iter()
                .find(|s| s.id == show_id.as_str())
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Show not found"))
        }

        async fn get_stream_url(&self, _media_id: &MediaItemId) -> Result<StreamInfo> {
            if self.should_fail.load(std::sync::atomic::Ordering::SeqCst) {
                anyhow::bail!("Mock get_stream_url failure");
            }

            Ok(StreamInfo {
                url: "http://mock.example.com/stream".to_string(),
                direct_play: true,
                video_codec: "h264".to_string(),
                audio_codec: "aac".to_string(),
                container: "mp4".to_string(),
                bitrate: 5000000,
                resolution: Resolution {
                    width: 1920,
                    height: 1080,
                },
                quality_options: vec![],
            })
        }

        async fn update_progress(
            &self,
            _media_id: &MediaItemId,
            _position: Duration,
            _duration: Duration,
        ) -> Result<()> {
            if self.should_fail.load(std::sync::atomic::Ordering::SeqCst) {
                anyhow::bail!("Mock update_progress failure");
            }

            Ok(())
        }

        async fn get_home_sections(&self) -> Result<Vec<HomeSection>> {
            if self.should_fail.load(std::sync::atomic::Ordering::SeqCst) {
                anyhow::bail!("Mock get_home_sections failure");
            }

            Ok(vec![])
        }

        async fn test_connection(
            &self,
            _url: &str,
            _auth_token: Option<&str>,
        ) -> Result<(bool, Option<u64>)> {
            if self.should_fail.load(std::sync::atomic::Ordering::SeqCst) {
                Ok((false, None))
            } else {
                Ok((true, Some(50)))
            }
        }
    }
}

/// Test fixtures
pub mod fixtures {
    use crate::models::*;
    use std::time::Duration;

    pub fn create_test_movie(id: &str) -> Movie {
        Movie {
            id: format!("movie_{}", id),
            backend_id: "test_backend".to_string(),
            title: format!("Test Movie {}", id),
            year: Some(2024),
            duration: Duration::from_secs(7200),
            rating: Some(8.5),
            poster_url: Some(format!("/posters/movie_{}.jpg", id)),
            backdrop_url: None,
            overview: Some("A test movie".to_string()),
            genres: vec!["Action".to_string()],
            cast: vec![],
            crew: vec![],
            added_at: None,
            updated_at: None,
            watched: false,
            view_count: 0,
            last_watched_at: None,
            playback_position: None,
            intro_marker: None,
            credits_marker: None,
        }
    }

    pub fn create_test_library(id: &str, library_type: LibraryType) -> Library {
        Library {
            id: format!("library_{}", id),
            title: format!("Test Library {}", id),
            library_type,
            icon: None,
            item_count: 0,
        }
    }

    pub fn create_test_user(id: &str) -> User {
        User {
            id: format!("user_{}", id),
            username: format!("testuser_{}", id),
            email: Some(format!("user_{}@test.com", id)),
            avatar_url: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::traits::MediaBackend;
    use crate::models::Credentials;

    #[tokio::test]
    async fn test_database_creation() {
        let db = TestDatabase::new().await.unwrap();

        // Test that connection works
        use sea_orm::{ConnectionTrait, Statement};
        let result = db
            .connection
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "SELECT 1",
            ))
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mock_backend() {
        use mock_backend::MockBackend;

        let backend = MockBackend::new();

        // Test normal operation
        let user = backend
            .authenticate(Credentials::Token {
                token: "test".to_string(),
            })
            .await
            .unwrap();
        assert_eq!(user.username, "Mock User");

        // Test failure mode
        backend.set_should_fail(true);
        assert!(
            backend
                .authenticate(Credentials::Token {
                    token: "test".to_string()
                })
                .await
                .is_err()
        );
    }
}
