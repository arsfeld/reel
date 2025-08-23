pub mod jellyfin;
pub mod local;
pub mod plex;
pub mod sync_strategy;
pub mod traits;

// Re-export commonly used types
pub use traits::MediaBackend;

use anyhow::Result;
use chrono::Utc;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;

// Internal use - import from self to avoid duplication with pub use

#[derive(Debug)]
pub struct BackendManager {
    backends: HashMap<String, Arc<dyn traits::MediaBackend>>,
    backend_order: Vec<String>, // Order of backends for display
    sync_manager: Arc<SyncManager>,
}

impl Default for BackendManager {
    fn default() -> Self {
        Self::new()
    }
}

impl BackendManager {
    pub fn new() -> Self {
        Self {
            backends: HashMap::new(),
            backend_order: Vec::new(),
            sync_manager: Arc::new(SyncManager::new()),
        }
    }

    pub fn register_backend(&mut self, name: String, backend: Arc<dyn traits::MediaBackend>) {
        if !self.backends.contains_key(&name) {
            self.backend_order.push(name.clone());
        }
        self.backends.insert(name, backend);
    }

    pub fn remove_backend(&mut self, name: &str) -> Option<Arc<dyn traits::MediaBackend>> {
        // Remove from order list
        if let Some(pos) = self.backend_order.iter().position(|x| x == name) {
            self.backend_order.remove(pos);
        }
        // Remove and return the backend
        self.backends.remove(name)
    }

    // Deprecated - kept for compatibility but all backends are now active
    pub fn set_active(&mut self, name: &str) -> Result<()> {
        if self.backends.contains_key(name) {
            // Move this backend to the front of the order
            if let Some(pos) = self.backend_order.iter().position(|x| x == name) {
                let backend = self.backend_order.remove(pos);
                self.backend_order.insert(0, backend);
            }
            Ok(())
        } else {
            anyhow::bail!("Backend '{}' not found", name)
        }
    }

    // Deprecated - returns the first backend in order for compatibility
    pub fn get_active(&self) -> Option<Arc<dyn traits::MediaBackend>> {
        self.backend_order
            .first()
            .and_then(|name| self.backends.get(name))
            .cloned()
    }

    // Deprecated - returns the first backend in order for compatibility
    pub fn get_active_backend(&self) -> Option<(String, Arc<dyn traits::MediaBackend>)> {
        self.backend_order.first().and_then(|name| {
            self.backends
                .get(name)
                .map(|backend| (name.clone(), backend.clone()))
        })
    }

    // Get all backends in order
    pub fn get_all_backends(&self) -> Vec<(String, Arc<dyn traits::MediaBackend>)> {
        self.backend_order
            .iter()
            .filter_map(|name| {
                self.backends
                    .get(name)
                    .map(|backend| (name.clone(), backend.clone()))
            })
            .collect()
    }

    // Reorder backends
    pub fn reorder_backends(&mut self, new_order: Vec<String>) {
        // Validate that all backend IDs exist
        if new_order.iter().all(|id| self.backends.contains_key(id)) {
            self.backend_order = new_order;
        }
    }

    // Move a backend up in the order
    pub fn move_backend_up(&mut self, backend_id: &str) {
        if let Some(pos) = self.backend_order.iter().position(|x| x == backend_id) {
            if pos > 0 {
                self.backend_order.swap(pos, pos - 1);
            }
        }
    }

    // Move a backend down in the order
    pub fn move_backend_down(&mut self, backend_id: &str) {
        if let Some(pos) = self.backend_order.iter().position(|x| x == backend_id) {
            if pos < self.backend_order.len() - 1 {
                self.backend_order.swap(pos, pos + 1);
            }
        }
    }

    pub fn get_backend(&self, name: &str) -> Option<Arc<dyn traits::MediaBackend>> {
        self.backends.get(name).cloned()
    }

    pub async fn refresh_backend(&self, backend_id: &str) -> Result<traits::SyncResult> {
        let backend = self
            .get_backend(backend_id)
            .ok_or_else(|| anyhow::anyhow!("Backend '{}' not found", backend_id))?;

        self.sync_manager.sync_backend(backend_id, backend).await
    }

    pub async fn refresh_all_backends(&self) -> Result<Vec<traits::SyncResult>> {
        let mut results = Vec::new();

        // Sync backends in order
        for backend_id in &self.backend_order {
            if let Some(backend) = self.backends.get(backend_id) {
                let result = self
                    .sync_manager
                    .sync_backend(backend_id, backend.clone())
                    .await?;
                results.push(result);
            }
        }

        Ok(results)
    }

    pub fn get_offline_backends(&self) -> Vec<String> {
        self.backends
            .iter()
            .filter_map(|(id, _backend)| {
                // We can't await in a non-async context, so this would need to be refactored
                // For now, just return the IDs
                Some(id.clone())
            })
            .collect()
    }

