pub mod api;
mod auth;
mod tests;

pub use api::{PlayQueueResponse, PlexApi, create_standard_headers};
pub use auth::{PlexAuth, PlexConnection, PlexPin, PlexServer};

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dirs;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, warn};

use super::traits::MediaBackend;
use crate::models::{
    AuthProvider, BackendId, Credentials, Episode, Library, LibraryId, MediaItemId, Movie, Season,
    Show, ShowId, Source, SourceId, SourceType, StreamInfo, User,
};
use crate::services::core::auth::AuthService;

#[allow(dead_code)] // Used via dynamic dispatch in BackendService
pub struct PlexBackend {
    base_url: Arc<RwLock<Option<String>>>,
    auth_token: Arc<RwLock<Option<String>>>,
    backend_id: String,
    last_sync_time: Arc<RwLock<Option<DateTime<Utc>>>>,
    api: Arc<RwLock<Option<PlexApi>>>,
    server_name: Arc<RwLock<Option<String>>>,
    server_info: Arc<RwLock<Option<ServerInfo>>>,
    auth_provider: Option<AuthProvider>,
    source: Option<Source>,
    /// Cached server connections for fast failover
    cached_connections: Arc<RwLock<Vec<PlexConnection>>>,
    /// Last time we discovered servers
    last_discovery: Arc<RwLock<Option<Instant>>>,
    /// Track the original URL to detect changes
    original_url: Arc<RwLock<Option<String>>>,
    /// Current PlayQueue state for proper playback tracking
    play_queue_state: Arc<Mutex<PlayQueueState>>,
}

#[derive(Debug, Clone, Default)]
struct PlayQueueState {
    play_queue_id: Option<i64>,
    play_queue_item_id: Option<i64>,
    play_queue_version: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub name: String,
    pub is_local: bool,
    pub is_relay: bool,
}

impl PlexBackend {
    // Internal helper method - used by initialize()
    async fn is_initialized(&self) -> bool {
        let has_token = self.auth_token.read().await.is_some();
        let has_api = self.api.read().await.is_some();
        let has_url = self.base_url.read().await.is_some();

        has_token && has_api && has_url
    }

    /// Get the Plex API client if available (for PlayQueue operations)
    pub async fn get_api_for_playqueue(&self) -> Option<PlexApi> {
        self.api.read().await.clone()
    }

    /// Create a temporary PlexBackend for authentication purposes
    pub fn new_for_auth(base_url: String, token: String) -> Self {
        let url = Some(base_url);
        Self {
            base_url: Arc::new(RwLock::new(url.clone())),
            auth_token: Arc::new(RwLock::new(Some(token))),
            backend_id: format!("temp_{}", uuid::Uuid::new_v4()),
            last_sync_time: Arc::new(RwLock::new(None)),
            api: Arc::new(RwLock::new(None)),
            server_name: Arc::new(RwLock::new(None)),
            server_info: Arc::new(RwLock::new(None)),
            auth_provider: None,
            source: None,
            cached_connections: Arc::new(RwLock::new(Vec::new())),
            last_discovery: Arc::new(RwLock::new(None)),
            original_url: Arc::new(RwLock::new(url)),
            play_queue_state: Arc::new(Mutex::new(PlayQueueState::default())),
        }
    }

    /// Create a new PlexBackend from an AuthProvider and Source
    pub fn from_auth(auth_provider: AuthProvider, source: Source) -> Result<Self> {
        // Validate that this is a Plex auth provider
        if !matches!(auth_provider, AuthProvider::PlexAccount { .. }) {
            return Err(anyhow!("Invalid auth provider type for Plex backend"));
        }

        // Validate that this is a Plex source
        if !matches!(source.source_type, SourceType::PlexServer { .. }) {
            return Err(anyhow!("Invalid source type for Plex backend"));
        }

        let original_url = source.connection_info.primary_url.clone();
        Ok(Self {
            base_url: Arc::new(RwLock::new(original_url.clone())),
            auth_token: Arc::new(RwLock::new(None)), // Will be loaded from AuthProvider
            backend_id: source.id.clone(),
            last_sync_time: Arc::new(RwLock::new(None)),
            api: Arc::new(RwLock::new(None)),
            server_name: Arc::new(RwLock::new(Some(source.name.clone()))),
            server_info: Arc::new(RwLock::new(None)),
            auth_provider: Some(auth_provider),
            source: Some(source),
            cached_connections: Arc::new(RwLock::new(Vec::new())),
            last_discovery: Arc::new(RwLock::new(None)),
            original_url: Arc::new(RwLock::new(original_url)),
            play_queue_state: Arc::new(Mutex::new(PlayQueueState::default())),
        })
    }

