use async_trait::async_trait;

use crate::models::{
    library::LibrarySection,
    media::{MediaItem, SourceType},
};

#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Not found: {0}")]
    NotFound(String),

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
    async fn metadata(&self, rating_key: &str) -> Result<MediaItem, SourceError>;
    async fn children(&self, rating_key: &str) -> Result<Vec<MediaItem>, SourceError>;
    fn playback_url(&self, part_key: &str) -> String;
    fn artwork_url(&self, path: &str, width: u32, height: u32) -> String;
}
