//! Integration test for Jellyfin authentication and sync flow
//!
//! Tests the complete flow:
//! 1. Backend authentication
//! 2. Library discovery
//! 3. Media sync to database
//! 4. Playback progress tracking

#![allow(dead_code, unused_imports)]

use anyhow::Result;
use mockito::Server;
use reel::backends::jellyfin::JellyfinBackend;
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

/// Test fixture for Jellyfin integration tests
struct JellyfinIntegrationTest {
    server: mockito::ServerGuard,
    backend: JellyfinBackend,
    db_connection: Arc<sea_orm::DatabaseConnection>,
    media_repo: Arc<MediaRepositoryImpl>,
    playback_repo: Arc<PlaybackRepositoryImpl>,
    _temp_dir: TempDir,
}

impl JellyfinIntegrationTest {
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
        let backend = JellyfinBackend::new_for_test(
            server.url(),
            "test_token".to_string(),
            "test_user".to_string(),
            "test_jellyfin".to_string(),
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
        use reel::db::entities::sources::ActiveModel;
        use sea_orm::{ActiveModelTrait, Set};

        let source = ActiveModel {
            id: Set("test_jellyfin".to_string()),
            name: Set("Test Jellyfin Server".to_string()),
            source_type: Set("jellyfin".to_string()),
            connection_url: Set(Some("http://localhost:8096".to_string())),
            is_owned: Set(true),
            is_online: Set(true),
            ..Default::default()
        };

        source.insert(conn.as_ref()).await?;
        Ok(())
    }

    fn mock_libraries_response(&self) -> serde_json::Value {
        json!({
            "Items": [
                {
                    "Id": "lib-1",
                    "Name": "Movies",
                    "CollectionType": "movies",
                    "Type": "CollectionFolder"
                },
                {
                    "Id": "lib-2",
                    "Name": "TV Shows",
                    "CollectionType": "tvshows",
                    "Type": "CollectionFolder"
                }
            ]
        })
    }

    fn mock_movies_response(&self) -> serde_json::Value {
        json!({
            "Items": [{
                "Id": "movie-1",
                "Name": "Integration Test Movie",
                "Type": "Movie",
                "ProductionYear": 2024,
                "CommunityRating": 8.5,
                "Overview": "A movie for integration testing",
                "RunTimeTicks": 72000000000i64, // 2 hours
                "Genres": ["Action", "Thriller"],
                "UserData": {
                    "PlaybackPositionTicks": 0,
                    "PlayCount": 0,
                    "Played": false
                },
                "DateCreated": "2024-01-01T00:00:00Z"
            }]
        })
    }
}

#[tokio::test]
async fn test_full_jellyfin_integration_flow() {
    // SETUP: Create test fixture
    let mut test = JellyfinIntegrationTest::new().await.unwrap();

    // STEP 1: Test library discovery
    let _mock_libraries = test
        .server
        .mock("GET", "/Users/test_user/Views")
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
        id: Set("lib-1".to_string()),
        source_id: Set("test_jellyfin".to_string()),
        title: Set(libraries[0].title.clone()),
        library_type: Set("movie".to_string()),
        ..Default::default()
    };
    lib.insert(test.db_connection.as_ref()).await.unwrap();

    // STEP 3: Test movie fetching
    let _mock_movies = test
        .server
        .mock("GET", "/Users/test_user/Items")
        .match_query(mockito::Matcher::AllOf(vec![
            mockito::Matcher::UrlEncoded("ParentId".into(), "lib-1".into()),
            mockito::Matcher::UrlEncoded("IncludeItemTypes".into(), "Movie".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(test.mock_movies_response().to_string())
        .create_async()
        .await;

    let library_id = LibraryId::new("lib-1");
    let movies = test.backend.get_movies(&library_id).await.unwrap();
    assert_eq!(movies.len(), 1);
    assert_eq!(movies[0].title, "Integration Test Movie");

    // STEP 4: Save movie to database
    let movie_model =
        MediaItem::Movie(movies[0].clone()).to_model("test_jellyfin", Some("lib-1".to_string()));
    // Use insert_silent to avoid MessageBroker issues in tests
    let inserted_movie = test.media_repo.insert_silent(movie_model).await.unwrap();

    // STEP 5: Verify movie in database
    let saved_movies = test.media_repo.find_by_library("lib-1").await.unwrap();
    assert_eq!(saved_movies.len(), 1);
    assert_eq!(saved_movies[0].title, "Integration Test Movie");

    // STEP 6: Test playback progress tracking
    // Use the actual media item ID from the database
    let media_id = MediaItemId::new(&inserted_movie.id);
    let position = Duration::from_secs(3600); // 1 hour
    let duration = Duration::from_secs(7200); // 2 hours

    // Mock the progress update endpoint
    let _mock_progress = test
        .server
        .mock("POST", "/Sessions/Playing/Progress")
        .match_header(
            "X-Emby-Authorization",
            mockito::Matcher::Regex(r#".*Token="test_token".*"#.to_string()),
        )
        .with_status(204)
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
async fn test_jellyfin_auth_failure_handling() {
    let mut test = JellyfinIntegrationTest::new().await.unwrap();

    // Mock authentication failure
    let _mock = test
        .server
        .mock("GET", "/Users/test_user/Views")
        .with_status(401)
        .with_body("Unauthorized")
        .create_async()
        .await;

    let result = test.backend.get_libraries().await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("401"));
}

#[tokio::test]
async fn test_jellyfin_network_error_handling() {
    let mut test = JellyfinIntegrationTest::new().await.unwrap();

    // Mock network error
    let _mock = test
        .server
        .mock("GET", "/Users/test_user/Views")
        .with_status(503)
        .with_body("Service Unavailable")
        .create_async()
        .await;

    let result = test.backend.get_libraries().await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("503"));
}
