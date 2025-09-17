//! Tests for the mapper module

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::db::entities::media_items::Model as MediaItemModel;
    use crate::models::{Episode, MediaItem, Movie, MusicAlbum, MusicTrack, Person, Photo, Show};
    use chrono::Utc;
    use std::time::Duration;

    fn create_test_movie() -> Movie {
        Movie {
            id: "movie-1".to_string(),
            backend_id: "backend-1".to_string(),
            title: "Test Movie".to_string(),
            year: Some(2024),
            duration: Duration::from_secs(7200),
            rating: Some(8.5),
            poster_url: Some("https://example.com/poster.jpg".to_string()),
            backdrop_url: Some("https://example.com/backdrop.jpg".to_string()),
            overview: Some("A test movie description".to_string()),
            genres: vec!["Action".to_string(), "Sci-Fi".to_string()],
            cast: vec![Person {
                name: "Actor 1".to_string(),
                role: Some("Lead".to_string()),
                image_url: None,
            }],
            crew: vec![Person {
                name: "Director 1".to_string(),
                role: Some("Director".to_string()),
                image_url: None,
            }],
            added_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
            watched: false,
            view_count: 0,
            last_watched_at: None,
            playback_position: None,
            intro_marker: None,
            credits_marker: None,
        }
    }

    fn create_test_show() -> Show {
        use crate::models::Season;

        Show {
            id: "show-1".to_string(),
            backend_id: "backend-1".to_string(),
            title: "Test Show".to_string(),
            year: Some(2024),
            rating: Some(9.0),
            poster_url: Some("https://example.com/show-poster.jpg".to_string()),
            backdrop_url: Some("https://example.com/show-backdrop.jpg".to_string()),
            overview: Some("A test show description".to_string()),
            genres: vec!["Drama".to_string(), "Mystery".to_string()],
            seasons: vec![Season {
                id: "season-1".to_string(),
                number: 1,
                episode_count: 10,
                overview: Some("Season 1 overview".to_string()),
            }],
            cast: vec![],
            added_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
            watched_episode_count: 5,
            total_episode_count: 10,
            last_watched_at: None,
        }
    }

    fn create_test_episode() -> Episode {
        Episode {
            id: "episode-1".to_string(),
            backend_id: "backend-1".to_string(),
            show_id: Some("show-1".to_string()),
            title: "Test Episode".to_string(),
            season_number: 1,
            episode_number: 1,
            overview: Some("Episode description".to_string()),
            thumbnail_url: Some("https://example.com/episode-thumb.jpg".to_string()),
            duration: Duration::from_secs(2700),
            air_date: Some(Utc::now()),
            watched: false,
            playback_position: None,
            intro_marker: None,
            credits_marker: None,
        }
    }

    #[test]
    fn test_movie_to_model_conversion() {
        let movie = create_test_movie();
        let media_item = MediaItem::Movie(movie.clone());

        let model = media_item.to_model("source-1", Some("library-1".to_string()));

        assert_eq!(model.id, "movie-1");
        assert_eq!(model.source_id, "source-1");
        assert_eq!(model.library_id, "library-1");
        assert_eq!(model.title, "Test Movie");
        assert_eq!(model.year, Some(2024));
        assert_eq!(model.media_type, "movie");
        assert_eq!(model.duration_ms, Some(7200000));
        assert_eq!(model.rating, Some(8.5));
        assert_eq!(
            model.poster_url,
            Some("https://example.com/poster.jpg".to_string())
        );
        assert_eq!(
            model.backdrop_url,
            Some("https://example.com/backdrop.jpg".to_string())
        );
        assert_eq!(model.overview, Some("A test movie description".to_string()));

        // Check genres
        assert!(model.genres.is_some());
        let genres_value = model.genres.unwrap();
        let genres: Vec<String> = serde_json::from_value(genres_value).unwrap();
        assert_eq!(genres, vec!["Action", "Sci-Fi"]);
    }

    #[test]
    fn test_show_to_model_conversion() {
        let show = create_test_show();
        let media_item = MediaItem::Show(show.clone());

        let model = media_item.to_model("source-2", Some("library-2".to_string()));

        assert_eq!(model.id, "show-1");
        assert_eq!(model.source_id, "source-2");
        assert_eq!(model.library_id, "library-2");
        assert_eq!(model.title, "Test Show");
        assert_eq!(model.year, Some(2024));
        assert_eq!(model.media_type, "show");
        assert_eq!(model.duration_ms, None);
        assert_eq!(model.rating, Some(9.0));

        // Check genres
        assert!(model.genres.is_some());
        let genres_value = model.genres.unwrap();
        let genres: Vec<String> = serde_json::from_value(genres_value).unwrap();
        assert_eq!(genres, vec!["Drama", "Mystery"]);
    }

    #[test]
    fn test_episode_to_model_conversion() {
        let episode = create_test_episode();
        let media_item = MediaItem::Episode(episode.clone());

        let model = media_item.to_model("source-3", None);

        assert_eq!(model.id, "episode-1");
        assert_eq!(model.source_id, "source-3");
        assert_eq!(model.library_id, "");
        assert_eq!(model.title, "Test Episode");
        assert_eq!(model.media_type, "episode");
        assert_eq!(model.duration_ms, Some(2700000));
        assert_eq!(model.parent_id, Some("show-1".to_string()));
        assert_eq!(model.season_number, Some(1));
        assert_eq!(model.episode_number, Some(1));
    }

    #[test]
    fn test_model_to_movie_conversion() {
        use crate::mapper::media_item_mapper::*;

        let mut model = MediaItemModel {
            id: "movie-1".to_string(),
            source_id: "source-1".to_string(),
            library_id: "library-1".to_string(),
            title: "Test Movie".to_string(),
            year: Some(2024),
            media_type: "movie".to_string(),
            duration_ms: Some(7200000),
            rating: Some(8.5),
            poster_url: Some("https://example.com/poster.jpg".to_string()),
            backdrop_url: Some("https://example.com/backdrop.jpg".to_string()),
            overview: Some("A test movie description".to_string()),
            genres: Some(serde_json::to_value(vec!["Action", "Sci-Fi"]).unwrap()),
            parent_id: None,
            season_number: None,
            episode_number: None,
            sort_title: Some("Test Movie".to_string()),
            added_at: Some(chrono::Utc::now().naive_utc()),
            updated_at: chrono::Utc::now().naive_utc(),
            metadata: None,
        };

        // Add metadata for cast and crew
        let metadata = serde_json::json!({
            "cast": [
                {
                    "name": "Actor 1",
                    "role": "Lead",
                    "image_url": null
                }
            ],
            "crew": [
                {
                    "name": "Director 1",
                    "role": "Director",
                    "image_url": null
                }
            ],
            "watched": false,
            "view_count": 0
        });
        model.metadata = Some(metadata);

        // Use the existing TryFrom implementation from db/entities/media_items.rs
        let media_item = MediaItem::try_from(model).unwrap();

        match media_item {
            MediaItem::Movie(movie) => {
                assert_eq!(movie.id, "movie-1");
                assert_eq!(movie.backend_id, "source-1");
                assert_eq!(movie.title, "Test Movie");
                assert_eq!(movie.year, Some(2024));
                assert_eq!(movie.duration.as_millis(), 7200000);
                assert_eq!(movie.rating, Some(8.5));
                assert_eq!(movie.genres, vec!["Action", "Sci-Fi"]);
                assert_eq!(movie.cast.len(), 1);
                assert_eq!(movie.cast[0].name, "Actor 1");
                assert_eq!(movie.crew.len(), 1);
                assert_eq!(movie.crew[0].name, "Director 1");
                assert!(!movie.watched);
                assert_eq!(movie.view_count, 0);
            }
            _ => panic!("Expected Movie variant"),
        }
    }

    #[test]
    fn test_model_to_show_conversion() {
        use crate::mapper::media_item_mapper::*;
        use crate::models::Season;

        let mut model = MediaItemModel {
            id: "show-1".to_string(),
            source_id: "source-1".to_string(),
            library_id: "library-1".to_string(),
            title: "Test Show".to_string(),
            year: Some(2024),
            media_type: "show".to_string(),
            duration_ms: None,
            rating: Some(9.0),
            poster_url: Some("https://example.com/show-poster.jpg".to_string()),
            backdrop_url: Some("https://example.com/show-backdrop.jpg".to_string()),
            overview: Some("A test show description".to_string()),
            genres: Some(serde_json::to_value(vec!["Drama", "Mystery"]).unwrap()),
            parent_id: None,
            season_number: None,
            episode_number: None,
            sort_title: Some("Test Show".to_string()),
            added_at: Some(chrono::Utc::now().naive_utc()),
            updated_at: chrono::Utc::now().naive_utc(),
            metadata: None,
        };

        // Add metadata for seasons and episodes
        let metadata = serde_json::json!({
            "seasons": [
                {
                    "id": "season-1",
                    "number": 1,
                    "episode_count": 10,
                    "overview": "Season 1 overview"
                }
            ],
            "watched_episode_count": 5,
            "total_episode_count": 10
        });
        model.metadata = Some(metadata);

        // Use the existing TryFrom implementation from db/entities/media_items.rs
        let media_item = MediaItem::try_from(model).unwrap();

        match media_item {
            MediaItem::Show(show) => {
                assert_eq!(show.id, "show-1");
                assert_eq!(show.backend_id, "source-1");
                assert_eq!(show.title, "Test Show");
                assert_eq!(show.year, Some(2024));
                assert_eq!(show.rating, Some(9.0));
                assert_eq!(show.genres, vec!["Drama", "Mystery"]);
                assert_eq!(show.seasons.len(), 1);
                assert_eq!(show.seasons[0].number, 1);
                assert_eq!(show.watched_episode_count, 5);
                assert_eq!(show.total_episode_count, 10);
            }
            _ => panic!("Expected Show variant"),
        }
    }

    #[test]
    fn test_model_to_episode_conversion() {
        use crate::mapper::media_item_mapper::*;

        let model = MediaItemModel {
            id: "episode-1".to_string(),
            source_id: "source-1".to_string(),
            library_id: "library-1".to_string(),
            title: "Test Episode".to_string(),
            year: None,
            media_type: "episode".to_string(),
            duration_ms: Some(2700000),
            rating: None,
            poster_url: Some("https://example.com/episode-thumb.jpg".to_string()),
            backdrop_url: None,
            overview: Some("Episode description".to_string()),
            genres: None,
            parent_id: Some("show-1".to_string()),
            season_number: Some(1),
            episode_number: Some(1),
            sort_title: Some("Test Episode".to_string()),
            added_at: Some(chrono::Utc::now().naive_utc()),
            updated_at: chrono::Utc::now().naive_utc(),
            metadata: None,
        };

        // Use the existing TryFrom implementation from db/entities/media_items.rs
        let media_item = MediaItem::try_from(model).unwrap();

        match media_item {
            MediaItem::Episode(episode) => {
                assert_eq!(episode.id, "episode-1");
                assert_eq!(episode.backend_id, "source-1");
                assert_eq!(episode.title, "Test Episode");
                assert_eq!(episode.show_id, Some("show-1".to_string()));
                assert_eq!(episode.season_number, 1);
                assert_eq!(episode.episode_number, 1);
                assert_eq!(episode.duration.as_millis(), 2700000);
            }
            _ => panic!("Expected Episode variant"),
        }
    }

    #[test]
    fn test_unknown_media_type_error() {
        use crate::mapper::media_item_mapper::*;

        let model = MediaItemModel {
            id: "unknown-1".to_string(),
            source_id: "source-1".to_string(),
            library_id: "library-1".to_string(),
            title: "Unknown Media".to_string(),
            year: None,
            media_type: "unknown_type".to_string(),
            duration_ms: None,
            rating: None,
            poster_url: None,
            backdrop_url: None,
            overview: None,
            genres: None,
            parent_id: None,
            season_number: None,
            episode_number: None,
            sort_title: Some("Unknown Media".to_string()),
            added_at: Some(chrono::Utc::now().naive_utc()),
            updated_at: chrono::Utc::now().naive_utc(),
            metadata: None,
        };

        let result = MediaItem::try_from(model);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Unknown media type")
        );
    }

    #[test]
    fn test_round_trip_conversion() {
        let movie = create_test_movie();
        let media_item = MediaItem::Movie(movie.clone());

        // Convert to model
        let model = media_item.to_model("source-1", Some("library-1".to_string()));

        // Convert back to MediaItem
        let converted = MediaItem::try_from(model).unwrap();

        match converted {
            MediaItem::Movie(converted_movie) => {
                assert_eq!(converted_movie.id, movie.id);
                assert_eq!(converted_movie.title, movie.title);
                assert_eq!(converted_movie.year, movie.year);
                assert_eq!(converted_movie.rating, movie.rating);
                assert_eq!(converted_movie.genres, movie.genres);
            }
            _ => panic!("Expected Movie variant"),
        }
    }
}
