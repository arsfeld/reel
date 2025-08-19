use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use super::traits::{MediaBackend, SearchResults};
use crate::models::{
    Credentials, Episode, Library, Movie, Show, StreamInfo, User,
};

#[derive(Debug)]
pub struct JellyfinBackend {
    client: Client,
    base_url: Arc<RwLock<Option<String>>>,
    api_key: Arc<RwLock<Option<String>>>,
    backend_id: String,
    last_sync_time: Arc<RwLock<Option<DateTime<Utc>>>>,
}

impl JellyfinBackend {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
        
        Self {
            client,
            base_url: Arc::new(RwLock::new(None)),
            api_key: Arc::new(RwLock::new(None)),
            backend_id: "jellyfin_default".to_string(),
            last_sync_time: Arc::new(RwLock::new(None)),
        }
    }
    
    pub fn with_id(id: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
        
        Self {
            client,
            base_url: Arc::new(RwLock::new(None)),
            api_key: Arc::new(RwLock::new(None)),
            backend_id: id,
            last_sync_time: Arc::new(RwLock::new(None)),
        }
    }
}

#[async_trait]
impl MediaBackend for JellyfinBackend {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    
    async fn initialize(&self) -> Result<Option<User>> {
        // TODO: Check for saved Jellyfin credentials and connect
        // For now, return None (not configured)
        Ok(None)
    }
    
    async fn is_initialized(&self) -> bool {
        let has_url = self.base_url.read().await.is_some();
        let has_key = self.api_key.read().await.is_some();
        has_url && has_key
    }
    
    async fn authenticate(&self, _credentials: Credentials) -> Result<User> {
        // TODO: Implement Jellyfin authentication
        todo!("Jellyfin authentication not yet implemented")
    }
    
    async fn get_libraries(&self) -> Result<Vec<Library>> {
        // TODO: Implement fetching libraries from Jellyfin
        todo!("Fetching libraries not yet implemented")
    }
    
    async fn get_movies(&self, _library_id: &str) -> Result<Vec<Movie>> {
        // TODO: Implement fetching movies from Jellyfin library
        todo!("Fetching movies not yet implemented")
    }
    
    async fn get_shows(&self, _library_id: &str) -> Result<Vec<Show>> {
        // TODO: Implement fetching shows from Jellyfin library
        todo!("Fetching shows not yet implemented")
    }
    
    async fn get_episodes(&self, _show_id: &str, _season: u32) -> Result<Vec<Episode>> {
        // TODO: Implement fetching episodes from Jellyfin
        todo!("Fetching episodes not yet implemented")
    }
    
    async fn get_stream_url(&self, _media_id: &str) -> Result<StreamInfo> {
        // TODO: Implement getting stream URL from Jellyfin
        todo!("Getting stream URL not yet implemented")
    }
    
    async fn update_progress(&self, _media_id: &str, _position: Duration) -> Result<()> {
        // TODO: Implement updating playback progress in Jellyfin
        todo!("Updating progress not yet implemented")
    }
    
    async fn search(&self, _query: &str) -> Result<SearchResults> {
        // TODO: Implement Jellyfin search
        todo!("Search not yet implemented")
    }
    
    async fn get_backend_id(&self) -> String {
        self.backend_id.clone()
    }
    
    async fn get_last_sync_time(&self) -> Option<DateTime<Utc>> {
        *self.last_sync_time.read().await
    }
    
    async fn supports_offline(&self) -> bool {
        true // Jellyfin supports offline functionality
    }
}