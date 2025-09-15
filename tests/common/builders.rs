use chrono::{DateTime, Utc};
use reel::models::*;
use std::time::Duration;

pub struct MediaItemBuilder {
    id: String,
    title: String,
    media_type: MediaType,
    year: Option<u32>,
    duration: Option<Duration>,
    rating: Option<f32>,
}

impl MediaItemBuilder {
    pub fn movie(title: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title: title.to_string(),
            media_type: MediaType::Movie,
            year: Some(2024),
            duration: Some(Duration::from_secs(120 * 60)),
            rating: Some(7.5),
        }
    }

    pub fn show(title: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title: title.to_string(),
            media_type: MediaType::Show,
            year: Some(2024),
            duration: None,
            rating: Some(8.0),
        }
    }

    pub fn with_id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }

    pub fn with_year(mut self, year: u32) -> Self {
        self.year = Some(year);
        self
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    pub fn with_rating(mut self, rating: f32) -> Self {
        self.rating = Some(rating);
        self
    }

    pub fn build_movie(self) -> Movie {
        Movie {
            id: self.id,
            backend_id: "test_backend".to_string(),
            title: self.title,
            year: self.year,
            duration: self.duration.unwrap_or(Duration::from_secs(120 * 60)),
            rating: self.rating,
            poster_url: None,
            backdrop_url: None,
            overview: None,
            genres: vec![],
            cast: vec![],
            crew: vec![],
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

    pub fn build_show(self) -> Show {
        Show {
            id: self.id,
            backend_id: "test_backend".to_string(),
            title: self.title,
            year: self.year,
            seasons: vec![],
            rating: self.rating,
            poster_url: None,
            backdrop_url: None,
            overview: None,
            genres: vec![],
            cast: vec![],
            added_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
            watched_episode_count: 0,
            total_episode_count: 0,
            last_watched_at: None,
        }
    }
}

pub struct LibraryBuilder {
    id: String,
    title: String,
    library_type: LibraryType,
    item_count: Option<i32>,
}

impl LibraryBuilder {
    pub fn new(title: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title: title.to_string(),
            library_type: LibraryType::Movies,
            item_count: Some(0),
        }
    }

    pub fn with_id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }

    pub fn with_type(mut self, library_type: LibraryType) -> Self {
        self.library_type = library_type;
        self
    }

    pub fn with_item_count(mut self, count: i32) -> Self {
        self.item_count = Some(count);
        self
    }

    pub fn build(self) -> Library {
        Library {
            id: self.id,
            title: self.title,
            library_type: self.library_type,
            icon: None,
            item_count: self.item_count,
        }
    }
}

pub struct SourceBuilder {
    id: String,
    name: String,
    source_type: SourceType,
    address: String,
}

impl SourceBuilder {
    pub fn plex(name: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            source_type: SourceType::Plex,
            address: "http://localhost:32400".to_string(),
        }
    }

    pub fn jellyfin(name: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            source_type: SourceType::Jellyfin,
            address: "http://localhost:8096".to_string(),
        }
    }

    pub fn with_id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }

    pub fn with_address(mut self, address: &str) -> Self {
        self.address = address.to_string();
        self
    }

    pub fn build(self) -> Source {
        Source {
            id: SourceId::from(self.id),
            name: self.name,
            source_type: self.source_type,
            connection_info: ConnectionInfo::Url(self.address),
            username: None,
            token: None,
            is_active: true,
            priority: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

pub struct EpisodeBuilder {
    id: String,
    show_id: String,
    title: String,
    season_number: u32,
    episode_number: u32,
    duration: Option<Duration>,
}

impl EpisodeBuilder {
    pub fn new(title: &str, season: u32, episode: u32) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            show_id: "test_show".to_string(),
            title: title.to_string(),
            season_number: season,
            episode_number: episode,
            duration: Some(Duration::from_secs(45 * 60)), // 45 minutes
        }
    }

    pub fn with_id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }

    pub fn with_show_id(mut self, show_id: &str) -> Self {
        self.show_id = show_id.to_string();
        self
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    pub fn build(self) -> Episode {
        Episode {
            id: self.id,
            backend_id: "test_backend".to_string(),
            show_id: ShowId::from(self.show_id),
            season_number: self.season_number,
            episode_number: self.episode_number,
            title: self.title,
            duration: self.duration.unwrap_or(Duration::from_secs(45 * 60)),
            air_date: None,
            overview: None,
            still_url: None,
            watched: false,
            playback_position: None,
            last_watched_at: None,
            intro_marker: None,
            credits_marker: None,
        }
    }
}
