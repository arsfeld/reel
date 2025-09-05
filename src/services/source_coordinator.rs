use anyhow::{Result, anyhow};
use chrono;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::backends::{
    BackendManager, jellyfin::JellyfinBackend, local::LocalBackend, plex::PlexBackend,
    traits::MediaBackend,
};
use crate::models::{AuthProvider, Library, Source, SourceType};
use crate::services::{AuthManager, DataService, SyncManager};

/// Status of a media source connection
#[derive(Debug, Clone)]
pub enum ConnectionStatus {
    Connected,
    Offline,
    NeedsAuth,
    Error(String),
}

/// Information about a source's current state
#[derive(Debug, Clone)]
pub struct SourceStatus {
    pub source_id: String,
    pub source_name: String,
    pub source_type: SourceType,
    pub connection_status: ConnectionStatus,
    pub library_count: usize,
}

/// Result of a sync operation
#[derive(Debug, Clone)]
pub struct SyncResult {
    pub source_id: String,
    pub success: bool,
    pub libraries_synced: usize,
    pub error: Option<String>,
}

/// Coordinates all source and backend management
///
/// This service centralizes the lifecycle management of media sources,
/// including authentication, backend creation, and sync coordination.
#[derive(Clone)]
pub struct SourceCoordinator {
    auth_manager: Arc<AuthManager>,
    backend_manager: Arc<RwLock<BackendManager>>,
    sync_manager: Arc<SyncManager>,
    data_service: Arc<DataService>,
    source_statuses: Arc<RwLock<HashMap<String, SourceStatus>>>,
}

