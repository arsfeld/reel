//! Test fixtures for integration tests
//!
//! Provides sample movies, shows, episodes, and libraries for testing

#![allow(dead_code)]

use reel::models::*;
use std::time::Duration;

/// Sample movie for testing
pub fn sample_movie_1() -> Movie {
    Movie {
        id: "test-movie-1".to_string(),
        backend_id: "test-backend".to_string(),
        title: "The Test Movie".to_string(),
        year: Some(2024),
        duration: Duration::from_secs(7200), // 2 hours
        rating: Some(8.5),
        poster_url: Some("/library/metadata/1/thumb".to_string()),
        backdrop_url: Some("/library/metadata/1/art".to_string()),
        overview: Some("A thrilling test movie about integration testing".to_string()),
        genres: vec!["Action".to_string(), "Adventure".to_string()],
        cast: vec![
            Person {
                id: "actor1".to_string(),
                name: "Test Actor".to_string(),
                role: Some("Lead".to_string()),
                image_url: None,
            },
            Person {
                id: "actor2".to_string(),
                name: "Sample Actress".to_string(),
                role: Some("Supporting".to_string()),
                image_url: None,
            },
        ],
        crew: vec![Person {
            id: "director1".to_string(),
            name: "Test Director".to_string(),
            role: Some("Director".to_string()),
            image_url: None,
        }],
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

/// Sample movie 2 with different properties
pub fn sample_movie_2() -> Movie {
    Movie {
        id: "test-movie-2".to_string(),
        backend_id: "test-backend".to_string(),
        title: "Integration Test: The Movie".to_string(),
        year: Some(2023),
        duration: Duration::from_secs(5400), // 1.5 hours
        rating: Some(7.8),
        poster_url: Some("/library/metadata/2/thumb".to_string()),
        backdrop_url: None,
        overview: Some("A comedic look at automated testing".to_string()),
        genres: vec!["Comedy".to_string(), "Documentary".to_string()],
        cast: vec![Person {
            id: "actor3".to_string(),
            name: "Mock Actor".to_string(),
            role: Some("Lead".to_string()),
            image_url: None,
        }],
        crew: vec![Person {
            id: "director2".to_string(),
            name: "Fixture Director".to_string(),
            role: Some("Director".to_string()),
            image_url: None,
        }],
        added_at: None,
        updated_at: None,
        watched: true,
        view_count: 3,
        last_watched_at: None,
        playback_position: Some(Duration::from_secs(3600)), // 1 hour in
        intro_marker: None,
        credits_marker: None,
    }
}

/// Sample TV show for testing
pub fn sample_show_1() -> Show {
    Show {
        id: "test-show-1".to_string(),
        backend_id: "test-backend".to_string(),
        title: "Test Show: The Series".to_string(),
        year: Some(2024),
        rating: Some(9.2),
        poster_url: Some("/library/metadata/3/thumb".to_string()),
        backdrop_url: Some("/library/metadata/3/art".to_string()),
        overview: Some("An episodic journey through test scenarios".to_string()),
        genres: vec!["Drama".to_string(), "Sci-Fi".to_string()],
        cast: vec![Person {
            id: "actor4".to_string(),
            name: "Lead Tester".to_string(),
            role: Some("Lead".to_string()),
            image_url: None,
        }],
        added_at: None,
        updated_at: None,
        total_episode_count: 20,
        watched_episode_count: 8,
        last_watched_at: None,
        seasons: vec![],
    }
}

/// Sample episode for testing
pub fn sample_episode_1() -> Episode {
    Episode {
        id: "test-episode-1".to_string(),
        show_id: Some("test-show-1".to_string()),
        backend_id: "test-backend".to_string(),
        title: "Pilot Episode".to_string(),
        season_number: 1,
        episode_number: 1,
        duration: Duration::from_secs(2700), // 45 minutes
        overview: Some("The beginning of our testing journey".to_string()),
        air_date: None,
        thumbnail_url: Some("/library/metadata/4/thumb".to_string()),
        watched: false,
        show_title: None,
        show_poster_url: None,
        view_count: 0,
        last_watched_at: None,
        playback_position: None,
        intro_marker: None,
        credits_marker: None,
    }
}

/// Sample episode 2 - partially watched
pub fn sample_episode_2() -> Episode {
    Episode {
        id: "test-episode-2".to_string(),
        show_id: Some("test-show-1".to_string()),
        backend_id: "test-backend".to_string(),
        title: "The Second Test".to_string(),
        season_number: 1,
        episode_number: 2,
        duration: Duration::from_secs(2700),
        overview: Some("Building on our test foundation".to_string()),
        air_date: None,
        thumbnail_url: Some("/library/metadata/5/thumb".to_string()),
        watched: false,
        show_title: None,
        show_poster_url: None,
        view_count: 1,
        last_watched_at: None,
        playback_position: Some(Duration::from_secs(1800)), // 30 min in
        intro_marker: Some(ChapterMarker {
            start_time: Duration::from_secs(10),
            end_time: Duration::from_secs(70),
            marker_type: ChapterType::Intro,
        }),
        credits_marker: Some(ChapterMarker {
            start_time: Duration::from_secs(2580),
            end_time: Duration::from_secs(2700),
            marker_type: ChapterType::Credits,
        }),
    }
}

/// Sample library for movies
pub fn sample_movie_library() -> Library {
    Library {
        id: "test-movie-lib".to_string(),
        title: "Test Movies".to_string(),
        library_type: LibraryType::Movies,
        icon: None,
        item_count: 2,
    }
}

/// Sample library for TV shows
pub fn sample_tv_library() -> Library {
    Library {
        id: "test-tv-lib".to_string(),
        title: "Test TV Shows".to_string(),
        library_type: LibraryType::Shows,
        icon: None,
        item_count: 1,
    }
}

/// Sample user for testing
pub fn sample_user() -> User {
    User {
        id: "test-user-1".to_string(),
        username: "testuser".to_string(),
        email: Some("test@example.com".to_string()),
        avatar_url: None,
    }
}

/// Sample stream info for testing
pub fn sample_stream_info() -> StreamInfo {
    StreamInfo {
        url: "http://localhost:32400/video/:/transcode/universal/start.mp4".to_string(),
        direct_play: true,
        video_codec: "h264".to_string(),
        audio_codec: "aac".to_string(),
        container: "mp4".to_string(),
        bitrate: 5000000,
        resolution: Resolution {
            width: 1920,
            height: 1080,
        },
        quality_options: vec![
            QualityOption {
                name: "Original (1080p)".to_string(),
                url: "http://localhost:32400/video/original.mp4".to_string(),
                resolution: Resolution {
                    width: 1920,
                    height: 1080,
                },
                bitrate: 5000000,
                requires_transcode: false,
            },
            QualityOption {
                name: "720p".to_string(),
                url: "http://localhost:32400/video/720p.mp4".to_string(),
                resolution: Resolution {
                    width: 1280,
                    height: 720,
                },
                bitrate: 3000000,
                requires_transcode: true,
            },
        ],
    }
}
