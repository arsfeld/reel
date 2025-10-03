use anyhow::Result;
use async_trait::async_trait;

use crate::db::connection::DatabaseConnection;
use crate::models::{Episode, MediaItem, MediaItemId, ShowId};
use crate::services::commands::Command;
use crate::services::core::media::MediaService;

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

/// Get playback progress for a media item
pub struct GetPlaybackProgressCommand {
    pub db: DatabaseConnection,
    pub media_id: MediaItemId,
    pub user_id: String,
}

#[async_trait]
impl Command<Option<crate::db::entities::PlaybackProgressModel>> for GetPlaybackProgressCommand {
    async fn execute(&self) -> Result<Option<crate::db::entities::PlaybackProgressModel>> {
        use crate::services::core::playback::PlaybackService;

        PlaybackService::get_progress(&self.db, &self.user_id, &self.media_id).await
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

/// Get episodes for a show
pub struct GetEpisodesCommand {
    pub db: DatabaseConnection,
    pub show_id: ShowId,
    pub season_number: Option<u32>,
}

#[async_trait]
impl Command<Vec<Episode>> for GetEpisodesCommand {
    async fn execute(&self) -> Result<Vec<Episode>> {
        // Use the new MediaService method to get episodes
        let items =
            MediaService::get_episodes_for_show(&self.db, &self.show_id, self.season_number)
                .await?;

        let mut episodes = Vec::new();
        for item in items {
            if let MediaItem::Episode(episode) = item {
                episodes.push(episode);
            }
        }

        // Sort by episode number
        episodes.sort_by_key(|e| e.episode_number);
        Ok(episodes)
    }
}

// Tests disabled temporarily - need proper database mocking support
#[cfg(test)]
#[allow(dead_code)]
mod tests {
    use super::*;
    use crate::db::entities::{libraries, media_items, playback_progress};
    use crate::models::{LibraryType, Movie};
    use chrono::Utc;
    // use sea_orm::{DatabaseBackend, MockDatabase, MockExecResult};
    use std::sync::Arc;
    use std::time::Duration;

    /* fn create_mock_db() -> DatabaseConnection {
        let db = MockDatabase::new(DatabaseBackend::Sqlite)
            .append_query_results([
                vec![libraries::Model {
                    id: 1,
                    source_id: 1,
                    backend_id: "lib1".to_string(),
                    name: "Movies".to_string(),
                    library_type: "movie".to_string(),
                    item_count: 10,
                    created_at: Utc::now().naive_utc(),
                    updated_at: Utc::now().naive_utc(),
                }],
            ])
            .append_query_results([
                vec![media_items::Model {
                    id: 1,
                    source_id: 1,
                    library_id: 1,
                    backend_id: "item1".to_string(),
                    title: "Test Movie".to_string(),
                    sort_title: "Test Movie".to_string(),
                    media_type: "movie".to_string(),
                    year: Some(2024),
                    duration_ms: 7200000,
                    rating: Some(8.5),
                    poster_url: None,
                    backdrop_url: None,
                    overview: None,
                    genres: None,
                    season_number: None,
                    episode_number: None,
                    show_id: None,
                    added_at: Some(Utc::now().naive_utc()),
                    updated_at: Some(Utc::now().naive_utc()),
                    last_watched_at: None,
                    watched: false,
                    view_count: 0,
                    created_at: Utc::now().naive_utc(),
                }],
            ])
            .append_query_results([
                vec![playback_progress::Model {
                    id: 1,
                    source_id: 1,
                    media_item_id: 1,
                    user_id: "user1".to_string(),
                    position_ms: 3600000,
                    duration_ms: 7200000,
                    watched: false,
                    updated_at: Utc::now().naive_utc(),
                }],
            ])
            .append_exec_results([
                MockExecResult {
                    last_insert_id: 1,
                    rows_affected: 1,
                },
            ])
            .into_connection();

        DatabaseConnection::Mock(Arc::new(db))
    } */

    /* Disabled - needs mock database support
    #[tokio::test]
    async fn test_get_libraries_command() {
        let db = create_mock_db();
        let command = GetLibrariesCommand { db };

        // Test command structure
        // Actual execution would need proper mock setup
        assert!(true);
    }

    #[tokio::test]
    async fn test_get_libraries_for_source_command() {
        let db = create_mock_db();
        let source_id = SourceId(1);

        let command = GetLibrariesForSourceCommand { db, source_id };

        // Verify command creation
        assert_eq!(command.source_id, SourceId(1));
    }

    #[tokio::test]
    async fn test_get_library_command() {
        let db = create_mock_db();
        let library_id = LibraryId(1);

        let command = GetLibraryCommand { db, library_id };

        // Verify command creation
        assert_eq!(command.library_id, LibraryId(1));
    }

    #[tokio::test]
    async fn test_get_media_items_command() {
        let db = create_mock_db();
        let library_id = LibraryId(1);

        let command = GetMediaItemsCommand {
            db,
            library_id,
            media_type: Some(MediaType::Movie),
            offset: 0,
            limit: 10,
        };

        assert_eq!(command.library_id, LibraryId(1));
        assert_eq!(command.offset, 0);
        assert_eq!(command.limit, 10);
    }

    #[tokio::test]
    async fn test_get_media_item_command() {
        let db = create_mock_db();
        let item_id = MediaItemId(1);

        let command = GetMediaItemCommand { db, item_id };

        assert_eq!(command.item_id, MediaItemId(1));
    }

    #[tokio::test]
    async fn test_search_media_command() {
        let db = create_mock_db();
        let query = "test".to_string();

        let command = SearchMediaCommand {
            db,
            query: query.clone(),
            library_id: Some(LibraryId(1)),
            media_type: Some(MediaType::Movie),
        };

        assert_eq!(command.query, "test");
        assert_eq!(command.library_id, Some(LibraryId(1)));
    }

    #[tokio::test]
    async fn test_get_recently_added_command() {
        let db = create_mock_db();

        let command = GetRecentlyAddedCommand { db, limit: 20 };

        assert_eq!(command.limit, 20);
    }

    #[tokio::test]
    async fn test_get_continue_watching_command() {
        let db = create_mock_db();

        let command = GetContinueWatchingCommand { db, limit: 10 };

        assert_eq!(command.limit, 10);
    }

    #[tokio::test]
    async fn test_get_playback_progress_command() {
        let db = create_mock_db();
        let media_id = MediaItemId(1);
        let user_id = "user1".to_string();

        let command = GetPlaybackProgressCommand {
            db,
            media_id,
            user_id: user_id.clone(),
        };

        assert_eq!(command.media_id, MediaItemId(1));
        assert_eq!(command.user_id, "user1");
    }

    #[tokio::test]
    async fn test_update_playback_progress_command() {
        let db = create_mock_db();
        let media_id = MediaItemId(1);

        let command = UpdatePlaybackProgressCommand {
            db,
            media_id,
            position_ms: 3600000,
            duration_ms: 7200000,
            watched: false,
        };

        assert_eq!(command.media_id, MediaItemId(1));
        assert_eq!(command.position_ms, 3600000);
        assert_eq!(command.duration_ms, 7200000);
        assert!(!command.watched);
    }

    #[tokio::test]
    async fn test_save_library_command() {
        let db = create_mock_db();
        let library = Library {
            id: "lib1".to_string(),
            name: "Movies".to_string(),
            library_type: LibraryType::Movie,
            item_count: 10,
        };
        let source_id = SourceId(1);

        let command = SaveLibraryCommand {
            db,
            library: library.clone(),
            source_id,
        };

        assert_eq!(command.library.id, "lib1");
        assert_eq!(command.source_id, SourceId(1));
    }

    #[tokio::test]
    async fn test_save_media_item_command() {
        let db = create_mock_db();
        let item = MediaItem::Movie(Movie {
            id: "movie1".to_string(),
            backend_id: "movie1".to_string(),
            title: "Test Movie".to_string(),
            year: Some(2024),
            duration: Duration::from_secs(7200),
            rating: Some(8.5),
            poster_url: None,
            backdrop_url: None,
            overview: None,
            genres: vec![],
            cast: vec![],
            crew: vec![],
            added_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
            watched: false,
            view_count: 0,
            last_watched_at: None,
            playback_position: None,
            intro_marker: None,
            credits_marker: None,
        });

        let command = SaveMediaItemCommand {
            db,
            item: item.clone(),
            library_id: LibraryId(1),
            source_id: SourceId(1),
        };

        assert_eq!(command.library_id, LibraryId(1));
        assert_eq!(command.source_id, SourceId(1));
    }

    #[tokio::test]
    async fn test_clear_library_command() {
        let db = create_mock_db();
        let library_id = LibraryId(1);

        let command = ClearLibraryCommand { db, library_id };

        assert_eq!(command.library_id, LibraryId(1));
    }

    #[tokio::test]
    async fn test_get_episodes_command() {
        let db = create_mock_db();
        let show_id = ShowId(1);

        let command = GetEpisodesCommand {
            db,
            show_id,
            season_number: Some(1),
        };

        assert_eq!(command.show_id, ShowId(1));
        assert_eq!(command.season_number, Some(1));
    }

    #[tokio::test]
    async fn test_clear_source_command() {
        let db = create_mock_db();
        let source_id = SourceId(1);

        let command = ClearSourceCommand { db, source_id };

        assert_eq!(command.source_id, SourceId(1));
    }

    #[tokio::test]
    async fn test_get_stream_url_command() {
        let db = create_mock_db();
        let media_item_id = MediaItemId(1);

        let command = GetStreamUrlCommand {
            db,
            media_item_id,
        };

        assert_eq!(command.media_item_id, MediaItemId(1));
    }
    */
}
