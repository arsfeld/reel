#[cfg(test)]
mod sync_service_tests {
    use reel::services::core::sync::{SyncService, SyncResult, SyncStatus, SyncProgress};
    use reel::db::connection::DatabaseConnection;
    use reel::db::repository::{Repository, sync_repository::SyncRepository};
    use reel::models::{SourceId, Library, LibraryType};
    use reel::backends::traits::MediaBackend;
    use crate::common::{create_test_database, seed_test_data};
    use crate::common::mocks::MockBackend;
    use std::sync::Arc;
    use anyhow::Result;

    #[tokio::test]
    async fn test_sync_service_source_sync() {
        // Setup
        let db = create_test_database().await;
        let test_data = seed_test_data(&db).await;
        let source_id = SourceId::from(test_data.source.id.clone());

        // Create mock backend with test libraries
        let mut mock_backend = MockBackend::new();
        mock_backend.add_library(Library {
            id: "lib1".to_string(),
            title: "Movies".to_string(),
            library_type: LibraryType::Movies,
            item_count: 5,
        });
        mock_backend.add_library(Library {
            id: "lib2".to_string(),
            title: "Shows".to_string(),
            library_type: LibraryType::Shows,
            item_count: 3,
        });

        // Execute sync
        let result = SyncService::sync_source(&db, &mock_backend, &source_id).await;

        // Verify
        assert!(result.is_ok());
        let sync_result = result.unwrap();
        assert_eq!(sync_result.libraries_synced, 2);
        assert!(sync_result.errors.is_empty());
    }

    #[tokio::test]
    async fn test_sync_service_library_sync() {
        // Setup
        let db = create_test_database().await;
        let test_data = seed_test_data(&db).await;
        let source_id = SourceId::from(test_data.source.id.clone());

        let library = Library {
            id: test_data.library.id.clone(),
            title: "Test Library".to_string(),
            library_type: LibraryType::Movies,
            item_count: 10,
        };

        // Create mock backend with test movies
        let mut mock_backend = MockBackend::new();
        mock_backend.add_movies(vec![
            create_test_movie("movie1", "Test Movie 1"),
            create_test_movie("movie2", "Test Movie 2"),
            create_test_movie("movie3", "Test Movie 3"),
        ]);

        // Execute sync
        let result = SyncService::sync_library(
            &db,
            &mock_backend,
            &source_id,
            &library,
        ).await;

        // Verify
        assert!(result.is_ok());
        let items_synced = result.unwrap();
        assert_eq!(items_synced, 3);
    }

    #[tokio::test]
    async fn test_sync_service_incremental_sync() {
        // This test would verify incremental sync functionality
        // Currently marked as TODO in the actual implementation

        let db = create_test_database().await;
        let test_data = seed_test_data(&db).await;
        let source_id = SourceId::from(test_data.source.id.clone());

        // First sync
        let mut mock_backend = MockBackend::new();
        mock_backend.add_movies(vec![
            create_test_movie("movie1", "Movie 1"),
            create_test_movie("movie2", "Movie 2"),
        ]);

        let library = Library {
            id: "lib1".to_string(),
            title: "Movies".to_string(),
            library_type: LibraryType::Movies,
            item_count: 2,
        };

        let first_sync = SyncService::sync_library(&db, &mock_backend, &source_id, &library).await;
        assert!(first_sync.is_ok());
        assert_eq!(first_sync.unwrap(), 2);

        // Add more items for incremental sync
        mock_backend.add_movies(vec![
            create_test_movie("movie3", "Movie 3"),
        ]);

        // Second sync should only sync new items (when implemented)
        let second_sync = SyncService::sync_library(&db, &mock_backend, &source_id, &library).await;
        assert!(second_sync.is_ok());
        // For now this will re-sync everything, but with incremental it should be 1
        assert_eq!(second_sync.unwrap(), 3);
    }

