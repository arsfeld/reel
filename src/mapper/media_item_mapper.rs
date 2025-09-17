//! MediaItem mapping implementations

use crate::db::entities::media_items::Model as MediaItemModel;
use crate::mapper::traits::{TryMapper, map_option, try_map_option};
use crate::models::{
    Episode, MediaItem, Movie, MusicAlbum, MusicTrack, Person, Photo, Season, Show,
};
use anyhow::{Result, anyhow};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use std::time::Duration;

/// Custom field transformers for MediaItem
pub struct DurationTransformer;

impl DurationTransformer {
    /// Convert milliseconds to Duration
    pub fn from_millis(ms: Option<i64>) -> Duration {
        ms.map(|ms| Duration::from_millis(ms as u64))
            .unwrap_or_default()
    }

    /// Convert Duration to milliseconds
    pub fn to_millis(duration: Duration) -> i64 {
        duration.as_millis() as i64
    }
}

pub struct DateTimeTransformer;

impl DateTimeTransformer {
    /// Parse datetime from RFC3339 string
    pub fn from_rfc3339(s: Option<&str>) -> Option<DateTime<Utc>> {
        s.and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
    }

    /// Convert NaiveDateTime to DateTime<Utc>
    pub fn from_naive(dt: Option<chrono::NaiveDateTime>) -> Option<DateTime<Utc>> {
        dt.map(|dt| dt.and_utc())
    }
}

pub struct JsonTransformer;

impl JsonTransformer {
    /// Extract and deserialize a field from JSON metadata
    pub fn extract<T: serde::de::DeserializeOwned>(
        metadata: &Option<serde_json::Value>,
        field: &str,
    ) -> Option<T> {
        metadata
            .as_ref()
            .and_then(|json| json.get(field))
            .and_then(|v| serde_json::from_value::<T>(v.clone()).ok())
    }

    /// Extract genres from database JSON
    pub fn extract_genres(genres: &Option<serde_json::Value>) -> Vec<String> {
        genres
            .as_ref()
            .and_then(|v| serde_json::from_value::<Vec<String>>(v.clone()).ok())
            .unwrap_or_default()
    }
}

