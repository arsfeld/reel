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

/// High-dynamic-range format on a media item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HdrFormat {
    /// Generic HDR (HDR10, HDR10+, HLG, …).
    Hdr,
    /// Dolby Vision (with or without HDR10 base layer).
    DolbyVision,
}

impl HdrFormat {
    /// Compact short string used for DB storage.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hdr => "hdr",
            Self::DolbyVision => "dolby_vision",
        }
    }

    /// Parse from the DB representation produced by `as_str`.
    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "hdr" => Some(Self::Hdr),
            "dolby_vision" => Some(Self::DolbyVision),
            _ => None,
        }
    }

    /// Map Plex's `videoDynamicRange` value to an `HdrFormat`. Returns `None`
    /// for SDR, empty strings, and anything unrecognised — callers should
    /// treat that as "no HDR signal." Case-insensitive.
    pub fn from_plex(raw: &str) -> Option<Self> {
        match raw.trim().to_lowercase().as_str() {
            "" | "sdr" => None,
            "dv" | "dolby vision" | "dolbyvision" | "dolby_vision" => Some(Self::DolbyVision),
            s if s.contains("hdr") || s == "hlg" => Some(Self::Hdr),
            _ => None,
        }
    }
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
    /// Poster of the parent series (Plex `grandparentThumb`) for an episode.
    /// Lets portrait shelf cards show the show's poster instead of the episode's
    /// landscape still. `None` for non-episodes and sources that don't report it.
    pub series_poster_path: Option<String>,
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
    /// Video resolution from the primary media version (e.g., "1080", "4k").
    pub video_resolution: Option<String>,
    /// High-dynamic-range format (HDR / Dolby Vision). `None` means SDR or
    /// no signal from the source.
    pub hdr: Option<HdrFormat>,
    pub added_at: String,
    pub updated_at: String,
    /// Saved playback position in milliseconds (e.g. a Plex On Deck view
    /// offset). `None` when the source reports no resume position.
    pub playback_position_ms: Option<i64>,
    /// The Plex library section key this item belongs to, when known. Transient
    /// (never persisted to the DB) — used to filter aggregate views (Home,
    /// Continue Watching) by the user's hidden-library set. `None` for sources
    /// or endpoints that do not report a section.
    pub library_section_id: Option<String>,
}

impl MediaItem {
    /// Display title with year suffix when available.
    pub fn display_title(&self) -> String {
        match self.year {
            Some(y) => format!("{} ({})", self.title, y),
            None => self.title.clone(),
        }
    }

    /// Poster to show on a portrait shelf card: the parent series poster for an
    /// episode (so a show's poster shows instead of a landscape episode still),
    /// falling back to the item's own poster.
    pub fn shelf_poster_path(&self) -> Option<&str> {
        self.series_poster_path
            .as_deref()
            .or(self.poster_path.as_deref())
    }

    /// Build a composite ID from source info.
    pub fn make_id(source_type: SourceType, source_id: &str, external_id: &str) -> String {
        format!("{}:{}:{}", source_type.as_str(), source_id, external_id)
    }

    /// Format video resolution for display on badges.
    /// "4k" → "4K", "1080" → "1080p", "720" → "720p", "480" → "SD", etc.
    pub fn format_resolution(raw: &str) -> &'static str {
        match raw.to_lowercase().as_str() {
            "4k" => "4K",
            "1080" => "1080p",
            "720" => "720p",
            "480" | "576" | "sd" => "SD",
            _ => "HD",
        }
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