    /// Get the current base URL being used
    pub async fn get_current_url(&self) -> Option<String> {
        self.base_url.read().await.clone()
    }

    /// Check if the URL has changed from the original
    pub async fn has_url_changed(&self) -> bool {
        let current = self.base_url.read().await.clone();
        let original = self.original_url.read().await.clone();
        current != original
    }

    /// Check if the server is reachable without blocking for too long
    pub async fn check_connectivity(&self) -> bool {
        if let Some(base_url) = self.base_url.read().await.as_ref()
            && let Some(token) = self.auth_token.read().await.as_ref()
        {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(2))
                .danger_accept_invalid_certs(true)
                .build();

            if let Ok(client) = client {
                let response = client
                    .get(format!("{}/identity", base_url))
                    .headers(create_standard_headers(Some(token)))
                    .send()
                    .await;

                return response.is_ok();
            }
        }
        false
    }

    /// Save the authentication token to keyring with file fallback

    /// Test all connections in parallel and return the fastest responding one
    async fn find_best_connection(
        &self,
        server: &PlexServer,
        token: &str,
    ) -> Result<PlexConnection> {
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
                    .timeout(Duration::from_secs(2))
                    .danger_accept_invalid_certs(true) // Plex uses self-signed certs
                    .build()?;

                // Try to access the server identity endpoint
                let response = client
                    .get(format!("{}/identity", uri))
                    .headers(create_standard_headers(Some(&token)))
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
                tracing::info!(
                    "Best connection found: {} (latency: {:?})",
                    result.0.uri,
                    result.1
                );
                Ok(result.0)
            }
            Err(_) => {
                // If all parallel attempts fail, try them sequentially with more time
                tracing::warn!("All parallel connection attempts failed, trying sequentially...");

                // Sort connections by priority: local non-relay first, then remote non-relay, then relay
                let mut sorted_connections = server.connections.clone();
                sorted_connections.sort_by_key(|c| {
                    if c.local && !c.relay {
                        0
                    } else if !c.relay {
                        1
                    } else {
                        2
                    }
                });

                for conn in sorted_connections {
                    let client = reqwest::Client::builder()
                        .timeout(Duration::from_secs(5))
                        .danger_accept_invalid_certs(true)
                        .build()?;

                    let response = client
                        .get(format!("{}/identity", conn.uri))
                        .headers(create_standard_headers(Some(token)))
                        .send()
                        .await;

                    if let Ok(resp) = response
                        && resp.status().is_success()
                    {
                        tracing::info!("Successfully connected to {} (fallback)", conn.uri);
                        return Ok(conn);
                    }
                }

                Err(anyhow!("Failed to connect to any server endpoint"))
            }
        }
    }

    /// Try to get a working connection from cache or rediscover
    async fn get_working_connection(&self) -> Result<String> {
        // First check if current base_url works with ConnectionCache
        if let Some(current_url) = self.base_url.read().await.as_ref() {
            use crate::models::SourceId;
            use crate::services::core::ConnectionService;

            let source_id = self.source.as_ref().map(|s| SourceId::new(s.id.clone()));
            if let Some(ref sid) = source_id {
                let cache = ConnectionService::cache();
                if cache.should_skip_test(sid).await {
                    // Recent successful connection, use it
                    tracing::info!("Using cached connection, skipping test");
                    return Ok(current_url.clone());
                }
            }

            // Quick test current URL
            if self.test_connection(current_url, 1).await {
                if let Some(ref sid) = source_id {
                    let cache = ConnectionService::cache();
                    cache.update_success(sid, 100).await;
                }
                return Ok(current_url.clone());
            }
        }

        // Try cached connections in parallel
        let cached = self.cached_connections.read().await.clone();
        if !cached.is_empty() {
            let token = self
                .auth_token
                .read()
                .await
                .clone()
                .ok_or_else(|| anyhow!("No auth token available"))?;

            // Test all cached connections in parallel
            match self.find_best_from_cached(&cached, &token).await {
                Ok(conn) => {
                    let new_url = conn.uri.clone();
                    *self.base_url.write().await = Some(new_url.clone());

                    // Update cache
                    if let Some(source) = &self.source {
                        use crate::services::core::ConnectionService;
                        let sid = SourceId::new(source.id.clone());
                        let cache = ConnectionService::cache();
                        cache.update_success(&sid, 100).await;
                    }

                    return Ok(new_url);
                }
                Err(e) => {
                    tracing::debug!("Cached connections failed: {}", e);
                }
            }
        }

        // Fall back to discovery if cache is stale (> 5 minutes)
        let should_rediscover = self
            .last_discovery
            .read()
            .await
            .map(|t| t.elapsed() > Duration::from_secs(300))
            .unwrap_or(true);

        if should_rediscover {
            tracing::info!("Rediscovering Plex servers...");
            // This will update cached_connections
            self.rediscover_servers().await?;

            // Try again with fresh connections
            if let Some(url) = self.base_url.read().await.as_ref() {
                return Ok(url.clone());
            }
        }

        Err(anyhow!("No working connection found"))
    }

    /// Test a specific connection quickly
    async fn test_connection(&self, url: &str, timeout_secs: u64) -> bool {
        let token = match self.auth_token.read().await.as_ref() {
            Some(t) => t.clone(),
            None => return false,
        };

        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .danger_accept_invalid_certs(true)
            .build()
        {
            Ok(c) => c,
            Err(_) => return false,
        };

        match client
            .get(format!("{}/identity", url))
            .headers(create_standard_headers(Some(&token)))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => true,
            _ => false,
        }
    }

    /// Find best connection from cached list
    async fn find_best_from_cached(
        &self,
        connections: &[PlexConnection],
        token: &str,
    ) -> Result<PlexConnection> {
        use futures::future::select_ok;

        let mut futures = Vec::new();
        for conn in connections {
            let uri = conn.uri.clone();
            let token = token.to_string();
            let conn_clone = conn.clone();

            let future = async move {
                let client = reqwest::Client::builder()
                    .timeout(Duration::from_secs(1))
                    .danger_accept_invalid_certs(true)
                    .build()?;

                let response = client
                    .get(format!("{}/identity", uri))
                    .headers(create_standard_headers(Some(&token)))
                    .send()
                    .await;

                match response {
                    Ok(resp) if resp.status().is_success() => Ok(conn_clone),
                    _ => Err(anyhow!("Connection failed")),
                }
            };

            futures.push(Box::pin(future));
        }

        match select_ok(futures).await {
            Ok((result, _)) => Ok(result),
            Err(_) => Err(anyhow!("All cached connections failed")),
        }
    }

    /// Rediscover servers and update cache
    async fn rediscover_servers(&self) -> Result<()> {
        let token = self
            .auth_token
            .read()
            .await
            .clone()
            .ok_or_else(|| anyhow!("No auth token available"))?;

        let servers = PlexAuth::discover_servers(&token).await?;

        // Find our server
        let target_server = if let Some(ref source) = self.source {
            if let SourceType::PlexServer { ref machine_id, .. } = source.source_type {
                servers.iter().find(|s| &s.client_identifier == machine_id)
            } else {
                servers.first()
            }
        } else {
            servers.first()
        };

        if let Some(server) = target_server {
            // Cache connections
            *self.cached_connections.write().await = server.connections.clone();
            *self.last_discovery.write().await = Some(Instant::now());

            // Find best connection
            match self.find_best_connection(server, &token).await {
                Ok(best_conn) => {
                    *self.base_url.write().await = Some(best_conn.uri.clone());
                    Ok(())
                }
                Err(e) => Err(e),
            }
        } else {
            Err(anyhow!("No matching server found"))
        }
    }

    /// Trigger background server discovery to refresh connections
    pub async fn refresh_connections_background(&self) {
        // Check if we should refresh (only if last discovery is old)
        let should_refresh = self
            .last_discovery
            .read()
            .await
            .map(|t| t.elapsed() > Duration::from_secs(60))
            .unwrap_or(true);

        if !should_refresh {
            tracing::debug!("Skipping background refresh, recently discovered");
            return;
        }

        // Clone necessary data for background task
        let backend = self.clone_for_background();

        // Spawn background task
        tokio::spawn(async move {
            tracing::info!("Starting background server discovery");
            match backend.rediscover_servers().await {
                Ok(()) => tracing::info!("Background server discovery completed successfully"),
                Err(e) => tracing::warn!("Background server discovery failed: {}", e),
            }
        });
    }

    /// Create a minimal clone for background operations
    fn clone_for_background(&self) -> PlexBackend {
        PlexBackend {
            base_url: self.base_url.clone(),
            auth_token: self.auth_token.clone(),
            backend_id: self.backend_id.clone(),
            last_sync_time: self.last_sync_time.clone(),
            api: self.api.clone(),
            server_name: self.server_name.clone(),
            server_info: self.server_info.clone(),
            auth_provider: self.auth_provider.clone(),
            source: self.source.clone(),
            cached_connections: self.cached_connections.clone(),
            last_discovery: self.last_discovery.clone(),
            original_url: self.original_url.clone(),
            play_queue_state: self.play_queue_state.clone(),
        }
    }

    /// Get the API client, ensuring it's initialized
    async fn get_api(&self) -> Result<PlexApi> {
        let api_guard = self.api.read().await;
        if let Some(api) = api_guard.as_ref() {
            // Return the existing API instance
            return Ok(api.clone());
        }

        // API not initialized, return error
        drop(api_guard);
        tracing::error!(
            "Plex API not initialized for backend {}. Please authenticate first.",
            self.backend_id
        );
        Err(anyhow::anyhow!(
            "Plex API not initialized. Please authenticate first."
        ))
    }

    // PlayQueue Management Methods

    /// Create a PlayQueue for a media item and store its state
    pub async fn create_play_queue(&self, media_id: &str, media_type: &str) -> Result<()> {
        let api = self.get_api().await?;

        // Create the PlayQueue
        let play_queue = api.create_play_queue(media_id, media_type).await?;

        // Store the PlayQueue state
        if let Some(container) = Some(&play_queue.media_container) {
            let mut state = self.play_queue_state.lock().await;
            state.play_queue_id = container.play_queue_id;
            state.play_queue_version = container.play_queue_version;

            // Find the current item ID (usually the first one)
            if let Some(first_item) = container.metadata.first() {
                state.play_queue_item_id = Some(first_item.play_queue_item_id);
            }

            debug!(
                "Created PlayQueue - ID: {:?}, Version: {:?}, Item: {:?}",
                state.play_queue_id, state.play_queue_version, state.play_queue_item_id
            );
        }

        Ok(())
    }

    /// Clear the current PlayQueue state
    pub async fn clear_play_queue(&self) {
        let mut state = self.play_queue_state.lock().await;
        state.play_queue_id = None;
        state.play_queue_item_id = None;
        state.play_queue_version = None;
        debug!("Cleared PlayQueue state");
    }

    /// Update the current PlayQueue item when playback progresses to the next item
    pub async fn set_play_queue_item(&self, item_id: i64) {
        let mut state = self.play_queue_state.lock().await;
        state.play_queue_item_id = Some(item_id);
        debug!("Updated PlayQueue item to: {}", item_id);
    }

    /// Get the current PlayQueue ID if available
    pub async fn get_play_queue_id(&self) -> Option<i64> {
        let state = self.play_queue_state.lock().await;
        state.play_queue_id
    }

    /// Check if we have an active PlayQueue
    pub async fn has_play_queue(&self) -> bool {
        let state = self.play_queue_state.lock().await;
        state.play_queue_id.is_some()
    }
}