/// Implementation for MediaItemModel to MediaItem conversion
// Note: TryFrom is already implemented in src/db/entities/media_items.rs
// This module provides the to_model() method for the reverse conversion
/*
impl TryFrom<MediaItemModel> for MediaItem {
    type Error = anyhow::Error;

    fn try_from(model: MediaItemModel) -> Result<Self, Self::Error> {
        // Parse metadata JSON if available
        let metadata = model
            .metadata
            .as_ref()
            .and_then(|json| serde_json::from_value::<serde_json::Value>(json.clone()).ok());

        let genres = JsonTransformer::extract_genres(&model.genres);
        let duration = DurationTransformer::from_millis(model.duration_ms);

        match model.media_type.as_str() {
            "movie" => {
                let cast = JsonTransformer::extract::<Vec<Person>>(&metadata, "cast")
                    .unwrap_or_default();
                let crew = JsonTransformer::extract::<Vec<Person>>(&metadata, "crew")
                    .unwrap_or_default();
                let watched = JsonTransformer::extract::<bool>(&metadata, "watched")
                    .unwrap_or(false);
                let view_count = JsonTransformer::extract::<u32>(&metadata, "view_count")
                    .unwrap_or(0);
                let last_watched_at = metadata.as_ref()
                    .and_then(|m| m.get("last_watched_at"))
                    .and_then(|v| v.as_str())
                    .and_then(|s| DateTimeTransformer::from_rfc3339(Some(s)));
                let playback_position = JsonTransformer::extract::<u64>(&metadata, "playback_position_ms")
                    .map(Duration::from_millis);

                Ok(MediaItem::Movie(Movie {
                    id: model.id.clone(),
                    backend_id: model.source_id.clone(),
                    title: model.title.clone(),
                    year: model.year.map(|y| y as u32),
                    duration,
                    rating: model.rating,
                    poster_url: model.poster_url.clone(),
                    backdrop_url: model.backdrop_url.clone(),
                    overview: model.overview.clone(),
                    genres,
                    cast,
                    crew,
                    added_at: DateTimeTransformer::from_naive(model.added_at),
                    updated_at: Some(model.updated_at.and_utc()),
                    watched,
                    view_count,
                    last_watched_at,
                    playback_position,
                    intro_marker: None,
                    credits_marker: None,
                }))
            }
            "show" => {
                let seasons = JsonTransformer::extract::<Vec<Season>>(&metadata, "seasons")
                    .unwrap_or_default();
                let cast = JsonTransformer::extract::<Vec<Person>>(&metadata, "cast")
                    .unwrap_or_default();
                let watched_episode_count = JsonTransformer::extract::<u32>(&metadata, "watched_episode_count")
                    .unwrap_or(0);
                let total_episode_count = JsonTransformer::extract::<u32>(&metadata, "total_episode_count")
                    .unwrap_or(0);
                let last_watched_at = metadata.as_ref()
                    .and_then(|m| m.get("last_watched_at"))
                    .and_then(|v| v.as_str())
                    .and_then(|s| DateTimeTransformer::from_rfc3339(Some(s)));

                Ok(MediaItem::Show(Show {
                    id: model.id.clone(),
                    backend_id: model.source_id.clone(),
                    title: model.title.clone(),
                    year: model.year.map(|y| y as u32),
                    rating: model.rating,
                    poster_url: model.poster_url.clone(),
                    backdrop_url: model.backdrop_url.clone(),
                    overview: model.overview.clone(),
                    genres,
                    seasons,
                    cast,
                    added_at: DateTimeTransformer::from_naive(model.added_at),
                    updated_at: Some(model.updated_at.and_utc()),
                    watched_episode_count,
                    total_episode_count,
                    last_watched_at,
                }))
            }
            "episode" => {
                let watched = JsonTransformer::extract::<bool>(&metadata, "watched")
                    .unwrap_or(false);
                let view_count = JsonTransformer::extract::<u32>(&metadata, "view_count")
                    .unwrap_or(0);
                let last_watched_at = metadata.as_ref()
                    .and_then(|m| m.get("last_watched_at"))
                    .and_then(|v| v.as_str())
                    .and_then(|s| DateTimeTransformer::from_rfc3339(Some(s)));
                let playback_position = JsonTransformer::extract::<u64>(&metadata, "playback_position_ms")
                    .map(Duration::from_millis);
                let air_date = metadata.as_ref()
                    .and_then(|m| m.get("air_date"))
                    .and_then(|v| v.as_str())
                    .and_then(|s| DateTimeTransformer::from_rfc3339(Some(s)));
                let show_title = JsonTransformer::extract::<String>(&metadata, "show_title");
                let show_poster_url = JsonTransformer::extract::<String>(&metadata, "show_poster_url");

                Ok(MediaItem::Episode(Episode {
                    id: model.id.clone(),
                    backend_id: model.source_id.clone(),
                    show_id: model.parent_id.clone(),
                    title: model.title.clone(),
                    season_number: model.season_number.unwrap_or(0) as u32,
                    episode_number: model.episode_number.unwrap_or(0) as u32,
                    overview: model.overview.clone(),
                    thumbnail_url: model.poster_url.clone(),
                    duration,
                    air_date,
                    watched,
                    view_count,
                    last_watched_at,
                    playback_position,
                    show_title,
                    show_poster_url,
                    intro_marker: None,
                    credits_marker: None,
                }))
            }
            "album" => Ok(MediaItem::MusicAlbum(MusicAlbum {
                id: model.id.clone(),
                title: model.title.clone(),
                artist: JsonTransformer::extract::<String>(&metadata, "artist")
                    .unwrap_or_else(|| "Unknown Artist".to_string()),
                year: model.year.map(|y| y as u32),
                cover_url: model.poster_url.clone(),
                track_count: JsonTransformer::extract::<u32>(&metadata, "track_count")
                    .unwrap_or(0),
                duration: duration,
                genres,
            })),
            "track" => Ok(MediaItem::MusicTrack(MusicTrack {
                id: model.id.clone(),
                title: model.title.clone(),
                artist: JsonTransformer::extract::<String>(&metadata, "artist")
                    .unwrap_or_else(|| "Unknown Artist".to_string()),
                album: JsonTransformer::extract::<String>(&metadata, "album")
                    .unwrap_or_else(|| "Unknown Album".to_string()),
                track_number: JsonTransformer::extract::<u32>(&metadata, "track_number"),
                duration,
                cover_url: model.poster_url.clone(),
            })),
            "photo" => Ok(MediaItem::Photo(Photo {
                id: model.id.clone(),
                title: model.title.clone(),
                thumbnail_url: model.poster_url.clone(),
                full_url: model.backdrop_url.clone(),
                date_taken: metadata.as_ref()
                    .and_then(|m| m.get("date_taken"))
                    .and_then(|v| v.as_str())
                    .and_then(|s| DateTimeTransformer::from_rfc3339(Some(s))),
                width: JsonTransformer::extract::<u32>(&metadata, "width"),
                height: JsonTransformer::extract::<u32>(&metadata, "height"),
                location: JsonTransformer::extract::<String>(&metadata, "location"),
            })),
            _ => Err(anyhow!("Unknown media type: {}", model.media_type)),
        }
    }
}
*/

