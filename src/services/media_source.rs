use async_trait::async_trait;

use crate::models::{
    detail::MediaDetail,
    hub::MediaHub,
    library::LibrarySection,
    media::{MediaItem, SourceType},
    playback::{PlaybackDecision, PlaybackRequest},
};
use crate::player::SkipMarkers;

#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Not supported: {0}")]
    NotSupported(String),

    #[error("Source error: {0}")]
    Other(String),
}

/// How to download an item's original file. Built from the item's part key
/// plus the live auth token, so it is regenerated rather than persisted (only
/// the `part_key` is stored — a token refresh never strands a download).
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct DownloadDescriptor {
    /// Direct, byte-rangeable URL of the original file.
    pub url: String,
    /// Source part key (persisted; the URL is rebuilt from it on each resume).
    pub part_key: String,
    /// Expected total size in bytes when the source knows it up front. Often
    /// `None` for items fetched from list views (the size lives on the detail
    /// metadata, not the list `MediaItem`); the transfer client then takes the
    /// total from the first response's `Content-Length`.
    pub expected_size: Option<u64>,
}

/// Abstract interface for a media source (Plex, Jellyfin, local, etc.).
#[async_trait]
pub trait MediaSource: Send + Sync {
    fn name(&self) -> &str;
    fn source_type(&self) -> SourceType;
    async fn test_connection(&self) -> Result<String, SourceError>;
    async fn libraries(&self) -> Result<Vec<LibrarySection>, SourceError>;
    async fn library_items(&self, library_key: &str) -> Result<Vec<MediaItem>, SourceError>;
    async fn metadata(&self, rating_key: &str) -> Result<MediaDetail, SourceError>;
    async fn children(&self, rating_key: &str) -> Result<Vec<MediaItem>, SourceError>;
    fn playback_url(&self, part_key: &str) -> String;
    fn artwork_url(&self, path: &str, width: u32, height: u32) -> String;

    /// Resolve how to play an item: ask the source's transcode decision engine
    /// and return the URL to hand the pipeline plus decision metadata (R1/R3/R4).
    /// Sources without a transcoder inherit this default and fall back to the
    /// direct-play [`Self::playback_url`]. Mirrors the `skip_markers` pattern.
    async fn resolve_playback(
        &self,
        _req: &PlaybackRequest,
    ) -> Result<PlaybackDecision, SourceError> {
        Err(SourceError::NotSupported(
            "Playback resolution not supported by this source".into(),
        ))
    }

    /// Describe how to download an item's original file. Default: not supported
    /// — local-filesystem sources inherit this (their media is already on
    /// disk), so only remote sources (Plex in v1) override it.
    fn download_descriptor(&self, _item: &MediaItem) -> Result<DownloadDescriptor, SourceError> {
        Err(SourceError::NotSupported(
            "Downloads not supported by this source".into(),
        ))
    }

    /// Fetch collections for a library section. Default: not supported.
    async fn collections(&self, _library_key: &str) -> Result<Vec<MediaItem>, SourceError> {
        Err(SourceError::NotSupported(
            "Collections not supported by this source".into(),
        ))
    }

    /// Fetch items in a collection. Default: not supported.
    async fn collection_items(&self, _collection_key: &str) -> Result<Vec<MediaItem>, SourceError> {
        Err(SourceError::NotSupported(
            "Collection items not supported by this source".into(),
        ))
    }

    /// Fetch recently added items across all libraries. Default: not supported.
    async fn recently_added(&self) -> Result<Vec<MediaItem>, SourceError> {
        Err(SourceError::NotSupported(
            "Recently added not supported by this source".into(),
        ))
    }

    /// Fetch recently added items for a single library, newest first.
    /// Default: not supported.
    async fn recently_added_in_library(
        &self,
        _library_key: &str,
    ) -> Result<Vec<MediaItem>, SourceError> {
        Err(SourceError::NotSupported(
            "Per-library recently added not supported by this source".into(),
        ))
    }

    /// Fetch on-deck / continue watching items. Default: not supported.
    async fn continue_watching(&self) -> Result<Vec<MediaItem>, SourceError> {
        Err(SourceError::NotSupported(
            "Continue watching not supported by this source".into(),
        ))
    }

    /// Fetch the source's curated home hubs (Recommended, "Because you
    /// watched", genre rows). Sources without a hub concept inherit this
    /// default, which is how the home view degrades for non-Plex sources.
    async fn hubs(&self) -> Result<Vec<MediaHub>, SourceError> {
        Err(SourceError::NotSupported(
            "Hubs not supported by this source".into(),
        ))
    }

    /// Report playback progress to the source. Default: no-op.
    async fn report_progress(
        &self,
        _rating_key: &str,
        _state: &str,
        _time_ms: i64,
        _duration_ms: i64,
    ) -> Result<(), SourceError> {
        Ok(())
    }

    /// Mark an item as watched. Default: no-op.
    async fn scrobble(&self, _rating_key: &str) -> Result<(), SourceError> {
        Ok(())
    }

    /// Mark an item as unwatched. Default: no-op.
    async fn unscrobble(&self, _rating_key: &str) -> Result<(), SourceError> {
        Ok(())
    }

    /// Fetch skip-intro / skip-credits markers for a media item.
    /// `duration_secs` is the total duration of the media in seconds,
    /// used for sanity-checking marker bounds.
    /// Default: not supported.
    async fn skip_markers(
        &self,
        _rating_key: &str,
        _duration_secs: f64,
    ) -> Result<SkipMarkers, SourceError> {
        Err(SourceError::NotSupported(
            "Skip markers not supported by this source".into(),
        ))
    }
}
