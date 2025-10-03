use crate::models::{
    Episode, MediaItem, Movie, MusicAlbum, MusicTrack, Person, Photo, Season, Show,
};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "media_items")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub library_id: String,
    pub source_id: String,
    pub media_type: String, // 'movie', 'show', 'episode', 'album', 'track', 'photo'
    pub title: String,
    pub sort_title: Option<String>,
    pub year: Option<i32>,
    pub duration_ms: Option<i64>,
    pub rating: Option<f32>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub overview: Option<String>,
    #[sea_orm(column_type = "Json")]
    pub genres: Option<Json>,
    pub added_at: Option<DateTime>,
    pub updated_at: DateTime,
    #[sea_orm(column_type = "Json")]
    pub metadata: Option<Json>, // Store type-specific fields
    pub parent_id: Option<String>,   // For episodes: ID of parent show
    pub season_number: Option<i32>,  // For episodes: season number
    pub episode_number: Option<i32>, // For episodes: episode number
    pub intro_marker_start_ms: Option<i64>,
    pub intro_marker_end_ms: Option<i64>,
    pub credits_marker_start_ms: Option<i64>,
    pub credits_marker_end_ms: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::libraries::Entity",
        from = "Column::LibraryId",
        to = "super::libraries::Column::Id"
    )]
    Library,
    #[sea_orm(
        belongs_to = "super::sources::Entity",
        from = "Column::SourceId",
        to = "super::sources::Column::Id"
    )]
    Source,
    #[sea_orm(has_many = "super::playback_progress::Entity")]
    PlaybackProgresses,
    #[sea_orm(has_many = "super::offline_content::Entity")]
    OfflineContents,
    #[sea_orm(
        belongs_to = "Entity",
        from = "Column::ParentId",
        to = "Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    ParentShow,
    #[sea_orm(has_many = "Entity")]
    Episodes,
}

impl Related<super::libraries::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Library.def()
    }
}

impl Related<super::sources::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Source.def()
    }
}

impl Related<super::playback_progress::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PlaybackProgresses.def()
    }
}

impl Related<super::offline_content::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::OfflineContents.def()
    }
}

// Self-referential relation for episodes -> parent show
impl Related<Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ParentShow.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

// Media type enum for type safety
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MediaType {
    Movie,
    Show,
    Episode,
    Album,
    Track,
    Photo,
}

impl MediaType {
    pub fn as_str(&self) -> &'static str {
        match self {
            MediaType::Movie => "movie",
            MediaType::Show => "show",
            MediaType::Episode => "episode",
            MediaType::Album => "album",
            MediaType::Track => "track",
            MediaType::Photo => "photo",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "movie" => Some(MediaType::Movie),
            "show" => Some(MediaType::Show),
            "episode" => Some(MediaType::Episode),
            "album" => Some(MediaType::Album),
            "track" => Some(MediaType::Track),
            "photo" => Some(MediaType::Photo),
            _ => None,
        }
    }
}

impl Model {
    pub fn get_media_type(&self) -> Option<MediaType> {
        MediaType::from_str(&self.media_type)
    }

    pub fn get_genres(&self) -> Vec<String> {
        self.genres
            .as_ref()
            .and_then(|j| serde_json::from_value::<Vec<String>>(j.clone()).ok())
            .unwrap_or_default()
    }

    pub fn get_metadata<T: for<'de> Deserialize<'de>>(&self) -> Option<T> {
        self.metadata
            .as_ref()
            .and_then(|j| serde_json::from_value::<T>(j.clone()).ok())
    }

    /// Check if this item is an episode
    pub fn is_episode(&self) -> bool {
        self.media_type == "episode"
    }

    /// Check if this item is a show
    pub fn is_show(&self) -> bool {
        self.media_type == "show"
    }

    /// Get episode info if this is an episode
    pub fn get_episode_info(&self) -> Option<(i32, i32)> {
        if self.is_episode() {
            match (self.season_number, self.episode_number) {
                (Some(s), Some(e)) => Some((s, e)),
                _ => None,
            }
        } else {
            None
        }
    }
}

