use anyhow::Result;
use async_trait::async_trait;

use crate::db::connection::DatabaseConnection;
use crate::models::{
    Episode, Library, LibraryId, MediaItem, MediaItemId, MediaType, ShowId, SourceId, StreamInfo,
};
use crate::services::commands::Command;
use crate::services::core::media::MediaService;

/// Get all libraries
pub struct GetLibrariesCommand {
    pub db: DatabaseConnection,
}

#[async_trait]
impl Command<Vec<Library>> for GetLibrariesCommand {
    async fn execute(&self) -> Result<Vec<Library>> {
        MediaService::get_libraries(&self.db).await
    }
}

/// Get libraries for a specific source
pub struct GetLibrariesForSourceCommand {
    pub db: DatabaseConnection,
    pub source_id: SourceId,
}

#[async_trait]
impl Command<Vec<Library>> for GetLibrariesForSourceCommand {
    async fn execute(&self) -> Result<Vec<Library>> {
        MediaService::get_libraries_for_source(&self.db, &self.source_id).await
    }
}

/// Get a specific library by ID
pub struct GetLibraryCommand {
    pub db: DatabaseConnection,
    pub library_id: LibraryId,
}

#[async_trait]
impl Command<Option<Library>> for GetLibraryCommand {
    async fn execute(&self) -> Result<Option<Library>> {
        MediaService::get_library(&self.db, &self.library_id).await
    }
}

/// Get media items for a library with pagination
pub struct GetMediaItemsCommand {
    pub db: DatabaseConnection,
    pub library_id: LibraryId,
    pub media_type: Option<MediaType>,
    pub offset: u32,
    pub limit: u32,
}

#[async_trait]
impl Command<Vec<MediaItem>> for GetMediaItemsCommand {
    async fn execute(&self) -> Result<Vec<MediaItem>> {
        MediaService::get_media_items(
            &self.db,
            &self.library_id,
            self.media_type.clone(),
            self.offset,
            self.limit,
        )
        .await
    }
}

/// Get a specific media item
pub struct GetMediaItemCommand {
    pub db: DatabaseConnection,
    pub item_id: MediaItemId,
}

#[async_trait]
impl Command<Option<MediaItem>> for GetMediaItemCommand {
    async fn execute(&self) -> Result<Option<MediaItem>> {
        MediaService::get_media_item(&self.db, &self.item_id).await
    }
}

/// Get detailed information about a media item
pub struct GetItemDetailsCommand {
    pub db: DatabaseConnection,
    pub item_id: MediaItemId,
}

#[async_trait]
impl Command<MediaItem> for GetItemDetailsCommand {
    async fn execute(&self) -> Result<MediaItem> {
        MediaService::get_item_details(&self.db, &self.item_id).await
    }
}

/// Search media items
pub struct SearchMediaCommand {
    pub db: DatabaseConnection,
    pub query: String,
    pub library_id: Option<LibraryId>,
    pub media_type: Option<MediaType>,
}

#[async_trait]
impl Command<Vec<MediaItem>> for SearchMediaCommand {
    async fn execute(&self) -> Result<Vec<MediaItem>> {
        MediaService::search_media(
            &self.db,
            &self.query,
            self.library_id.as_ref(),
            self.media_type.clone(),
        )
        .await
    }
}

/// Get recently added media
pub struct GetRecentlyAddedCommand {
    pub db: DatabaseConnection,
    pub limit: u32,
}

#[async_trait]
impl Command<Vec<MediaItem>> for GetRecentlyAddedCommand {
    async fn execute(&self) -> Result<Vec<MediaItem>> {
        MediaService::get_recently_added(&self.db, self.limit).await
    }
}

/// Get continue watching items
pub struct GetContinueWatchingCommand {
    pub db: DatabaseConnection,
    pub limit: u32,
}

#[async_trait]
impl Command<Vec<MediaItem>> for GetContinueWatchingCommand {
    async fn execute(&self) -> Result<Vec<MediaItem>> {
        MediaService::get_continue_watching(&self.db, self.limit).await
    }
}

/// Update playback progress
pub struct UpdatePlaybackProgressCommand {
    pub db: DatabaseConnection,
    pub media_id: MediaItemId,
    pub position_ms: i64,
    pub duration_ms: i64,
    pub watched: bool,
}

#[async_trait]
impl Command<()> for UpdatePlaybackProgressCommand {
    async fn execute(&self) -> Result<()> {
        MediaService::update_playback_progress(
            &self.db,
            &self.media_id,
            self.position_ms,
            self.duration_ms,
            self.watched,
        )
        .await
    }
}

/// Save a library
pub struct SaveLibraryCommand {
    pub db: DatabaseConnection,
    pub library: Library,
    pub source_id: SourceId,
}

#[async_trait]
impl Command<()> for SaveLibraryCommand {
    async fn execute(&self) -> Result<()> {
        MediaService::save_library(&self.db, self.library.clone(), &self.source_id).await
    }
}

/// Save a media item
pub struct SaveMediaItemCommand {
    pub db: DatabaseConnection,
    pub item: MediaItem,
    pub library_id: LibraryId,
    pub source_id: SourceId,
}

#[async_trait]
impl Command<()> for SaveMediaItemCommand {
    async fn execute(&self) -> Result<()> {
        MediaService::save_media_item(
            &self.db,
            self.item.clone(),
            &self.library_id,
            &self.source_id,
        )
        .await
    }
}

/// Clear all media for a library
pub struct ClearLibraryCommand {
    pub db: DatabaseConnection,
    pub library_id: LibraryId,
}

#[async_trait]
impl Command<()> for ClearLibraryCommand {
    async fn execute(&self) -> Result<()> {
        MediaService::clear_library(&self.db, &self.library_id).await
    }
}

/// Get episodes for a show
pub struct GetEpisodesCommand {
    pub db: DatabaseConnection,
    pub show_id: ShowId,
    pub season_number: Option<u32>,
}

#[async_trait]
impl Command<Vec<Episode>> for GetEpisodesCommand {
    async fn execute(&self) -> Result<Vec<Episode>> {
        // For now, return episodes from the database
        // In the future, this could fetch from the backend if needed
        let items = MediaService::get_media_items(
            &self.db,
            &LibraryId::new("episodes"),
            Some(MediaType::Show),
            0,
            50,
        )
        .await?;

        let mut episodes = Vec::new();
        for item in items {
            if let MediaItem::Episode(episode) = item {
                // Filter by show_id and season if specified
                if episode.show_id.as_ref().map(|s| s.as_str()) == Some(self.show_id.as_str()) {
                    if let Some(season) = self.season_number {
                        if episode.season_number == season {
                            episodes.push(episode);
                        }
                    } else {
                        episodes.push(episode);
                    }
                }
            }
        }

        // Sort by episode number
        episodes.sort_by_key(|e| e.episode_number);
        Ok(episodes)
    }
}

/// Clear all data for a source
pub struct ClearSourceCommand {
    pub db: DatabaseConnection,
    pub source_id: SourceId,
}

#[async_trait]
impl Command<()> for ClearSourceCommand {
    async fn execute(&self) -> Result<()> {
        MediaService::clear_source(&self.db, &self.source_id).await
    }
}

/// Get stream URL for a media item
pub struct GetStreamUrlCommand {
    pub db: DatabaseConnection,
    pub media_item_id: MediaItemId,
}

#[async_trait]
impl Command<StreamInfo> for GetStreamUrlCommand {
    async fn execute(&self) -> Result<StreamInfo> {
        // Use the stateless BackendService - pure function approach
        crate::services::core::BackendService::get_stream_url(&self.db, &self.media_item_id).await
    }
}