#[async_trait]
impl MediaBackend for PlexBackend {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn initialize(&self) -> Result<Option<User>> {
        // If we have an AuthProvider, use its token
        let token = if let Some(auth_provider) = &self.auth_provider {
            match auth_provider {
                AuthProvider::PlexAccount { token, .. } => {
                    if !token.is_empty() {
                        token.clone()
                    } else {
                        // Try to get token from keyring via AuthService
                        match AuthService::load_credentials(&SourceId::new(auth_provider.id()))
                            .await
                        {
                            Ok(Some(Credentials::Token { token, .. })) => token,
                            _ => {
                                tracing::warn!("No token found in AuthProvider or keyring");
                                return Ok(None);
                            }
                        }
                    }
                }
                _ => {
                    tracing::error!("Invalid AuthProvider type for Plex backend");
                    return Ok(None);
                }
            }
        } else {
            // Legacy path: Try to get token from keyring first, then fallback to file
            let token = {
                // Try keyring first
                let service = "dev.arsfeld.Reel";
                let account = &self.backend_id;

                tracing::info!(
                    "Looking for saved token - service: '{}', account: '{}'",
                    service,
                    account
                );

                match keyring::Entry::new(service, account) {
                    Ok(entry) => match entry.get_password() {
                        Ok(token) => {
                            tracing::info!(
                                "Successfully retrieved token from keyring for backend {}",
                                self.backend_id
                            );
                            Some(token)
                        }
                        Err(e) => {
                            let error_str = e.to_string();
                            tracing::debug!(
                                "Keyring error for backend {}: {}",
                                self.backend_id,
                                error_str
                            );
                            None
                        }
                    },
                    Err(e) => {
                        tracing::debug!("Failed to create keyring entry: {}", e);
                        None
                    }
                }
            };

            // If keyring failed, try file fallback
            if let Some(token) = token {
                token
            } else {
                // Try to read from fallback file
                let config_dir = dirs::config_dir()
                    .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
                let token_file = config_dir
                    .join("reel")
                    .join(format!(".{}.token", self.backend_id));

                if token_file.exists() {
                    tracing::info!(
                        "Checking fallback token file for backend {}",
                        self.backend_id
                    );
                    match std::fs::read(&token_file) {
                        Ok(obfuscated) => {
                            // De-obfuscate the token
                            let token_bytes: Vec<u8> = obfuscated
                                .iter()
                                .enumerate()
                                .map(|(i, &b)| b ^ ((i as u8) + 42))
                                .collect();

                            match String::from_utf8(token_bytes) {
                                Ok(token) => {
                                    tracing::info!(
                                        "Successfully retrieved token from file storage for backend {}",
                                        self.backend_id
                                    );
                                    token
                                }
                                Err(e) => {
                                    tracing::error!("Failed to decode token from file: {}", e);
                                    return Ok(None);
                                }
                            }
                        }
                        Err(e) => {
                            tracing::debug!("Failed to read token file: {}", e);
                            return Ok(None);
                        }
                    }
                } else {
                    tracing::debug!(
                        "No saved token found for backend {} in keyring or file",
                        self.backend_id
                    );
                    return Ok(None);
                }
            }
        };

