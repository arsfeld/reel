//! Integration test for Plex authentication and sync flow
//!
//! Tests the complete flow:
//! 1. Backend authentication
//! 2. Library discovery
//! 3. Media sync to database
//! 4. Playback progress tracking

use anyhow::Result;
use mockito::Server;
use reel::backends::plex::PlexBackend;
use reel::backends::traits::MediaBackend;
use reel::db::connection::Database;
use reel::db::repository::Repository;
use reel::db::repository::media_repository::{MediaRepository, MediaRepositoryImpl};
use reel::db::repository::playback_repository::{PlaybackRepository, PlaybackRepositoryImpl};
use reel::models::*;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

/// Test fixture that sets up the complete stack
struct PlexIntegrationTest {
    server: mockito::ServerGuard,
    backend: PlexBackend,
    db_connection: Arc<sea_orm::DatabaseConnection>,
    media_repo: Arc<MediaRepositoryImpl>,
    playback_repo: Arc<PlaybackRepositoryImpl>,
    _temp_dir: TempDir,
}

impl PlexIntegrationTest {
    async fn new() -> Result<Self> {
        // Create mock server
        let server = Server::new_async().await;

        // Create test database
        let temp_dir = TempDir::new()?;
        let db_path = temp_dir.path().join("test.db");
        let db = Database::connect(&db_path).await?;
        db.migrate().await?;
        let db_connection = db.get_connection();

        // Create repositories
        let media_repo = Arc::new(MediaRepositoryImpl::new(db_connection.clone()));
        let playback_repo = Arc::new(PlaybackRepositoryImpl::new(db_connection.clone()));

        // Create backend with test server URL
        let backend = PlexBackend::new_for_test(
            server.url(),
            "test_token".to_string(),
            "test_plex".to_string(),
        )
        .await;

        // Create test source in database
        Self::create_test_source(&db_connection).await?;

        Ok(Self {
            server,
            backend,
            db_connection,
            media_repo,
            playback_repo,
            _temp_dir: temp_dir,
        })
    }

    async fn create_test_source(conn: &Arc<sea_orm::DatabaseConnection>) -> Result<()> {
        use reel::db::entities::sources::{self, ActiveModel, Entity};
        use sea_orm::{ActiveModelTrait, Set};

        let source = ActiveModel {
            id: Set("test_plex".to_string()),
            name: Set("Test Plex Server".to_string()),
            source_type: Set("plex".to_string()),
            connection_url: Set(Some("http://localhost:32400".to_string())),
            is_owned: Set(true),
            is_online: Set(true),
            ..Default::default()
        };

        source.insert(conn.as_ref()).await?;
        Ok(())
    }

    fn mock_libraries_response(&self) -> serde_json::Value {
        json!({
            "MediaContainer": {
                "size": 2,
                "Directory": [
                    {
                        "key": "1",
                        "type": "movie",
                        "title": "Movies",
                        "uuid": "abc-123"
                    },
                    {
                        "key": "2",
                        "type": "show",
                        "title": "TV Shows",
                        "uuid": "def-456"
                    }
                ]
            }
        })
    }

    fn mock_movies_response(&self) -> serde_json::Value {
        json!({
            "MediaContainer": {
                "size": 1,
                "Metadata": [{
                    "ratingKey": "movie-1",
                    "key": "/library/metadata/movie-1",
                    "guid": "plex://movie/1234",
                    "type": "movie",
                    "title": "Integration Test Movie",
                    "year": 2024,
                    "rating": 8.5,
                    "duration": 7200000,
                    "summary": "A movie for integration testing",
                    "thumb": "/library/metadata/movie-1/thumb",
                    "Genre": [{"tag": "Action"}, {"tag": "Thriller"}],
                    "addedAt": 1234567890,
                    "updatedAt": 1234567890
                }]
            }
        })
    }
}