    /// Resume progress as a 0..1 fraction, derived from the saved playback
    /// position against the item's runtime. `None` when either the position or
    /// a positive runtime is unknown. Used to draw the Continue Watching
    /// progress bar for On Deck items that carry their own view offset.
    pub fn resume_fraction(&self) -> Option<f64> {
        let pos = self.playback_position_ms? as f64;
        let total = self.runtime_minutes? as f64 * 60_000.0;
        if total <= 0.0 {
            return None;
        }
        Some((pos / total).clamp(0.0, 1.0))
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
            series_poster_path: None,
            backdrop_path: Some("/library/metadata/123/art/1234567".to_string()),
            genres: vec!["Science Fiction".to_string(), "Adventure".to_string()],
            parent_id: None,
            season_number: None,
            episode_number: None,
            air_date: None,
            file_path: Some("/library/parts/456/file.mkv".to_string()),
            video_resolution: Some("1080".to_string()),
            hdr: None,
            added_at: "2024-01-15".to_string(),
            updated_at: "2024-01-15".to_string(),
            playback_position_ms: None,
            library_section_id: None,
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
    fn shelf_poster_prefers_series_then_own() {
        let mut m = test_movie();
        // No series poster: falls back to the item's own poster.
        assert_eq!(m.shelf_poster_path(), m.poster_path.as_deref());
        // Series poster present: takes precedence.
        m.series_poster_path = Some("/series/poster".to_string());
        assert_eq!(m.shelf_poster_path(), Some("/series/poster"));
        // Neither: None.
        m.poster_path = None;
        m.series_poster_path = None;
        assert_eq!(m.shelf_poster_path(), None);
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
    fn resume_fraction_half_watched() {
        let mut movie = test_movie();
        movie.runtime_minutes = Some(100);
        movie.playback_position_ms = Some(50 * 60_000);
        assert_eq!(movie.resume_fraction(), Some(0.5));
    }

    #[test]
    fn resume_fraction_none_without_position() {
        let mut movie = test_movie();
        movie.playback_position_ms = None;
        assert_eq!(movie.resume_fraction(), None);
    }

    #[test]
    fn resume_fraction_clamped_to_one() {
        let mut movie = test_movie();
        movie.runtime_minutes = Some(10);
        movie.playback_position_ms = Some(999 * 60_000);
        assert_eq!(movie.resume_fraction(), Some(1.0));
    }

    #[test]
    fn resume_fraction_none_with_zero_runtime() {
        let mut movie = test_movie();
        movie.runtime_minutes = Some(0);
        movie.playback_position_ms = Some(1000);
        assert_eq!(movie.resume_fraction(), None);
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

    #[test]
    fn format_resolution_4k() {
        assert_eq!(MediaItem::format_resolution("4k"), "4K");
        assert_eq!(MediaItem::format_resolution("4K"), "4K");
    }

    #[test]
    fn format_resolution_1080p() {
        assert_eq!(MediaItem::format_resolution("1080"), "1080p");
    }

    #[test]
    fn format_resolution_720p() {
        assert_eq!(MediaItem::format_resolution("720"), "720p");
    }

    #[test]
    fn format_resolution_sd() {
        assert_eq!(MediaItem::format_resolution("480"), "SD");
        assert_eq!(MediaItem::format_resolution("576"), "SD");
        assert_eq!(MediaItem::format_resolution("sd"), "SD");
    }

    #[test]
    fn format_resolution_unknown_defaults_to_hd() {
        assert_eq!(MediaItem::format_resolution("unknown"), "HD");
    }

    // --- HdrFormat ---

    #[test]
    fn hdr_from_plex_maps_hdr_variants_to_hdr() {
        assert_eq!(HdrFormat::from_plex("HDR"), Some(HdrFormat::Hdr));
        assert_eq!(HdrFormat::from_plex("HDR10"), Some(HdrFormat::Hdr));
        assert_eq!(HdrFormat::from_plex("HDR10+"), Some(HdrFormat::Hdr));
        assert_eq!(HdrFormat::from_plex("HLG"), Some(HdrFormat::Hdr));
    }

    #[test]
    fn hdr_from_plex_maps_dolby_vision_variants() {
        assert_eq!(
            HdrFormat::from_plex("Dolby Vision"),
            Some(HdrFormat::DolbyVision)
        );
        assert_eq!(HdrFormat::from_plex("DV"), Some(HdrFormat::DolbyVision));
        assert_eq!(
            HdrFormat::from_plex("DolbyVision"),
            Some(HdrFormat::DolbyVision)
        );
    }

    #[test]
    fn hdr_from_plex_returns_none_for_sdr_or_empty() {
        assert_eq!(HdrFormat::from_plex("SDR"), None);
        assert_eq!(HdrFormat::from_plex(""), None);
        assert_eq!(HdrFormat::from_plex("   "), None);
        assert_eq!(HdrFormat::from_plex("unknown"), None);
    }

    #[test]
    fn hdr_from_plex_is_case_insensitive() {
        assert_eq!(HdrFormat::from_plex("hdr"), Some(HdrFormat::Hdr));
        assert_eq!(HdrFormat::from_plex("hdr10"), Some(HdrFormat::Hdr));
        assert_eq!(
            HdrFormat::from_plex("dolby vision"),
            Some(HdrFormat::DolbyVision)
        );
    }

    #[test]
    fn hdr_db_str_roundtrip() {
        for fmt in [HdrFormat::Hdr, HdrFormat::DolbyVision] {
            assert_eq!(HdrFormat::from_db_str(fmt.as_str()), Some(fmt));
        }
        assert_eq!(HdrFormat::from_db_str("nope"), None);
        assert_eq!(HdrFormat::from_db_str(""), None);
    }
}
