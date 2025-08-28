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
        if let Some(pos) = self.backend_order.iter().position(|x| x == backend_id)
            && pos > 0
        {
            self.backend_order.swap(pos, pos - 1);
        }
    }

    // Move a backend down in the order
    pub fn move_backend_down(&mut self, backend_id: &str) {
        if let Some(pos) = self.backend_order.iter().position(|x| x == backend_id)
            && pos < self.backend_order.len() - 1
        {
            self.backend_order.swap(pos, pos + 1);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Credentials, Episode, Library, Movie, Resolution, Show, StreamInfo, User};
    use async_trait::async_trait;
    use chrono::{DateTime, Utc};
    use std::any::Any;
    use std::time::Duration;

    #[derive(Debug, Clone)]
    struct MockBackend {
        id: String,
    }

    #[async_trait]
    impl traits::MediaBackend for MockBackend {
        async fn initialize(&self) -> Result<Option<User>> {
            Ok(None)
        }

        async fn is_initialized(&self) -> bool {
            false
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        async fn authenticate(&self, _credentials: Credentials) -> Result<User> {
            Ok(User {
                id: "test".to_string(),
                username: "test".to_string(),
                email: None,
                avatar_url: None,
            })
        }

        async fn get_libraries(&self) -> Result<Vec<Library>> {
            Ok(Vec::new())
        }

        async fn get_movies(&self, _library_id: &str) -> Result<Vec<Movie>> {
            Ok(Vec::new())
        }

        async fn get_shows(&self, _library_id: &str) -> Result<Vec<Show>> {
            Ok(Vec::new())
        }

        async fn get_episodes(&self, _show_id: &str, _season: u32) -> Result<Vec<Episode>> {
            Ok(Vec::new())
        }

        async fn get_stream_url(&self, _media_id: &str) -> Result<StreamInfo> {
            Ok(StreamInfo {
                url: "http://test.com/stream".to_string(),
                direct_play: true,
                video_codec: String::new(),
                audio_codec: String::new(),
                container: String::new(),
                bitrate: 0,
                resolution: Resolution {
                    width: 1920,
                    height: 1080,
                },
                quality_options: vec![],
            })
        }

        async fn update_progress(
            &self,
            _media_id: &str,
            _position: Duration,
            _duration: Duration,
        ) -> Result<()> {
            Ok(())
        }

        async fn mark_watched(&self, _media_id: &str) -> Result<()> {
            Ok(())
        }

        async fn mark_unwatched(&self, _media_id: &str) -> Result<()> {
            Ok(())
        }

        async fn get_watch_status(&self, _media_id: &str) -> Result<traits::WatchStatus> {
            Ok(traits::WatchStatus {
                watched: false,
                view_count: 0,
                last_watched_at: None,
                playback_position: None,
            })
        }

        async fn search(&self, _query: &str) -> Result<traits::SearchResults> {
            Ok(traits::SearchResults {
                movies: Vec::new(),
                shows: Vec::new(),
                episodes: Vec::new(),
            })
        }

        async fn get_backend_id(&self) -> String {
            self.id.clone()
        }

        async fn get_last_sync_time(&self) -> Option<DateTime<Utc>> {
            None
        }

        async fn supports_offline(&self) -> bool {
            false
        }
    }

    #[test]
    fn test_backend_manager_new() {
        let manager = BackendManager::new();
        assert_eq!(manager.backends.len(), 0);
        assert_eq!(manager.backend_order.len(), 0);
    }

    #[test]
    fn test_backend_manager_default() {
        let manager = BackendManager::default();
        assert_eq!(manager.backends.len(), 0);
        assert_eq!(manager.backend_order.len(), 0);
    }

    #[test]
    fn test_register_backend() {
        let mut manager = BackendManager::new();
        let backend = Arc::new(MockBackend {
            id: "test1".to_string(),
        });

        manager.register_backend("test1".to_string(), backend.clone());

        assert_eq!(manager.backends.len(), 1);
        assert_eq!(manager.backend_order.len(), 1);
        assert_eq!(manager.backend_order[0], "test1");
        assert!(manager.backends.contains_key("test1"));
    }

    #[test]
    fn test_register_backend_duplicate() {
        let mut manager = BackendManager::new();
        let backend1 = Arc::new(MockBackend {
            id: "test1".to_string(),
        });
        let backend2 = Arc::new(MockBackend {
            id: "test2".to_string(),
        });

        manager.register_backend("test".to_string(), backend1);
        manager.register_backend("test".to_string(), backend2);

        // Should replace the backend but not duplicate in order
        assert_eq!(manager.backends.len(), 1);
        assert_eq!(manager.backend_order.len(), 1);
    }

    #[test]
    fn test_remove_backend() {
        let mut manager = BackendManager::new();
        let backend = Arc::new(MockBackend {
            id: "test1".to_string(),
        });

        manager.register_backend("test1".to_string(), backend.clone());
        let removed = manager.remove_backend("test1");

        assert!(removed.is_some());
        assert_eq!(manager.backends.len(), 0);
        assert_eq!(manager.backend_order.len(), 0);
    }

    #[test]
    fn test_remove_backend_nonexistent() {
        let mut manager = BackendManager::new();
        let removed = manager.remove_backend("nonexistent");

        assert!(removed.is_none());
    }

    #[test]
    fn test_get_all_backends() {
        let mut manager = BackendManager::new();
        let backend1 = Arc::new(MockBackend {
            id: "test1".to_string(),
        });
        let backend2 = Arc::new(MockBackend {
            id: "test2".to_string(),
        });

        manager.register_backend("test1".to_string(), backend1);
        manager.register_backend("test2".to_string(), backend2);

        let all_backends = manager.get_all_backends();
        assert_eq!(all_backends.len(), 2);
        assert_eq!(all_backends[0].0, "test1");
        assert_eq!(all_backends[1].0, "test2");
    }

    #[test]
    fn test_reorder_backends() {
        let mut manager = BackendManager::new();
        let backend1 = Arc::new(MockBackend {
            id: "test1".to_string(),
        });
        let backend2 = Arc::new(MockBackend {
            id: "test2".to_string(),
        });
        let backend3 = Arc::new(MockBackend {
            id: "test3".to_string(),
        });

        manager.register_backend("test1".to_string(), backend1);
        manager.register_backend("test2".to_string(), backend2);
        manager.register_backend("test3".to_string(), backend3);

        manager.reorder_backends(vec![
            "test3".to_string(),
            "test1".to_string(),
            "test2".to_string(),
        ]);

        assert_eq!(manager.backend_order, vec!["test3", "test1", "test2"]);
    }

    #[test]
    fn test_reorder_backends_invalid() {
        let mut manager = BackendManager::new();
        let backend1 = Arc::new(MockBackend {
            id: "test1".to_string(),
        });

        manager.register_backend("test1".to_string(), backend1);
        let original_order = manager.backend_order.clone();

        // Try to reorder with invalid backend ID
        manager.reorder_backends(vec!["test1".to_string(), "invalid".to_string()]);

        // Order should remain unchanged
        assert_eq!(manager.backend_order, original_order);
    }

    #[test]
    fn test_move_backend_up() {
        let mut manager = BackendManager::new();
        let backend1 = Arc::new(MockBackend {
            id: "test1".to_string(),
        });
        let backend2 = Arc::new(MockBackend {
            id: "test2".to_string(),
        });
        let backend3 = Arc::new(MockBackend {
            id: "test3".to_string(),
        });

        manager.register_backend("test1".to_string(), backend1);
        manager.register_backend("test2".to_string(), backend2);
        manager.register_backend("test3".to_string(), backend3);

        manager.move_backend_up("test2");

        assert_eq!(manager.backend_order, vec!["test2", "test1", "test3"]);
    }

    #[test]
    fn test_move_backend_up_first_position() {
        let mut manager = BackendManager::new();
        let backend1 = Arc::new(MockBackend {
            id: "test1".to_string(),
        });
        let backend2 = Arc::new(MockBackend {
            id: "test2".to_string(),
        });

        manager.register_backend("test1".to_string(), backend1);
        manager.register_backend("test2".to_string(), backend2);

        let original_order = manager.backend_order.clone();
        manager.move_backend_up("test1");

        // Should not change since already at first position
        assert_eq!(manager.backend_order, original_order);
    }

    #[test]
    fn test_move_backend_down() {
        let mut manager = BackendManager::new();
        let backend1 = Arc::new(MockBackend {
            id: "test1".to_string(),
        });
        let backend2 = Arc::new(MockBackend {
            id: "test2".to_string(),
        });
        let backend3 = Arc::new(MockBackend {
            id: "test3".to_string(),
        });

        manager.register_backend("test1".to_string(), backend1);
        manager.register_backend("test2".to_string(), backend2);
        manager.register_backend("test3".to_string(), backend3);

        manager.move_backend_down("test2");

        assert_eq!(manager.backend_order, vec!["test1", "test3", "test2"]);
    }

    #[test]
    fn test_move_backend_down_last_position() {
        let mut manager = BackendManager::new();
        let backend1 = Arc::new(MockBackend {
            id: "test1".to_string(),
        });
        let backend2 = Arc::new(MockBackend {
            id: "test2".to_string(),
        });

        manager.register_backend("test1".to_string(), backend1);
        manager.register_backend("test2".to_string(), backend2);

        let original_order = manager.backend_order.clone();
        manager.move_backend_down("test2");

        // Should not change since already at last position
        assert_eq!(manager.backend_order, original_order);
    }

    #[test]
    fn test_get_backend() {
        let mut manager = BackendManager::new();
        let backend = Arc::new(MockBackend {
            id: "test1".to_string(),
        });

        manager.register_backend("test1".to_string(), backend.clone());

        let retrieved = manager.get_backend("test1");
        assert!(retrieved.is_some());

        let nonexistent = manager.get_backend("nonexistent");
        assert!(nonexistent.is_none());
    }

    #[test]
    fn test_get_offline_backends() {
        let mut manager = BackendManager::new();
        let backend1 = Arc::new(MockBackend {
            id: "test1".to_string(),
        });
        let backend2 = Arc::new(MockBackend {
            id: "test2".to_string(),
        });

        manager.register_backend("backend1".to_string(), backend1);
        manager.register_backend("backend2".to_string(), backend2);

        let offline_backends = manager.get_offline_backends();
        assert_eq!(offline_backends.len(), 2);
        assert!(offline_backends.contains(&"backend1".to_string()));
        assert!(offline_backends.contains(&"backend2".to_string()));
    }

    #[test]
    fn test_list_backends() {
        let mut manager = BackendManager::new();
        let backend1 = Arc::new(MockBackend {
            id: "plex_test".to_string(),
        });
        let backend2 = Arc::new(MockBackend {
            id: "jellyfin_test".to_string(),
        });
        let backend3 = Arc::new(MockBackend {
            id: "local".to_string(),
        });

        manager.register_backend("plex_test".to_string(), backend1);
        manager.register_backend("jellyfin_test".to_string(), backend2);
        manager.register_backend("local".to_string(), backend3);

        let backends = manager.list_backends();
        assert_eq!(backends.len(), 3);

        let plex_backend = backends.iter().find(|(id, _)| id == "plex_test");
        assert!(plex_backend.is_some());
        assert!(matches!(
            plex_backend.unwrap().1.backend_type,
            traits::BackendType::Plex
        ));

        let jellyfin_backend = backends.iter().find(|(id, _)| id == "jellyfin_test");
        assert!(jellyfin_backend.is_some());
        assert!(matches!(
            jellyfin_backend.unwrap().1.backend_type,
            traits::BackendType::Jellyfin
        ));

        let local_backend = backends.iter().find(|(id, _)| id == "local");
        assert!(local_backend.is_some());
        assert!(matches!(
            local_backend.unwrap().1.backend_type,
            traits::BackendType::Local
        ));
    }

    #[test]
    fn test_unregister_backend() {
        let mut manager = BackendManager::new();
        let backend = Arc::new(MockBackend {
            id: "test1".to_string(),
        });

        manager.register_backend("test1".to_string(), backend);
        assert_eq!(manager.backends.len(), 1);
        assert_eq!(manager.backend_order.len(), 1);

        manager.unregister_backend("test1");
        assert_eq!(manager.backends.len(), 0);
        assert_eq!(manager.backend_order.len(), 0);
    }

    #[tokio::test]
    async fn test_refresh_all_backends() {
        let manager = BackendManager::new();
        let results = manager.refresh_all_backends().await.unwrap();
        assert_eq!(results.len(), 0);
    }
}