#[tokio::test]
async fn test_full_plex_integration_flow() {
    // SETUP: Create test fixture
    let mut test = PlexIntegrationTest::new().await.unwrap();

    // STEP 1: Test library discovery
    let _mock_libraries = test
        .server
        .mock("GET", "/library/sections")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(test.mock_libraries_response().to_string())
        .create_async()
        .await;

    let libraries = test.backend.get_libraries().await.unwrap();
    assert_eq!(libraries.len(), 2);
    assert_eq!(libraries[0].title, "Movies");
    assert_eq!(libraries[0].library_type, LibraryType::Movies);

    // STEP 2: Create library in database
    use reel::db::entities::libraries::ActiveModel as LibraryActiveModel;
    use sea_orm::{ActiveModelTrait, Set};

    let lib = LibraryActiveModel {
        id: Set("1".to_string()),
        source_id: Set("test_plex".to_string()),
        title: Set(libraries[0].title.clone()),
        library_type: Set("movie".to_string()),
        ..Default::default()
    };
    lib.insert(test.db_connection.as_ref()).await.unwrap();

    // STEP 3: Test movie fetching
    let _mock_movies = test
        .server
        .mock("GET", "/library/sections/1/all?includeExtras=1&includeRelated=1&includePopularLeaves=1&includeGuids=1")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(test.mock_movies_response().to_string())
        .create_async()
        .await;

    let library_id = LibraryId::new("1");
    let movies = test.backend.get_movies(&library_id).await.unwrap();
    assert_eq!(movies.len(), 1);
    assert_eq!(movies[0].title, "Integration Test Movie");

    // STEP 4: Save movie to database
    let movie_model =
        MediaItem::Movie(movies[0].clone()).to_model("test_plex", Some("1".to_string()));
    // Use insert_silent to avoid MessageBroker issues in tests
    let inserted_movie = test.media_repo.insert_silent(movie_model).await.unwrap();

    // STEP 5: Verify movie in database
    let saved_movies = test.media_repo.find_by_library("1").await.unwrap();
    assert_eq!(saved_movies.len(), 1);
    assert_eq!(saved_movies[0].title, "Integration Test Movie");

    // STEP 6: Test playback progress tracking
    // Use the actual media item ID from the database
    let media_id = MediaItemId::new(&inserted_movie.id);
    let position = Duration::from_secs(3600); // 1 hour
    let duration = Duration::from_secs(7200); // 2 hours

    // Mock the progress update endpoint with proper query parameters
    let _mock_progress = test
        .server
        .mock("POST", "/:/timeline")
        .match_query(mockito::Matcher::AllOf(vec![
            mockito::Matcher::UrlEncoded("ratingKey".into(), "movie-1".into()),
            mockito::Matcher::UrlEncoded("key".into(), "/library/metadata/movie-1".into()),
            mockito::Matcher::UrlEncoded("identifier".into(), "com.plexapp.plugins.library".into()),
            mockito::Matcher::UrlEncoded("state".into(), "playing".into()),
            mockito::Matcher::UrlEncoded("time".into(), "3600000".into()), // 1 hour in ms
            mockito::Matcher::UrlEncoded("duration".into(), "7200000".into()), // 2 hours in ms
        ]))
        .with_status(200)
        .create_async()
        .await;

    // Update progress via backend
    test.backend
        .update_progress(&media_id, position, duration)
        .await
        .unwrap();

    // Save progress to database
    let position_ms = position.as_millis() as i64;
    let duration_ms = duration.as_millis() as i64;
    test.playback_repo
        .upsert_progress(&inserted_movie.id, None, position_ms, duration_ms)
        .await
        .unwrap();

    // Verify progress in database
    let progress = test
        .playback_repo
        .find_by_media_id(&inserted_movie.id)
        .await
        .unwrap();
    assert!(progress.is_some());
    let progress = progress.unwrap();
    assert_eq!(progress.position_ms, 3600000); // 1 hour in ms
    assert_eq!(progress.duration_ms, 7200000); // 2 hours in ms
}

#[tokio::test]
async fn test_plex_auth_failure_handling() {
    let mut test = PlexIntegrationTest::new().await.unwrap();

    // Mock authentication failure
    let _mock = test
        .server
        .mock("GET", "/library/sections")
        .with_status(401)
        .with_body("Unauthorized")
        .create_async()
        .await;

    let result = test.backend.get_libraries().await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("401"));
}

#[tokio::test]
async fn test_plex_network_error_handling() {
    let mut test = PlexIntegrationTest::new().await.unwrap();

    // Mock network error
    let _mock = test
        .server
        .mock("GET", "/library/sections")
        .with_status(503)
        .with_body("Service Unavailable")
        .create_async()
        .await;

    let result = test.backend.get_libraries().await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("503"));
}
