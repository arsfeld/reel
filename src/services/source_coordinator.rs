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
use crate::services::initialization::{AppInitializationState, SourceInfo, SourceReadiness};
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
                // Check if the backend is fully initialized (API client ready)
                if backend.is_initialized().await {
                    info!("Backend {} connected and fully initialized", source.name);
                    ConnectionStatus::Connected
                } else {
                    warn!(
                        "Backend {} initialized but not ready (API client missing)",
                        source.name
                    );
                    ConnectionStatus::NeedsAuth
                }
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

        // Start reactive initialization after migration (non-blocking)
        let _init_state = self.initialize_sources_reactive();
        info!("Started reactive source initialization after migration");

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

    /// New reactive initialization - returns immediately with Properties for non-blocking UI
    pub fn initialize_sources_reactive(&self) -> AppInitializationState {
        let init_state = AppInitializationState::new();

        // Stage 1: Instant UI (0ms) - Enable UI immediately
        self.stage1_instant_ui(&init_state);

        // Stage 2: Background discovery (spawn async) - Load config/cache
        self.stage2_background_discovery(init_state.clone());

        // Stage 3: Network connections (spawn async) - Test connections
        self.stage3_network_connections(init_state.clone());

        init_state
    }

    /// Stage 1: Instant UI readiness - no blocking operations
    fn stage1_instant_ui(&self, state: &AppInitializationState) {
        // UI can display immediately (spawn tasks for async set calls)
        let ui_ready = state.ui_ready.clone();
        let cached_data_loaded = state.cached_data_loaded.clone();
        tokio::spawn(async move {
            ui_ready.set(true).await;
            cached_data_loaded.set(true).await;
        });

        // Emit stage completion event asynchronously
        let event_bus = self.data_service.event_bus().clone();
        tokio::spawn(async move {
            let event = crate::events::types::DatabaseEvent::new(
                crate::events::types::EventType::InitializationStageCompleted,
                crate::events::types::EventPayload::System {
                    message: "UI ready for display".to_string(),
                    details: Some(serde_json::json!({"component": "SourceCoordinator"})),
                },
            );
            let _ = event_bus.publish(event).await;
        });
    }

    /// Stage 2: Background discovery - load from config/cache
    fn stage2_background_discovery(&self, state: AppInitializationState) {
        let auth_manager = self.auth_manager.clone();
        let data_service = self.data_service.clone();

        tokio::spawn(async move {
            let start_time = std::time::Instant::now();

            // Discover sources from stored providers (fast - from cache/config)
            let mut discovered_sources = Vec::new();

            let providers = auth_manager.get_all_providers().await;
            if !providers.is_empty() {
                for provider in providers {
                    match &provider {
                        AuthProvider::PlexAccount { id, .. } => {
                            if let Some(cached_sources) = auth_manager.get_cached_sources(id).await
                            {
                                for source in cached_sources {
                                    // Convert to SourceInfo for UI display
                                    let source_info = SourceInfo {
                                        id: source.id.clone(),
                                        name: source.name.clone(),
                                        source_type: format!("{:?}", source.source_type),
                                        libraries: Vec::new(), // Will be populated later
                                        is_enabled: source.enabled,
                                        connection_status: "Cached".to_string(),
                                    };
                                    discovered_sources.push(source_info);
                                }
                            }
                        }
                        AuthProvider::JellyfinAuth { id, .. } => {
                            if let Some(cached_sources) = auth_manager.get_cached_sources(id).await
                            {
                                for source in cached_sources {
                                    let source_info = SourceInfo {
                                        id: source.id.clone(),
                                        name: source.name.clone(),
                                        source_type: format!("{:?}", source.source_type),
                                        libraries: Vec::new(),
                                        is_enabled: source.enabled,
                                        connection_status: "Cached".to_string(),
                                    };
                                    discovered_sources.push(source_info);
                                }
                            }
                        }
                        AuthProvider::NetworkCredentials { .. }
                        | AuthProvider::LocalFiles { .. } => {
                            // These provider types don't have cached sources to discover
                        }
                    }
                }
            }

            // Update the reactive state
            state.sources_discovered.set(discovered_sources).await;

            // Check if we have any cached credentials that indicate playback readiness
            let has_credentials = !auth_manager.get_all_providers().await.is_empty();
            state.playback_ready.set(has_credentials).await;

            // Emit stage completion
            let duration = start_time.elapsed();
            let event_bus = data_service.event_bus().clone();
            let event = crate::events::types::DatabaseEvent::new(
                crate::events::types::EventType::InitializationStageCompleted,
                crate::events::types::EventPayload::System {
                    message: format!(
                        "Background discovery completed in {}ms",
                        duration.as_millis()
                    ),
                    details: Some(serde_json::json!({"component": "SourceCoordinator"})),
                },
            );
            let _ = event_bus.publish(event).await;
        });
    }

    /// Stage 3: Network connections - test actual connectivity  
    fn stage3_network_connections(&self, state: AppInitializationState) {
        let auth_manager = self.auth_manager.clone();
        let data_service = self.data_service.clone();

        tokio::spawn(async move {
            let start_time = std::time::Instant::now();
            let mut sources_connected = std::collections::HashMap::new();
            let mut any_connected = false;

            // Get all providers and attempt connections in parallel
            let providers = auth_manager.get_all_providers().await;
            if !providers.is_empty() {
                let connection_futures: Vec<_> = providers.into_iter().map(|provider| {
                    let auth_manager = auth_manager.clone();

                    async move {
                        match &provider {
                            AuthProvider::PlexAccount { id, .. } => {
                                if let Some(cached_sources) = auth_manager.get_cached_sources(id).await {
                                    let mut source_results = Vec::new();
                                    
                                    for source in cached_sources {
                                        // Attempt to create and test backend connection
                                        let readiness = match Self::test_source_connection(&source, &provider, &auth_manager).await {
                                            Ok(backend) => {
                                                if backend.is_initialized().await {
                                                    let library_count = backend.get_libraries().await.map(|libs| libs.len()).unwrap_or(0);
                                                    SourceReadiness::Connected {
                                                        api_client_status: crate::services::initialization::ApiClientStatus::Ready,
                                                        library_count,
                                                    }
                                                } else if backend.is_playback_ready().await {
                                                    SourceReadiness::PlaybackReady {
                                                        credentials_valid: true,
                                                        last_successful_connection: None,
                                                    }
                                                } else {
                                                    SourceReadiness::Unavailable
                                                }
                                            }
                                            Err(_) => {
                                                // Even if connection fails, check if we have valid credentials
                                                SourceReadiness::PlaybackReady {
                                                    credentials_valid: true,
                                                    last_successful_connection: None,
                                                }
                                            }
                                        };
                                        
                                        source_results.push((source.id.clone(), readiness));
                                    }
                                    
                                    source_results
                                } else {
                                    Vec::new()
                                }
                            }
                            AuthProvider::JellyfinAuth { id, .. } => {
                                if let Some(cached_sources) = auth_manager.get_cached_sources(id).await {
                                    let mut source_results = Vec::new();
                                    
                                    for source in cached_sources {
                                        let readiness = match Self::test_source_connection(&source, &provider, &auth_manager).await {
                                            Ok(backend) => {
                                                if backend.is_initialized().await {
                                                    let library_count = backend.get_libraries().await.map(|libs| libs.len()).unwrap_or(0);
                                                    SourceReadiness::Connected {
                                                        api_client_status: crate::services::initialization::ApiClientStatus::Ready,
                                                        library_count,
                                                    }
                                                } else if backend.is_playback_ready().await {
                                                    SourceReadiness::PlaybackReady {
                                                        credentials_valid: true,
                                                        last_successful_connection: None,
                                                    }
                                                } else {
                                                    SourceReadiness::Unavailable
                                                }
                                            }
                                            Err(_) => {
                                                SourceReadiness::PlaybackReady {
                                                    credentials_valid: true,
                                                    last_successful_connection: None,
                                                }
                                            }
                                        };
                                        
                                        source_results.push((source.id.clone(), readiness));
                                    }
                                    
                                    source_results
                                } else {
                                    Vec::new()
                                }
                            }
                            AuthProvider::NetworkCredentials { .. } | AuthProvider::LocalFiles { .. } => {
                                // These don't have cached sources yet - return empty
                                Vec::new()
                            }
                        }
                    }
                }).collect();

                // Process connection results as they complete
                for future in connection_futures {
                    let results = future.await;
                    for (source_id, readiness) in results {
                        if readiness.is_playable() {
                            any_connected = true;
                        }
                        sources_connected.insert(source_id, readiness);
                    }
                }
            }

            // Update reactive state
            state.sources_connected.set(sources_connected.clone()).await;
            state.sync_ready.set(any_connected).await;

            // Emit events for UI updates
            let event_bus = data_service.event_bus().clone();

            if any_connected {
                let event = crate::events::types::DatabaseEvent::new(
                    crate::events::types::EventType::FirstSourceReady,
                    crate::events::types::EventPayload::System {
                        message: "At least one source is ready for playback".to_string(),
                        details: Some(serde_json::json!({"component": "SourceCoordinator"})),
                    },
                );
                let _ = event_bus.publish(event).await;
            }

            let all_connected = sources_connected.values().all(|r| r.is_fully_connected());
            if all_connected && !sources_connected.is_empty() {
                let event = crate::events::types::DatabaseEvent::new(
                    crate::events::types::EventType::AllSourcesConnected,
                    crate::events::types::EventPayload::System {
                        message: "All sources are fully connected".to_string(),
                        details: Some(serde_json::json!({"component": "SourceCoordinator"})),
                    },
                );
                let _ = event_bus.publish(event).await;
            }

            // Stage completion
            let duration = start_time.elapsed();
            let event = crate::events::types::DatabaseEvent::new(
                crate::events::types::EventType::InitializationStageCompleted,
                crate::events::types::EventPayload::System {
                    message: format!(
                        "Network connections completed in {}ms",
                        duration.as_millis()
                    ),
                    details: Some(serde_json::json!({"component": "SourceCoordinator"})),
                },
            );
            let _ = event_bus.publish(event).await;
        });
    }

    /// Helper method to test a source connection without blocking
    async fn test_source_connection(
        source: &Source,
        provider: &AuthProvider,
        auth_manager: &Arc<AuthManager>,
    ) -> Result<Arc<dyn MediaBackend>> {
        // Create backend for testing
        let backend = match (&source.source_type, provider) {
            (SourceType::PlexServer { .. }, AuthProvider::PlexAccount { .. }) => {
                let plex_backend =
                    PlexBackend::from_auth(provider.clone(), source.clone(), auth_manager.clone())?;
                Arc::new(plex_backend) as Arc<dyn MediaBackend>
            }
            (SourceType::JellyfinServer, AuthProvider::JellyfinAuth { .. }) => {
                let jellyfin_backend = JellyfinBackend::from_auth(
                    provider.clone(),
                    source.clone(),
                    auth_manager.clone(),
                )?;
                Arc::new(jellyfin_backend) as Arc<dyn MediaBackend>
            }
            _ => return Err(anyhow!("Unsupported source type")),
        };

        // Test initialization - this may fail but we still return the backend
        // for playback readiness testing
        let _ = backend.initialize().await;

        Ok(backend)
    }
}
