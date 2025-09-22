pub mod auth_provider;
pub mod connection;
mod identifiers;
pub mod playlist_context;

pub use auth_provider::{AuthProvider, ConnectionInfo, Source, SourceType};
pub use connection::{ServerConnection, ServerConnections};
pub use identifiers::{BackendId, LibraryId, MediaItemId, ShowId, SourceId};
pub use playlist_context::{EpisodeInfo, PlayQueueInfo, PlaylistContext, QueueItem};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Library {
    pub id: String,
    pub title: String,
    pub library_type: LibraryType,
    pub icon: Option<String>,
    pub item_count: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LibraryType {
    Movies,
    Shows,
    Music,
    Photos,
    Mixed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum MediaType {
    Movie,
    Show,
    Music,
    Photo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Movie {
    pub id: String,
    pub backend_id: String, // Which backend this movie came from
    pub title: String,
    pub year: Option<u32>,
    pub duration: Duration,
    pub rating: Option<f32>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub overview: Option<String>,
    pub genres: Vec<String>,
    pub cast: Vec<Person>,
    pub crew: Vec<Person>,
    pub added_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub watched: bool,
    pub view_count: u32,
    pub last_watched_at: Option<DateTime<Utc>>,
    pub playback_position: Option<Duration>,
    pub intro_marker: Option<ChapterMarker>, // Intro/opening credits marker
    pub credits_marker: Option<ChapterMarker>, // End credits marker
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Show {
    pub id: String,
    pub backend_id: String, // Which backend this show came from
    pub title: String,
    pub year: Option<u32>,
    pub seasons: Vec<Season>,
    pub rating: Option<f32>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub overview: Option<String>,
    pub genres: Vec<String>,
    pub cast: Vec<Person>,
    pub added_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub watched_episode_count: u32,
    pub total_episode_count: u32,
    pub last_watched_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Season {
    pub id: String,
    pub season_number: u32,
    pub episode_count: u32,
    pub poster_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    pub id: String,
    pub backend_id: String,      // Which backend this episode came from
    pub show_id: Option<String>, // Parent show ID
    pub title: String,
    pub season_number: u32,
    pub episode_number: u32,
    pub duration: Duration,
    pub thumbnail_url: Option<String>,
    pub overview: Option<String>,
    pub air_date: Option<DateTime<Utc>>,
    pub watched: bool,
    pub view_count: u32,
    pub last_watched_at: Option<DateTime<Utc>>,
    pub playback_position: Option<Duration>,
    pub show_title: Option<String>,            // Parent show name
    pub show_poster_url: Option<String>,       // Parent show poster URL
    pub intro_marker: Option<ChapterMarker>,   // Intro/opening credits marker
    pub credits_marker: Option<ChapterMarker>, // End credits marker
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterMarker {
    pub start_time: Duration,
    pub end_time: Duration,
    pub marker_type: ChapterType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChapterType {
    Intro,
    Credits,
    Recap,
    Preview,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Person {
    pub id: String,
    pub name: String,
    pub role: Option<String>,
    pub image_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamInfo {
    pub url: String,
    pub direct_play: bool,
    pub video_codec: String,
    pub audio_codec: String,
    pub container: String,
    pub bitrate: u64,
    pub resolution: Resolution,
    pub quality_options: Vec<QualityOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityOption {
    pub name: String,
    pub resolution: Resolution,
    pub bitrate: u64,
    pub url: String,
    pub requires_transcode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicAlbum {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub year: Option<u32>,
    pub track_count: u32,
    pub duration: Duration,
    pub cover_url: Option<String>,
    pub genres: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicTrack {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub track_number: Option<u32>,
    pub duration: Duration,
    pub cover_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Photo {
    pub id: String,
    pub title: String,
    pub date_taken: Option<DateTime<Utc>>,
    pub thumbnail_url: Option<String>,
    pub full_url: Option<String>,
}

/// Generic media item that can hold any type of media
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MediaItem {
    Movie(Movie),
    Show(Show),
    Episode(Episode),
    MusicAlbum(MusicAlbum),
    MusicTrack(MusicTrack),
    Photo(Photo),
}

/// Homepage section with a collection of media items
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomeSection {
    pub id: String,
    pub title: String,
    pub section_type: HomeSectionType,
    pub items: Vec<MediaItem>,
}

/// Homepage section with database models for UI display
#[derive(Debug, Clone)]
pub struct HomeSectionWithModels {
    pub id: String,
    pub title: String,
    pub section_type: HomeSectionType,
    pub items: Vec<crate::db::entities::MediaItemModel>,
}

/// Type of homepage section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HomeSectionType {
    RecentlyAdded,
    ContinueWatching,
    Suggested,
    TopRated,
    Trending,
    RecentlyPlayed,
    Custom(String),
}

impl MediaType {
    pub fn from_media_item(item: &MediaItem) -> Self {
        match item {
            MediaItem::Movie(_) => MediaType::Movie,
            MediaItem::Show(_) | MediaItem::Episode(_) => MediaType::Show,
            MediaItem::MusicAlbum(_) | MediaItem::MusicTrack(_) => MediaType::Music,
            MediaItem::Photo(_) => MediaType::Photo,
        }
    }
}

impl MediaItem {
    pub fn id(&self) -> &str {
        match self {
            MediaItem::Movie(m) => &m.id,
            MediaItem::Show(s) => &s.id,
            MediaItem::Episode(e) => &e.id,
            MediaItem::MusicAlbum(a) => &a.id,
            MediaItem::MusicTrack(t) => &t.id,
            MediaItem::Photo(p) => &p.id,
        }
    }

    pub fn backend_id(&self) -> &str {
        match self {
            MediaItem::Movie(m) => &m.backend_id,
            MediaItem::Show(s) => &s.backend_id,
            MediaItem::Episode(e) => &e.backend_id,
            MediaItem::MusicAlbum(_) => "", // TODO: Add backend_id to music/photo models
            MediaItem::MusicTrack(_) => "",
            MediaItem::Photo(_) => "",
        }
    }

    pub fn title(&self) -> &str {
        match self {
            MediaItem::Movie(m) => &m.title,
            MediaItem::Show(s) => &s.title,
            MediaItem::Episode(e) => &e.title,
            MediaItem::MusicAlbum(a) => &a.title,
            MediaItem::MusicTrack(t) => &t.title,
            MediaItem::Photo(p) => &p.title,
        }
    }

    pub fn is_watched(&self) -> bool {
        match self {
            MediaItem::Movie(m) => m.watched,
            MediaItem::Show(s) => {
                s.watched_episode_count > 0 && s.watched_episode_count == s.total_episode_count
            }
            MediaItem::Episode(e) => e.watched,
            _ => false,
        }
    }

    pub fn is_partially_watched(&self) -> bool {
        match self {
            MediaItem::Show(s) => {
                s.watched_episode_count > 0 && s.watched_episode_count < s.total_episode_count
            }
            MediaItem::Movie(m) => m.playback_position.is_some() && !m.watched,
            MediaItem::Episode(e) => e.playback_position.is_some() && !e.watched,
            _ => false,
        }
    }

    pub fn watch_progress(&self) -> Option<f32> {
        match self {
            MediaItem::Show(s) if s.total_episode_count > 0 => {
                Some(s.watched_episode_count as f32 / s.total_episode_count as f32)
            }
            MediaItem::Movie(m) => m
                .playback_position
                .map(|pos| pos.as_secs_f32() / m.duration.as_secs_f32()),
            MediaItem::Episode(e) => e
                .playback_position
                .map(|pos| pos.as_secs_f32() / e.duration.as_secs_f32()),
            _ => None,
        }
    }

    pub fn playback_position(&self) -> Option<Duration> {
        match self {
            MediaItem::Movie(m) => m.playback_position,
            MediaItem::Episode(e) => e.playback_position,
            _ => None,
        }
    }

    pub fn duration(&self) -> Option<Duration> {
        match self {
            MediaItem::Movie(m) => Some(m.duration),
            MediaItem::Episode(e) => Some(e.duration),
            _ => None,
        }
    }

    // TODO: Helper methods for Cocoa frontend - some fields don't exist in current MediaItem structure
    pub fn year(&self) -> Option<u32> {
        match self {
            MediaItem::Movie(m) => m.year,
            MediaItem::Show(s) => s.year,
            _ => None,
        }
    }

    pub fn content_rating(&self) -> Option<&str> {
        // TODO: content_rating field doesn't exist, stub for now
        None
    }

    pub fn duration_millis(&self) -> Option<u64> {
        self.duration().map(|d| d.as_millis() as u64)
    }

    pub fn rating(&self) -> Option<f32> {
        match self {
            MediaItem::Movie(m) => m.rating,
            MediaItem::Show(s) => s.rating,
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Credentials {
    UsernamePassword { username: String, password: String },
    Token { token: String },
    ApiKey { key: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlexCredentials {
    pub auth_token: String,
    pub client_id: String,
    pub client_name: String,
    pub device_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JellyfinCredentials {
    pub server_url: String,
    pub username: String,
    pub access_token: String,
    pub user_id: String,
    pub device_id: String,
}

/// Utility functions for source display names
pub mod source_utils {
    /// Create a friendly display name for a source based on its type and name
    pub fn create_friendly_name(source_name: &str, source_type: &str) -> String {
        match source_type {
            "plex" => {
                // For Plex sources, the name should already be the server name (e.g., "Storage")
                // If it starts with "Plex - ", extract the server name
                if let Some(server_name) = source_name.strip_prefix("Plex - ") {
                    if server_name.is_empty() {
                        "Plex Server".to_string()
                    } else {
                        server_name.to_string()
                    }
                } else if source_name.starts_with("plex_server_")
                    && source_name.ends_with(" Source")
                {
                    // Handle database source names like "plex_server_ID Source"
                    // Extract the actual server name which should be set in database via sync
                    // For now, fall back to generic name until we fix database sync
                    "Plex Server".to_string()
                } else {
                    // Use the name as-is (this should be the server name like "Storage")
                    source_name.to_string()
                }
            }
            "jellyfin" => {
                // For Jellyfin sources, extract server name from "Jellyfin - URL" format
                if let Some(server_part) = source_name.strip_prefix("Jellyfin - ") {
                    // Extract hostname from URL like "http://storage:8096" -> "storage"
                    if server_part.contains("://") {
                        if let Some(domain_part) = server_part.split("://").nth(1) {
                            if let Some(hostname) = domain_part.split(':').next() {
                                format!("Jellyfin ({})", hostname)
                            } else {
                                format!("Jellyfin ({})", domain_part)
                            }
                        } else {
                            "Jellyfin Server".to_string()
                        }
                    } else {
                        format!("Jellyfin ({})", server_part)
                    }
                } else if source_name.starts_with("source_jellyfin_") {
                    // Handle legacy source names that show the ID instead of name
                    "Jellyfin Server".to_string()
                } else {
                    source_name.to_string()
                }
            }
            "local" => {
                // For local sources, keep the name as-is or provide a default
                if source_name == "Local Files" || source_name.is_empty() {
                    "Local Files".to_string()
                } else {
                    source_name.to_string()
                }
            }
            _ => {
                // For unknown types, use the original name
                source_name.to_string()
            }
        }
    }
}
