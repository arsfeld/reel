#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::models::{LibraryId, LibraryType, MediaItemId};
    use mockito::Server;
    use serde_json::json;
    use std::time::Duration;

    async fn create_test_backend(server: &Server) -> JellyfinBackend {
        let backend = JellyfinBackend::with_id("test_jellyfin".to_string());
        backend.set_base_url(server.url()).await;

        // Set up the backend with test credentials
        *backend.api_key.write().await = Some("test_token".to_string());
        *backend.user_id.write().await = Some("test_user_id".to_string());

        // Create and set the API
        let api = JellyfinApi::with_backend_id(
            server.url(),
            "test_token".to_string(),
            "test_user_id".to_string(),
            "test_jellyfin".to_string(),
        );
        *backend.api.write().await = Some(api);

        backend
    }

    fn create_auth_response() -> serde_json::Value {
        json!({
            "User": {
                "Id": "test_user_id",
                "Name": "TestUser",
                "ServerId": "test_server_id",
                "PrimaryImageTag": "abc123"
            },
            "AccessToken": "test_access_token",
            "ServerId": "test_server_id"
        })
    }

    fn create_libraries_response() -> serde_json::Value {
        json!({
            "Items": [
                {
                    "Id": "library-1",
                    "Name": "Movies",
                    "CollectionType": "movies",
                    "Type": "CollectionFolder",
                    "ImageTags": {
                        "Primary": "movie-thumb"
                    }
                },
                {
                    "Id": "library-2",
                    "Name": "TV Shows",
                    "CollectionType": "tvshows",
                    "Type": "CollectionFolder",
                    "ImageTags": {
                        "Primary": "tv-thumb"
                    }
                }
            ]
        })
    }

    fn create_movies_response() -> serde_json::Value {
        json!({
            "Items": [
                {
                    "Id": "movie-1",
                    "Name": "Test Movie",
                    "Type": "Movie",
                    "ProductionYear": 2024,
                    "CommunityRating": 8.5,
                    "OfficialRating": "PG-13",
                    "Overview": "A test movie for unit testing",
                    "RunTimeTicks": 72000000000i64,  // 2 hours in ticks (100-nanosecond intervals)
                    "DateCreated": "2024-01-01T00:00:00Z",
                    "PremiereDate": "2024-01-01T00:00:00Z",
                    "UserData": {
                        "PlaybackPositionTicks": 0,
                        "PlayCount": 2,
                        "IsFavorite": false,
                        "Played": true
                    },
                    "ImageTags": {
                        "Primary": "movie-thumb-1",
                        "Backdrop": "movie-backdrop-1"
                    },
                    "BackdropImageTags": ["backdrop-1"],
                    "Genres": ["Action", "Adventure"],
                    "Studios": [{"Name": "Test Studios"}],
                    "People": [
                        {
                            "Name": "John Director",
                            "Type": "Director"
                        },
                        {
                            "Name": "Jane Writer",
                            "Type": "Writer"
                        },
                        {
                            "Name": "Actor One",
                            "Type": "Actor"
                        },
                        {
                            "Name": "Actor Two",
                            "Type": "Actor"
                        }
                    ]
                }
            ]
        })
    }

    fn create_shows_response() -> serde_json::Value {
        json!({
            "Items": [
                {
                    "Id": "show-1",
                    "Name": "Test Show",
                    "Type": "Series",
                    "ProductionYear": 2024,
                    "CommunityRating": 9.0,
                    "OfficialRating": "TV-14",
                    "Overview": "A test show for unit testing",
                    "RunTimeTicks": 18000000000i64,  // 30 minutes in ticks
                    "DateCreated": "2024-01-01T00:00:00Z",
                    "PremiereDate": "2024-01-01T00:00:00Z",
                    "UserData": {
                        "UnplayedItemCount": 5,
                        "PlayedPercentage": 50.0,
                        "PlayCount": 10,
                        "IsFavorite": false
                    },
                    "ImageTags": {
                        "Primary": "show-thumb-1",
                        "Banner": "show-banner-1",
                        "Backdrop": "show-backdrop-1"
                    },
                    "BackdropImageTags": ["backdrop-1"],
                    "Genres": ["Drama", "Sci-Fi"],
                    "Studios": [{"Name": "Test Network"}],
                    "ChildCount": 1
                }
            ]
        })
    }

    fn create_seasons_response() -> serde_json::Value {
        json!({
            "Items": [
                {
                    "Id": "season-1",
                    "Name": "Season 1",
                    "Type": "Season",
                    "SeriesId": "show-1",
                    "SeriesName": "Test Show",
                    "IndexNumber": 1,
                    "ChildCount": 10,
                    "UserData": {
                        "UnplayedItemCount": 5,
                        "PlayedPercentage": 50.0
                    }
                }
            ]
        })
    }

    fn create_playback_info_response() -> serde_json::Value {
        json!({
            "MediaSources": [
                {
                    "Id": "media-source-1",
                    "Container": "mp4",
                    "Protocol": "Http",
                    "MediaStreams": [
                        {
                            "Type": "Video",
                            "Codec": "h264",
                            "Height": 1080,
                            "Width": 1920,
                            "BitRate": 5000000
                        },
                        {
                            "Type": "Audio",
                            "Codec": "aac",
                            "Channels": 2,
                            "SampleRate": 48000
                        }
                    ],
                    "Path": "/Videos/movie-1/stream.mp4",
                    "DirectStreamUrl": "/Videos/movie-1/stream?api_key=test_token",
                    "Size": 1073741824,
                    "Bitrate": 5000000,
                    "RunTimeTicks": 72000000000i64
                }
            ]
        })
    }

    fn create_server_info_response() -> serde_json::Value {
        json!({
            "LocalAddress": "http://localhost:8096",
            "ServerName": "Test Jellyfin Server",
            "Version": "10.8.0",
            "ProductName": "Jellyfin Server",
            "Id": "test_server_id"
        })
    }

    #[tokio::test]
    async fn test_username_password_authentication() {
        let mut server = Server::new_async().await;

        let _m = server
            .mock("POST", "/Users/AuthenticateByName")
            .match_header("Content-Type", "application/json")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(create_auth_response().to_string())
            .create_async()
            .await;

        let auth_result = JellyfinApi::authenticate(&server.url(), "testuser", "testpass")
            .await
            .unwrap();

        assert_eq!(auth_result.user.name, "TestUser");
        assert_eq!(auth_result.user.id, "test_user_id");
        assert_eq!(auth_result.access_token, "test_access_token");
    }

    #[tokio::test]
    async fn test_library_enumeration() {
        let mut server = Server::new_async().await;
        let backend = create_test_backend(&server).await;

        let _m = server
            .mock("GET", "/Users/test_user_id/Views")
            .match_header(
                "X-Emby-Authorization",
                mockito::Matcher::Regex(r#".*Token="test_token".*"#.to_string()),
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(create_libraries_response().to_string())
            .create_async()
            .await;

        let libraries = backend.get_libraries().await.unwrap();

        assert_eq!(libraries.len(), 2);
        assert_eq!(libraries[0].title, "Movies");
        assert_eq!(libraries[0].library_type, LibraryType::Movies);
        assert_eq!(libraries[1].title, "TV Shows");
        assert_eq!(libraries[1].library_type, LibraryType::Shows);
    }

    #[tokio::test]
    async fn test_media_item_retrieval_movies() {
        let mut server = Server::new_async().await;
        let backend = create_test_backend(&server).await;

        let _m = server
            .mock("GET", "/Users/test_user_id/Items")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("ParentId".into(), "library-1".into()),
                mockito::Matcher::UrlEncoded("IncludeItemTypes".into(), "Movie".into()),
                mockito::Matcher::UrlEncoded(
                    "Fields".into(),
                    "Overview,Genres,DateCreated,MediaStreams,People,ProviderIds,RunTimeTicks"
                        .into(),
                ),
                mockito::Matcher::UrlEncoded("SortBy".into(), "SortName".into()),
            ]))
            .match_header(
                "X-Emby-Authorization",
                mockito::Matcher::Regex(r#".*Token="test_token".*"#.to_string()),
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(create_movies_response().to_string())
            .create_async()
            .await;

        let library_id = LibraryId::new("library-1");
        let movies = backend.get_movies(&library_id).await.unwrap();

        assert_eq!(movies.len(), 1);
        let movie = &movies[0];
        assert_eq!(movie.title, "Test Movie");
        assert_eq!(movie.year, Some(2024));
        assert_eq!(movie.rating, Some(8.5));
        assert_eq!(movie.duration, Duration::from_secs(7200));
        assert!(movie.watched);
        assert_eq!(movie.view_count, 2);
        assert_eq!(movie.genres.len(), 2);
    }

    #[tokio::test]
    async fn test_media_item_retrieval_shows() {
        let mut server = Server::new_async().await;
        let backend = create_test_backend(&server).await;

        // Mock shows response
        let _m1 = server
            .mock("GET", "/Users/test_user_id/Items")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("ParentId".into(), "library-2".into()),
                mockito::Matcher::UrlEncoded("IncludeItemTypes".into(), "Series".into()),
                mockito::Matcher::UrlEncoded(
                    "Fields".into(),
                    "Overview,Genres,DateCreated,ChildCount,People".into(),
                ),
                mockito::Matcher::UrlEncoded("SortBy".into(), "SortName".into()),
            ]))
            .match_header(
                "X-Emby-Authorization",
                mockito::Matcher::Regex(r#".*Token="test_token".*"#.to_string()),
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(create_shows_response().to_string())
            .create_async()
            .await;

        // Mock seasons response
        let _m2 = server
            .mock("GET", "/Shows/show-1/Seasons")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("userId".into(), "test_user_id".into()),
                mockito::Matcher::UrlEncoded("Fields".into(), "ItemCounts".into()),
            ]))
            .match_header(
                "X-Emby-Authorization",
                mockito::Matcher::Regex(r#".*Token="test_token".*"#.to_string()),
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(create_seasons_response().to_string())
            .create_async()
            .await;

        let library_id = LibraryId::new("library-2");
        let shows = backend.get_shows(&library_id).await.unwrap();

        assert_eq!(shows.len(), 1);
        let show = &shows[0];
        assert_eq!(show.title, "Test Show");
        assert_eq!(show.year, Some(2024));
        assert_eq!(show.rating, Some(9.0));
        assert_eq!(show.total_episode_count, 10);
        assert_eq!(show.watched_episode_count, 5);
    }

    #[tokio::test]
    async fn test_streaming_url_generation() {
        let mut server = Server::new_async().await;
        let backend = create_test_backend(&server).await;

        let _m1 = server
            .mock("POST", "/Items/movie-1/PlaybackInfo")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("UserId".into(), "test_user_id".into()),
                mockito::Matcher::UrlEncoded("StartTimeTicks".into(), "0".into()),
                mockito::Matcher::UrlEncoded("IsPlayback".into(), "true".into()),
                mockito::Matcher::UrlEncoded("AutoOpenLiveStream".into(), "true".into()),
                mockito::Matcher::UrlEncoded("MediaSourceId".into(), "movie-1".into()),
            ]))
            .match_header(
                "X-Emby-Authorization",
                mockito::Matcher::Regex(r#".*Token="test_token".*"#.to_string()),
            )
            .match_header("Content-Type", "application/json")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(create_playback_info_response().to_string())
            .create_async()
            .await;

        let _m2 = server
            .mock("GET", "/System/Info/Public")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(create_server_info_response().to_string())
            .create_async()
            .await;

        // Mock the playback start report that happens after getting stream URL
        let _m3 = server
            .mock("POST", "/Sessions/Playing")
            .match_header(
                "X-Emby-Authorization",
                mockito::Matcher::Regex(r#".*Token="test_token".*"#.to_string()),
            )
            .with_status(204) // No content response
            .create_async()
            .await;

        let media_id = MediaItemId::new("movie-1");
        let stream_info = backend.get_stream_url(&media_id).await.unwrap();

        assert!(stream_info.url.contains("/Videos/movie-1/stream"));
        assert!(stream_info.url.contains("api_key=test_token"));
        // Jellyfin provides transcoding options
        assert!(!stream_info.quality_options.is_empty());
    }

    #[tokio::test]
    async fn test_playback_progress_reporting() {
        let mut server = Server::new_async().await;
        let backend = create_test_backend(&server).await;

        let _m = server
            .mock("POST", "/Sessions/Playing/Progress")
            .match_header(
                "X-Emby-Authorization",
                mockito::Matcher::Regex(r#".*Token="test_token".*"#.to_string()),
            )
            .with_status(204) // No content response
            .create_async()
            .await;

        let media_id = MediaItemId::new("movie-1");
        let position = Duration::from_secs(300); // 5 minutes
        let duration = Duration::from_secs(7200); // 2 hours

        backend
            .update_progress(&media_id, position, duration)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_connection_retry_logic() {
        let mut server = Server::new_async().await;
        let backend = create_test_backend(&server).await;

        // First request fails with network error (503)
        let _m1 = server
            .mock("GET", "/Users/test_user_id/Views")
            .with_status(503)
            .with_body("Service Unavailable")
            .expect(1)
            .create_async()
            .await;

        // Verify error handling
        let result = backend.get_libraries().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("503"));
    }

    #[tokio::test]
    async fn test_api_version_compatibility() {
        let mut server = Server::new_async().await;

        // Test that we can get server info to check version
        let _m = server
            .mock("GET", "/System/Info/Public")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(create_server_info_response().to_string())
            .create_async()
            .await;

        let api = JellyfinApi::with_backend_id(
            server.url(),
            "test_token".to_string(),
            "test_user_id".to_string(),
            "test".to_string(),
        );

        let server_info = api.get_server_info().await.unwrap();
        assert_eq!(server_info.server_name, "Test Jellyfin Server");
    }

    #[tokio::test]
    async fn test_error_handling_invalid_credentials() {
        let mut server = Server::new_async().await;

        let _m = server
            .mock("POST", "/Users/AuthenticateByName")
            .with_status(401)
            .with_body(
                json!({
                    "error": "Invalid username or password"
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = JellyfinApi::authenticate(&server.url(), "wronguser", "wrongpass").await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("401"));
    }

    #[tokio::test]
    async fn test_error_handling_server_error() {
        let mut server = Server::new_async().await;
        let backend = create_test_backend(&server).await;

        let _m = server
            .mock("GET", "/Users/test_user_id/Items")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("ParentId".into(), "library-1".into()),
                mockito::Matcher::UrlEncoded("IncludeItemTypes".into(), "Movie".into()),
                mockito::Matcher::UrlEncoded(
                    "Fields".into(),
                    "Overview,Genres,DateCreated,MediaStreams,People,ProviderIds,RunTimeTicks"
                        .into(),
                ),
                mockito::Matcher::UrlEncoded("SortBy".into(), "SortName".into()),
            ]))
            .with_status(500)
            .with_body("Internal Server Error")
            .create_async()
            .await;

        let library_id = LibraryId::new("library-1");
        let result = backend.get_movies(&library_id).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("500"));
    }

    #[tokio::test]
    async fn test_empty_library_handling() {
        let mut server = Server::new_async().await;
        let backend = create_test_backend(&server).await;

        let empty_response = json!({
            "Items": []
        });

        let _m = server
            .mock("GET", "/Users/test_user_id/Views")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(empty_response.to_string())
            .create_async()
            .await;

        let libraries = backend.get_libraries().await.unwrap();
        assert_eq!(libraries.len(), 0);
    }

    #[tokio::test]
    async fn test_malformed_response_handling() {
        let mut server = Server::new_async().await;
        let backend = create_test_backend(&server).await;

        let _m = server
            .mock("GET", "/Users/test_user_id/Views")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("{ invalid json")
            .create_async()
            .await;

        let result = backend.get_libraries().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_connection_timeout() {
        let mut server = Server::new_async().await;
        let backend = create_test_backend(&server).await;

        let _m = server
            .mock("GET", "/Users/test_user_id/Views")
            .with_status(200)
            .with_chunked_body(|w| {
                std::thread::sleep(std::time::Duration::from_secs(35));
                w.write_all(b"timeout")
            })
            .expect(0)
            .create_async()
            .await;

        let result = backend.get_libraries().await;
        assert!(result.is_err());
    }
}
