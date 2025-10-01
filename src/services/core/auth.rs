use anyhow::{Context, Result};
use tracing::{debug, info, warn};

use crate::backends::traits::MediaBackend;
use crate::db::connection::DatabaseConnection;
use crate::db::entities::{AuthTokenModel, SourceModel};
use crate::db::repository::{
    AuthTokenRepository, AuthTokenRepositoryImpl, Repository, SourceRepositoryImpl,
};
use crate::models::Credentials;
use crate::models::auth_provider::{ConnectionInfo, Source, SourceType};
use crate::models::{SourceId, User};

/// Pure functions for authentication operations
pub struct AuthService;

impl AuthService {
    /// Migrate credentials from keyring to database if they exist
    pub async fn migrate_credentials_from_keyring(
        db: &DatabaseConnection,
        source_id: &SourceId,
    ) -> Result<bool> {
        // Try multiple keyring naming patterns for backward compatibility
        let service_patterns = vec![
            format!("reel.{}", source_id),       // New pattern
            format!("gnome-reel.{}", source_id), // GNOME keyring pattern
        ];

        for service_name in &service_patterns {
            // Check if keyring is available and has credentials (token)
            if let Ok(entry) = keyring::Entry::new(service_name, "token")
                && let Ok(token) = entry.get_password()
            {
                info!(
                    "Found keyring token for source: {} (service: {}), migrating to database",
                    source_id, service_name
                );

                // Save to database
                let credentials = Credentials::Token {
                    token: token.clone(),
                };
                if let Err(e) = Self::save_credentials(db, source_id, &credentials).await {
                    warn!(
                        "Failed to migrate token to database for source {}: {}",
                        source_id, e
                    );
                    return Ok(false);
                }

                // Remove from keyring after successful migration
                let _ = entry.delete_credential();
                info!(
                    "Successfully migrated token for source: {} from keyring to database",
                    source_id
                );
                return Ok(true);
            }

            // Check for API key
            if let Ok(entry) = keyring::Entry::new(service_name, "api_key")
                && let Ok(key) = entry.get_password()
            {
                info!(
                    "Found keyring API key for source: {} (service: {}), migrating to database",
                    source_id, service_name
                );

                // Save to database
                let credentials = Credentials::ApiKey { key: key.clone() };
                if let Err(e) = Self::save_credentials(db, source_id, &credentials).await {
                    warn!(
                        "Failed to migrate API key to database for source {}: {}",
                        source_id, e
                    );
                    return Ok(false);
                }

                // Remove from keyring after successful migration
                let _ = entry.delete_credential();
                info!(
                    "Successfully migrated API key for source: {} from keyring to database",
                    source_id
                );
                return Ok(true);
            }
        }

        debug!(
            "No keyring credentials found for source: {} in any pattern",
            source_id
        );
        Ok(false)
    }
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

    /// Save authentication credentials to database
    pub async fn save_credentials(
        db: &DatabaseConnection,
        source_id: &SourceId,
        credentials: &Credentials,
    ) -> Result<()> {
        let repo = AuthTokenRepositoryImpl::new(db.clone());

        let (token_type, token_value) = match credentials {
            Credentials::UsernamePassword {
                username, password, ..
            } => {
                // For username/password, we'll store the password with the username as the type
                // This is a temporary solution - ideally we'd store both separately
                (format!("password_{}", username), password.clone())
            }
            Credentials::Token { token, .. } => ("token".to_string(), token.clone()),
            Credentials::ApiKey { key, .. } => ("api_key".to_string(), key.clone()),
        };

        let auth_token = AuthTokenModel {
            id: 0, // Will be set by database
            source_id: source_id.to_string(),
            token_type,
            token: token_value,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
            expires_at: None, // TODO: Handle token expiration if needed
        };

        repo.upsert(auth_token)
            .await
            .with_context(|| format!("Failed to save credentials for source: {}", source_id))?;

        debug!("Saved credentials for source: {} to database", source_id);
        Ok(())
    }

