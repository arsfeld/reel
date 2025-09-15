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
use sea_orm::{ActiveModelTrait, Set};

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
        let media_repo = MediaRepositoryImpl::new(db.clone());
        let media_item = media_repo
            .find_by_id(media_item_id.as_str())
            .await?
            .ok_or_else(|| anyhow::anyhow!("Media item not found"))?;

        // Load source configuration
        let source_repo = SourceRepositoryImpl::new(db.clone());
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
                    // Use the actual machine_id field from the database
                    machine_id: entity.machine_id.clone().unwrap_or_default(),
                    owned: entity.is_owned,
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
        use crate::db::repository::{LibraryRepository, LibraryRepositoryImpl};
        use std::sync::Arc;

        // Load source configuration
        let source_repo = SourceRepositoryImpl::new(db.clone());
        let source_entity = source_repo
            .find_by_id(source_id.as_str())
            .await?
            .ok_or_else(|| anyhow::anyhow!("Source not found"))?;

        // Create backend and perform sync
        let backend = Self::create_backend_for_source(db, &source_entity).await?;

        // Get libraries from backend
        let libraries = backend.get_libraries().await?;

        // Save libraries to database
        let library_repo = LibraryRepositoryImpl::new(db.clone());

        // Get existing libraries for this source to track what needs to be deleted
        let existing_libraries = library_repo.find_by_source(source_id.as_str()).await?;
        let existing_ids: std::collections::HashSet<String> = existing_libraries
            .iter()
            .map(|lib| lib.id.clone())
            .collect();

        // Track which libraries we've seen from the backend
        let mut seen_ids = std::collections::HashSet::new();

        // Upsert libraries - update if exists, insert if new
        let mut items_synced = 0;
        for library in &libraries {
            seen_ids.insert(library.id.clone());

            // Check if library already exists
            if let Some(existing) = library_repo.find_by_id(&library.id).await? {
                // Update existing library
                let mut updated = existing;
                updated.title = library.title.clone();
                updated.library_type = format!("{:?}", library.library_type).to_lowercase();
                updated.icon = library.icon.clone();
                updated.updated_at = chrono::Utc::now().naive_utc();

                library_repo.update(updated).await?;
            } else {
                // Insert new library
                let library_model = crate::db::entities::libraries::Model {
                    id: library.id.clone(),
                    source_id: source_id.as_str().to_string(),
                    title: library.title.clone(),
                    library_type: format!("{:?}", library.library_type).to_lowercase(),
                    icon: library.icon.clone(),
                    item_count: 0, // Will be updated when syncing media items
                    created_at: chrono::Utc::now().naive_utc(),
                    updated_at: chrono::Utc::now().naive_utc(),
                };

                library_repo.insert(library_model).await?;
            }
            items_synced += 1;

            // TODO: Sync media items for each library
            // This would involve calling backend.get_movies(library_id) or backend.get_shows(library_id)
            // and saving those to the database as well
        }

        // Delete libraries that no longer exist on the backend
        for existing_id in existing_ids {
            if !seen_ids.contains(&existing_id) {
                library_repo.delete(&existing_id).await?;
            }
        }

        // Update source last_sync time
        let mut source_active: crate::db::entities::sources::ActiveModel = source_entity.into();
        source_active.last_sync = sea_orm::Set(Some(chrono::Utc::now().naive_utc()));
        source_active.update(db.as_ref()).await?;

        Ok(crate::backends::traits::SyncResult {
            backend_id: crate::models::BackendId::new(source_id.as_str()),
            success: true,
            items_synced,
            duration: std::time::Duration::from_secs(0),
            errors: Vec::new(),
        })
    }

    /// Test connection for a source - stateless connection test
    pub async fn test_connection(db: &DatabaseConnection, source_id: &SourceId) -> Result<bool> {
        // Load source and try to create backend
        let source_repo = SourceRepositoryImpl::new(db.clone());
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

    /// Update playback progress on the backend server
    pub async fn update_playback_progress(
        db: &DatabaseConnection,
        source_id: &str,
        media_id: &MediaItemId,
        position: std::time::Duration,
        duration: std::time::Duration,
    ) -> Result<()> {
        // Load source configuration
        let source_repo = SourceRepositoryImpl::new(db.clone());
        let source_entity = source_repo
            .find_by_id(source_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Source not found"))?;

        // Create backend and update progress
        let backend = Self::create_backend_for_source(db, &source_entity).await?;
        backend.update_progress(media_id, position, duration).await
    }
}
