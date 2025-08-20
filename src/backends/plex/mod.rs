mod auth;
mod api;

pub use auth::{PlexAuth, PlexPin, PlexUser, PlexServer, PlexConnection};
use api::PlexApi;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use std::fmt;
use dirs;

use super::traits::{MediaBackend, SearchResults, WatchStatus};
use crate::models::{
    Credentials, Episode, Library, Movie, Show, StreamInfo, User,
};

pub struct PlexBackend {
    client: Client,
    base_url: Arc<RwLock<Option<String>>>,
    auth_token: Arc<RwLock<Option<String>>>,
    backend_id: String,
    last_sync_time: Arc<RwLock<Option<DateTime<Utc>>>>,
    api: Arc<RwLock<Option<PlexApi>>>,
    server_name: Arc<RwLock<Option<String>>>,
    server_info: Arc<RwLock<Option<ServerInfo>>>,
}

#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub name: String,
    pub is_local: bool,
    pub is_relay: bool,
    pub uri: String,
}

impl PlexBackend {
    pub fn new() -> Self {
        Self::with_id("plex".to_string())
    }
    
    pub fn with_id(id: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
        
        Self {
            client,
            base_url: Arc::new(RwLock::new(None)),
            auth_token: Arc::new(RwLock::new(None)),
            backend_id: id,
            last_sync_time: Arc::new(RwLock::new(None)),
            api: Arc::new(RwLock::new(None)),
            server_name: Arc::new(RwLock::new(None)),
            server_info: Arc::new(RwLock::new(None)),
        }
    }
    
    pub async fn get_server_info(&self) -> Option<ServerInfo> {
        self.server_info.read().await.clone()
    }
    
    /// Authenticate with PIN and select a server
    pub async fn authenticate_with_pin(&self, pin: &PlexPin, server: &PlexServer) -> Result<()> {
        // Poll for the auth token
        let token = PlexAuth::poll_for_token(&pin.id).await?;
        
        // Store the auth token
        *self.auth_token.write().await = Some(token.clone());
        
        // Find the best connection for the server
        let connection = server.connections
            .iter()
            .find(|c| !c.relay) // Prefer direct connections
            .or_else(|| server.connections.first())
            .ok_or_else(|| anyhow::anyhow!("No valid connection found for server"))?;
        
        // Store the base URL
        *self.base_url.write().await = Some(connection.uri.clone());
        
        // Create the API client
        let api = PlexApi::new(connection.uri.clone(), token);
        *self.api.write().await = Some(api);
        
        Ok(())
    }
    
    /// Test all connections in parallel and return the fastest responding one
    async fn find_best_connection(&self, server: &PlexServer, token: &str) -> Result<PlexConnection> {
        use futures::future::select_ok;
        use std::time::Instant;
        
        if server.connections.is_empty() {
            return Err(anyhow!("No connections available for server"));
        }
        
        // Create futures for testing each connection
        let mut connection_futures = Vec::new();
        
        for conn in &server.connections {
            let uri = conn.uri.clone();
            let token = token.to_string();
            let conn_clone = conn.clone();
            
            let future = async move {
                let start = Instant::now();
                let client = reqwest::Client::builder()
                    .timeout(Duration::from_secs(3))
                    .danger_accept_invalid_certs(true) // Plex uses self-signed certs
                    .build()?;
                
                // Try to access the server identity endpoint
                let response = client
                    .get(format!("{}/identity", uri))
                    .header("X-Plex-Token", &token)
                    .header("Accept", "application/json")
                    .send()
                    .await;
                
                match response {
                    Ok(resp) if resp.status().is_success() => {
                        let latency = start.elapsed();
                        tracing::debug!("Connection {} responded in {:?}", uri, latency);
                        Ok((conn_clone, latency))
                    }
                    Ok(resp) => {
                        tracing::debug!("Connection {} returned status: {}", uri, resp.status());
                        Err(anyhow!("Connection failed with status: {}", resp.status()))
                    }
                    Err(e) => {
                        tracing::debug!("Connection {} failed: {}", uri, e);
                        Err(anyhow!("Connection failed: {}", e))
                    }
                }
            };
            
            connection_futures.push(Box::pin(future));
        }
        
        // Race all connections and pick the first successful one
        match select_ok(connection_futures).await {
            Ok((result, _remaining)) => {
                tracing::info!("Best connection found: {} (latency: {:?})", result.0.uri, result.1);
                Ok(result.0)
            }
            Err(_) => {
                // If all parallel attempts fail, try them sequentially with more time
                tracing::warn!("All parallel connection attempts failed, trying sequentially...");
                
                // Sort connections by priority: local non-relay first, then remote non-relay, then relay
                let mut sorted_connections = server.connections.clone();
                sorted_connections.sort_by_key(|c| {
                    if c.local && !c.relay { 0 }
                    else if !c.relay { 1 }
                    else { 2 }
                });
                
                for conn in sorted_connections {
                    let client = reqwest::Client::builder()
                        .timeout(Duration::from_secs(10))
                        .danger_accept_invalid_certs(true)
                        .build()?;
                    
                    let response = client
                        .get(format!("{}/identity", conn.uri))
                        .header("X-Plex-Token", token)
                        .header("Accept", "application/json")
                        .send()
                        .await;
                    
                    if let Ok(resp) = response {
                        if resp.status().is_success() {
                            tracing::info!("Successfully connected to {} (fallback)", conn.uri);
                            return Ok(conn);
                        }
                    }
                }
                
                Err(anyhow!("Failed to connect to any server endpoint"))
            }
        }
    }
    