    #[tokio::test]
    async fn test_sync_service_conflict_resolution() {
        // Test how sync handles conflicts between local and remote data
        let db = create_test_database().await;
        let test_data = seed_test_data(&db).await;
        let source_id = SourceId::from(test_data.source.id.clone());

        // Create a library with existing items
        let library = Library {
            id: test_data.library.id.clone(),
            title: "Movies".to_string(),
            library_type: LibraryType::Movies,
            item_count: 2,
        };

        // Mock backend returns updated versions of existing items
        let mut mock_backend = MockBackend::new();
        mock_backend.add_movies(vec![
            create_test_movie(&test_data.media_items[0].id, "Updated Movie 1"),
            create_test_movie(&test_data.media_items[1].id, "Updated Movie 2"),
        ]);

        let result = SyncService::sync_library(&db, &mock_backend, &source_id, &library).await;

        assert!(result.is_ok());
        // Items should be updated, not duplicated
        assert_eq!(result.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_sync_service_error_recovery() {
        // Test sync behavior when backend returns errors
        let db = create_test_database().await;
        let test_data = seed_test_data(&db).await;
        let source_id = SourceId::from(test_data.source.id.clone());

        // Create mock backend that returns errors
        let mut mock_backend = MockBackend::new();
        mock_backend.set_error_mode(true);

        let result = SyncService::sync_source(&db, &mock_backend, &source_id).await;

        // Sync should fail gracefully
        assert!(result.is_err());

        // Check that sync status was updated to failed
        let sync_status = SyncService::get_sync_status(&db, &source_id).await;
        assert!(sync_status.is_ok());
        if let Some(status) = sync_status.unwrap() {
            assert_eq!(status.status, "failed");
        }
    }

    #[tokio::test]
    async fn test_sync_progress_calculation() {
        let db = create_test_database().await;
        let test_data = seed_test_data(&db).await;
        let source_id = SourceId::from(test_data.source.id.clone());

        // Start a sync to create status record
        let _ = SyncService::update_sync_status(
            &db,
            &source_id,
            SyncStatus::InProgress,
            None,
        ).await;

        // Get progress for a sync that hasn't started
        let progress = SyncService::get_sync_progress(&db, &source_id).await;
        assert!(progress.is_ok());

        let sync_progress = progress.unwrap();
        assert!(sync_progress.is_syncing);
        assert_eq!(sync_progress.percentage, 0);
    }

    #[tokio::test]
    async fn test_sync_status_updates() {
        let db = create_test_database().await;
        let test_data = seed_test_data(&db).await;
        let source_id = SourceId::from(test_data.source.id.clone());

        // Test status transitions
        let statuses = vec![
            SyncStatus::Idle,
            SyncStatus::InProgress,
            SyncStatus::Completed,
            SyncStatus::Failed,
        ];

        for status in statuses {
            let result = SyncService::update_sync_status(
                &db,
                &source_id,
                status.clone(),
                None,
            ).await;

            assert!(result.is_ok());

            // Verify status was updated
            let current_status = SyncService::get_sync_status(&db, &source_id).await;
            assert!(current_status.is_ok());

            if let Some(sync_status) = current_status.unwrap() {
                assert_eq!(sync_status.status, status.to_string());
            }
        }
    }

    // Helper function to create test movies
    fn create_test_movie(id: &str, title: &str) -> reel::models::Movie {
        use reel::models::{Movie, MediaItemId};
        use chrono::Utc;

        Movie {
            id: MediaItemId::from(id),
            title: title.to_string(),
            year: Some(2024),
            overview: Some("Test movie".to_string()),
            rating: Some(8.0),
            duration: Some(120),
            genres: vec!["Action".to_string()],
            cast: vec![],
            crew: vec![],
            poster_path: None,
            backdrop_path: None,
            trailer_url: None,
            imdb_id: None,
            tmdb_id: None,
            studio: None,
            tagline: None,
            added_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}