/// Convert database Model to domain MediaItem
impl TryFrom<Model> for MediaItem {
    type Error = anyhow::Error;

    fn try_from(model: Model) -> Result<Self, Self::Error> {
        // Parse metadata JSON if available
        let metadata = model
            .metadata
            .as_ref()
            .and_then(|json| serde_json::from_value::<serde_json::Value>(json.clone()).ok())
            .unwrap_or_else(|| serde_json::json!({}));

        let genres = model.get_genres();
        let duration = model
            .duration_ms
            .map(|ms| Duration::from_millis(ms as u64))
            .unwrap_or_default();

        match model.media_type.as_str() {
            "movie" => {
                // Extract movie-specific fields from metadata
                let cast = metadata
                    .get("cast")
                    .and_then(|v| serde_json::from_value::<Vec<Person>>(v.clone()).ok())
                    .unwrap_or_default();
                let crew = metadata
                    .get("crew")
                    .and_then(|v| serde_json::from_value::<Vec<Person>>(v.clone()).ok())
                    .unwrap_or_default();
                let watched = metadata
                    .get("watched")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let view_count = metadata
                    .get("view_count")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as u32)
                    .unwrap_or(0);
                let last_watched_at = metadata
                    .get("last_watched_at")
                    .and_then(|v| v.as_str())
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc));
                let playback_position = metadata
                    .get("playback_position_ms")
                    .and_then(|v| v.as_u64())
                    .map(Duration::from_millis);

                // Deserialize intro marker if both start and end are present
                let intro_marker = match (model.intro_marker_start_ms, model.intro_marker_end_ms) {
                    (Some(start_ms), Some(end_ms)) => Some(crate::models::ChapterMarker {
                        start_time: Duration::from_millis(start_ms as u64),
                        end_time: Duration::from_millis(end_ms as u64),
                        marker_type: crate::models::ChapterType::Intro,
                    }),
                    _ => None,
                };