    /// Get the API client, ensuring it's initialized
    async fn get_api(&self) -> Result<PlexApi> {
        let api_guard = self.api.read().await;
        if let Some(api) = api_guard.as_ref() {
            let base_url = self.base_url.read().await
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Base URL not set"))?
                .clone();
            let auth_token = self.auth_token.read().await
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Auth token not set"))?
                .clone();
            Ok(PlexApi::new(base_url, auth_token))
        } else {
            Err(anyhow::anyhow!("Plex API not initialized. Please authenticate first."))
        }
    }
}

#[async_trait]
impl MediaBackend for PlexBackend {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    
    async fn initialize(&self) -> Result<Option<User>> {
        // Check for saved Plex token
        let config_dir = dirs::config_dir().ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?;
        let token_file = config_dir.join("reel").join("plex_token");
        
        if !token_file.exists() {
            return Ok(None);
        }
        
        let token = std::fs::read_to_string(&token_file)?;
        let token = token.trim();
        
        if token.is_empty() {
            return Ok(None);
        }
        
        // Get user info with the saved token
        let plex_user = match PlexAuth::get_user(&token).await {
            Ok(user) => user,
            Err(e) => {
                tracing::error!("Failed to get user info with saved token: {}", e);
                // Token might be expired, remove it
                std::fs::remove_file(&token_file).ok();
                return Ok(None);
            }
        };
        
        // Store the token
        *self.auth_token.write().await = Some(token.to_string());
        
        // Try to discover and connect to the best server
        match PlexAuth::discover_servers(&token).await {
            Ok(servers) => {
                if let Some(server) = servers.first() {
                    // Test all connections in parallel and use the fastest one
                    match self.find_best_connection(&server, &token).await {
                        Ok(best_conn) => {
                            *self.base_url.write().await = Some(best_conn.uri.clone());
                            *self.server_name.write().await = Some(server.name.clone());
                            
                            // Store server info
                            *self.server_info.write().await = Some(ServerInfo {
                                name: server.name.clone(),
                                is_local: best_conn.local,
                                is_relay: best_conn.relay,
                                uri: best_conn.uri.clone(),
                            });
                            
                            // Create and store the API client
                            let api = PlexApi::new(best_conn.uri.clone(), token.to_string());
                            *self.api.write().await = Some(api);
                            
                            tracing::info!("Connected to Plex server: {} at {} ({})", 
                                server.name, 
                                best_conn.uri,
                                if best_conn.local { "local" } else if best_conn.relay { "relay" } else { "remote" }
                            );
                        }
                        Err(e) => {
                            tracing::warn!("Failed to connect to any server endpoint: {}", e);
                        }
                    }
                } else {
                    tracing::warn!("No Plex servers found");
                }
            }
            Err(e) => {
                tracing::warn!("Failed to discover servers (will retry later): {}", e);
                // Still return success since we have the user authenticated
                // Server discovery can be retried later
            }
        }
        
        Ok(Some(User {
            id: plex_user.id.to_string(),
            username: plex_user.username,
            email: Some(plex_user.email),
            avatar_url: plex_user.thumb,
        }))
    }
    
    async fn is_initialized(&self) -> bool {
        let has_token = self.auth_token.read().await.is_some();
        let has_api = self.api.read().await.is_some();
        let has_url = self.base_url.read().await.is_some();
        
        has_token && has_api && has_url
    }
    