        if token.is_empty() {
            return Ok(None);
        }

        // Get user info with the saved token
        let plex_user = match PlexAuth::get_user(&token).await {
            Ok(user) => user,
            Err(e) => {
                let error_str = e.to_string();
                tracing::error!("Failed to get user info with saved token: {}", error_str);

                // Only delete token if it's an authentication error
                if error_str.contains("Authentication failed")
                    || error_str.contains("Invalid or expired token")
                {
                    tracing::info!("Token appears to be invalid, removing from storage");

                    // Try to delete from keyring
                    if let Ok(entry) = keyring::Entry::new("dev.arsfeld.Reel", &self.backend_id) {
                        entry.delete_credential().ok();
                    }

                    // Also delete from file fallback
                    if let Some(config_dir) = dirs::config_dir() {
                        let token_file = config_dir
                            .join("reel")
                            .join(format!(".{}.token", self.backend_id));
                        std::fs::remove_file(token_file).ok();
                    }

                    return Ok(None);
                } else if error_str.contains("Network error") {
                    tracing::warn!("Network error while validating token, will use cached data");
                    // Store the token even though we can't validate it
                    *self.auth_token.write().await = Some(token.to_string());

                    // Return a minimal user object to indicate partial success
                    // The username will be loaded from cache later
                    return Ok(Some(User {
                        id: "offline".to_string(),
                        username: "Offline Mode".to_string(),
                        email: None,
                        avatar_url: None,
                    }));
                } else {
                    tracing::warn!("Server error while validating token, will use cached data");
                    // Store the token for retry
                    *self.auth_token.write().await = Some(token.to_string());

                    // Return a minimal user object
                    return Ok(Some(User {
                        id: "offline".to_string(),
                        username: "Offline Mode".to_string(),
                        email: None,
                        avatar_url: None,
                    }));
                }
            }
        };

