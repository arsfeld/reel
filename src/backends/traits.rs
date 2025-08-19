use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::time::Duration;

use crate::models::{
    Credentials, Episode, Library, Movie, Show, StreamInfo, User,
};

#[async_trait]
pub trait MediaBackend: Send + Sync + std::fmt::Debug {
    /// Initialize the backend with stored credentials
    /// Returns Ok(Some(user)) if successfully connected, Ok(None) if no credentials, Err if failed
    async fn initialize(&self) -> Result<Option<User>>;
    
    /// Check if the backend is initialized and ready to use
    async fn is_initialized(&self) -> bool;
    
    /// Get the backend as Any for downcasting
    fn as_any(&self) -> &dyn std::any::Any;
    
    async fn authenticate(&self, credentials: Credentials) -> Result<User>;
    
    async fn get_libraries(&self) -> Result<Vec<Library>>;
    
    async fn get_movies(&self, library_id: &str) -> Result<Vec<Movie>>;
    
    async fn get_shows(&self, library_id: &str) -> Result<Vec<Show>>;
    
    async fn get_episodes(&self, show_id: &str, season: u32) -> Result<Vec<Episode>>;
    
    async fn get_stream_url(&self, media_id: &str) -> Result<StreamInfo>;
    
    async fn update_progress(&self, media_id: &str, position: Duration) -> Result<()>;
    
    async fn search(&self, query: &str) -> Result<SearchResults>;
    
    // Sync support methods
    async fn get_backend_id(&self) -> String;
    
    async fn get_last_sync_time(&self) -> Option<DateTime<Utc>>;
    
    async fn supports_offline(&self) -> bool;
}

#[derive(Debug, Clone)]
pub struct SearchResults {
    pub movies: Vec<Movie>,
    pub shows: Vec<Show>,
    pub episodes: Vec<Episode>,
}

#[derive(Debug, Clone)]
pub struct SyncResult {
    pub backend_id: String,
    pub success: bool,
    pub items_synced: usize,
    pub duration: Duration,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum SyncType {
    Full,           // Full sync of all data
    Incremental,    // Only changes since last sync
    Library(String), // Specific library
    Media(String),   // Specific media item
}

#[derive(Debug, Clone)]
pub enum SyncPriority {
    High,
    Normal,
    Low,
}

#[derive(Debug, Clone)]
pub enum SyncStatus {
    Idle,
    Syncing { progress: f32, current_item: String },
    Completed { at: DateTime<Utc>, items_synced: usize },
    Failed { error: String, at: DateTime<Utc> },
}

#[derive(Debug, Clone)]
pub struct SyncTask {
    pub backend_id: String,
    pub sync_type: SyncType,
    pub priority: SyncPriority,
    pub scheduled_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct BackendOfflineInfo {
    pub total_items: usize,
    pub size_mb: u64,
    pub last_sync: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct OfflineStatus {
    pub total_size_mb: u64,
    pub used_size_mb: u64,
    pub items_count: usize,
    pub backends: std::collections::HashMap<String, BackendOfflineInfo>,
}