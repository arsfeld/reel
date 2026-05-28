use async_trait::async_trait;

use crate::models::{
    detail::MediaDetail,
    hub::MediaHub,
    library::LibrarySection,
    media::{MediaItem, SourceType},
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
