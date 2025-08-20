use anyhow::Result;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::backends::BackendManager;
use crate::config::Config;
use crate::models::{Library, MediaItem, User};
use crate::services::{CacheManager, SyncManager};

#[derive(Debug, Clone)]
pub enum PlaybackState {
    Idle,
    Loading,
    Playing,
    Paused,
    Stopped,
}

pub struct AppState {
    pub backend_manager: Arc<RwLock<BackendManager>>,
    pub current_user: Arc<RwLock<Option<User>>>,
    pub current_library: Arc<RwLock<Option<Library>>>,
    pub libraries: Arc<RwLock<HashMap<String, Vec<Library>>>>, // backend_id -> libraries
    pub library_items: Arc<RwLock<HashMap<String, Vec<MediaItem>>>>, // library_id -> items
    pub cache_manager: Arc<CacheManager>,
    pub sync_manager: Arc<SyncManager>,
    pub playback_state: Arc<RwLock<PlaybackState>>,
    pub config: Arc<Config>,
}

impl AppState {
    pub fn new(config: Arc<Config>) -> Result<Self> {
        let backend_manager = Arc::new(RwLock::new(BackendManager::new()));
        let cache_manager = Arc::new(CacheManager::new()?);
        let sync_manager = Arc::new(SyncManager::new(cache_manager.clone()));

        Ok(Self {
            backend_manager,
            current_user: Arc::new(RwLock::new(None)),
            current_library: Arc::new(RwLock::new(None)),
            libraries: Arc::new(RwLock::new(HashMap::new())),
            library_items: Arc::new(RwLock::new(HashMap::new())),
            cache_manager,
            sync_manager,
            playback_state: Arc::new(RwLock::new(PlaybackState::Idle)),
            config,
        })
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

    /// Sync data from the active backend
    pub async fn sync_active_backend(&self) -> Result<()> {
        let backend_manager = self.backend_manager.read().await;

        if let Some((backend_id, backend)) = backend_manager.get_active_backend() {
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

            Ok(())
        } else {
            Err(anyhow::anyhow!("No active backend configured"))
        }
    }

    /// Get cached libraries for the active backend
    pub async fn get_cached_libraries(&self) -> Result<Vec<Library>> {
        let backend_manager = self.backend_manager.read().await;

        if let Some((backend_id, _)) = backend_manager.get_active_backend() {
            self.sync_manager.get_cached_libraries(&backend_id).await
        } else {
            Ok(Vec::new())
        }
    }

    /// Get libraries for a specific backend from state cache
    pub async fn get_libraries_for_backend(&self, backend_id: &str) -> Vec<Library> {
        let libraries = self.libraries.read().await;
        libraries.get(backend_id).cloned().unwrap_or_default()
    }

    /// Get libraries for the active backend
    pub async fn get_active_backend_libraries(&self) -> Vec<Library> {
        let backend_manager = self.backend_manager.read().await;

        if let Some((backend_id, _)) = backend_manager.get_active_backend() {
            drop(backend_manager); // Release the lock before calling async method
            self.get_libraries_for_backend(&backend_id).await
        } else {
            Vec::new()
        }
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

    /// Get the currently active backend ID
    pub async fn get_active_backend_id(&self) -> Option<String> {
        let backend_manager = self.backend_manager.read().await;
        backend_manager.get_active_backend().map(|(id, _)| id)
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
