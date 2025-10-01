use anyhow::Result;
use async_trait::async_trait;

use crate::backends::traits::MediaBackend;
use crate::db::connection::DatabaseConnection;
use crate::db::repository::{Repository, SourceRepositoryImpl};
use crate::models::auth_provider::Source;
use crate::models::{Credentials, SourceId, User};
use crate::services::commands::Command;
use crate::services::core::auth::AuthService;

/// Authenticate with a backend
pub struct AuthenticateCommand<'a> {
    pub backend: &'a dyn MediaBackend,
    pub credentials: Credentials,
}

#[async_trait]
impl<'a> Command<User> for AuthenticateCommand<'a> {
    async fn execute(&self) -> Result<User> {
        AuthService::authenticate(self.backend, self.credentials.clone()).await
    }
}

/// Save authentication credentials
pub struct SaveCredentialsCommand {
    pub db: DatabaseConnection,
    pub source_id: SourceId,
    pub credentials: Credentials,
}

#[async_trait]
impl Command<()> for SaveCredentialsCommand {
    async fn execute(&self) -> Result<()> {
        AuthService::save_credentials(&self.db, &self.source_id, &self.credentials).await
    }
}

/// Load authentication credentials
pub struct LoadCredentialsCommand {
    pub db: DatabaseConnection,
    pub source_id: SourceId,
}

#[async_trait]
impl Command<Option<Credentials>> for LoadCredentialsCommand {
    async fn execute(&self) -> Result<Option<Credentials>> {
        AuthService::load_credentials(&self.db, &self.source_id).await
    }
}

/// Remove authentication credentials
pub struct RemoveCredentialsCommand {
    pub db: DatabaseConnection,
    pub source_id: SourceId,
}

#[async_trait]
impl Command<()> for RemoveCredentialsCommand {
    async fn execute(&self) -> Result<()> {
        AuthService::remove_credentials(&self.db, &self.source_id).await
    }
}

/// Create and authenticate a new source
pub struct CreateSourceCommand<'a> {
    pub db: DatabaseConnection,
    pub backend: &'a dyn MediaBackend,
    pub source_type: String,
    pub name: String,
    pub credentials: Credentials,
    pub server_url: Option<String>,
    pub machine_id: Option<String>, // For Plex servers
    pub is_owned: Option<bool>,     // For Plex servers
}

#[async_trait]
impl<'a> Command<Source> for CreateSourceCommand<'a> {
    async fn execute(&self) -> Result<Source> {
        AuthService::create_source(
            &self.db,
            self.backend,
            self.source_type.clone(),
            self.name.clone(),
            self.credentials.clone(),
            self.server_url.clone(),
            self.machine_id.clone(),
            self.is_owned,
        )
        .await
    }
}

/// Remove a source and its credentials
pub struct RemoveSourceCommand {
    pub db: DatabaseConnection,
    pub source_id: SourceId,
}

#[async_trait]
impl Command<()> for RemoveSourceCommand {
    async fn execute(&self) -> Result<()> {
        AuthService::remove_source(&self.db, &self.source_id).await
    }
}

/// Test connection to a backend
pub struct TestConnectionCommand<'a> {
    pub backend: &'a dyn MediaBackend,
    pub credentials: Credentials,
}

#[async_trait]
impl<'a> Command<bool> for TestConnectionCommand<'a> {
    async fn execute(&self) -> Result<bool> {
        AuthService::test_connection(self.backend, self.credentials.clone()).await
    }
}

/// Re-authenticate an existing source
pub struct ReauthSourceCommand<'a> {
    pub db: DatabaseConnection,
    pub backend: &'a dyn MediaBackend,
    pub source_id: SourceId,
}

#[async_trait]
impl<'a> Command<()> for ReauthSourceCommand<'a> {
    async fn execute(&self) -> Result<()> {
        AuthService::reauth_source(&self.db, self.backend, &self.source_id).await
    }
}

/// Load all sources from the database
pub struct LoadSourcesCommand {
    pub db: DatabaseConnection,
}