    pub fn list_backends(&self) -> Vec<(String, traits::BackendInfo)> {
        // This is synchronous for now - ideally we'd make this async
        // But for the preferences UI, we can use cached info
        self.backends
            .iter()
            .map(|(id, backend)| {
                // Create a default BackendInfo - this should be cached/stored
                let info = traits::BackendInfo {
                    name: id.clone(),
                    display_name: format!("{} Backend", id),
                    backend_type: if id.starts_with("plex") {
                        traits::BackendType::Plex
                    } else if id.starts_with("jellyfin") {
                        traits::BackendType::Jellyfin
                    } else {
                        traits::BackendType::Local
                    },
                    server_name: None,
                    server_version: None,
                    connection_type: traits::ConnectionType::Unknown,
                    is_local: false,
                    is_relay: false,
                };
                (id.clone(), info)
            })
            .collect()
    }

    pub fn unregister_backend(&mut self, name: &str) {
        self.backends.remove(name);
        self.backend_order.retain(|x| x != name);
    }
}

#[derive(Debug)]
pub struct SyncManager {
    cache: Arc<CacheManager>,
    sync_queue: Arc<RwLock<VecDeque<traits::SyncTask>>>,
    sync_status: Arc<RwLock<HashMap<String, traits::SyncStatus>>>,
    strategy: Arc<sync_strategy::SyncStrategy>,
}

impl SyncManager {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(CacheManager::new()),
            sync_queue: Arc::new(RwLock::new(VecDeque::new())),
            sync_status: Arc::new(RwLock::new(HashMap::new())),
            strategy: Arc::new(sync_strategy::SyncStrategy::default()),
        }
    }

    pub fn with_strategy(strategy: sync_strategy::SyncStrategy) -> Self {
        Self {
            cache: Arc::new(CacheManager::new()),
            sync_queue: Arc::new(RwLock::new(VecDeque::new())),
            sync_status: Arc::new(RwLock::new(HashMap::new())),
            strategy: Arc::new(strategy),
        }
    }

    pub async fn sync_backend(
        &self,
        backend_id: &str,
        backend: Arc<dyn traits::MediaBackend>,
    ) -> Result<traits::SyncResult> {
        // Update sync status
        {
            let mut status = self.sync_status.write().await;
            status.insert(
                backend_id.to_string(),
                traits::SyncStatus::Syncing {
                    progress: 0.0,
                    current_item: "Starting sync...".to_string(),
                },
            );
        }

        let start_time = std::time::Instant::now();
        let errors = Vec::new();
        let items_synced = 0;

        // TODO: Implement actual sync logic
        // For now, just a stub that marks sync as completed

        // Simulate some sync work
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Update sync status to completed
        {
            let mut status = self.sync_status.write().await;
            status.insert(
                backend_id.to_string(),
                traits::SyncStatus::Completed {
                    at: Utc::now(),
                    items_synced,
                },
            );
        }

        Ok(traits::SyncResult {
            backend_id: backend_id.to_string(),
            success: errors.is_empty(),
            items_synced,
            duration: start_time.elapsed(),
            errors,
        })
    }

    pub async fn sync_library(&self, _backend_id: &str, _library_id: &str) -> Result<()> {
        // TODO: Implement library-specific sync
        todo!("Library sync not yet implemented")
    }

    pub async fn get_sync_status(&self, backend_id: &str) -> traits::SyncStatus {
        let status = self.sync_status.read().await;
        status
            .get(backend_id)
            .cloned()
            .unwrap_or(traits::SyncStatus::Idle)
    }

    pub async fn schedule_sync(&self, task: traits::SyncTask) {
        let mut queue = self.sync_queue.write().await;

        // Insert based on priority
        let position = match task.priority {
            traits::SyncPriority::High => 0,
            traits::SyncPriority::Normal => queue.len() / 2,
            traits::SyncPriority::Low => queue.len(),
        };

        if position >= queue.len() {
            queue.push_back(task);
        } else {
            queue.insert(position, task);
        }
    }

    pub async fn cancel_sync(&self, backend_id: &str) {
        let mut queue = self.sync_queue.write().await;
        queue.retain(|task| task.backend_id != backend_id);

        let mut status = self.sync_status.write().await;
        status.insert(backend_id.to_string(), traits::SyncStatus::Idle);
    }
}

#[derive(Debug)]
pub struct CacheManager {
    // In a real implementation, these would use SQLite via sqlx
    image_cache: Arc<ImageCache>,
    metadata_cache: Arc<MetadataCache>,
    offline_store: Arc<OfflineStore>,
}

impl CacheManager {
    pub fn new() -> Self {
        Self {
            image_cache: Arc::new(ImageCache::new()),
            metadata_cache: Arc::new(MetadataCache::new()),
            offline_store: Arc::new(OfflineStore::new()),
        }
    }

