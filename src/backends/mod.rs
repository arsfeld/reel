pub mod jellyfin;
pub mod local;
pub mod plex;
pub mod sync_strategy;
pub mod traits;

// Re-export commonly used types
pub use traits::MediaBackend;

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;

// Internal use - import from self to avoid duplication with pub use

#[derive(Debug)]
pub struct BackendManager {
    backends: HashMap<String, Arc<dyn traits::MediaBackend>>,
    backend_order: Vec<String>, // Order of backends for display
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

    pub fn get_offline_backends(&self) -> Vec<String> {
        self.backends.keys().cloned().collect()
    }

    pub fn list_backends(&self) -> Vec<(String, traits::BackendInfo)> {
        self.backends
            .iter()
            .map(|(id, backend)| {
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

    pub async fn refresh_all_backends(&self) -> Result<Vec<traits::SyncResult>> {
        Ok(Vec::new())
    }
}
