//! Diesel row structs and conversions to/from the domain models.
//!
//! Domain types in `crate::models` carry zero diesel derives. These row
//! structs mirror the DB columns exactly; the `TryFrom`/`From` impls below are
//! the single place where enum↔TEXT, JSON `genres`/`config`, `hdr`, and the
//! `watched`/`enabled` integer↔bool conversions happen. The domain `MediaItem`
//! also carries transient fields (`series_poster_path`, `playback_position_ms`,
//! `watched`, `library_section_id`) that are not columns; they default on read.

use diesel::prelude::*;

use crate::models::media::{HdrFormat, MediaItem, MediaType, SourceType};
use crate::models::source::{Source, SourceConfig};
use crate::models::watch::WatchProgress;
use crate::schema::{media_items, sources, watch_progress};

use super::DbError;

// ---------------------------------------------------------------------------
// media_items
// ---------------------------------------------------------------------------

/// Read projection for `media_items`. Field order mirrors `schema.rs` for
/// positional `Queryable` mapping.
#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = media_items)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct MediaItemRow {
    pub id: String,
    pub source_type: String,
    pub source_id: String,
    pub external_id: String,
    pub media_type: String,
    pub title: String,
    pub year: Option<i32>,
    pub overview: Option<String>,
    pub content_rating: Option<String>,
    pub rating: Option<f64>,
    pub runtime_minutes: Option<i32>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub genres: String,
    pub parent_id: Option<String>,
    pub season_number: Option<i32>,
    pub episode_number: Option<i32>,
    pub air_date: Option<String>,
    pub file_path: Option<String>,
    pub video_resolution: Option<String>,
    pub hdr: Option<String>,
    pub added_at: String,
    pub updated_at: String,
}

/// Insert projection for `media_items`.
#[derive(Debug, Insertable)]
#[diesel(table_name = media_items)]
pub struct NewMediaItemRow {
    pub id: String,
    pub source_type: String,
    pub source_id: String,
    pub external_id: String,
    pub media_type: String,
    pub title: String,
    pub year: Option<i32>,
    pub overview: Option<String>,
    pub content_rating: Option<String>,
    pub rating: Option<f64>,
    pub runtime_minutes: Option<i32>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub genres: String,
    pub parent_id: Option<String>,
    pub season_number: Option<i32>,
    pub episode_number: Option<i32>,
    pub air_date: Option<String>,
    pub file_path: Option<String>,
    pub video_resolution: Option<String>,
    pub hdr: Option<String>,
    pub added_at: String,
    pub updated_at: String,
}

impl TryFrom<MediaItemRow> for MediaItem {
    type Error = DbError;

    fn try_from(r: MediaItemRow) -> Result<Self, DbError> {
        let source_type = SourceType::from_str(&r.source_type)
            .ok_or_else(|| DbError::Data(format!("Unknown source type: {}", r.source_type)))?;
        let media_type = MediaType::from_str(&r.media_type)
            .ok_or_else(|| DbError::Data(format!("Unknown media type: {}", r.media_type)))?;
        let genres: Vec<String> = serde_json::from_str(&r.genres)
            .map_err(|e| DbError::Data(format!("Failed to parse genres: {e}")))?;
        let hdr = r.hdr.as_deref().and_then(HdrFormat::from_db_str);

        Ok(MediaItem {
            id: r.id,
            source_type,
            source_id: r.source_id,
            external_id: r.external_id,
            media_type,
            title: r.title,
            year: r.year,
            overview: r.overview,
            content_rating: r.content_rating,
            rating: r.rating,
            runtime_minutes: r.runtime_minutes,
            poster_path: r.poster_path,
            // Transient: never persisted.
            series_poster_path: None,
            backdrop_path: r.backdrop_path,
            genres,
            parent_id: r.parent_id,
            season_number: r.season_number,
            episode_number: r.episode_number,
            air_date: r.air_date,
            file_path: r.file_path,
            video_resolution: r.video_resolution,
            hdr,
            added_at: r.added_at,
            updated_at: r.updated_at,
            // Transient: filled from the source at runtime, not the DB.
            playback_position_ms: None,
            watched: false,
            library_section_id: None,
        })
    }
}

impl TryFrom<&MediaItem> for NewMediaItemRow {
    type Error = DbError;

    fn try_from(m: &MediaItem) -> Result<Self, DbError> {
        let genres = serde_json::to_string(&m.genres)
            .map_err(|e| DbError::Data(format!("Failed to serialize genres: {e}")))?;

        Ok(NewMediaItemRow {
            id: m.id.clone(),
            source_type: m.source_type.as_str().to_string(),
            source_id: m.source_id.clone(),
            external_id: m.external_id.clone(),
            media_type: m.media_type.as_str().to_string(),
            title: m.title.clone(),
            year: m.year,
            overview: m.overview.clone(),
            content_rating: m.content_rating.clone(),
            rating: m.rating,
            runtime_minutes: m.runtime_minutes,
            poster_path: m.poster_path.clone(),
            backdrop_path: m.backdrop_path.clone(),
            genres,
            parent_id: m.parent_id.clone(),
            season_number: m.season_number,
            episode_number: m.episode_number,
            air_date: m.air_date.clone(),
            file_path: m.file_path.clone(),
            video_resolution: m.video_resolution.clone(),
            hdr: m.hdr.map(|h| h.as_str().to_string()),
            added_at: m.added_at.clone(),
            updated_at: m.updated_at.clone(),
        })
    }
}

