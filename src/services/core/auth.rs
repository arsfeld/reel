use anyhow::{Context, Result};
use keyring::Entry;
use tracing::{debug, info};

use crate::backends::jellyfin::JellyfinBackend;
use crate::backends::local::LocalBackend;
use crate::backends::plex::PlexBackend;
use crate::backends::traits::MediaBackend;
use crate::db::connection::DatabaseConnection;
use crate::db::entities::SourceModel;
use crate::db::repository::{
    Repository, SourceRepositoryImpl, source_repository::SourceRepository,
};
use crate::models::Credentials;
use crate::models::auth_provider::{ConnectionInfo, Source, SourceType};
use crate::models::{SourceId, User};

/// Pure functions for authentication operations
pub struct AuthService;

impl AuthService {
    /// Authenticate with a backend
    pub async fn authenticate(
        backend: &dyn MediaBackend,
        credentials: Credentials,
    ) -> Result<User> {
        backend
            .authenticate(credentials)
            .await
            .context("Failed to authenticate with backend")
    }

    /// Save authentication credentials directly to keyring
    pub async fn save_credentials(source_id: &SourceId, credentials: &Credentials) -> Result<()> {
        let service_name = format!("gnome-reel.{}", source_id);

        match credentials {
            Credentials::UsernamePassword {
                username, password, ..
            } => {
                let entry = Entry::new(&service_name, username)?;
                entry.set_password(password).with_context(|| {
                    format!("Failed to save password for source: {}", source_id)
                })?;
                debug!("Saved credentials for source: {}", source_id);
            }
            Credentials::Token { token, .. } => {
                let entry = Entry::new(&service_name, "token")?;
                entry
                    .set_password(token)
                    .with_context(|| format!("Failed to save token for source: {}", source_id))?;
                debug!("Saved token for source: {}", source_id);
            }
            Credentials::ApiKey { key, .. } => {
                let entry = Entry::new(&service_name, "api_key")?;
                entry
                    .set_password(key)
                    .with_context(|| format!("Failed to save API key for source: {}", source_id))?;
                debug!("Saved API key for source: {}", source_id);
            }
        }

        Ok(())
    }

    /// Load authentication credentials directly from keyring
    pub async fn load_credentials(source_id: &SourceId) -> Result<Option<Credentials>> {
        let service_name = format!("gnome-reel.{}", source_id);

        // Try to load as token first
        if let Ok(entry) = Entry::new(&service_name, "token") {
            if let Ok(token) = entry.get_password() {
                debug!("Found token credentials for source: {}", source_id);
                return Ok(Some(Credentials::Token { token }));
            }
        }

        debug!("No credentials found for source: {}", source_id);
        Ok(None)
    }

    /// Remove authentication credentials directly from keyring
    pub async fn remove_credentials(source_id: &SourceId) -> Result<()> {
        let service_name = format!("gnome-reel.{}", source_id);

        // Try to remove token
        if let Ok(entry) = Entry::new(&service_name, "token") {
            let _ = entry.delete_credential(); // Ignore errors - credential might not exist
        }

        debug!("Removed credentials for source: {}", source_id);
        Ok(())
    }

    /// Create and authenticate a new source
    pub async fn create_source(
        db: &DatabaseConnection,
        backend: &dyn MediaBackend,
        source_type: String,
        name: String,
        credentials: Credentials,
        server_url: Option<String>,
    ) -> Result<Source> {
        // Authenticate first
        let user = backend.authenticate(credentials.clone()).await?;

        // Generate source ID
        let source_id = SourceId::new(format!("{}_{}", source_type, user.id));

        // Use the provided server URL

        // Save to database
        let repo = SourceRepositoryImpl::new_without_events(db.clone());
        let entity = SourceModel {
            id: source_id.to_string(),
            name,
            source_type,
            auth_provider_id: Some(user.id.clone()),
            connection_url: server_url,
            connections: None, // Will be populated later by connection discovery
            machine_id: None,  // Will be set for Plex servers
            is_owned: true,    // Default to owned
            is_online: true,
            last_sync: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };

        // Create source model for return
        let source = Source {
            id: source_id.to_string(),
            name: entity.name.clone(),
            source_type: SourceType::PlexServer {
                machine_id: user.id.clone(),
                owned: true,
            },
            auth_provider_id: entity.auth_provider_id.clone(),
            connection_info: ConnectionInfo {
                primary_url: entity.connection_url.clone(),
                is_online: true,
                last_check: Some(chrono::Utc::now()),
            },
            enabled: true,
            last_sync: entity.last_sync.map(|dt| dt.and_utc()),
            library_count: 0,
        };

        repo.insert(entity).await?;

        // Save credentials
        Self::save_credentials(&source_id, &credentials).await?;

        info!("Created new source: {}", source_id);
        Ok(source)
    }

    /// Remove a source and its credentials
    pub async fn remove_source(db: &DatabaseConnection, source_id: &SourceId) -> Result<()> {
        // Remove credentials
        Self::remove_credentials(source_id).await?;

        // Remove from database
        let repo = SourceRepositoryImpl::new_without_events(db.clone());
        repo.delete(&source_id.to_string()).await?;

        info!("Removed source: {}", source_id);
        Ok(())
    }

    /// Test connection to a backend
    pub async fn test_connection(
        backend: &dyn MediaBackend,
        credentials: Credentials,
    ) -> Result<bool> {
        match Self::authenticate(backend, credentials).await {
            Ok(_) => Ok(true),
            Err(e) => {
                debug!("Connection test failed: {}", e);
                Ok(false)
            }
        }
    }

    /// Re-authenticate an existing source
    pub async fn reauth_source(
        db: &DatabaseConnection,
        backend: &dyn MediaBackend,
        source_id: &SourceId,
    ) -> Result<()> {
        // Load existing credentials
        let credentials = Self::load_credentials(source_id)
            .await?
            .context("No credentials found for source")?;

        // Re-authenticate
        let user = Self::authenticate(backend, credentials.clone()).await?;

        // Update source in database
        let repo = SourceRepositoryImpl::new_without_events(db.clone());
        if let Some(mut source) = repo.find_by_id(&source_id.to_string()).await? {
            source.is_online = true;
            source.auth_provider_id = Some(user.id);
            source.updated_at = chrono::Utc::now().naive_utc();
            repo.update(source).await?;
        }

        info!("Re-authenticated source: {}", source_id);
        Ok(())
    }
}
