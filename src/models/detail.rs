use super::media::MediaItem;

/// A cast member (actor) with optional character name and photo.
#[derive(Debug, Clone, PartialEq)]
pub struct CastMember {
    pub name: String,
    pub character: Option<String>,
    /// Relative path to the actor's photo (e.g., Plex thumb path).
    pub photo_path: Option<String>,
}

/// Technical information about the media file.
#[derive(Debug, Clone, PartialEq)]
pub struct TechnicalInfo {
    pub video_resolution: Option<String>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub audio_channels: Option<i32>,
    pub container: Option<String>,
    pub bitrate_kbps: Option<i64>,
    pub file_size_bytes: Option<i64>,
}

/// A reference to a collection this item belongs to.
#[derive(Debug, Clone, PartialEq)]
pub struct CollectionRef {
    pub name: String,
}

/// Detailed metadata for a media item, returned by detail page queries.
/// Wraps the base `MediaItem` with additional enrichment data.
#[derive(Debug, Clone)]
pub struct MediaDetail {
    pub item: MediaItem,
    pub cast: Vec<CastMember>,
    pub directors: Vec<String>,
    pub writers: Vec<String>,
    pub technical: Option<TechnicalInfo>,
    pub collections: Vec<CollectionRef>,
}

impl MediaDetail {
    /// Create a minimal MediaDetail from a MediaItem (no enrichment).
    pub fn from_item(item: MediaItem) -> Self {
        Self {
            item,
            cast: Vec::new(),
            directors: Vec::new(),
            writers: Vec::new(),
            technical: None,
            collections: Vec::new(),
        }
    }
}

impl TechnicalInfo {
    /// Format resolution with "p" suffix (e.g., "1080" → "1080p", "4k" → "4K").
    pub fn display_resolution(&self) -> Option<String> {
        self.video_resolution.as_ref().map(|r| {
            if r.eq_ignore_ascii_case("4k") || r == "2160" {
                "4K".to_string()
            } else {
                format!("{r}p")
            }
        })
    }

    /// Format audio channels (e.g., 6 → "5.1", 8 → "7.1", 2 → "Stereo").
    pub fn display_audio_channels(&self) -> Option<String> {
        self.audio_channels.map(|ch| match ch {
            1 => "Mono".to_string(),
            2 => "Stereo".to_string(),
            6 => "5.1".to_string(),
            8 => "7.1".to_string(),
            n => format!("{n}ch"),
        })
    }

    /// Format file size for display (e.g., "12.5 GB", "800 MB").
    pub fn display_file_size(&self) -> Option<String> {
        self.file_size_bytes.map(|bytes| {
            let gb = bytes as f64 / 1_073_741_824.0;
            if gb >= 1.0 {
                format!("{gb:.1} GB")
            } else {
                let mb = bytes as f64 / 1_048_576.0;
                format!("{mb:.0} MB")
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_resolution_standard() {
        let info = TechnicalInfo {
            video_resolution: Some("1080".to_string()),
            video_codec: None,
            audio_codec: None,
            audio_channels: None,
            container: None,
            bitrate_kbps: None,
            file_size_bytes: None,
        };
        assert_eq!(info.display_resolution(), Some("1080p".to_string()));
    }

    #[test]
    fn display_resolution_4k() {
        let info = TechnicalInfo {
            video_resolution: Some("4k".to_string()),
            video_codec: None,
            audio_codec: None,
            audio_channels: None,
            container: None,
            bitrate_kbps: None,
            file_size_bytes: None,
        };
        assert_eq!(info.display_resolution(), Some("4K".to_string()));
    }

    #[test]
    fn display_resolution_2160() {
        let info = TechnicalInfo {
            video_resolution: Some("2160".to_string()),
            video_codec: None,
            audio_codec: None,
            audio_channels: None,
            container: None,
            bitrate_kbps: None,
            file_size_bytes: None,
        };
        assert_eq!(info.display_resolution(), Some("4K".to_string()));
    }

    #[test]
    fn display_resolution_none() {
        let info = TechnicalInfo {
            video_resolution: None,
            video_codec: None,
            audio_codec: None,
            audio_channels: None,
            container: None,
            bitrate_kbps: None,
            file_size_bytes: None,
        };
        assert_eq!(info.display_resolution(), None);
    }

    #[test]
    fn display_audio_channels_surround() {
        let info = TechnicalInfo {
            video_resolution: None,
            video_codec: None,
            audio_codec: None,
            audio_channels: Some(6),
            container: None,
            bitrate_kbps: None,
            file_size_bytes: None,
        };
        assert_eq!(info.display_audio_channels(), Some("5.1".to_string()));
    }

    #[test]
    fn display_audio_channels_stereo() {
        let info = TechnicalInfo {
            video_resolution: None,
            video_codec: None,
            audio_codec: None,
            audio_channels: Some(2),
            container: None,
            bitrate_kbps: None,
            file_size_bytes: None,
        };
        assert_eq!(info.display_audio_channels(), Some("Stereo".to_string()));
    }

    #[test]
    fn display_audio_channels_atmos() {
        let info = TechnicalInfo {
            video_resolution: None,
            video_codec: None,
            audio_codec: None,
            audio_channels: Some(8),
            container: None,
            bitrate_kbps: None,
            file_size_bytes: None,
        };
        assert_eq!(info.display_audio_channels(), Some("7.1".to_string()));
    }

    #[test]
    fn display_file_size_gb() {
        let info = TechnicalInfo {
            video_resolution: None,
            video_codec: None,
            audio_codec: None,
            audio_channels: None,
            container: None,
            bitrate_kbps: None,
            file_size_bytes: Some(15_000_000_000),
        };
        assert_eq!(info.display_file_size(), Some("14.0 GB".to_string()));
    }

    #[test]
    fn display_file_size_mb() {
        let info = TechnicalInfo {
            video_resolution: None,
            video_codec: None,
            audio_codec: None,
            audio_channels: None,
            container: None,
            bitrate_kbps: None,
            file_size_bytes: Some(500_000_000),
        };
        assert_eq!(info.display_file_size(), Some("477 MB".to_string()));
    }

    #[test]
    fn display_file_size_none() {
        let info = TechnicalInfo {
            video_resolution: None,
            video_codec: None,
            audio_codec: None,
            audio_channels: None,
            container: None,
            bitrate_kbps: None,
            file_size_bytes: None,
        };
        assert_eq!(info.display_file_size(), None);
    }

    #[test]
    fn media_detail_from_item_has_empty_enrichment() {
        use crate::models::media::{MediaItem, MediaType, SourceType};

        let item = MediaItem {
            id: "test:1".to_string(),
            source_type: SourceType::Plex,
            source_id: "http://localhost".to_string(),
            external_id: "1".to_string(),
            media_type: MediaType::Movie,
            title: "Test".to_string(),
            year: None,
            overview: None,
            content_rating: None,
            rating: None,
            runtime_minutes: None,
            poster_path: None,
            backdrop_path: None,
            genres: vec![],
            parent_id: None,
            season_number: None,
            episode_number: None,
            air_date: None,
            file_path: None,
            video_resolution: None,
            hdr: None,
            added_at: String::new(),
            updated_at: String::new(),
            playback_position_ms: None,
            library_section_id: None,
        };

        let detail = MediaDetail::from_item(item);
        assert!(detail.cast.is_empty());
        assert!(detail.directors.is_empty());
        assert!(detail.writers.is_empty());
        assert!(detail.technical.is_none());
        assert!(detail.collections.is_empty());
    }
}
