use anyhow::Result;
use async_trait::async_trait;

use crate::backends::traits::MediaBackend;
use crate::db::connection::DatabaseConnection;
use crate::models::{Library, SourceId};
use crate::services::commands::Command;
use crate::services::core::sync::{SyncResult, SyncService};

/// Sync all libraries for a source
pub struct SyncSourceCommand<'a> {
    pub db: DatabaseConnection,
    pub backend: &'a dyn MediaBackend,
    pub source_id: SourceId,
}

#[async_trait]
impl<'a> Command<SyncResult> for SyncSourceCommand<'a> {
    async fn execute(&self) -> Result<SyncResult> {
        SyncService::sync_source(&self.db, self.backend, &self.source_id).await
    }
}

/// Sync a single library
pub struct SyncLibraryCommand<'a> {
    pub db: DatabaseConnection,
    pub backend: &'a dyn MediaBackend,
    pub source_id: SourceId,
    pub library: Library,
}

#[async_trait]
impl<'a> Command<usize> for SyncLibraryCommand<'a> {
    async fn execute(&self) -> Result<usize> {
        SyncService::sync_library(&self.db, self.backend, &self.source_id, &self.library).await
    }
}

// Tests disabled temporarily - need proper database mocking support
#[cfg(test)]
#[allow(dead_code)]
mod tests {
    use super::*;
    use crate::db::connection::DatabaseConnection;
    use crate::db::entities::{libraries, sources};
    use crate::models::{Library, LibraryType, Movie, Show, SourceId};
    use crate::services::core::sync::SyncResult;
    use async_trait::async_trait;
    use chrono::Utc;
    // use sea_orm::{entity::prelude::*, DatabaseBackend, MockDatabase, MockExecResult};
    use std::sync::Arc;
    use std::time::Duration;

    /* fn create_mock_db() -> DatabaseConnection {
        let db = MockDatabase::new(DatabaseBackend::Sqlite)
            .append_query_results([
                vec![sources::Model {
                    id: 1,
                    name: "Test Source".to_string(),
                    backend_type: "plex".to_string(),
                    server_url: "http://localhost:32400".to_string(),
                    auth_token: Some("test_token".to_string()),
                    username: None,
                    password: None,
                    created_at: Utc::now().naive_utc(),
                    updated_at: Utc::now().naive_utc(),
                    last_sync: None,
                    is_active: true,
                }],
            ])
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
            .append_exec_results([
                MockExecResult {
                    last_insert_id: 1,
                    rows_affected: 1,
                },
            ])
            .into_connection();

        DatabaseConnection::Mock(Arc::new(db))
    } */

    struct TestBackend;

    #[async_trait]
    impl MediaBackend for TestBackend {
        async fn authenticate(
            &self,
            _credentials: crate::backends::Credentials,
        ) -> Result<crate::models::User> {
            Ok(crate::models::User {
                id: "user1".to_string(),
                username: "testuser".to_string(),
                token: Some("token".to_string()),
            })
        }

        async fn get_libraries(&self) -> Result<Vec<Library>> {
            Ok(vec![
                Library {
                    id: "lib1".to_string(),
                    name: "Movies".to_string(),
                    library_type: LibraryType::Movie,
                    item_count: 10,
                },
                Library {
                    id: "lib2".to_string(),
                    name: "TV Shows".to_string(),
                    library_type: LibraryType::Show,
                    item_count: 5,
                },
            ])
        }

        async fn get_movies(&self, library_id: &str) -> Result<Vec<Movie>> {
            if library_id == "lib1" {
                Ok(vec![Movie {
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
                }])
            } else {
                Ok(vec![])
            }
        }

        async fn get_shows(&self, library_id: &str) -> Result<Vec<Show>> {
            if library_id == "lib2" {
                Ok(vec![Show {
                    id: "show1".to_string(),
                    backend_id: "show1".to_string(),
                    title: "Test Show".to_string(),
                    year: Some(2024),
                    rating: Some(9.0),
                    poster_url: None,
                    backdrop_url: None,
                    overview: None,
                    genres: vec![],
                    seasons: vec![],
                    cast: vec![],
                    added_at: Some(Utc::now()),
                    updated_at: Some(Utc::now()),
                    watched_episode_count: 0,
                    total_episode_count: 10,
                    last_watched_at: None,
                }])
            } else {
                Ok(vec![])
            }
        }

        async fn get_episodes(&self, _show_id: &str) -> Result<Vec<crate::models::Episode>> {
            Ok(vec![])
        }

        async fn get_playback_progress(&self, _item_id: &str) -> Result<Option<Duration>> {
            Ok(None)
        }

        async fn update_playback_progress(
            &self,
            _item_id: &str,
            _position: Duration,
        ) -> Result<()> {
            Ok(())
        }

        async fn get_stream_url(&self, item_id: &str, _quality: Option<String>) -> Result<String> {
            Ok(format!("http://localhost:32400/stream/{}", item_id))
        }

        async fn check_connection(&self) -> Result<bool> {
            Ok(true)
        }

        fn backend_type(&self) -> crate::backends::BackendType {
            crate::backends::BackendType::Plex
        }
    }

    #[tokio::test]
    async fn test_sync_source_command_creation() {
        // let db = create_mock_db();
        let backend = TestBackend;
        let source_id = SourceId(1);

        let command = SyncSourceCommand {
            db: db.clone(),
            backend: &backend,
            source_id: source_id.clone(),
        };

        assert_eq!(command.source_id, source_id);
    }

    #[tokio::test]
    async fn test_sync_library_command_creation() {
        // let db = create_mock_db();
        let backend = TestBackend;
        let source_id = SourceId(1);
        let library = Library {
            id: "lib1".to_string(),
            name: "Movies".to_string(),
            library_type: LibraryType::Movie,
            item_count: 10,
        };

        let command = SyncLibraryCommand {
            db: db.clone(),
            backend: &backend,
            source_id: source_id.clone(),
            library: library.clone(),
        };

        assert_eq!(command.source_id, source_id);
        assert_eq!(command.library.id, "lib1");
    }
}
