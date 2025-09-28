#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::models::{LibraryId, LibraryType, MediaItemId};
    use mockito::Server;
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use std::time::Duration;

    async fn create_test_backend(server: &Server) -> PlexBackend {
        let backend = PlexBackend::new_for_auth(server.url(), "test_token".to_string());

        {
            let mut api = backend.api.write().await;
            *api = Some(PlexApi::with_backend_id(
                server.url(),
                "test_token".to_string(),
                "test_backend".to_string(),
            ));
        }

        backend
    }

    fn create_libraries_response() -> serde_json::Value {
        json!({
            "MediaContainer": {
                "size": 2,
                "allowSync": true,
                "Directory": [
                    {
                        "allowSync": true,
                        "art": "/:/resources/movie-fanart.jpg",
                        "composite": "/library/sections/1/composite/1234",
                        "filters": true,
                        "refreshing": false,
                        "thumb": "/:/resources/movie.png",
                        "key": "1",
                        "type": "movie",
                        "title": "Movies",
                        "agent": "tv.plex.agents.movie",
                        "scanner": "Plex Movie",
                        "language": "en",
                        "uuid": "abc-123",
                        "updatedAt": 1234567890,
                        "createdAt": 1234567890
                    },
                    {
                        "allowSync": true,
                        "art": "/:/resources/show-fanart.jpg",
                        "composite": "/library/sections/2/composite/1234",
                        "filters": true,
                        "refreshing": false,
                        "thumb": "/:/resources/show.png",
                        "key": "2",
                        "type": "show",
                        "title": "TV Shows",
                        "agent": "tv.plex.agents.series",
                        "scanner": "Plex TV Series",
                        "language": "en",
                        "uuid": "def-456",
                        "updatedAt": 1234567890,
                        "createdAt": 1234567890
                    }
                ]
            }
        })
    }

    fn create_movies_response() -> serde_json::Value {
        json!({
            "MediaContainer": {
                "size": 1,
                "Metadata": [
                    {
                        "ratingKey": "movie-1",
                        "key": "/library/metadata/movie-1",
                        "guid": "plex://movie/1234",
                        "studio": "Test Studios",
                        "type": "movie",
                        "title": "Test Movie",
                        "contentRating": "PG-13",
                        "summary": "A test movie for unit testing",
                        "rating": 8.5,
                        "audienceRating": 8.0,
                        "viewCount": 2,
                        "lastViewedAt": 1234567890,
                        "year": 2024,
                        "tagline": "Testing is fun",
                        "thumb": "/library/metadata/movie-1/thumb/1234",
                        "art": "/library/metadata/movie-1/art/1234",
                        "duration": 7200000,
                        "originallyAvailableAt": "2024-01-01",
                        "addedAt": 1234567890,
                        "updatedAt": 1234567890,
                        "audienceRatingImage": "rottentomatoes://image.rating.upright",
                        "chapterSource": "media",
                        "primaryExtraKey": "/library/metadata/5678",
                        "ratingImage": "rottentomatoes://image.rating.ripe",
                        "Genre": [
                            {"tag": "Action"},
                            {"tag": "Adventure"}
                        ],
                        "Director": [
                            {"tag": "John Director"}
                        ],
                        "Writer": [
                            {"tag": "Jane Writer"}
                        ],
                        "Country": [
                            {"tag": "United States"}
                        ],
                        "Role": [
                            {"tag": "Actor One"},
                            {"tag": "Actor Two"}
                        ]
                    }
                ]
            }
        })
    }

    fn create_shows_response() -> serde_json::Value {
        json!({
            "MediaContainer": {
                "size": 1,
                "Metadata": [
                    {
                        "ratingKey": "show-1",
                        "key": "/library/metadata/show-1/children",
                        "guid": "plex://show/1234",
                        "studio": "Test Network",
                        "type": "show",
                        "title": "Test Show",
                        "contentRating": "TV-14",
                        "summary": "A test show for unit testing",
                        "index": 1,
                        "rating": 9.0,
                        "viewCount": 10,
                        "lastViewedAt": 1234567890,
                        "year": 2024,
                        "thumb": "/library/metadata/show-1/thumb/1234",
                        "art": "/library/metadata/show-1/art/1234",
                        "banner": "/library/metadata/show-1/banner/1234",
                        "duration": 1800000,
                        "originallyAvailableAt": "2024-01-01",
                        "leafCount": 10,
                        "viewedLeafCount": 5,
                        "childCount": 1,
                        "addedAt": 1234567890,
                        "updatedAt": 1234567890,
                        "Genre": [
                            {"tag": "Drama"},
                            {"tag": "Sci-Fi"}
                        ]
                    }
                ]
            }
        })
    }

    fn create_identity_response() -> serde_json::Value {
        json!({
            "MediaContainer": {
                "machineIdentifier": "test_machine_id",
                "version": "1.32.8.1234-abcdef"
            }
        })
    }

    fn create_stream_response() -> String {
        r#"{
            "MediaContainer": {
                "size": 1,
                "Metadata": [{
                    "ratingKey": "movie-1",
                    "title": "Test Movie",
                    "Media": [{
                        "id": "media-1",
                        "duration": 7200000,
                        "bitrate": 5000,
                        "width": 1920,
                        "height": 1080,
                        "audioCodec": "aac",
                        "videoCodec": "h264",
                        "Part": [{
                            "id": "part-1",
                            "key": "/library/parts/1/file.mp4",
                            "container": "mp4"
                        }]
                    }]
                }]
            }
        }"#
        .to_string()
    }

    #[tokio::test]
    async fn test_oauth_authentication_flow() {
        // Note: This test would actually make a real HTTP request to plex.tv
        // For true unit testing, we'd need to mock the PLEX_TV_URL constant
        // or refactor PlexAuth to accept a base URL parameter

        // For now, test the PIN structure parsing
        let pin_response = PlexPinResponse {
            id: 1234,
            code: "TEST-CODE".to_string(),
            auth_token: None,
        };

        let pin = PlexPin {
            id: pin_response.id.to_string(),
            code: pin_response.code.clone(),
        };

        assert_eq!(pin.id, "1234");
        assert_eq!(pin.code, "TEST-CODE");
    }

    // Internal struct for testing
    #[derive(Debug, Serialize, Deserialize)]
    struct PlexPinResponse {
        id: i32,
        code: String,
        #[serde(rename = "authToken")]
        auth_token: Option<String>,
    }

    #[tokio::test]
    async fn test_library_retrieval() {
        let mut server = Server::new_async().await;
        let backend = create_test_backend(&server).await;

        let _m = server
            .mock("GET", "/library/sections")
            .match_header("X-Plex-Token", "test_token")
            .match_header("Accept", "application/json")
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
    async fn test_movie_fetching() {
        let mut server = Server::new_async().await;
        let backend = create_test_backend(&server).await;

        let _m = server
            .mock("GET", "/library/sections/1/all")
            .match_header("X-Plex-Token", "test_token")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(create_movies_response().to_string())
            .create_async()
            .await;

        let library_id = LibraryId::new("1");
        let movies = backend.get_movies(&library_id).await.unwrap();

        assert_eq!(movies.len(), 1);
        let movie = &movies[0];
        assert_eq!(movie.title, "Test Movie");
        assert_eq!(movie.year, Some(2024));
        assert_eq!(movie.rating, Some(8.5));
        assert_eq!(movie.duration, Duration::from_millis(7200000));
        assert!(movie.watched);
        assert_eq!(movie.view_count, 2);
        assert_eq!(movie.genres.len(), 2);
    }

    #[tokio::test]
    async fn test_show_fetching() {
        let mut server = Server::new_async().await;
        let backend = create_test_backend(&server).await;

        // Mock the shows response
        let _m1 = server
            .mock("GET", "/library/sections/2/all")
            .match_header("X-Plex-Token", "test_token")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(create_shows_response().to_string())
            .create_async()
            .await;

        // Mock the seasons response for the show
        let seasons_response = json!({
            "MediaContainer": {
                "size": 1,
                "Metadata": [
                    {
                        "ratingKey": "season-1",
                        "parentRatingKey": "show-1",
                        "key": "/library/metadata/season-1/children",
                        "title": "Season 1",
                        "index": 1,
                        "leafCount": 10,
                        "viewedLeafCount": 5
                    }
                ]
            }
        });

        let _m2 = server
            .mock("GET", "/library/metadata/show-1/children")
            .match_header("X-Plex-Token", "test_token")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(seasons_response.to_string())
            .create_async()
            .await;

        let library_id = LibraryId::new("2");
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
    async fn test_stream_url_generation() {
        let mut server = Server::new_async().await;
        let backend = create_test_backend(&server).await;

        let _m1 = server
            .mock("GET", "/library/metadata/movie-1")
            .match_header("X-Plex-Token", "test_token")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(create_stream_response())
            .create_async()
            .await;

        let _m2 = server
            .mock("GET", "/identity")
            .match_header("X-Plex-Token", "test_token")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(create_identity_response().to_string())
            .create_async()
            .await;

        let media_id = MediaItemId::new("movie-1");
        let stream_info = backend.get_stream_url(&media_id).await.unwrap();

        assert!(stream_info.url.contains("/library/parts/1/file.mp4"));
        assert!(stream_info.url.contains("X-Plex-Token=test_token"));
        // Should have original + 720p + 480p + 360p (1080p is the original, so not duplicated)
        assert_eq!(stream_info.quality_options.len(), 4);
        // Verify quality options are correct
        assert_eq!(stream_info.quality_options[0].name, "Original (1080p)");
        assert!(!stream_info.quality_options[0].requires_transcode);
        assert_eq!(stream_info.quality_options[1].name, "720p");
        assert!(stream_info.quality_options[1].requires_transcode);
    }

    #[tokio::test]
    async fn test_progress_update() {
        let mut server = Server::new_async().await;
        let backend = create_test_backend(&server).await;

        let _m = server
            .mock("GET", "/:/timeline")
            .match_header("X-Plex-Token", "test_token")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded(
                    "identifier".into(),
                    "com.plexapp.plugins.library".into(),
                ),
                mockito::Matcher::UrlEncoded("key".into(), "/library/metadata/movie-1".into()),
                mockito::Matcher::UrlEncoded("ratingKey".into(), "movie-1".into()),
                mockito::Matcher::UrlEncoded("state".into(), "playing".into()),
                mockito::Matcher::UrlEncoded("time".into(), "5000".into()),
                mockito::Matcher::UrlEncoded("duration".into(), "7200000".into()),
                mockito::Matcher::UrlEncoded("playbackTime".into(), "5000".into()),
            ]))
            .with_status(200)
            .create_async()
            .await;

        let media_id = MediaItemId::new("movie-1");
        let position = Duration::from_secs(5);
        let duration = Duration::from_secs(7200); // 2 hours
        backend
            .update_progress(&media_id, position, duration)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_error_handling_invalid_credentials() {
        let mut server = Server::new_async().await;
        let backend = create_test_backend(&server).await;

        let _m = server
            .mock("GET", "/library/sections")
            .with_status(401)
            .with_body("Unauthorized")
            .create_async()
            .await;

        let result = backend.get_libraries().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("401"));
    }

    #[tokio::test]
    async fn test_error_handling_server_error() {
        let mut server = Server::new_async().await;
        let backend = create_test_backend(&server).await;

        let _m = server
            .mock("GET", "/library/sections/1/all")
            .with_status(500)
            .with_body("Internal Server Error")
            .create_async()
            .await;

        let library_id = LibraryId::new("1");
        let result = backend.get_movies(&library_id).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("500"));
    }

    #[tokio::test]
    async fn test_rate_limiting_response() {
        // Note: Currently the Plex backend doesn't implement retry logic for rate limiting
        // This test verifies that a 429 response is handled as an error
        let mut server = Server::new_async().await;
        let backend = create_test_backend(&server).await;

        let _m = server
            .mock("GET", "/library/sections")
            .with_status(429)
            .with_header("Retry-After", "1")
            .with_body("Rate limit exceeded")
            .create_async()
            .await;

        let result = backend.get_libraries().await;

        // Currently, rate limiting returns an error
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("429"));
    }

    #[tokio::test]
    async fn test_empty_library_handling() {
        let mut server = Server::new_async().await;
        let backend = create_test_backend(&server).await;

        let empty_response = json!({
            "MediaContainer": {
                "size": 0,
                "Directory": []
            }
        });

        let _m = server
            .mock("GET", "/library/sections")
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
            .mock("GET", "/library/sections")
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
            .mock("GET", "/library/sections")
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

    #[tokio::test]
    async fn test_get_home_users() {
        let mut server = Server::new_async().await;

        let home_users_response = json!([
            {
                "id": "user1",
                "title": "Main User",
                "protected": false,
                "admin": true,
                "thumb": "https://plex.tv/users/abc/avatar"
            },
            {
                "id": "user2",
                "title": "Kids Profile",
                "protected": true,
                "admin": false,
                "thumb": null
            }
        ]);

        let _m = server
            .mock("GET", "/api/v2/home/users")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(home_users_response.to_string())
            .create_async()
            .await;

        let result = PlexAuth::get_home_users_with_url("test_token", &server.url()).await;

        assert!(result.is_ok());
        let users = result.unwrap();
        assert_eq!(users.len(), 2);

        assert_eq!(users[0].id, "user1");
        assert_eq!(users[0].name, "Main User");
        assert_eq!(users[0].is_protected, false);
        assert_eq!(users[0].is_admin, true);
        assert!(users[0].thumb.is_some());

        assert_eq!(users[1].id, "user2");
        assert_eq!(users[1].name, "Kids Profile");
        assert_eq!(users[1].is_protected, true);
        assert_eq!(users[1].is_admin, false);
        assert!(users[1].thumb.is_none());
    }

    #[tokio::test]
    async fn test_get_home_users_unauthorized() {
        let mut server = Server::new_async().await;

        let _m = server
            .mock("GET", "/api/v2/home/users")
            .with_status(401)
            .with_body("Unauthorized")
            .create_async()
            .await;

        let result = PlexAuth::get_home_users_with_url("invalid_token", &server.url()).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Authentication failed")
        );
    }

    #[tokio::test]
    async fn test_switch_to_user_success() {
        let mut server = Server::new_async().await;

        let switch_response = json!({
            "authToken": "new_user_token_12345"
        });

        let _m = server
            .mock("POST", "/api/home/users/user2/switch")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(switch_response.to_string())
            .create_async()
            .await;

        let result =
            PlexAuth::switch_to_user_with_url("main_token", "user2", None, &server.url()).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "new_user_token_12345");
    }

    #[tokio::test]
    async fn test_switch_to_user_with_pin() {
        let mut server = Server::new_async().await;

        let switch_response = json!({
            "authToken": "protected_user_token"
        });

        let _m = server
            .mock("POST", "/api/home/users/user3/switch")
            .match_body(mockito::Matcher::Json(json!({"pin": "1234"})))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(switch_response.to_string())
            .create_async()
            .await;

        let result =
            PlexAuth::switch_to_user_with_url("main_token", "user3", Some("1234"), &server.url())
                .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "protected_user_token");
    }

    #[tokio::test]
    async fn test_switch_to_user_invalid_pin() {
        let mut server = Server::new_async().await;

        let _m = server
            .mock("POST", "/api/home/users/user3/switch")
            .with_status(401)
            .with_body("Invalid PIN")
            .create_async()
            .await;

        let result = PlexAuth::switch_to_user_with_url(
            "main_token",
            "user3",
            Some("wrong_pin"),
            &server.url(),
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid PIN"));
    }

    #[tokio::test]
    async fn test_switch_to_user_pin_required() {
        let mut server = Server::new_async().await;

        let _m = server
            .mock("POST", "/api/home/users/user3/switch")
            .with_status(403)
            .with_body("PIN required")
            .create_async()
            .await;

        let result =
            PlexAuth::switch_to_user_with_url("main_token", "user3", None, &server.url()).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("PIN required"));
    }
}