impl SourceCoordinator {
    pub fn new(
        auth_manager: Arc<AuthManager>,
        backend_manager: Arc<RwLock<BackendManager>>,
        sync_manager: Arc<SyncManager>,
        data_service: Arc<DataService>,
    ) -> Self {
        Self {
            auth_manager,
            backend_manager,
            sync_manager,
            data_service,
            source_statuses: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get the auth manager
    pub fn get_auth_manager(&self) -> &Arc<AuthManager> {
        &self.auth_manager
    }

    /// Add a Plex account and discover its servers
    pub async fn add_plex_account(&self, token: &str) -> Result<Vec<Source>> {
        info!("Adding Plex account through SourceCoordinator");

        // Use AuthManager to add the account and discover servers
        let (provider_id, sources) = self.auth_manager.add_plex_account(token).await?;

        // Get the provider for backend creation
        let provider = self
            .auth_manager
            .get_provider(&provider_id)
            .await
            .ok_or_else(|| anyhow!("Failed to retrieve newly added provider"))?;

        // Save all discovered sources to database first
        self.data_service
            .sync_sources_to_database(&provider_id, &sources)
            .await?;
        info!(
            "Saved {} Plex sources to database for provider {}",
            sources.len(),
            provider_id
        );

        // Create backends for each discovered source
        for source in &sources {
            match self
                .create_and_register_backend(provider.clone(), source.clone())
                .await
            {
                Ok(status) => {
                    info!("Successfully created backend for source: {}", source.name);
                    let mut statuses = self.source_statuses.write().await;
                    statuses.insert(source.id.clone(), status);
                }
                Err(e) => {
                    error!("Failed to create backend for source {}: {}", source.name, e);
                }
            }
        }

        Ok(sources)
    }

    /// Add a Jellyfin server
    pub async fn add_jellyfin_source(
        &self,
        server_url: &str,
        username: &str,
        password: &str,
        access_token: &str,
        user_id: &str,
    ) -> Result<Source> {
        info!("Adding Jellyfin source through SourceCoordinator");

        // Use AuthManager to add the Jellyfin auth
        let (provider_id, source) = self
            .auth_manager
            .add_jellyfin_auth(server_url, username, password, access_token, user_id)
            .await?;

        // Get the provider for backend creation
        let provider = self
            .auth_manager
            .get_provider(&provider_id)
            .await
            .ok_or_else(|| anyhow!("Failed to retrieve newly added provider"))?;

        // Save the source to the database
        let source_type_str = match &source.source_type {
            SourceType::PlexServer { .. } => "plex",
            SourceType::JellyfinServer => "jellyfin",
            SourceType::NetworkShare { .. } => "local",
            SourceType::LocalFolder { .. } => "local",
        };

        let source_model = crate::db::entities::SourceModel {
            id: source.id.clone(),
            name: source.name.clone(),
            source_type: source_type_str.to_string(),
            auth_provider_id: source.auth_provider_id.clone(),
            connection_url: source.connection_info.primary_url.clone(),
            is_online: true,
            last_sync: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };

        self.data_service.add_source(source_model).await?;
        info!("Saved Jellyfin source to database: {}", source.id);

        // Create and register the backend
        let status = self
            .create_and_register_backend(provider, source.clone())
            .await?;

        let mut statuses = self.source_statuses.write().await;
        statuses.insert(source.id.clone(), status);

        Ok(source)
    }

    /// Initialize all configured sources at startup - offline-first approach
    pub async fn initialize_all_sources(&self) -> Result<Vec<SourceStatus>> {
        info!("Initializing all sources - offline-first");

        let providers = self.auth_manager.get_all_providers().await;
        info!("Found {} auth providers to initialize", providers.len());
        for provider in &providers {
            info!(
                "Auth provider: {} ({})",
                provider.id(),
                provider.provider_type()
            );
        }

        let mut expected_source_ids = Vec::new();
        let mut all_statuses = Vec::new();

        for provider in providers {
            match &provider {
                AuthProvider::PlexAccount { id, .. } => {
                    // First try to get cached sources for instant display
                    let cached_sources = self.auth_manager.get_cached_sources(id).await;

                    if let Some(sources) = cached_sources {
                        info!(
                            "Loading {} cached Plex sources for provider {}",
                            sources.len(),
                            id
                        );

                        // Ensure cached sources are also in database (in case they're missing)
                        if let Err(e) = self
                            .data_service
                            .sync_sources_to_database(id, &sources)
                            .await
                        {
                            warn!("Failed to sync cached sources to database: {}", e);
                        }

                        for source in sources {
                            expected_source_ids.push(source.id.clone());

                            // Initialize the source (create backend and test connection)
                            match self
                                .initialize_source(provider.clone(), source.clone())
                                .await
                            {
                                Ok(status) => {
                                    info!(
                                        "Initialized cached source: {} - {:?}",
                                        source.name, status.connection_status
                                    );
                                    all_statuses.push(status);
                                }
                                Err(e) => {
                                    error!(
                                        "Failed to initialize cached source {}: {}",
                                        source.name, e
                                    );
                                    // Even if initialization fails, add an offline status
                                    let status = SourceStatus {
                                        source_id: source.id.clone(),
                                        source_name: source.name.clone(),
                                        source_type: source.source_type.clone(),
                                        connection_status: ConnectionStatus::Error(e.to_string()),
                                        library_count: 0,
                                    };
                                    all_statuses.push(status.clone());

                                    let mut statuses = self.source_statuses.write().await;
                                    statuses.insert(source.id.clone(), status);
                                }
                            }
                        }

                        // Trigger background refresh to update any changes
                        self.refresh_sources_background(id).await;
                    } else {
                        // No cache, try to discover online
                        match self.auth_manager.discover_plex_sources(id).await {
                            Ok(sources) => {
                                // Save discovered sources to database
                                if let Err(e) = self
                                    .data_service
                                    .sync_sources_to_database(id, &sources)
                                    .await
                                {
                                    error!("Failed to sync Plex sources to database: {}", e);
                                }

                                for source in sources {
                                    expected_source_ids.push(source.id.clone());
                                    match self.initialize_source(provider.clone(), source).await {
                                        Ok(status) => all_statuses.push(status),
                                        Err(e) => {
                                            error!("Failed to initialize Plex source: {}", e);
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                error!(
                                    "Failed to discover Plex sources for provider {}: {}",
                                    id, e
                                );
                            }
                        }
                    }
                }
                AuthProvider::JellyfinAuth { id, server_url, .. } => {
                    info!("Processing Jellyfin auth provider: {}", id);

                    // Create source from Jellyfin auth provider
                    let source = Source::new(
                        format!("source_{}", id),
                        format!("Jellyfin - {}", server_url),
                        SourceType::JellyfinServer,
                        Some(id.clone()),
                    );

                    info!("Created Jellyfin source: {} ({})", source.name, source.id);

                    // Sync Jellyfin source to database like Plex does
                    if let Err(e) = self
                        .data_service
                        .sync_sources_to_database(id, &[source.clone()])
                        .await
                    {
                        warn!("Failed to sync Jellyfin source to database: {}", e);
                    }

                    expected_source_ids.push(source.id.clone());
                    info!("Initializing Jellyfin source: {}", source.id);
                    match self.initialize_source(provider.clone(), source).await {
                        Ok(status) => {
                            info!(
                                "Successfully initialized Jellyfin source: {} - status: {:?}",
                                status.source_id, status.connection_status
                            );
                            all_statuses.push(status)
                        }
                        Err(e) => {
                            error!("Failed to initialize Jellyfin source: {}", e);
                        }
                    }
                }
                AuthProvider::LocalFiles { id } => {
                    // Create source from local files provider
                    let source = Source::new(
                        format!("source_{}", id),
                        "Local Files".to_string(),
                        SourceType::LocalFolder {
                            path: std::path::PathBuf::from("~/Videos"),
                        },
                        Some(id.clone()),
                    );

                    expected_source_ids.push(source.id.clone());
                    match self.initialize_source(provider.clone(), source).await {
                        Ok(status) => all_statuses.push(status),
                        Err(e) => {
                            error!("Failed to initialize local source: {}", e);
                        }
                    }
                }
                _ => {
                    warn!("Unhandled provider type: {:?}", provider);
                }
            }
        }

        // Perform authoritative cleanup - mark any sources not in config as offline
        if !expected_source_ids.is_empty() {
            info!(
                "Cleaning up database to match config - {} expected sources",
                expected_source_ids.len()
            );
            match self
                .data_service
                .cleanup_sources_from_config(&expected_source_ids)
                .await
            {
                Ok(archived_sources) => {
                    if !archived_sources.is_empty() {
                        info!(
                            "Config-based cleanup completed - {} invalid sources marked offline",
                            archived_sources.len()
                        );
                    }
                }
                Err(e) => {
                    error!("Failed to cleanup sources from config: {}", e);
                }
            }
        }

        // Log final statuses for debugging
        let final_statuses = self.source_statuses.read().await;
        info!("Final source statuses after initialization:");
        for (source_id, status) in final_statuses.iter() {
            info!(
                "  {} - {} ({:?})",
                source_id, status.source_name, status.connection_status
            );
        }
        drop(final_statuses);

        Ok(all_statuses)
    }

    /// Sync a specific source
    pub async fn sync_source(&self, source_id: &str) -> Result<SyncResult> {
        info!("Syncing source: {}", source_id);

        // Get the backend for this source
        let backend = {
            let backend_manager = self.backend_manager.read().await;
            backend_manager
                .get_backend(source_id)
                .ok_or_else(|| anyhow!("No backend found for source: {}", source_id))?
        };

        // Perform the sync
        let libraries = match backend.get_libraries().await {
            Ok(libs) => libs,
            Err(e) => {
                return Ok(SyncResult {
                    source_id: source_id.to_string(),
                    success: false,
                    libraries_synced: 0,
                    error: Some(e.to_string()),
                });
            }
        };

        // Sync each library
        let mut synced_count = 0;
        for library in &libraries {
            // We already have the backend, so we can sync directly
            match self
                .sync_manager
                .sync_backend(source_id, backend.clone())
                .await
            {
                Ok(_) => {
                    synced_count += 1;
                }
                Err(e) => {
                    error!("Failed to sync library {}: {}", library.title, e);
                }
            }
            // For now, we sync the entire backend again for each library
            // This is inefficient but works until we have proper library-level sync
            break; // Only sync once for the entire backend
        }

        if !libraries.is_empty() {
            synced_count = libraries.len(); // Report all libraries as synced
        }

        // Update status
        let library_count = libraries.len();
        if let Some(status) = self.source_statuses.write().await.get_mut(source_id) {
            status.library_count = library_count;
        }

        // Update the source's library count in the auth manager cache
        if let Err(e) = self
            .auth_manager
            .update_source_library_count(source_id, library_count)
            .await
        {
            warn!("Failed to update source library count in cache: {}", e);
        }

        Ok(SyncResult {
            source_id: source_id.to_string(),
            success: true,
            libraries_synced: synced_count,
            error: None,
        })
    }

    /// Sync all visible sources
    pub async fn sync_all_visible_sources(&self) -> Result<Vec<SyncResult>> {
        info!("Syncing all visible sources");

        let mut results = Vec::new();
        let statuses = self.source_statuses.read().await;

        info!("Found {} sources in statuses map", statuses.len());
        info!("Current sources in statuses map:");
        for (source_id, status) in statuses.iter() {
            info!(
                "  {} - {} ({:?})",
                source_id, status.source_name, status.connection_status
            );
        }

        for (source_id, status) in statuses.iter() {
            info!(
                "Checking source {} ({}) with status: {:?}",
                source_id, status.source_name, status.connection_status
            );
            if matches!(status.connection_status, ConnectionStatus::Connected) {
                info!("Source {} is connected, syncing...", source_id);
                match self.sync_source(source_id).await {
                    Ok(result) => results.push(result),
                    Err(e) => {
                        error!("Failed to sync source {}: {}", source_id, e);
                        results.push(SyncResult {
                            source_id: source_id.clone(),
                            success: false,
                            libraries_synced: 0,
                            error: Some(e.to_string()),
                        });
                    }
                }
            }
        }

        Ok(results)
    }

    /// Get all visible libraries from connected sources
    pub async fn get_visible_libraries(&self) -> Result<Vec<(Source, Library)>> {
        let mut all_libraries = Vec::new();
        let providers = self.auth_manager.get_all_providers().await;

        for provider in providers {
            let sources = match &provider {
                AuthProvider::PlexAccount { id, .. } => self
                    .auth_manager
                    .discover_plex_sources(id)
                    .await
                    .unwrap_or_default(),
                AuthProvider::JellyfinAuth { id, server_url, .. } => {
                    vec![Source::new(
                        format!("source_{}", id),
                        format!("Jellyfin - {}", server_url),
                        SourceType::JellyfinServer,
                        Some(id.clone()),
                    )]
                }
                AuthProvider::LocalFiles { id } => {
                    vec![Source::new(
                        format!("source_{}", id),
                        "Local Files".to_string(),
                        SourceType::LocalFolder {
                            path: std::path::PathBuf::from("~/Videos"),
                        },
                        Some(id.clone()),
                    )]
                }
                _ => vec![],
            };

            for source in sources {
                let backend = {
                    let backend_manager = self.backend_manager.read().await;
                    backend_manager.get_backend(&source.id)
                };
                if let Some(backend) = backend {
                    match backend.get_libraries().await {
                        Ok(libraries) => {
                            for library in libraries {
                                all_libraries.push((source.clone(), library));
                            }
                        }
                        Err(e) => {
                            warn!("Failed to get libraries for source {}: {}", source.id, e);
                        }
                    }
                }
            }
        }

        Ok(all_libraries)
    }

    /// Remove a source and its associated backend
    pub async fn remove_source(&self, source_id: &str) -> Result<()> {
        info!("Removing source: {}", source_id);

        // Remove from backend manager
        {
            let mut backend_manager = self.backend_manager.write().await;
            backend_manager.remove_backend(source_id);
        }

        // Remove from statuses
        self.source_statuses.write().await.remove(source_id);

        // Remove from AuthManager when it supports source removal
        // For now, we just log it
        info!("Note: Source removal from AuthManager not yet implemented");

        Ok(())
    }

    /// Get the current status of a source
    pub async fn get_source_status(&self, source_id: &str) -> Option<SourceStatus> {
        self.source_statuses.read().await.get(source_id).cloned()
    }

    /// Get all source statuses
    pub async fn get_all_source_statuses(&self) -> Vec<SourceStatus> {
        self.source_statuses
            .read()
            .await
            .values()
            .cloned()
            .collect()
    }

    // Private helper methods

    async fn initialize_source(
        &self,
        provider: AuthProvider,
        source: Source,
    ) -> Result<SourceStatus> {
        info!("Initializing source: {} ({})", source.name, source.id);
        let status = self
            .create_and_register_backend(provider, source.clone())
            .await?;

        let mut statuses = self.source_statuses.write().await;
        info!(
            "Adding source {} to statuses map with status: {:?}",
            source.id, status.connection_status
        );
        statuses.insert(source.id.clone(), status.clone());

        Ok(status)
    }

    async fn create_and_register_backend(
        &self,
        provider: AuthProvider,
        source: Source,
    ) -> Result<SourceStatus> {
        // Create the appropriate backend based on provider type
        let backend: Arc<dyn MediaBackend> = match &provider {
            AuthProvider::PlexAccount { .. } => Arc::new(PlexBackend::from_auth(
                provider,
                source.clone(),
                self.auth_manager.clone(),
            )?),
            AuthProvider::JellyfinAuth { .. } => Arc::new(JellyfinBackend::from_auth(
                provider,
                source.clone(),
                self.auth_manager.clone(),
            )?),
            AuthProvider::LocalFiles { .. } => Arc::new(LocalBackend::from_auth(
                provider,
                source.clone(),
                self.auth_manager.clone(),
                Some(self.data_service.clone()),
            )?),
            _ => return Err(anyhow!("Unsupported provider type")),
        };

        // Register with backend manager
        {
            let mut backend_manager = self.backend_manager.write().await;
            backend_manager.register_backend(source.id.clone(), backend.clone());
        }

        // Test connection
        let connection_status = match backend.initialize().await {
            Ok(_) => {
                info!("Backend {} connected successfully", source.name);
                ConnectionStatus::Connected
            }
            Err(e) => {
                warn!("Backend {} failed to connect: {}", source.name, e);
                ConnectionStatus::Error(e.to_string())
            }
        };

        Ok(SourceStatus {
            source_id: source.id,
            source_name: source.name,
            source_type: source.source_type,
            connection_status,
            library_count: 0,
        })
    }

    /// Migrate legacy backends to the new AuthProvider model
    pub async fn migrate_legacy_backends(&self) -> Result<()> {
        info!("Checking for legacy backends to migrate");

        // This will be handled by AuthManager's migrate_legacy_backends
        // which also cleans up incorrectly added backends
        self.auth_manager.migrate_legacy_backends().await?;

        // Re-initialize all sources after migration
        self.initialize_all_sources().await?;

        Ok(())
    }

    // Additional methods for backend management

    /// Get a specific backend by ID
    pub async fn get_backend(&self, backend_id: &str) -> Option<Arc<dyn MediaBackend>> {
        let backend_manager = self.backend_manager.read().await;
        backend_manager.get_backend(backend_id)
    }

    /// Get all backends
    pub async fn get_all_backends(&self) -> Vec<(String, Arc<dyn MediaBackend>)> {
        let backend_manager = self.backend_manager.read().await;
        backend_manager.get_all_backends()
    }

    /// List all backends with their info
    pub async fn list_backends(&self) -> Vec<(String, crate::backends::traits::BackendInfo)> {
        let backend_manager = self.backend_manager.read().await;
        backend_manager.list_backends()
    }

    /// Reorder backends
    pub async fn reorder_backends(&self, new_order: Vec<String>) -> Result<()> {
        let mut backend_manager = self.backend_manager.write().await;
        backend_manager.reorder_backends(new_order);
        Ok(())
    }

    /// Move a backend up in the order
    pub async fn move_backend_up(&self, backend_id: &str) -> Result<()> {
        let mut backend_manager = self.backend_manager.write().await;
        backend_manager.move_backend_up(backend_id);
        Ok(())
    }

    /// Move a backend down in the order
    pub async fn move_backend_down(&self, backend_id: &str) -> Result<()> {
        let mut backend_manager = self.backend_manager.write().await;
        backend_manager.move_backend_down(backend_id);
        Ok(())
    }

    /// Refresh all backends
    pub async fn refresh_all_backends(&self) -> Result<Vec<crate::backends::traits::SyncResult>> {
        let backend_manager = self.backend_manager.read().await;
        backend_manager.refresh_all_backends().await
    }

    /// Force a complete sync of all sources from config to database
    /// This will update the database cache with the latest source information and friendly names
    pub async fn force_sync_all_sources(&self) -> Result<()> {
        info!("Forcing complete sync of all sources from config to database");

        let providers = self.auth_manager.get_all_providers().await;

        for provider in providers {
            match &provider {
                AuthProvider::PlexAccount { id, .. } => {
                    // Sync cached Plex sources
                    if let Some(sources) = self.auth_manager.get_cached_sources(id).await {
                        info!("Syncing {} Plex sources for provider {}", sources.len(), id);
                        if let Err(e) = self
                            .data_service
                            .sync_sources_to_database(id, &sources)
                            .await
                        {
                            error!("Failed to sync Plex sources for provider {}: {}", id, e);
                        }
                    }

                    // Also try to discover fresh sources if online
                    if let Ok(fresh_sources) = self.auth_manager.discover_plex_sources(id).await {
                        info!(
                            "Syncing {} fresh Plex sources for provider {}",
                            fresh_sources.len(),
                            id
                        );
                        if let Err(e) = self
                            .data_service
                            .sync_sources_to_database(id, &fresh_sources)
                            .await
                        {
                            error!(
                                "Failed to sync fresh Plex sources for provider {}: {}",
                                id, e
                            );
                        }
                    }
                }
                AuthProvider::JellyfinAuth { id, server_url, .. } => {
                    // Create and sync Jellyfin source
                    let source = crate::models::Source::new(
                        format!("source_{}", id),
                        format!("Jellyfin - {}", server_url),
                        crate::models::SourceType::JellyfinServer,
                        Some(id.clone()),
                    );

                    info!("Syncing Jellyfin source for provider {}", id);
                    if let Err(e) = self
                        .data_service
                        .sync_sources_to_database(id, &[source])
                        .await
                    {
                        error!("Failed to sync Jellyfin source for provider {}: {}", id, e);
                    }
                }
                AuthProvider::LocalFiles { id } => {
                    // Create and sync Local source
                    let source = crate::models::Source::new(
                        format!("source_{}", id),
                        "Local Files".to_string(),
                        crate::models::SourceType::LocalFolder {
                            path: std::path::PathBuf::from("~/Videos"),
                        },
                        Some(id.clone()),
                    );

                    info!("Syncing Local source for provider {}", id);
                    if let Err(e) = self
                        .data_service
                        .sync_sources_to_database(id, &[source])
                        .await
                    {
                        error!("Failed to sync Local source for provider {}: {}", id, e);
                    }
                }
                _ => {}
            }
        }

        info!("Complete source sync finished");
        Ok(())
    }

    /// Refresh sources in background and sync to database (for dynamic Plex servers)
    pub async fn refresh_sources_background(&self, provider_id: &str) {
        let provider_id = provider_id.to_string();
        let self_clone = Arc::new(self.clone());

        tokio::spawn(async move {
            info!(
                "Background refresh of sources with database sync for provider {}",
                provider_id
            );

            // Discover latest sources
            match self_clone
                .auth_manager
                .discover_plex_sources(&provider_id)
                .await
            {
                Ok(sources) => {
                    // Sync to database (handles upsert and cleanup)
                    if let Err(e) = self_clone
                        .data_service
                        .sync_sources_to_database(&provider_id, &sources)
                        .await
                    {
                        error!(
                            "Failed to sync sources to database during background refresh: {}",
                            e
                        );
                    } else {
                        info!(
                            "Successfully synced {} sources to database for provider {}",
                            sources.len(),
                            provider_id
                        );
                    }
                }
                Err(e) => {
                    error!(
                        "Failed to discover sources during background refresh: {}",
                        e
                    );
                }
            }
        });
    }
}
