use anyhow::Result;
use async_trait::async_trait;

use crate::backends::traits::MediaBackend;
use crate::db::connection::DatabaseConnection;
use crate::db::repository::{Repository, SourceRepositoryImpl};
use crate::models::auth_provider::Source;
use crate::models::{Credentials, SourceId, User};
use crate::services::commands::Command;
use crate::services::core::auth::AuthService;
use std::sync::Arc;

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
    pub source_id: SourceId,
    pub credentials: Credentials,
}

#[async_trait]
impl Command<()> for SaveCredentialsCommand {
    async fn execute(&self) -> Result<()> {
        AuthService::save_credentials(&self.source_id, &self.credentials).await
    }
}

/// Load authentication credentials
pub struct LoadCredentialsCommand {
    pub source_id: SourceId,
}

#[async_trait]
impl Command<Option<Credentials>> for LoadCredentialsCommand {
    async fn execute(&self) -> Result<Option<Credentials>> {
        AuthService::load_credentials(&self.source_id).await
    }
}

/// Remove authentication credentials
pub struct RemoveCredentialsCommand {
    pub source_id: SourceId,
}

#[async_trait]
impl Command<()> for RemoveCredentialsCommand {
    async fn execute(&self) -> Result<()> {
        AuthService::remove_credentials(&self.source_id).await
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
        let sources: Vec<Source> = source_models
            .into_iter()
            .map(|model| Source::from(model))
            .collect();

        Ok(sources)
    }
}