                // Deserialize credits marker if both start and end are present
                let credits_marker =
                    match (model.credits_marker_start_ms, model.credits_marker_end_ms) {
                        (Some(start_ms), Some(end_ms)) => Some(crate::models::ChapterMarker {
                            start_time: Duration::from_millis(start_ms as u64),
                            end_time: Duration::from_millis(end_ms as u64),
                            marker_type: crate::models::ChapterType::Credits,
                        }),
                        _ => None,
                    };

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
                    added_at: model.added_at.map(|dt| dt.and_utc()),
                    updated_at: Some(model.updated_at.and_utc()),
                    watched,
                    view_count,
                    last_watched_at,
                    playback_position,
                    intro_marker,
                    credits_marker,
                }))
            }
            "show" => {
                let seasons = metadata
                    .get("seasons")
                    .and_then(|v| serde_json::from_value::<Vec<Season>>(v.clone()).ok())
                    .unwrap_or_default();
                let cast = metadata
                    .get("cast")
                    .and_then(|v| serde_json::from_value::<Vec<Person>>(v.clone()).ok())
                    .unwrap_or_default();
                let watched_episode_count = metadata
                    .get("watched_episode_count")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as u32)
                    .unwrap_or(0);
                let total_episode_count = metadata
                    .get("total_episode_count")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as u32)
                    .unwrap_or(0);
                let last_watched_at = metadata
                    .get("last_watched_at")
                    .and_then(|v| v.as_str())
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc));

                Ok(MediaItem::Show(Show {
                    id: model.id.clone(),
                    backend_id: model.source_id.clone(),
                    title: model.title.clone(),
                    year: model.year.map(|y| y as u32),
                    seasons,
                    rating: model.rating,
                    poster_url: model.poster_url.clone(),
                    backdrop_url: model.backdrop_url.clone(),
                    overview: model.overview.clone(),
                    genres,
                    cast,
                    added_at: model.added_at.map(|dt| dt.and_utc()),
                    updated_at: Some(model.updated_at.and_utc()),
                    watched_episode_count,
                    total_episode_count,
                    last_watched_at,
                }))
            }
            "episode" => {
                let show_id = model.parent_id.clone();
                let show_title = metadata
                    .get("show_title")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let show_poster_url = metadata
                    .get("show_poster_url")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let watched = metadata
                    .get("watched")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let view_count = metadata
                    .get("view_count")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as u32)
                    .unwrap_or(0);
                let last_watched_at = metadata
                    .get("last_watched_at")
                    .and_then(|v| v.as_str())
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc));
                let playback_position = metadata
                    .get("playback_position_ms")
                    .and_then(|v| v.as_u64())
                    .map(Duration::from_millis);
                let air_date = metadata
                    .get("air_date")
                    .and_then(|v| v.as_str())
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc));

                // Deserialize intro marker if both start and end are present
                let intro_marker = match (model.intro_marker_start_ms, model.intro_marker_end_ms) {
                    (Some(start_ms), Some(end_ms)) => Some(crate::models::ChapterMarker {
                        start_time: Duration::from_millis(start_ms as u64),
                        end_time: Duration::from_millis(end_ms as u64),
                        marker_type: crate::models::ChapterType::Intro,
                    }),
                    _ => None,
                };

                // Deserialize credits marker if both start and end are present
                let credits_marker =
                    match (model.credits_marker_start_ms, model.credits_marker_end_ms) {
                        (Some(start_ms), Some(end_ms)) => Some(crate::models::ChapterMarker {
                            start_time: Duration::from_millis(start_ms as u64),
                            end_time: Duration::from_millis(end_ms as u64),
                            marker_type: crate::models::ChapterType::Credits,
                        }),
                        _ => None,
                    };

                Ok(MediaItem::Episode(Episode {
                    id: model.id.clone(),
                    backend_id: model.source_id.clone(),
                    show_id,
                    title: model.title.clone(),
                    season_number: model.season_number.unwrap_or(0) as u32,
                    episode_number: model.episode_number.unwrap_or(0) as u32,
                    duration,
                    thumbnail_url: model.poster_url.clone(),
                    overview: model.overview.clone(),
                    air_date,
                    watched,
                    view_count,
                    last_watched_at,
                    playback_position,
                    show_title,
                    show_poster_url,
                    intro_marker,
                    credits_marker,
                }))
            }
            "album" => {
                let artist = metadata
                    .get("artist")
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .unwrap_or_default();
                let track_count = metadata
                    .get("track_count")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as u32)
                    .unwrap_or(0);
                // Album duration is the sum of all tracks, stored if available
                let album_duration = metadata
                    .get("total_duration_ms")
                    .and_then(|v| v.as_u64())
                    .map(Duration::from_millis)
                    .unwrap_or(duration);

                Ok(MediaItem::MusicAlbum(MusicAlbum {
                    id: model.id.clone(),
                    title: model.title.clone(),
                    artist,
                    year: model.year.map(|y| y as u32),
                    track_count,
                    duration: album_duration,
                    cover_url: model.poster_url.clone(),
                    genres,
                }))
            }
            "track" => {
                let artist = metadata
                    .get("artist")
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .unwrap_or_default();
                let album = metadata
                    .get("album")
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .unwrap_or_default();
                let track_number = metadata
                    .get("track_number")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as u32);

                Ok(MediaItem::MusicTrack(MusicTrack {
                    id: model.id.clone(),
                    title: model.title.clone(),
                    artist,
                    album,
                    track_number,
                    duration,
                    cover_url: model.poster_url.clone(),
                }))
            }
            "photo" => {
                let date_taken = metadata
                    .get("date_taken")
                    .and_then(|v| v.as_str())
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc));

                Ok(MediaItem::Photo(Photo {
                    id: model.id.clone(),
                    title: model.title.clone(),
                    date_taken,
                    thumbnail_url: model.poster_url.clone(),
                    full_url: model.backdrop_url.clone(),
                }))
            }
            _ => Err(anyhow::anyhow!("Unknown media type: {}", model.media_type)),
        }
    }
}

// Note: We retain full cache key in domain model IDs to ensure
// uniqueness across backends/libraries and to match event payload IDs.