#[async_trait]
impl Command<Vec<Source>> for LoadSourcesCommand {
    async fn execute(&self) -> Result<Vec<Source>> {
        let repo = SourceRepositoryImpl::new(self.db.clone());
        let source_models = repo.find_all().await?;

        // Convert SourceModel to Source
        let sources: Vec<Source> = source_models.into_iter().map(Source::from).collect();

        Ok(sources)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::entities::sources;
    use crate::models::{Episode, Library, LibraryType, Movie, Show};
    use chrono::Utc;
    // use sea_orm::{DatabaseBackend, MockDatabase, MockExecResult};
    use std::sync::Arc;
    use std::time::Duration;

    struct TestBackend {
        should_auth_succeed: bool,
        should_connect: bool,
    }

    // Temporarily disabled - needs proper trait alignment
    #[allow(dead_code)]
    struct TestBackendImpl;

    /*
    #[async_trait]
    impl MediaBackend for TestBackend {
        async fn authenticate(&self, _credentials: Credentials) -> Result<User> {
            if self.should_auth_succeed {
                Ok(User {
                    id: "test_user".to_string(),
                    username: "testuser".to_string(),
                    token: Some("test_token".to_string()),
                })
            } else {
                Err(anyhow::anyhow!("Authentication failed"))
            }
        }

        async fn get_libraries(&self) -> Result<Vec<Library>> {
            Ok(vec![])
        }

        async fn get_movies(&self, _library_id: &str) -> Result<Vec<Movie>> {
            Ok(vec![])
        }

        async fn get_shows(&self, _library_id: &str) -> Result<Vec<Show>> {
            Ok(vec![])
        }

        async fn get_episodes(&self, _show_id: &str) -> Result<Vec<Episode>> {
            Ok(vec![])
        }

        async fn get_playback_progress(&self, _item_id: &str) -> Result<Option<Duration>> {
            Ok(None)
        }

        async fn update_playback_progress(&self, _item_id: &str, _position: Duration) -> Result<()> {
            Ok(())
        }

        async fn get_stream_url(&self, item_id: &str, _quality: Option<String>) -> Result<String> {
            Ok(format!("http://test.local/stream/{}", item_id))
        }

        async fn check_connection(&self) -> Result<bool> {
            Ok(self.should_connect)
        }

        fn backend_type(&self) -> BackendType {
            BackendType::Plex
        }
    }
    */

    // fn create_mock_db() -> DatabaseConnection {
    //     // Mock database creation would go here
    //     // For now, tests that require a database will be skipped
    //     todo!()
    // }

    // Tests disabled temporarily until proper mocking is available
    /*
    #[tokio::test]
    async fn test_authenticate_command_success() {
        let backend = TestBackend {
            should_auth_succeed: true,
            should_connect: true,
        };
        let credentials = Credentials::Token {
            token: "test_token".to_string(),
        };

        let command = AuthenticateCommand {
            backend: &backend,
            credentials,
        };

        let result = command.execute().await;
        assert!(result.is_ok());
        let user = result.unwrap();
        assert_eq!(user.username, "testuser");
    }

    #[tokio::test]
    async fn test_authenticate_command_failure() {
        let backend = TestBackend {
            should_auth_succeed: false,
            should_connect: true,
        };
        let credentials = Credentials::Token {
            token: "wrong_token".to_string(),
        };

        let command = AuthenticateCommand {
            backend: &backend,
            credentials,
        };

        let result = command.execute().await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Authentication failed");
    }

    #[tokio::test]
    async fn test_test_connection_command_success() {
        let backend = TestBackend {
            should_auth_succeed: true,
            should_connect: true,
        };
        let credentials = Credentials::Token {
            token: "test_token".to_string(),
        };

        let command = TestConnectionCommand {
            backend: &backend,
            credentials,
        };

        let result = command.execute().await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_test_connection_command_failure() {
        let backend = TestBackend {
            should_auth_succeed: true,
            should_connect: false,
        };
        let credentials = Credentials::Token {
            token: "test_token".to_string(),
        };

        let command = TestConnectionCommand {
            backend: &backend,
            credentials,
        };

        let result = command.execute().await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_save_credentials_command() {
        let source_id = SourceId(1);
        let credentials = Credentials::Token {
            token: "test_token".to_string(),
        };

        let command = SaveCredentialsCommand {
            source_id: source_id.clone(),
            credentials: credentials.clone(),
        };

        // This test would need actual keyring mock to work properly
        // For now, we just verify the command can be created
        assert_eq!(command.source_id, source_id);
    }

    #[tokio::test]
    async fn test_load_sources_command() {
        // This test requires proper database mock setup
        // For now, we just verify the command can be created
        // let db = create_mock_db();
        // let command = LoadSourcesCommand { db };
        assert!(true);
    }
    */
}