    /// Load authentication credentials from database
    pub async fn load_credentials(
        db: &DatabaseConnection,
        source_id: &SourceId,
    ) -> Result<Option<Credentials>> {
        let repo = AuthTokenRepositoryImpl::new(db.clone());

        // Try to find token credentials
        if let Some(auth_token) = repo
            .find_by_source_and_type(source_id.as_ref(), "token")
            .await?
        {
            debug!(
                "Found token credentials for source: {} in database",
                source_id
            );
            return Ok(Some(Credentials::Token {
                token: auth_token.token,
            }));
        }

        // Try to find API key credentials
        if let Some(auth_token) = repo
            .find_by_source_and_type(source_id.as_ref(), "api_key")
            .await?
        {
            debug!(
                "Found API key credentials for source: {} in database",
                source_id
            );
            return Ok(Some(Credentials::ApiKey {
                key: auth_token.token,
            }));
        }

        // For username/password, we'd need to know the username
        // This is a limitation of the current design - we'll need to improve this

        // No credentials in database, attempt migration from keyring
        debug!(
            "No credentials found for source: {} in database, attempting keyring migration",
            source_id
        );
        if Self::migrate_credentials_from_keyring(db, source_id).await? {
            // Migration successful, try loading again from database
            let repo = AuthTokenRepositoryImpl::new(db.clone());

            // Try to find token credentials again
            if let Some(auth_token) = repo
                .find_by_source_and_type(source_id.as_ref(), "token")
                .await?
            {
                return Ok(Some(Credentials::Token {
                    token: auth_token.token,
                }));
            }

            // Try to find API key credentials again
            if let Some(auth_token) = repo
                .find_by_source_and_type(source_id.as_ref(), "api_key")
                .await?
            {
                return Ok(Some(Credentials::ApiKey {
                    key: auth_token.token,
                }));
            }
        }

        debug!(
            "No credentials found for source: {} in database or keyring",
            source_id
        );
        Ok(None)
    }

    /// Remove authentication credentials from database
    pub async fn remove_credentials(db: &DatabaseConnection, source_id: &SourceId) -> Result<()> {
        let repo = AuthTokenRepositoryImpl::new(db.clone());
        let deleted_count = repo.delete_by_source(source_id.as_ref()).await?;
        debug!(
            "Removed {} credential(s) for source: {} from database",
            deleted_count, source_id
        );
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
        machine_id: Option<String>,
        is_owned: Option<bool>,
    ) -> Result<Source> {
        // Authenticate first
        let user = backend.authenticate(credentials.clone()).await?;

        // Generate source ID
        let source_id = SourceId::new(format!("{}_{}", source_type, user.id));

        // Use the provided server URL

        // Create source type enum with provided machine_id if available
        let source_type_enum = match source_type.as_str() {
            "plex" => SourceType::PlexServer {
                machine_id: machine_id.clone().unwrap_or_default(),
                owned: is_owned.unwrap_or(true),
            },
            "jellyfin" => SourceType::JellyfinServer,
            "local" => SourceType::LocalFolder {
                path: std::path::PathBuf::from("/"),
            },
            _ => SourceType::PlexServer {
                machine_id: machine_id.clone().unwrap_or_default(),
                owned: is_owned.unwrap_or(true),
            },
        };

        // Save to database
        let repo = SourceRepositoryImpl::new(db.clone());
        let entity = SourceModel {
            id: source_id.to_string(),
            name: name.clone(),
            source_type,
            auth_provider_id: Some(user.id.clone()),
            connection_url: server_url.clone(),
            connections: None, // Will be populated later by connection discovery
            machine_id,        // Set the machine_id if provided
            is_owned: is_owned.unwrap_or(true), // Use provided value or default to owned
            is_online: true,
            last_sync: None,
            last_connection_test: None,
            connection_failure_count: 0,
            connection_quality: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };

        let source = Source {
            id: source_id.to_string(),
            name: entity.name.clone(),
            source_type: source_type_enum,
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
        Self::save_credentials(db, &source_id, &credentials).await?;

        info!("Created new source: {}", source_id);
        Ok(source)
    }

    /// Remove a source and its credentials
    pub async fn remove_source(db: &DatabaseConnection, source_id: &SourceId) -> Result<()> {
        // Remove credentials
        Self::remove_credentials(db, source_id).await?;

        // Remove from database
        let repo = SourceRepositoryImpl::new(db.clone());
        repo.delete(source_id.as_ref()).await?;

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
        let credentials = Self::load_credentials(db, source_id)
            .await?
            .context("No credentials found for source")?;

        // Re-authenticate
        let user = Self::authenticate(backend, credentials.clone()).await?;

        // Update source in database
        let repo = SourceRepositoryImpl::new(db.clone());
        if let Some(mut source) = repo.find_by_id(source_id.as_ref()).await? {
            source.is_online = true;
            source.auth_provider_id = Some(user.id);
            source.updated_at = chrono::Utc::now().naive_utc();
            repo.update(source).await?;
        }

        info!("Re-authenticated source: {}", source_id);
        Ok(())
    }
}
