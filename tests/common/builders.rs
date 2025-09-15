use chrono::Utc;
use reel::models::*;

pub struct MediaItemBuilder {
    id: String,
    title: String,
    media_type: MediaType,
    year: Option<i32>,
    duration_ms: Option<i64>,
    rating: Option<f32>,
}

impl MediaItemBuilder {
    pub fn movie(title: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title: title.to_string(),
            media_type: MediaType::Movie,
            year: Some(2024),
            duration_ms: Some(120 * 60 * 1000),
            rating: Some(7.5),
        }
    }

    pub fn show(title: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title: title.to_string(),
            media_type: MediaType::Show,
            year: Some(2024),
            duration_ms: None,
            rating: Some(8.0),
        }
    }

    pub fn with_id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }

    pub fn with_year(mut self, year: i32) -> Self {
        self.year = Some(year);
        self
    }

    pub fn with_duration_ms(mut self, duration_ms: i64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }

    pub fn with_rating(mut self, rating: f32) -> Self {
        self.rating = Some(rating);
        self
    }

    pub fn build_movie(self) -> Movie {
        Movie {
            id: self.id,
            source_id: "test_source".to_string(),
            library_id: "test_library".to_string(),
            title: self.title.clone(),
            sort_title: self.title,
            year: self.year,
            duration_ms: self.duration_ms,
            rating: self.rating,
            added_at: Utc::now().timestamp(),
            updated_at: Utc::now().timestamp(),
            ..Default::default()
        }
    }

    pub fn build_show(self) -> Show {
        Show {
            id: self.id,
            source_id: "test_source".to_string(),
            library_id: "test_library".to_string(),
            title: self.title.clone(),
            sort_title: self.title,
            year: self.year,
            rating: self.rating,
            season_count: 3,
            episode_count: 30,
            added_at: Utc::now().timestamp(),
            updated_at: Utc::now().timestamp(),
            ..Default::default()
        }
    }
}

pub struct LibraryBuilder {
    id: String,
    name: String,
    library_type: MediaType,
    item_count: i32,
}

impl LibraryBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            library_type: MediaType::Movie,
            item_count: 0,
        }
    }

    pub fn with_id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }

    pub fn with_type(mut self, library_type: MediaType) -> Self {
        self.library_type = library_type;
        self
    }

    pub fn with_item_count(mut self, count: i32) -> Self {
        self.item_count = count;
        self
    }

    pub fn build(self) -> Library {
        Library {
            id: self.id,
            source_id: "test_source".to_string(),
            name: self.name,
            library_type: self.library_type,
            item_count: self.item_count,
            created_at: Utc::now().timestamp(),
            updated_at: Utc::now().timestamp(),
            ..Default::default()
        }
    }
}

pub struct SourceBuilder {
    id: String,
    name: String,
    server_type: ServerType,
    address: String,
}

impl SourceBuilder {
    pub fn plex(name: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            server_type: ServerType::Plex,
            address: "http://localhost:32400".to_string(),
        }
    }

    pub fn jellyfin(name: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            server_type: ServerType::Jellyfin,
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
            id: self.id,
            name: self.name,
            server_type: self.server_type,
            address: self.address,
            machine_identifier: Some(uuid::Uuid::new_v4().to_string()),
            access_token: Some("test_token".to_string()),
            is_active: true,
            created_at: Utc::now().timestamp(),
            updated_at: Utc::now().timestamp(),
            ..Default::default()
        }
    }
}

pub struct EpisodeBuilder {
    id: String,
    show_id: String,
    title: String,
    season_number: i32,
    episode_number: i32,
    duration_ms: Option<i64>,
}

impl EpisodeBuilder {
    pub fn new(title: &str, season: i32, episode: i32) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            show_id: "test_show".to_string(),
            title: title.to_string(),
            season_number: season,
            episode_number: episode,
            duration_ms: Some(45 * 60 * 1000), // 45 minutes
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

    pub fn with_duration_ms(mut self, duration_ms: i64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }

    pub fn build(self) -> Episode {
        Episode {
            id: self.id,
            source_id: "test_source".to_string(),
            show_id: self.show_id,
            season_id: format!("season_{}", self.season_number),
            title: self.title.clone(),
            sort_title: self.title,
            season_number: self.season_number,
            episode_number: self.episode_number,
            duration_ms: self.duration_ms,
            added_at: Utc::now().timestamp(),
            updated_at: Utc::now().timestamp(),
            ..Default::default()
        }
    }
}
