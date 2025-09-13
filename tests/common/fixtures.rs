use super::builders::*;
use gnome_reel::models::*;

pub struct Fixtures;

impl Fixtures {
    pub fn movies() -> Vec<Movie> {
        vec![
            MediaItemBuilder::movie("The Matrix")
                .with_id("matrix")
                .with_year(1999)
                .with_rating(8.7)
                .build_movie(),
            MediaItemBuilder::movie("Inception")
                .with_id("inception")
                .with_year(2010)
                .with_rating(8.8)
                .build_movie(),
            MediaItemBuilder::movie("Interstellar")
                .with_id("interstellar")
                .with_year(2014)
                .with_rating(8.6)
                .build_movie(),
            MediaItemBuilder::movie("The Dark Knight")
                .with_id("dark_knight")
                .with_year(2008)
                .with_rating(9.0)
                .build_movie(),
            MediaItemBuilder::movie("Pulp Fiction")
                .with_id("pulp_fiction")
                .with_year(1994)
                .with_rating(8.9)
                .build_movie(),
        ]
    }

    pub fn shows() -> Vec<Show> {
        vec![
            MediaItemBuilder::show("Breaking Bad")
                .with_id("breaking_bad")
                .with_year(2008)
                .with_rating(9.5)
                .build_show(),
            MediaItemBuilder::show("Game of Thrones")
                .with_id("got")
                .with_year(2011)
                .with_rating(9.3)
                .build_show(),
            MediaItemBuilder::show("The Office")
                .with_id("the_office")
                .with_year(2005)
                .with_rating(8.9)
                .build_show(),
        ]
    }

    pub fn episodes(show_id: &str) -> Vec<Episode> {
        vec![
            EpisodeBuilder::new("Pilot", 1, 1)
                .with_show_id(show_id)
                .with_id(&format!("{}_s01e01", show_id))
                .build(),
            EpisodeBuilder::new("The Next Episode", 1, 2)
                .with_show_id(show_id)
                .with_id(&format!("{}_s01e02", show_id))
                .build(),
            EpisodeBuilder::new("Season Premiere", 2, 1)
                .with_show_id(show_id)
                .with_id(&format!("{}_s02e01", show_id))
                .build(),
        ]
    }

    pub fn libraries() -> Vec<Library> {
        vec![
            LibraryBuilder::new("Movies")
                .with_id("movies_lib")
                .with_type(MediaType::Movie)
                .with_item_count(100)
                .build(),
            LibraryBuilder::new("TV Shows")
                .with_id("shows_lib")
                .with_type(MediaType::Show)
                .with_item_count(50)
                .build(),
            LibraryBuilder::new("Documentaries")
                .with_id("docs_lib")
                .with_type(MediaType::Movie)
                .with_item_count(25)
                .build(),
        ]
    }

    pub fn plex_source() -> Source {
        SourceBuilder::plex("Main Plex Server")
            .with_id("plex_main")
            .with_address("http://plex.local:32400")
            .build()
    }

    pub fn jellyfin_source() -> Source {
        SourceBuilder::jellyfin("Jellyfin Server")
            .with_id("jellyfin_main")
            .with_address("http://jellyfin.local:8096")
            .build()
    }

    pub fn playback_progress() -> PlaybackProgress {
        PlaybackProgress {
            id: uuid::Uuid::new_v4().to_string(),
            media_item_id: "matrix".to_string(),
            user_id: "test_user".to_string(),
            position_ms: 45 * 60 * 1000,  // 45 minutes
            duration_ms: 136 * 60 * 1000, // 2h 16m
            played: false,
            played_at: None,
            updated_at: chrono::Utc::now().timestamp(),
        }
    }

    pub fn continue_watching() -> Vec<MediaItem> {
        vec![
            MediaItem::Movie(
                MediaItemBuilder::movie("Inception")
                    .with_id("inception_cw")
                    .build_movie(),
            ),
            MediaItem::Episode(
                EpisodeBuilder::new("The One Where", 2, 5)
                    .with_id("friends_s02e05")
                    .build(),
            ),
        ]
    }

    pub fn recently_added() -> Vec<MediaItem> {
        vec![
            MediaItem::Movie(
                MediaItemBuilder::movie("New Release")
                    .with_id("new_movie")
                    .with_year(2024)
                    .build_movie(),
            ),
            MediaItem::Show(
                MediaItemBuilder::show("New Series")
                    .with_id("new_show")
                    .with_year(2024)
                    .build_show(),
            ),
        ]
    }
}

pub fn generate_bulk_media(count: usize, media_type: MediaType) -> Vec<MediaItem> {
    (0..count)
        .map(|i| match media_type {
            MediaType::Movie => MediaItem::Movie(
                MediaItemBuilder::movie(&format!("Movie {}", i))
                    .with_id(&format!("movie_{}", i))
                    .with_year(2000 + (i as i32 % 24))
                    .with_rating(5.0 + (i as f32 % 50) / 10.0)
                    .build_movie(),
            ),
            MediaType::Show => MediaItem::Show(
                MediaItemBuilder::show(&format!("Show {}", i))
                    .with_id(&format!("show_{}", i))
                    .with_year(2000 + (i as i32 % 24))
                    .with_rating(5.0 + (i as f32 % 50) / 10.0)
                    .build_show(),
            ),
            _ => unreachable!(),
        })
        .collect()
}

pub fn cleanup_test_data() {
    // Placeholder for any cleanup operations needed
    // Most cleanup happens automatically via TempDir in TestContext
}
