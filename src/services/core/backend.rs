use crate::backends::{jellyfin::JellyfinBackend, plex::PlexBackend, traits::MediaBackend};
use crate::db::connection::DatabaseConnection;
use crate::db::repository::{
    Repository,
    media_repository::{MediaRepository, MediaRepositoryImpl},
    source_repository::{SourceRepository, SourceRepositoryImpl},
};
use crate::models::{
    AuthProvider, ConnectionInfo, Credentials, MediaItemId, Source, SourceId, SourceType,
    StreamInfo,
};
use crate::services::core::auth::AuthService;
use anyhow::{Context, Result};

/// Stateless backend service following Relm4's pure function pattern
/// All backend operations are pure functions that take dependencies as parameters
pub struct BackendService;

impl BackendService {
    /// Get stream URL for a media item - pure function that creates backend on demand
    pub async fn get_stream_url(
        db: &DatabaseConnection,
        media_item_id: &MediaItemId,
    ) -> Result<StreamInfo> {
        // Load media item to find its source
        let media_repo = MediaRepositoryImpl::new_without_events(db.clone());
        let media_item = media_repo
            .find_by_id(media_item_id.as_str())
            .await?
            .ok_or_else(|| anyhow::anyhow!("Media item not found"))?;

        // Load source configuration
        let source_repo = SourceRepositoryImpl::new_without_events(db.clone());
        let source_entity = source_repo
            .find_by_id(&media_item.source_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Source not found"))?;

        // Create backend and get stream URL
        let backend = Self::create_backend_for_source(db, &source_entity).await?;
        backend.get_stream_url(media_item_id).await
    }

    /// Create a backend instance for a source - stateless factory
    async fn create_backend_for_source(
        db: &DatabaseConnection,
        source_entity: &crate::db::entities::sources::Model,
    ) -> Result<Box<dyn MediaBackend>> {
        // Load credentials from secure storage
        let source_id = SourceId::new(source_entity.id.clone());
        let credentials = AuthService::load_credentials(&source_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("No credentials found for source"))?;

        // Create AuthProvider based on credentials and source type
        let auth_provider = Self::create_auth_provider(&credentials, source_entity)?;

        // Create Source struct from entity
        let source = Self::entity_to_source(source_entity);

        // Create and initialize the appropriate backend
        let backend: Box<dyn MediaBackend> = match source_entity.source_type.as_str() {
            "plex" | "PlexServer" => {
                let backend = PlexBackend::from_auth(auth_provider, source)
                    .context("Failed to create Plex backend")?;
                backend.initialize().await?;
                Box::new(backend)
            }
            "jellyfin" | "JellyfinServer" => {
                let backend = JellyfinBackend::from_auth(auth_provider, source)
                    .context("Failed to create Jellyfin backend")?;
                backend.initialize().await?;
                Box::new(backend)
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported source type: {}",
                    source_entity.source_type
                ));
            }
        };

        Ok(backend)
    }

    /// Create AuthProvider from credentials - pure transformation
    fn create_auth_provider(
        credentials: &Credentials,
        source: &crate::db::entities::sources::Model,
    ) -> Result<AuthProvider> {
        let auth_provider = match credentials {
            Credentials::Token { token } => AuthProvider::PlexAccount {
                id: source.auth_provider_id.clone().unwrap_or_default(),
                username: String::new(),
                email: String::new(),
                token: token.clone(),
                refresh_token: None,
                token_expiry: None,
            },
            Credentials::UsernamePassword { username, .. } => AuthProvider::JellyfinAuth {
                id: source.auth_provider_id.clone().unwrap_or_default(),
                server_url: source.connection_url.clone().unwrap_or_default(),
                username: username.clone(),
                user_id: String::new(),
                access_token: String::new(), // Will be populated during initialization
            },
            _ => return Err(anyhow::anyhow!("Unsupported credential type")),
        };

        Ok(auth_provider)
    }

    /// Convert database entity to Source model - pure transformation
    fn entity_to_source(entity: &crate::db::entities::sources::Model) -> Source {
        Source {
            id: entity.id.clone(),
            name: entity.name.clone(),
            source_type: match entity.source_type.as_str() {
                "plex" | "PlexServer" => SourceType::PlexServer {
                    machine_id: entity.auth_provider_id.clone().unwrap_or_default(),
                    owned: true,
                },
                "jellyfin" | "JellyfinServer" => SourceType::JellyfinServer,
                _ => SourceType::LocalFolder {
                    path: std::path::PathBuf::new(),
                },
            },
            auth_provider_id: entity.auth_provider_id.clone(),
            connection_info: ConnectionInfo {
                primary_url: entity.connection_url.clone(),
                is_online: entity.is_online,
                last_check: Some(chrono::Utc::now()),
            },
            enabled: true,
            last_sync: entity.last_sync.map(|dt| dt.and_utc()),
            library_count: 0,
        }
    }

    /// Sync a source - creates backend on demand, performs sync, then discards
    pub async fn sync_source(
        db: &DatabaseConnection,
        source_id: &SourceId,
    ) -> Result<crate::backends::traits::SyncResult> {
        // Load source configuration
        let source_repo = SourceRepositoryImpl::new_without_events(db.clone());
        let source_entity = source_repo
            .find_by_id(source_id.as_str())
            .await?
            .ok_or_else(|| anyhow::anyhow!("Source not found"))?;

        // Create backend and perform sync
        let backend = Self::create_backend_for_source(db, &source_entity).await?;

        // Get libraries and sync them
        let libraries = backend.get_libraries().await?;

        Ok(crate::backends::traits::SyncResult {
            backend_id: crate::models::BackendId::new(source_id.as_str()),
            success: true,
            items_synced: libraries.len(),
            duration: std::time::Duration::from_secs(0),
            errors: Vec::new(),
        })
    }

    /// Test connection for a source - stateless connection test
    pub async fn test_connection(db: &DatabaseConnection, source_id: &SourceId) -> Result<bool> {
        // Load source and try to create backend
        let source_repo = SourceRepositoryImpl::new_without_events(db.clone());
        let source_entity = source_repo
            .find_by_id(source_id.as_str())
            .await?
            .ok_or_else(|| anyhow::anyhow!("Source not found"))?;

        // Try to create and initialize backend
        match Self::create_backend_for_source(db, &source_entity).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}