// ---------------------------------------------------------------------------
// sources
// ---------------------------------------------------------------------------

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = sources)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct SourceRow {
    pub id: String,
    pub source_type: String,
    pub name: String,
    pub config: String,
    pub enabled: i32,
    pub last_synced_at: Option<String>,
}

/// Insert + update projection for `sources`. `treat_none_as_null` so clearing
/// `last_synced_at` writes NULL (matching the legacy unconditional UPDATE)
/// rather than skipping the column.
#[derive(Debug, Insertable, AsChangeset)]
#[diesel(table_name = sources, treat_none_as_null = true)]
pub struct NewSourceRow {
    pub id: String,
    pub source_type: String,
    pub name: String,
    pub config: String,
    pub enabled: i32,
    pub last_synced_at: Option<String>,
}

impl TryFrom<SourceRow> for Source {
    type Error = DbError;

    fn try_from(r: SourceRow) -> Result<Self, DbError> {
        let source_type = SourceType::from_str(&r.source_type)
            .ok_or_else(|| DbError::Data(format!("Unknown source type: {}", r.source_type)))?;
        let config: SourceConfig = serde_json::from_str(&r.config)
            .map_err(|e| DbError::Data(format!("Failed to parse config: {e}")))?;

        Ok(Source {
            id: r.id,
            source_type,
            name: r.name,
            config,
            enabled: r.enabled != 0,
            last_synced_at: r.last_synced_at,
        })
    }
}

impl TryFrom<&Source> for NewSourceRow {
    type Error = DbError;

    fn try_from(s: &Source) -> Result<Self, DbError> {
        let config = serde_json::to_string(&s.config)
            .map_err(|e| DbError::Data(format!("Failed to serialize config: {e}")))?;

        Ok(NewSourceRow {
            id: s.id.clone(),
            source_type: s.source_type.as_str().to_string(),
            name: s.name.clone(),
            config,
            enabled: s.enabled as i32,
            last_synced_at: s.last_synced_at.clone(),
        })
    }
}

// ---------------------------------------------------------------------------
// watch_progress
// ---------------------------------------------------------------------------

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = watch_progress)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct WatchProgressRow {
    pub media_item_id: String,
    pub position_seconds: f64,
    pub duration_seconds: f64,
    pub watched: i32,
    pub last_watched_at: String,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = watch_progress)]
pub struct NewWatchProgressRow {
    pub media_item_id: String,
    pub position_seconds: f64,
    pub duration_seconds: f64,
    pub watched: i32,
    pub last_watched_at: String,
}

impl From<WatchProgressRow> for WatchProgress {
    fn from(r: WatchProgressRow) -> Self {
        WatchProgress {
            media_item_id: r.media_item_id,
            position_seconds: r.position_seconds,
            duration_seconds: r.duration_seconds,
            watched: r.watched != 0,
            last_watched_at: r.last_watched_at,
        }
    }
}