    async fn authenticate(&self, credentials: Credentials) -> Result<User> {
        match credentials {
            Credentials::Token { token } => {
                // Store the token for later use
                *self.auth_token.write().await = Some(token.clone());
                
                // Get user info from Plex
                let plex_user = PlexAuth::get_user(&token).await?;
                
                Ok(User {
                    id: plex_user.id.to_string(),
                    username: plex_user.username,
                    email: Some(plex_user.email),
                    avatar_url: plex_user.thumb,
                })
            }
            _ => Err(anyhow::anyhow!("Plex only supports token authentication"))
        }
    }
    
    async fn get_libraries(&self) -> Result<Vec<Library>> {
        let api = self.get_api().await?;
        api.get_libraries().await
    }
    
    async fn get_movies(&self, library_id: &str) -> Result<Vec<Movie>> {
        let api = self.get_api().await?;
        api.get_movies(library_id).await
    }
    
    async fn get_shows(&self, library_id: &str) -> Result<Vec<Show>> {
        let api = self.get_api().await?;
        api.get_shows(library_id).await
    }
    
    async fn get_episodes(&self, show_id: &str, season_number: u32) -> Result<Vec<Episode>> {
        let api = self.get_api().await?;
        
        // First, get the seasons for this show to find the correct season ID
        let seasons = api.get_seasons(show_id).await?;
        
        // Find the season with the matching season number
        let season = seasons.iter()
            .find(|s| s.season_number == season_number)
            .ok_or_else(|| anyhow!("Season {} not found for show {}", season_number, show_id))?;
        
        // Now get the episodes for the correct season
        api.get_episodes(&season.id).await
    }
    
    async fn get_stream_url(&self, media_id: &str) -> Result<StreamInfo> {
        let api = self.get_api().await?;
        api.get_stream_url(media_id).await
    }
    
    async fn update_progress(&self, media_id: &str, position: Duration) -> Result<()> {
        let api = self.get_api().await?;
        api.update_progress(media_id, position).await
    }
    
    async fn mark_watched(&self, media_id: &str) -> Result<()> {
        let api = self.get_api().await?;
        api.mark_watched(media_id).await
    }
    
    async fn mark_unwatched(&self, media_id: &str) -> Result<()> {
        let api = self.get_api().await?;
        api.mark_unwatched(media_id).await
    }
    
    async fn get_watch_status(&self, media_id: &str) -> Result<super::traits::WatchStatus> {
        // For now, return a default status - could fetch from API if needed
        // In practice, the watch status is already included in get_movies/shows/episodes
        Ok(super::traits::WatchStatus {
            watched: false,
            view_count: 0,
            last_watched_at: None,
            playback_position: None,
        })
    }
    
    async fn search(&self, _query: &str) -> Result<SearchResults> {
        // TODO: Implement Plex search
        todo!("Search not yet implemented")
    }
    
    async fn get_home_sections(&self) -> Result<Vec<crate::models::HomeSection>> {
        let api = self.get_api().await?;
        api.get_home_sections().await
    }
    
    async fn get_backend_info(&self) -> super::traits::BackendInfo {
        let server_info = self.server_info.read().await;
        let server_name = self.server_name.read().await;
        
        if let Some(info) = server_info.as_ref() {
            super::traits::BackendInfo {
                name: self.backend_id.clone(),
                display_name: format!("Plex ({})", info.name),
                backend_type: super::traits::BackendType::Plex,
                server_name: Some(info.name.clone()),
                server_version: None, // Could fetch this from API if needed
                connection_type: if info.is_local {
                    super::traits::ConnectionType::Local
                } else if info.is_relay {
                    super::traits::ConnectionType::Relay
                } else {
                    super::traits::ConnectionType::Remote
                },
                is_local: info.is_local,
                is_relay: info.is_relay,
            }
        } else {
            super::traits::BackendInfo {
                name: self.backend_id.clone(),
                display_name: "Plex".to_string(),
                backend_type: super::traits::BackendType::Plex,
                server_name: server_name.clone(),
                server_version: None,
                connection_type: super::traits::ConnectionType::Unknown,
                is_local: false,
                is_relay: false,
            }
        }
    }
    
    async fn get_backend_id(&self) -> String {
        self.backend_id.clone()
    }
    
    async fn get_last_sync_time(&self) -> Option<DateTime<Utc>> {
        *self.last_sync_time.read().await
    }
    
    async fn supports_offline(&self) -> bool {
        true // Plex supports offline functionality
    }
}

impl fmt::Debug for PlexBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlexBackend")
            .field("backend_id", &self.backend_id)
            .field("has_base_url", &self.base_url.try_read().map(|u| u.is_some()).unwrap_or(false))
            .field("has_auth_token", &self.auth_token.try_read().map(|t| t.is_some()).unwrap_or(false))
            .finish()
    }
}