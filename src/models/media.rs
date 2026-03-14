/// Type of media content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    Movie,
    Show,
    Season,
    Episode,
    Collection,
}

impl MediaType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Movie => "movie",
            Self::Show => "show",
            Self::Season => "season",
            Self::Episode => "episode",
            Self::Collection => "collection",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "movie" => Some(Self::Movie),
            "show" => Some(Self::Show),
            "season" => Some(Self::Season),
            "episode" => Some(Self::Episode),
            "collection" => Some(Self::Collection),
            _ => None,
        }
    }
}

/// Source type for media items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceType {
    Plex,
    Local,
}

impl SourceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Plex => "plex",
            Self::Local => "local",
        }
    }

    #[allow(dead_code)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "plex" => Some(Self::Plex),
            "local" => Some(Self::Local),
            _ => None,
        }
    }
}

/// A media item in the library (movie, show, season, or episode).
#[derive(Debug, Clone, PartialEq)]
pub struct MediaItem {
    /// Composite ID: "{source_type}:{source_id}:{external_id}"
    pub id: String,
    pub source_type: SourceType,
    /// Server URL or local path root
    pub source_id: String,
    /// Rating key (Plex) or file path (local)
    pub external_id: String,
    pub media_type: MediaType,
    pub title: String,
    pub year: Option<i32>,
    pub overview: Option<String>,
    pub content_rating: Option<String>,
    pub rating: Option<f64>,
    pub runtime_minutes: Option<i32>,
    /// Relative path for poster image (e.g., /library/metadata/123/thumb/...)
    pub poster_path: Option<String>,
    /// Relative path for backdrop image
    pub backdrop_path: Option<String>,
    pub genres: Vec<String>,
    /// Parent item ID (e.g., show ID for a season, season ID for an episode)
    pub parent_id: Option<String>,
    pub season_number: Option<i32>,
    pub episode_number: Option<i32>,
    pub air_date: Option<String>,
    /// File path or Plex part key for playback
    pub file_path: Option<String>,
    pub added_at: String,
    pub updated_at: String,
}

impl MediaItem {
    /// Display title with year suffix when available.
    pub fn display_title(&self) -> String {
        match self.year {
            Some(y) => format!("{} ({})", self.title, y),
            None => self.title.clone(),
        }
    }

    /// Build a composite ID from source info.
    pub fn make_id(source_type: SourceType, source_id: &str, external_id: &str) -> String {
        format!("{}:{}:{}", source_type.as_str(), source_id, external_id)
    }

    /// Format runtime as "Xh Ym" or "Xm".
    pub fn format_runtime(&self) -> Option<String> {
        self.runtime_minutes.map(|mins| {
            if mins >= 60 {
                format!("{}h {}m", mins / 60, mins % 60)
            } else {
                format!("{}m", mins)
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_movie() -> MediaItem {
        MediaItem {
            id: "plex:http://localhost:32400:123".to_string(),
            source_type: SourceType::Plex,
            source_id: "http://localhost:32400".to_string(),
            external_id: "123".to_string(),
            media_type: MediaType::Movie,
            title: "Dune".to_string(),
            year: Some(2021),
            overview: Some("A noble family becomes embroiled in a war.".to_string()),
            content_rating: Some("PG-13".to_string()),
            rating: Some(8.0),
            runtime_minutes: Some(155),
            poster_path: Some("/library/metadata/123/thumb/1234567".to_string()),
            backdrop_path: Some("/library/metadata/123/art/1234567".to_string()),
            genres: vec!["Science Fiction".to_string(), "Adventure".to_string()],
            parent_id: None,
            season_number: None,
            episode_number: None,
            air_date: None,
            file_path: Some("/library/parts/456/file.mkv".to_string()),
            added_at: "2024-01-15".to_string(),
            updated_at: "2024-01-15".to_string(),
        }
    }

    #[test]
    fn display_title_with_year() {
        let movie = test_movie();
        assert_eq!(movie.display_title(), "Dune (2021)");
    }

    #[test]
    fn display_title_without_year() {
        let mut movie = test_movie();
        movie.year = None;
        assert_eq!(movie.display_title(), "Dune");
    }

    #[test]
    fn make_id_format() {
        let id = MediaItem::make_id(SourceType::Plex, "http://localhost:32400", "123");
        assert_eq!(id, "plex:http://localhost:32400:123");
    }

    #[test]
    fn make_id_local() {
        let id = MediaItem::make_id(SourceType::Local, "/media/movies", "dune.mkv");
        assert_eq!(id, "local:/media/movies:dune.mkv");
    }

    #[test]
    fn format_runtime_hours_and_minutes() {
        let movie = test_movie();
        assert_eq!(movie.format_runtime(), Some("2h 35m".to_string()));
    }

    #[test]
    fn format_runtime_minutes_only() {
        let mut movie = test_movie();
        movie.runtime_minutes = Some(45);
        assert_eq!(movie.format_runtime(), Some("45m".to_string()));
    }

    #[test]
    fn format_runtime_none() {
        let mut movie = test_movie();
        movie.runtime_minutes = None;
        assert_eq!(movie.format_runtime(), None);
    }

    #[test]
    fn format_runtime_exact_hour() {
        let mut movie = test_movie();
        movie.runtime_minutes = Some(120);
        assert_eq!(movie.format_runtime(), Some("2h 0m".to_string()));
    }

    #[test]
    fn media_type_roundtrip() {
        for mt in [
            MediaType::Movie,
            MediaType::Show,
            MediaType::Season,
            MediaType::Episode,
        ] {
            assert_eq!(MediaType::from_str(mt.as_str()), Some(mt));
        }
    }

    #[test]
    fn media_type_unknown_returns_none() {
        assert_eq!(MediaType::from_str("unknown"), None);
    }

    #[test]
    fn source_type_roundtrip() {
        for st in [SourceType::Plex, SourceType::Local] {
            assert_eq!(SourceType::from_str(st.as_str()), Some(st));
        }
    }

    #[test]
    fn source_type_unknown_returns_none() {
        assert_eq!(SourceType::from_str("jellyfin"), None);
    }

    #[test]
    fn media_item_equality() {
        let a = test_movie();
        let b = test_movie();
        assert_eq!(a, b);
    }
}