    pub async fn get_or_fetch<T, F, Fut>(&self, key: &str, fetcher: F) -> Result<T>
    where
        T: serde::Serialize + serde::de::DeserializeOwned + Clone + Send,
        F: FnOnce() -> Fut + Send,
        Fut: std::future::Future<Output = Result<T>> + Send,
    {
        // Check cache first
        if let Some(cached) = self.metadata_cache.get(key).await {
            return Ok(cached);
        }

        // Try to fetch from backend
        match fetcher().await {
            Ok(data) => {
                self.metadata_cache.set(key, &data).await?;
                Ok(data)
            }
            Err(e) => {
                // If fetch fails, try offline store
                if let Some(offline) = self.offline_store.get(key).await? {
                    Ok(offline)
                } else {
                    Err(e)
                }
            }
        }
    }

    pub async fn store_for_offline(
        &self,
        backend_id: &str,
        data: &impl serde::Serialize,
    ) -> Result<()> {
        self.offline_store.store(backend_id, data).await
    }

    pub async fn get_offline_data<T: serde::de::DeserializeOwned>(
        &self,
        backend_id: &str,
    ) -> Result<Option<T>> {
        self.offline_store.get(backend_id).await
    }

    pub async fn clear_backend_cache(&self, backend_id: &str) -> Result<()> {
        self.metadata_cache.clear_backend(backend_id).await?;
        self.offline_store.clear_backend(backend_id).await?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct ImageCache {
    // Stub implementation
}

impl ImageCache {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn get(&self, _url: &str) -> Option<Vec<u8>> {
        // TODO: Implement image cache retrieval
        None
    }

    pub async fn set(&self, _url: &str, _data: &[u8]) -> Result<()> {
        // TODO: Implement image cache storage
        Ok(())
    }
}

#[derive(Debug)]
pub struct MetadataCache {
    cache: Arc<RwLock<HashMap<String, String>>>, // JSON serialized data
}

impl MetadataCache {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        let cache = self.cache.read().await;
        cache
            .get(key)
            .and_then(|json| serde_json::from_str(json).ok())
    }

    pub async fn set<T: serde::Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let json = serde_json::to_string(value)?;
        let mut cache = self.cache.write().await;
        cache.insert(key.to_string(), json);
        Ok(())
    }

    pub async fn clear_backend(&self, backend_id: &str) -> Result<()> {
        let mut cache = self.cache.write().await;
        cache.retain(|k, _| !k.starts_with(backend_id));
        Ok(())
    }
}

#[derive(Debug)]
pub struct OfflineStore {
    store: Arc<RwLock<HashMap<String, String>>>, // JSON serialized data
}

impl OfflineStore {
    pub fn new() -> Self {
        Self {
            store: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn store(&self, key: &str, data: &impl serde::Serialize) -> Result<()> {
        let json = serde_json::to_string(data)?;
        let mut store = self.store.write().await;
        store.insert(key.to_string(), json);
        Ok(())
    }

    pub async fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let store = self.store.read().await;
        match store.get(key) {
            Some(json) => Ok(Some(serde_json::from_str(json)?)),
            None => Ok(None),
        }
    }

    pub async fn store_library(
        &self,
        backend_id: &str,
        library: &crate::models::Library,
    ) -> Result<()> {
        let key = format!("{}_library_{}", backend_id, library.id);
        self.store(&key, library).await
    }

    pub async fn store_media_batch(
        &self,
        backend_id: &str,
        media: &[crate::models::Movie],
    ) -> Result<()> {
        for movie in media {
            let key = format!("{}_movie_{}", backend_id, movie.id);
            self.store(&key, movie).await?;
        }
        Ok(())
    }

    pub async fn get_libraries(&self, _backend_id: &str) -> Result<Vec<crate::models::Library>> {
        // TODO: Implement proper filtering by backend_id
        // For now, return empty vec
        Ok(Vec::new())
    }

    pub async fn get_movies(
        &self,
        _backend_id: &str,
        _library_id: &str,
    ) -> Result<Vec<crate::models::Movie>> {
        // TODO: Implement proper filtering by backend_id and library_id
        // For now, return empty vec
        Ok(Vec::new())
    }

    pub async fn mark_for_offline(&self, _media_id: &str) -> Result<()> {
        // TODO: Implement marking media for offline availability
        Ok(())
    }

    pub async fn is_available_offline(&self, _media_id: &str) -> bool {
        // TODO: Check if media is available offline
        false
    }

    pub async fn clear_backend(&self, backend_id: &str) -> Result<()> {
        let mut store = self.store.write().await;
        store.retain(|k, _| !k.starts_with(backend_id));
        Ok(())
    }
}