        // Store the token
        *self.auth_token.write().await = Some(token.to_string());

        // Store the original URL if not already set
        if self.original_url.read().await.is_none() {
            *self.original_url.write().await = self.base_url.read().await.clone();
        }

        // Check if we already have a URL from the source
        let existing_url = self.base_url.read().await.clone();

        if let Some(url) = existing_url {
            // We already have a URL from the source
            tracing::info!("Using existing URL from source: {}", url);

            // Check if we can skip testing via cache
            use crate::models::SourceId;
            use crate::services::core::ConnectionService;

            let source_id = self.source.as_ref().map(|s| SourceId::new(s.id.clone()));
            let should_test = if let Some(ref sid) = source_id {
                let cache = ConnectionService::cache();
                !cache.should_skip_test(sid).await
            } else {
                true // Always test if no source ID
            };

            if should_test {
                // Test if the URL is actually reachable
                let test_client = reqwest::Client::builder()
                    .timeout(Duration::from_secs(2))
                    .danger_accept_invalid_certs(true)
                    .build()?;

                let test_result = test_client
                    .get(format!("{}/identity", url))
                    .headers(create_standard_headers(Some(&token)))
                    .send()
                    .await;

                if let Err(e) = test_result {
                    tracing::warn!(
                        "Saved URL {} is not reachable: {}. Will try to discover servers.",
                        url,
                        e
                    );
                    // Clear the bad URL and fall through to discovery
                    *self.base_url.write().await = None;

                    // Update cache to mark failure
                    if let Some(ref sid) = source_id {
                        let cache = ConnectionService::cache();
                        cache.update_failure(sid).await;
                    }
                } else {
                    tracing::info!("URL {} is reachable, using it", url);

                    // Update cache with success
                    if let Some(ref sid) = source_id {
                        let cache = ConnectionService::cache();
                        cache.update_success(sid, 100).await; // Assuming ~100ms for successful test
                    }
                }
            } else {
                tracing::info!("Skipping URL test due to recent cache (URL: {})", url);
            }

            // If URL is still valid (not cleared due to failure), create API client
            if self.base_url.read().await.is_some() {
                // Create and store the API client
                let api = PlexApi::with_backend_id(
                    url.clone(),
                    token.to_string(),
                    self.backend_id.clone(),
                );
                *self.api.write().await = Some(api);

                // Store server info
                if let Some(ref source) = self.source {
                    *self.server_info.write().await = Some(ServerInfo {
                        name: source.name.clone(),
                        is_local: url.contains("192.168.")
                            || url.contains("10.")
                            || url.contains("172.")
                            || url.contains("localhost"),
                        is_relay: url.contains("plex.direct"),
                    });
                }

                // CRITICAL FIX: Populate cached_connections even when using existing URL
                // This prevents get_working_connection() from having to rediscover servers
                if self.cached_connections.read().await.is_empty() {
                    tracing::info!("Populating connection cache for existing URL");

                    // Try to discover all connections for this server
                    match PlexAuth::discover_servers(&token).await {
                        Ok(servers) => {
                            // Find the server that matches our current URL
                            if let Some(ref source) = self.source {
                                if let SourceType::PlexServer { ref machine_id, .. } =
                                    source.source_type
                                {
                                    if let Some(server) =
                                        servers.iter().find(|s| &s.client_identifier == machine_id)
                                    {
                                        tracing::info!(
                                            "Found {} connections for server {}",
                                            server.connections.len(),
                                            server.name
                                        );
                                        *self.cached_connections.write().await =
                                            server.connections.clone();
                                        *self.last_discovery.write().await = Some(Instant::now());
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            tracing::debug!("Could not populate connection cache: {}", e);
                            // Not critical - we can still use the single URL we have
                        }
                    }
                }
            }
        }

        // If we don't have a URL or the saved one failed, discover servers
        if self.base_url.read().await.is_none() {
            // No URL saved, need to discover servers
            tracing::info!("No saved URL, discovering servers...");

            match PlexAuth::discover_servers(&token).await {
                Ok(servers) => {
                    tracing::info!("Discovered {} Plex servers", servers.len());

                    // Log server details for debugging
                    for server in &servers {
                        tracing::debug!(
                            "Server: {} (ID: {}, Owned: {})",
                            server.name,
                            server.client_identifier,
                            server.owned
                        );
                    }

                    // Find the right server based on our source's machine_id if available
                    let target_server = if let Some(ref source) = self.source {
                        if let SourceType::PlexServer { ref machine_id, .. } = source.source_type {
                            tracing::info!("Looking for server with machine_id: {}", machine_id);
                            let found = servers.iter().find(|s| &s.client_identifier == machine_id);
                            if found.is_none() && !machine_id.is_empty() {
                                tracing::warn!(
                                    "Could not find server with machine_id: {}. Available servers: {:?}",
                                    machine_id,
                                    servers
                                        .iter()
                                        .map(|s| &s.client_identifier)
                                        .collect::<Vec<_>>()
                                );
                            }
                            found
                        } else {
                            tracing::info!("Source type is not PlexServer, using first server");
                            servers.first()
                        }
                    } else {
                        tracing::info!("No source info available, using first server");
                        servers.first()
                    };

                    if let Some(server) = target_server {
                        tracing::info!(
                            "Selected server: {} (ID: {})",
                            server.name,
                            server.client_identifier
                        );
                        // Test all connections in parallel and use the fastest one
                        match self.find_best_connection(server, &token).await {
                            Ok(best_conn) => {
                                *self.base_url.write().await = Some(best_conn.uri.clone());
                                *self.server_name.write().await = Some(server.name.clone());

                                // Store server info
                                *self.server_info.write().await = Some(ServerInfo {
                                    name: server.name.clone(),
                                    is_local: best_conn.local,
                                    is_relay: best_conn.relay,
                                });

                                // Cache all connections for this server for fast failover
                                *self.cached_connections.write().await = server.connections.clone();
                                *self.last_discovery.write().await = Some(Instant::now());

                                // Create and store the API client
                                let api = PlexApi::with_backend_id(
                                    best_conn.uri.clone(),
                                    token.to_string(),
                                    self.backend_id.clone(),
                                );
                                *self.api.write().await = Some(api);

                                tracing::info!(
                                    "Connected to Plex server: {} at {} ({})",
                                    server.name,
                                    best_conn.uri,
                                    if best_conn.local {
                                        "local"
                                    } else if best_conn.relay {
                                        "relay"
                                    } else {
                                        "remote"
                                    }
                                );

                                // TODO: Update the source with the working URL using DatabaseConnection
                                // This functionality needs to be moved to stateless service
                                tracing::info!(
                                    "Found working connection URL: {} ({})",
                                    best_conn.uri,
                                    if best_conn.local { "local" } else { "remote" }
                                );
                            }
                            Err(e) => {
                                tracing::warn!("Failed to connect to any server endpoint: {}", e);
                                // Return error if we can't connect to the server
                                return Err(anyhow::anyhow!(
                                    "Failed to connect to Plex server '{}': {}. The server may be offline or unreachable.",
                                    server.name,
                                    e
                                ));
                            }
                        }
                    } else {
                        tracing::warn!("No matching Plex server found for this source");
                        // Return error if we couldn't find or connect to any server
                        return Err(anyhow::anyhow!(
                            "No matching Plex server found. The server may be offline or the source configuration may be incorrect."
                        ));
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to discover servers: {}", e);
                    // Return error if we can't discover servers
                    return Err(anyhow::anyhow!(
                        "Failed to discover Plex servers: {}. Please check your network connection.",
                        e
                    ));
                }
            }
        }

        // Only return success if we have successfully initialized the API
        if self.is_initialized().await {
            Ok(Some(User {
                id: plex_user.id.to_string(),
                username: plex_user.username,
                email: Some(plex_user.email),
                avatar_url: plex_user.thumb,
            }))
        } else {
            Err(anyhow::anyhow!(
                "Failed to initialize Plex backend. API client not properly configured."
            ))
        }
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
            _ => Err(anyhow::anyhow!("Plex only supports token authentication")),
        }
    }

    async fn get_libraries(&self) -> Result<Vec<Library>> {
        let api = self.get_api().await?;
        api.get_libraries().await
    }

    async fn get_movies(&self, library_id: &LibraryId) -> Result<Vec<Movie>> {
        let api = self.get_api().await?;
        api.get_movies(&library_id.to_string()).await
    }

    async fn get_shows(&self, library_id: &LibraryId) -> Result<Vec<Show>> {
        let api = self.get_api().await?;
        api.get_shows(&library_id.to_string()).await
    }

    async fn get_seasons(&self, show_id: &ShowId) -> Result<Vec<Season>> {
        let api = self.get_api().await?;
        api.get_seasons(&show_id.to_string()).await
    }

    async fn get_episodes(&self, show_id: &ShowId, season_number: u32) -> Result<Vec<Episode>> {
        let api = self.get_api().await?;

        // First, get the seasons for this show to find the correct season ID
        let seasons = api.get_seasons(&show_id.to_string()).await?;

        // Find the season with the matching season number
        let season = seasons
            .iter()
            .find(|s| s.season_number == season_number)
            .ok_or_else(|| anyhow!("Season {} not found for show {}", season_number, show_id))?;

        // Now get the episodes for the correct season
        let mut episodes = api.get_episodes(&season.id).await?;

        // Ensure show_id is set correctly for all episodes
        for episode in &mut episodes {
            if episode.show_id.is_none() {
                episode.show_id = Some(show_id.to_string());
            }
        }

        Ok(episodes)
    }

    async fn get_stream_url(&self, media_id: &MediaItemId) -> Result<StreamInfo> {
        tracing::info!(
            "get_stream_url() called for media_id: {} on backend: {}",
            media_id,
            self.backend_id
        );

        // Extract the actual Plex rating key from the composite ID
        // Format: "backend_id:library_id:type:rating_key" or variations
        let media_id_str = media_id.as_str();
        let rating_key = if media_id_str.contains(':') {
            // Split and get the last part which should be the rating key
            media_id_str.split(':').next_back().unwrap_or(media_id_str)
        } else {
            // If no separator, assume it's already just the rating key
            media_id_str
        };

        tracing::info!(
            "Extracted rating key: {} from media_id: {}",
            rating_key,
            media_id
        );

        // Optimize: First ensure we have a working connection without full re-initialization
        let working_url = match self.get_working_connection().await {
            Ok(url) => {
                tracing::info!("Got working connection quickly: {}", url);
                url
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to get working connection: {}, falling back to API",
                    e
                );
                // Fall back to existing API if available
                let api = self.get_api().await?;
                tracing::info!("Got API client, fetching stream URL from Plex API");
                let result = api.get_stream_url(rating_key).await;
                match &result {
                    Ok(info) => tracing::info!("Successfully got stream URL: {}", info.url),
                    Err(e) => tracing::error!("Failed to get stream URL: {}", e),
                }
                return result;
            }
        };

        // Create temporary API with working URL if needed
        let api = if let Some(existing_api) = self.api.read().await.as_ref() {
            existing_api.clone()
        } else {
            // Create temporary API with the working URL
            let token = self
                .auth_token
                .read()
                .await
                .clone()
                .ok_or_else(|| anyhow!("No auth token available"))?;
            let temp_api =
                PlexApi::with_backend_id(working_url.clone(), token, self.backend_id.clone());
            // Store it for future use
            *self.api.write().await = Some(temp_api.clone());
            temp_api
        };

        tracing::info!("Fetching stream URL from Plex API");
        let result = api.get_stream_url(rating_key).await;
        match &result {
            Ok(info) => tracing::info!("Successfully got stream URL: {}", info.url),
            Err(e) => tracing::error!("Failed to get stream URL: {}", e),
        }
        result
    }

    async fn update_progress(
        &self,
        media_id: &MediaItemId,
        position: Duration,
        duration: Duration,
    ) -> Result<()> {
        // Extract the actual Plex rating key from the composite ID
        let media_id_str = media_id.as_str();
        let rating_key = if media_id_str.contains(':') {
            media_id_str.split(':').next_back().unwrap_or(media_id_str)
        } else {
            media_id_str
        };

        let api = self.get_api().await?;

        // Check if we have an active PlayQueue
        let play_queue_state = self.play_queue_state.lock().await;
        if let (Some(queue_id), Some(item_id)) = (
            play_queue_state.play_queue_id,
            play_queue_state.play_queue_item_id,
        ) {
            drop(play_queue_state); // Release the lock early

            // Use PlayQueue-based progress tracking
            debug!(
                "Using PlayQueue progress tracking - queue: {}, item: {}",
                queue_id, item_id
            );
            api.update_play_queue_progress(
                queue_id, item_id, rating_key, position, duration, "playing",
            )
            .await
        } else {
            drop(play_queue_state); // Release the lock early

            // Fall back to direct timeline updates
            debug!("Using direct timeline progress tracking");
            api.update_progress(rating_key, position, duration).await
        }
    }

    // Removed unused trait methods: mark_watched, mark_unwatched, get_watch_status, search,
    // get_backend_info, get_last_sync_time, supports_offline, fetch_episode_markers,
    // fetch_media_markers, find_next_episode

    async fn get_home_sections(&self) -> Result<Vec<crate::models::HomeSection>> {
        let api = self.get_api().await?;
        api.get_home_sections().await
    }

    async fn get_backend_id(&self) -> BackendId {
        BackendId::new(&self.backend_id)
    }
}

impl PlexBackend {
    /// Perform a global search across all libraries
    pub async fn search(
        &self,
        query: &str,
        limit: Option<usize>,
    ) -> Result<Vec<crate::models::MediaItem>> {
        let api = self.get_api().await?;
        api.search_global(query, limit).await
    }

    /// Search within a specific library
    pub async fn search_library(
        &self,
        library_id: &str,
        query: &str,
        media_type: Option<&str>,
        sort: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<crate::models::MediaItem>> {
        let api = self.get_api().await?;
        api.search_library(library_id, query, media_type, sort, limit)
            .await
    }

    /// Search with filters
    pub async fn search_with_filters(
        &self,
        library_id: &str,
        query: Option<&str>,
        genre: Option<&str>,
        year: Option<u32>,
        rating_min: Option<f32>,
        unwatched: Option<bool>,
        sort: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<crate::models::MediaItem>> {
        let api = self.get_api().await?;
        api.search_with_filters(
            library_id, query, genre, year, rating_min, unwatched, sort, limit,
        )
        .await
    }
}

impl fmt::Debug for PlexBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlexBackend")
            .field("backend_id", &self.backend_id)
            .field(
                "has_base_url",
                &self
                    .base_url
                    .try_read()
                    .map(|u| u.is_some())
                    .unwrap_or(false),
            )
            .field(
                "has_auth_token",
                &self
                    .auth_token
                    .try_read()
                    .map(|t| t.is_some())
                    .unwrap_or(false),
            )
            .finish()
    }
}