/// Implementation for MediaItem to MediaItemModel conversion
impl MediaItem {
    /// Convert MediaItem to MediaItemModel for database storage
    pub fn to_model(&self, source_id: &str, library_id: Option<String>) -> MediaItemModel {
        let (
            title,
            year,
            duration_ms,
            rating,
            poster_url,
            backdrop_url,
            overview,
            genres,
            media_type,
        ) = match self {
            MediaItem::Movie(movie) => (
                movie.title.clone(),
                movie.year.map(|y| y as i32),
                Some(DurationTransformer::to_millis(movie.duration)),
                movie.rating,
                movie.poster_url.clone(),
                movie.backdrop_url.clone(),
                movie.overview.clone(),
                if movie.genres.is_empty() {
                    None
                } else {
                    serde_json::to_value(&movie.genres).ok()
                },
                "movie".to_string(),
            ),
            MediaItem::Show(show) => (
                show.title.clone(),
                show.year.map(|y| y as i32),
                None,
                show.rating,
                show.poster_url.clone(),
                show.backdrop_url.clone(),
                show.overview.clone(),
                if show.genres.is_empty() {
                    None
                } else {
                    serde_json::to_value(&show.genres).ok()
                },
                "show".to_string(),
            ),
            MediaItem::Episode(episode) => (
                episode.title.clone(),
                None,
                Some(DurationTransformer::to_millis(episode.duration)),
                None,
                episode.thumbnail_url.clone(),
                episode.thumbnail_url.clone(),
                episode.overview.clone(),
                None,
                "episode".to_string(),
            ),
            MediaItem::MusicAlbum(album) => (
                album.title.clone(),
                album.year.map(|y| y as i32),
                None,
                None,
                album.cover_url.clone(),
                None,
                None,
                if album.genres.is_empty() {
                    None
                } else {
                    serde_json::to_value(&album.genres).ok()
                },
                "album".to_string(),
            ),
            MediaItem::MusicTrack(track) => (
                track.title.clone(),
                None,
                Some(DurationTransformer::to_millis(track.duration)),
                None,
                track.cover_url.clone(),
                None,
                None,
                None,
                "track".to_string(),
            ),
            MediaItem::Photo(photo) => (
                photo.title.clone(),
                None,
                None,
                None,
                photo.thumbnail_url.clone(),
                photo.full_url.clone(),
                None,
                None,
                "photo".to_string(),
            ),
        };

        // Extract parent show ID for episodes
        let parent_id = match self {
            MediaItem::Episode(episode) => episode.show_id.clone(),
            // MusicTrack doesn't have album_id field in current model
            _ => None,
        };

        // Extract season and episode numbers
        let (season_number, episode_number) = match self {
            MediaItem::Episode(episode) => (
                Some(episode.season_number as i32),
                Some(episode.episode_number as i32),
            ),
            _ => (None, None),
        };

        MediaItemModel {
            id: self.id().to_string(),
            source_id: source_id.to_string(),
            library_id: library_id.unwrap_or_default(),
            title: title.clone(),
            year,
            media_type,
            duration_ms,
            rating,
            poster_url,
            backdrop_url,
            overview,
            genres,
            parent_id,
            season_number,
            episode_number,
            sort_title: Some(title),
            added_at: Some(chrono::Utc::now().naive_utc()),
            updated_at: chrono::Utc::now().naive_utc(),
            metadata: serde_json::to_value(self).ok(),
        }
    }
}