impl From<&WatchProgress> for NewWatchProgressRow {
    fn from(w: &WatchProgress) -> Self {
        NewWatchProgressRow {
            media_item_id: w.media_item_id.clone(),
            position_seconds: w.position_seconds,
            duration_seconds: w.duration_seconds,
            watched: w.watched as i32,
            last_watched_at: w.last_watched_at.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Map an insert row back into a read row (same column values) so a
    /// conversion round-trip can be exercised without a database.
    fn read_row(n: NewMediaItemRow) -> MediaItemRow {
        MediaItemRow {
            id: n.id,
            source_type: n.source_type,
            source_id: n.source_id,
            external_id: n.external_id,
            media_type: n.media_type,
            title: n.title,
            year: n.year,
            overview: n.overview,
            content_rating: n.content_rating,
            rating: n.rating,
            runtime_minutes: n.runtime_minutes,
            poster_path: n.poster_path,
            backdrop_path: n.backdrop_path,
            genres: n.genres,
            parent_id: n.parent_id,
            season_number: n.season_number,
            episode_number: n.episode_number,
            air_date: n.air_date,
            file_path: n.file_path,
            video_resolution: n.video_resolution,
            hdr: n.hdr,
            added_at: n.added_at,
            updated_at: n.updated_at,
        }
    }

    fn movie() -> MediaItem {
        MediaItem {
            id: "plex:s:123".into(),
            source_type: SourceType::Plex,
            source_id: "s".into(),
            external_id: "123".into(),
            media_type: MediaType::Movie,
            title: "Dune".into(),
            year: Some(2021),
            overview: Some("desc".into()),
            content_rating: Some("PG-13".into()),
            rating: Some(8.5),
            runtime_minutes: Some(155),
            poster_path: Some("/p".into()),
            series_poster_path: Some("transient".into()),
            backdrop_path: Some("/b".into()),
            genres: vec!["Sci-Fi".into(), "Adventure".into()],
            parent_id: None,
            season_number: None,
            episode_number: None,
            air_date: None,
            file_path: Some("/f.mkv".into()),
            video_resolution: Some("4k".into()),
            hdr: Some(HdrFormat::DolbyVision),
            added_at: "2024-01-15".into(),
            updated_at: "2024-01-16".into(),
            playback_position_ms: Some(99_000),
            watched: true,
            library_section_id: Some("1".into()),
        }
    }

    #[test]
    fn media_item_roundtrip_preserves_persisted_fields() {
        let m = movie();
        let row = read_row(NewMediaItemRow::try_from(&m).unwrap());
        let back = MediaItem::try_from(row).unwrap();

        assert_eq!(back.id, m.id);
        assert_eq!(back.source_type, m.source_type);
        assert_eq!(back.media_type, m.media_type);
        assert_eq!(back.title, m.title);
        assert_eq!(back.year, m.year);
        assert_eq!(back.rating, m.rating);
        assert_eq!(back.genres, m.genres);
        assert_eq!(back.video_resolution, m.video_resolution);
        assert_eq!(back.hdr, m.hdr);
        assert_eq!(back.added_at, m.added_at);
        assert_eq!(back.updated_at, m.updated_at);
    }

    #[test]
    fn media_item_roundtrip_defaults_transient_fields() {
        let m = movie(); // has transient fields set
        let row = read_row(NewMediaItemRow::try_from(&m).unwrap());
        let back = MediaItem::try_from(row).unwrap();

        // Transient fields are not columns; they reset to defaults on read.
        assert_eq!(back.series_poster_path, None);
        assert_eq!(back.playback_position_ms, None);
        assert!(!back.watched);
        assert_eq!(back.library_section_id, None);
    }

    #[test]
    fn empty_genres_roundtrip() {
        let mut m = movie();
        m.genres = vec![];
        let new = NewMediaItemRow::try_from(&m).unwrap();
        assert_eq!(new.genres, "[]");
        let back = MediaItem::try_from(read_row(new)).unwrap();
        assert!(back.genres.is_empty());
    }

    #[test]
    fn hdr_variants_roundtrip() {
        for hdr in [None, Some(HdrFormat::Hdr), Some(HdrFormat::DolbyVision)] {
            let mut m = movie();
            m.hdr = hdr;
            let back = MediaItem::try_from(read_row(NewMediaItemRow::try_from(&m).unwrap())).unwrap();
            assert_eq!(back.hdr, hdr);
        }
    }

    #[test]
    fn unknown_media_type_is_data_error() {
        let m = movie();
        let mut row = read_row(NewMediaItemRow::try_from(&m).unwrap());
        row.media_type = "bogus".into();
        let err = MediaItem::try_from(row).unwrap_err();
        assert!(matches!(err, DbError::Data(_)));
    }

    #[test]
    fn source_config_json_roundtrip() {
        let source = Source {
            id: "plex:http://s".into(),
            source_type: SourceType::Plex,
            name: "My Server".into(),
            config: SourceConfig {
                url: "http://s:32400".into(),
                token: "tok".into(),
            },
            enabled: true,
            last_synced_at: Some("2024-03-14T10:00:00Z".into()),
        };
        let new = NewSourceRow::try_from(&source).unwrap();
        let row = SourceRow {
            id: new.id,
            source_type: new.source_type,
            name: new.name,
            config: new.config,
            enabled: new.enabled,
            last_synced_at: new.last_synced_at,
        };
        let back = Source::try_from(row).unwrap();
        assert_eq!(back.config, source.config);
        assert!(back.enabled);
        assert_eq!(back.last_synced_at, source.last_synced_at);
    }

    #[test]
    fn watch_progress_watched_bool_roundtrip() {
        for watched in [false, true] {
            let wp = WatchProgress {
                media_item_id: "m1".into(),
                position_seconds: 6479.5,
                duration_seconds: 7200.0,
                watched,
                last_watched_at: "2026-03-14T12:00:00Z".into(),
            };
            let new = NewWatchProgressRow::from(&wp);
            assert_eq!(new.watched, watched as i32);
            let row = WatchProgressRow {
                media_item_id: new.media_item_id,
                position_seconds: new.position_seconds,
                duration_seconds: new.duration_seconds,
                watched: new.watched,
                last_watched_at: new.last_watched_at,
            };
            let back = WatchProgress::from(row);
            assert_eq!(back.watched, watched);
            assert_eq!(back.position_seconds, 6479.5);
        }
    }
}
