use anyhow::Result;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::log::{info, warn};

use crate::backends::BackendManager;
use crate::config::Config;
use crate::db::{Database, DatabaseConnection};
use crate::events::EventBus;
use crate::models::{Library, MediaItem, User};
use crate::services::{DataService, SourceCoordinator, SyncManager, auth_manager::AuthManager};

#[derive(Debug, Clone)]
pub enum PlaybackState {
    Idle,
    Loading,
    Playing,
    Paused,
    Stopped,
}

pub struct AppState {
    backend_manager: Arc<RwLock<BackendManager>>, // Made private
    pub auth_manager: Arc<AuthManager>,
    pub source_coordinator: Arc<SourceCoordinator>, // Made non-optional
    pub current_user: Arc<RwLock<Option<User>>>,
    pub current_library: Arc<RwLock<Option<Library>>>,
    pub libraries: Arc<RwLock<HashMap<String, Vec<Library>>>>, // backend_id -> libraries
    pub library_items: Arc<RwLock<HashMap<String, Vec<MediaItem>>>>, // library_id -> items
    pub data_service: Arc<DataService>,
    pub sync_manager: Arc<SyncManager>,
    pub playback_state: Arc<RwLock<PlaybackState>>,
    pub config: Arc<RwLock<Config>>,
    pub database: Arc<Database>,
    pub db_connection: DatabaseConnection,
    pub event_bus: Arc<EventBus>, // Event bus is now a required part of AppState
}

impl AppState {
    pub fn new(config: Arc<RwLock<Config>>) -> Result<Self> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async { Self::new_async(config).await })
    }

    async fn new_async(config: Arc<RwLock<Config>>) -> Result<Self> {
        // Create database connection once
        let database = Arc::new(Database::new().await?);
        let db_connection = database.get_connection();

        // Run migrations
        database.migrate().await?;

        // Create the event bus - this is central to the application
        let event_bus = Arc::new(EventBus::new(1000));

        let backend_manager = Arc::new(RwLock::new(BackendManager::new()));
        let auth_manager = Arc::new(AuthManager::new(config.clone(), event_bus.clone()));

        // Pass database connection and event bus to DataService
        let data_service = Arc::new(DataService::new(db_connection.clone(), event_bus.clone()));
        let sync_manager = Arc::new(SyncManager::new(data_service.clone(), event_bus.clone()));

        // Create SourceCoordinator directly
        let source_coordinator = Arc::new(SourceCoordinator::new(
            auth_manager.clone(),
            backend_manager.clone(),
            sync_manager.clone(),
            data_service.clone(),
        ));

        Ok(Self {
            backend_manager,
            auth_manager,
            source_coordinator,
            current_user: Arc::new(RwLock::new(None)),
            current_library: Arc::new(RwLock::new(None)),
            libraries: Arc::new(RwLock::new(HashMap::new())),
            library_items: Arc::new(RwLock::new(HashMap::new())),
            data_service,
            sync_manager,
            playback_state: Arc::new(RwLock::new(PlaybackState::Idle)),
            config,
            database,
            db_connection,
            event_bus,
        })
    }

    // This method is no longer needed since source_coordinator is initialized in new()
    pub fn get_source_coordinator(&self) -> &Arc<SourceCoordinator> {
        &self.source_coordinator
    }

    pub async fn set_user(&self, user: User) {
        let mut current_user = self.current_user.write().await;
        *current_user = Some(user);
    }

    pub async fn get_user(&self) -> Option<User> {
        self.current_user.read().await.clone()
    }

    pub async fn set_library(&self, library: Library) {
        let mut current_library = self.current_library.write().await;
        *current_library = Some(library);
    }

    pub async fn set_playback_state(&self, state: PlaybackState) {
        let mut playback_state = self.playback_state.write().await;
        *playback_state = state;
    }

    /// Sync data from all backends
    pub async fn sync_all_backends(&self) -> Result<Vec<crate::services::sync::SyncResult>> {
        let all_backends = self.source_coordinator.get_all_backends().await;

        let mut results = Vec::new();

        for (backend_id, backend) in all_backends {
            let sync_result = self
                .sync_manager
                .sync_backend(&backend_id, backend.clone())
                .await?;

            if sync_result.success {
                tracing::info!(
                    "Successfully synced {} items from backend {}",
                    sync_result.items_synced,
                    backend_id
                );
            } else {
                tracing::warn!(
                    "Sync completed with errors for backend {}: {:?}",
                    backend_id,
                    sync_result.errors
                );
            }

            results.push(sync_result);
        }

        Ok(results)
    }

    /// Get cached libraries for a specific backend
    pub async fn get_cached_libraries(&self, backend_id: &str) -> Result<Vec<Library>> {
        self.sync_manager.get_cached_libraries(backend_id).await
    }

    /// Get libraries for a specific backend from state cache
    pub async fn get_libraries_for_backend(&self, backend_id: &str) -> Vec<Library> {
        let libraries = self.libraries.read().await;
        libraries.get(backend_id).cloned().unwrap_or_default()
    }

    /// Update libraries for a backend
    pub async fn set_libraries_for_backend(&self, backend_id: String, libraries: Vec<Library>) {
        let mut lib_map = self.libraries.write().await;
        lib_map.insert(backend_id, libraries);
    }

    /// Get items for a specific library
    pub async fn get_library_items(&self, library_id: &str) -> Vec<MediaItem> {
        let items = self.library_items.read().await;
        items.get(library_id).cloned().unwrap_or_default()
    }

    /// Set items for a specific library
    pub async fn set_library_items(&self, library_id: String, items: Vec<MediaItem>) {
        let mut items_map = self.library_items.write().await;
        items_map.insert(library_id, items);
    }

    /// Get all configured backends
    pub async fn get_all_backends(&self) -> Vec<(String, crate::backends::traits::BackendInfo)> {
        self.source_coordinator.list_backends().await
    }

    /// Get libraries from all backends
    pub async fn get_all_libraries(&self) -> Vec<(String, Vec<Library>)> {
        let all_backends = self.source_coordinator.get_all_backends().await;

        let mut all_libraries = Vec::new();

        info!(
            "Getting all libraries, found {} backends",
            all_backends.len()
        );
        for (backend_id, _backend) in all_backends {
            info!("Checking libraries for backend: {}", backend_id);
            // Try to get cached libraries for this backend
            if let Ok(libraries) = self.sync_manager.get_cached_libraries(&backend_id).await {
                info!(
                    "Found {} cached libraries for backend {}",
                    libraries.len(),
                    backend_id
                );
                if !libraries.is_empty() {
                    all_libraries.push((backend_id, libraries));
                }
            } else {
                warn!("Failed to get cached libraries for backend {}", backend_id);
            }
        }
        info!("Returning {} library groups", all_libraries.len());
        all_libraries
    }
}

impl fmt::Debug for AppState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppState")
            .field(
                "has_user",
                &self
                    .current_user
                    .try_read()
                    .map(|u| u.is_some())
                    .unwrap_or(false),
            )
            .field(
                "has_library",
                &self
                    .current_library
                    .try_read()
                    .map(|l| l.is_some())
                    .unwrap_or(false),
            )
            .field("playback_state", &self.playback_state.try_read().ok())
            .finish()
    }
}
