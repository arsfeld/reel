#[cfg(test)]
mod tests {
    use crate::common::{fixtures::Fixtures, mocks::MockBackend, TestApp};
    use gnome_reel::{
        models::*,
        services::{data::DataService, source_coordinator::SourceCoordinator},
    };
    use std::sync::Arc;

    #[tokio::test]
    async fn test_media_service_get_items_pagination() {
        let (app, test_data) = TestApp::with_test_data().await;
        let data_service = Arc::new(DataService::new(app.db.clone()));

        // Test first page
        let page1 = data_service
            .get_media_items_paginated(&test_data.library.id, 0, 2)
            .await
            .unwrap();
        assert_eq!(page1.len(), 2);

        // Test second page
        let page2 = data_service
            .get_media_items_paginated(&test_data.library.id, 2, 2)
            .await
            .unwrap();
        assert_eq!(page2.len(), 2);

        // Test last page
        let page3 = data_service
            .get_media_items_paginated(&test_data.library.id, 4, 2)
            .await
            .unwrap();
        assert_eq!(page3.len(), 1); // Only 5 items total
    }

    #[tokio::test]
    async fn test_media_service_get_item_details() {
        let (app, test_data) = TestApp::with_test_data().await;
        let data_service = Arc::new(DataService::new(app.db.clone()));

        // Get existing item
        let item = data_service
            .get_media_item(&test_data.media_items[0].id)
            .await
            .unwrap();
        assert_eq!(item.id, test_data.media_items[0].id);
        assert_eq!(item.title, test_data.media_items[0].title);

        // Try non-existent item
        let result = data_service.get_media_item("non_existent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_media_service_search_functionality() {
        let (app, test_data) = TestApp::with_test_data().await;
        let data_service = Arc::new(DataService::new(app.db.clone()));

        // Search for specific movie
        let results = data_service.search_media("Movie 1").await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].title.contains("Movie 1"));

        // Search with partial match
        let results = data_service.search_media("Movie").await.unwrap();
        assert_eq!(results.len(), 5); // All test movies

        // Search with no matches
        let results = data_service.search_media("NonExistent").await.unwrap();
        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_media_service_filter_by_type() {
        let app = TestApp::new().await;
        let data_service = Arc::new(DataService::new(app.db.clone()));

        // Add mixed media types
        let source = Fixtures::plex_source();
        data_service.save_source(&source).await.unwrap();

        let movie_lib = LibraryBuilder::new("Movies")
            .with_type(MediaType::Movie)
            .build();
        data_service.save_library(&movie_lib).await.unwrap();

        let show_lib = LibraryBuilder::new("Shows")
            .with_type(MediaType::Show)
            .build();
        data_service.save_library(&show_lib).await.unwrap();

        // Add items
        for movie in Fixtures::movies().iter().take(3) {
            let mut movie = movie.clone();
            movie.library_id = movie_lib.id.clone();
            movie.source_id = source.id.clone();
            data_service.save_media_item(&MediaItem::Movie(movie)).await.unwrap();
        }

        for show in Fixtures::shows().iter().take(2) {
            let mut show = show.clone();
            show.library_id = show_lib.id.clone();
            show.source_id = source.id.clone();
            data_service.save_media_item(&MediaItem::Show(show)).await.unwrap();
        }

        // Test filtering
        let movies = data_service
            .get_media_items_by_type(&movie_lib.id, MediaType::Movie)
            .await
            .unwrap();
        assert_eq!(movies.len(), 3);

        let shows = data_service
            .get_media_items_by_type(&show_lib.id, MediaType::Show)
            .await
            .unwrap();
        assert_eq!(shows.len(), 2);
    }

    #[tokio::test]
    async fn test_media_service_cache_integration() {
        let app = TestApp::new().await;
        let data_service = Arc::new(DataService::new(app.db.clone()));

        // Create test data
        let source = Fixtures::plex_source();
        data_service.save_source(&source).await.unwrap();

        let library = LibraryBuilder::new("Test Library").build();
        data_service.save_library(&library).await.unwrap();

        let movie = MediaItemBuilder::movie("Cache Test")
            .with_id("cache_test")
            .build_movie();
        let mut movie = movie.clone();
        movie.library_id = library.id.clone();
        movie.source_id = source.id.clone();

        // Save item (should cache)
        data_service
            .save_media_item(&MediaItem::Movie(movie.clone()))
            .await
            .unwrap();

        // First get (from cache)
        let item1 = data_service.get_media_item(&movie.id).await.unwrap();
        assert_eq!(item1.title, movie.title);

        // Second get (should still be from cache)
        let item2 = data_service.get_media_item(&movie.id).await.unwrap();
        assert_eq!(item2.title, movie.title);

        // Verify both results are identical
        assert_eq!(item1.id, item2.id);
        assert_eq!(item1.updated_at, item2.updated_at);
    }
}

// Import builders for convenience
use crate::common::builders::{LibraryBuilder, MediaItemBuilder};