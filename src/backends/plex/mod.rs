mod api;
mod auth;

pub use api::PlexApi;
pub use auth::{PlexAuth, PlexConnection, PlexPin, PlexServer};

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dirs;
use reqwest::Client;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::info;

use super::traits::{MediaBackend, SearchResults};
use crate::models::{
    AuthProvider, ChapterMarker, Credentials, Episode, Library, Movie, Show, Source, SourceType,
    StreamInfo, User,
};
use crate::services::{auth_manager::AuthManager, cache::CacheManager};

pub struct PlexBackend {
    client: Client,
    base_url: Arc<RwLock<Option<String>>>,
    auth_token: Arc<RwLock<Option<String>>>,
    backend_id: String,
    last_sync_time: Arc<RwLock<Option<DateTime<Utc>>>>,
    api: Arc<RwLock<Option<PlexApi>>>,
    server_name: Arc<RwLock<Option<String>>>,
    server_info: Arc<RwLock<Option<ServerInfo>>>,
    cache: Option<Arc<CacheManager>>,
    auth_provider: Option<AuthProvider>,
    source: Option<Source>,
    auth_manager: Option<Arc<AuthManager>>,
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
        Self::with_cache(id, None)
    }

    pub fn with_cache(id: String, cache: Option<Arc<CacheManager>>) -> Self {
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
            cache,
            auth_provider: None,
            source: None,
            auth_manager: None,
        }
    }

    /// Create a new PlexBackend from an AuthProvider and Source
    pub fn from_auth(
        auth_provider: AuthProvider,
        source: Source,
        auth_manager: Arc<AuthManager>,
        cache: Option<Arc<CacheManager>>,
    ) -> Result<Self> {
        // Validate that this is a Plex auth provider
        if !matches!(auth_provider, AuthProvider::PlexAccount { .. }) {
            return Err(anyhow!("Invalid auth provider type for Plex backend"));
        }

        // Validate that this is a Plex source
        if !matches!(source.source_type, SourceType::PlexServer { .. }) {
            return Err(anyhow!("Invalid source type for Plex backend"));
        }

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Ok(Self {
            client,
            base_url: Arc::new(RwLock::new(source.connection_info.primary_url.clone())),
            auth_token: Arc::new(RwLock::new(None)), // Will be loaded from AuthProvider
            backend_id: source.id.clone(),
            last_sync_time: Arc::new(RwLock::new(None)),
            api: Arc::new(RwLock::new(None)),
            server_name: Arc::new(RwLock::new(Some(source.name.clone()))),
            server_info: Arc::new(RwLock::new(None)),
            cache,
            auth_provider: Some(auth_provider),
            source: Some(source),
            auth_manager: Some(auth_manager),
        })
    }

    pub fn set_cache(&mut self, cache: Arc<CacheManager>) {
        self.cache = Some(cache);
    }

    pub async fn get_server_info(&self) -> Option<ServerInfo> {
        self.server_info.read().await.clone()
    }

    pub async fn get_api_client(&self) -> Option<PlexApi> {
        self.api.read().await.clone()
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
                    .header("X-Plex-Token", token)
                    .send()
                    .await;

                return response.is_ok();
            }
        }
        false
    }

    /// Save the authentication token to keyring with file fallback
    pub async fn save_token(&self, token: &str) -> Result<()> {
        // Try keyring first
        let service = "dev.arsfeld.Reel";
        let account = &self.backend_id;

        tracing::info!(
            "Attempting to save token to keyring - service: '{}', account: '{}'",
            service,
            account
        );

        match keyring::Entry::new(service, account) {
            Ok(entry) => match entry.set_password(token) {
                Ok(_) => {
                    tracing::info!(
                        "Token successfully saved to keyring for backend {}",
                        self.backend_id
                    );
                    return Ok(());
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to save to keyring: {}, falling back to file storage",
                        e
                    );
                }
            },
            Err(e) => {
                tracing::warn!(
                    "Failed to create keyring entry: {}, falling back to file storage",
                    e
                );
            }
        }

        // Fallback to file storage (encrypted with simple obfuscation)
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
        let token_file = config_dir
            .join("reel")
            .join(format!(".{}.token", self.backend_id));

        // Create directory if it doesn't exist
        if let Some(parent) = token_file.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Simple obfuscation - not real encryption but better than plaintext
        let obfuscated = token
            .bytes()
            .enumerate()
            .map(|(i, b)| b ^ ((i as u8) + 42))
            .collect::<Vec<u8>>();

        std::fs::write(&token_file, obfuscated)?;

        // Set restrictive permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&token_file)?.permissions();
            perms.set_mode(0o600); // Read/write for owner only
            std::fs::set_permissions(&token_file, perms)?;
        }

        tracing::info!(
            "Token saved to fallback file storage for backend {}",
            self.backend_id
        );
        Ok(())
    }

    /// Authenticate with PIN and select a server
    pub async fn authenticate_with_pin(&self, pin: &PlexPin, server: &PlexServer) -> Result<()> {
        // Poll for the auth token
        let token = PlexAuth::poll_for_token(&pin.id).await?;

        // Store the auth token in memory
        *self.auth_token.write().await = Some(token.clone());

        // Save token to keyring
        if let Err(e) = self.save_token(&token).await {
            tracing::error!("Failed to save token to keyring: {}", e);
            // Continue anyway - the token is in memory
        }

        // Find the best connection for the server
        let connection = server
            .connections
            .iter()
            .find(|c| !c.relay) // Prefer direct connections
            .or_else(|| server.connections.first())
            .ok_or_else(|| anyhow::anyhow!("No valid connection found for server"))?;

        // Store the base URL
        *self.base_url.write().await = Some(connection.uri.clone());

        // Create the API client with cache
        let api = PlexApi::with_cache(
            connection.uri.clone(),
            token,
            self.cache.clone(),
            self.backend_id.clone(),
        );
        *self.api.write().await = Some(api);

        Ok(())
    }

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
                        .timeout(Duration::from_secs(10))
                        .danger_accept_invalid_certs(true)
                        .build()?;

                    let response = client
                        .get(format!("{}/identity", conn.uri))
                        .header("X-Plex-Token", token)
                        .header("Accept", "application/json")
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

    /// Get the API client, ensuring it's initialized
    async fn get_api(&self) -> Result<PlexApi> {
        tracing::debug!("get_api() called for backend {}", self.backend_id);

        let api_guard = self.api.read().await;
        if let Some(_api) = api_guard.as_ref() {
            tracing::debug!("API client exists, checking base_url and auth_token");

            let base_url = self
                .base_url
                .read()
                .await
                .as_ref()
                .ok_or_else(|| {
                    tracing::error!("Base URL not set for backend {}", self.backend_id);
                    anyhow::anyhow!("Base URL not set")
                })?
                .clone();

            tracing::debug!("Base URL: {}", base_url);

            let auth_token = self
                .auth_token
                .read()
                .await
                .as_ref()
                .ok_or_else(|| {
                    tracing::error!("Auth token not set for backend {}", self.backend_id);
                    anyhow::anyhow!("Auth token not set")
                })?
                .clone();

            tracing::debug!("Auth token length: {}", auth_token.len());

            Ok(PlexApi::with_cache(
                base_url,
                auth_token,
                self.cache.clone(),
                self.backend_id.clone(),
            ))
        } else {
            tracing::error!(
                "Plex API not initialized for backend {}. Please authenticate first.",
                self.backend_id
            );
            Err(anyhow::anyhow!(
                "Plex API not initialized. Please authenticate first."
            ))
        }
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
                    } else if let Some(auth_manager) = &self.auth_manager {
                        // Try to get token from keyring via AuthManager
                        match auth_manager.get_credentials(auth_provider.id(), "token") {
                            Ok(t) => t,
                            Err(_) => {
                                tracing::warn!("No token found in AuthProvider or keyring");
                                return Ok(None);
                            }
                        }
                    } else {
                        tracing::warn!("No token in AuthProvider and no AuthManager available");
                        return Ok(None);
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

        // Check if we already have a URL from the source
        let existing_url = self.base_url.read().await.clone();

        if let Some(url) = existing_url {
            // We already have a URL from the source, just use it
            tracing::info!("Using existing URL from source: {}", url);

            // Test if the URL is actually reachable
            let test_client = reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .danger_accept_invalid_certs(true)
                .build()?;

            let test_result = test_client
                .get(format!("{}/identity", url))
                .header("X-Plex-Token", &token)
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
            } else {
                tracing::info!("URL {} is reachable, using it", url);

                // Create and store the API client with cache
                let api = PlexApi::with_cache(
                    url.clone(),
                    token.to_string(),
                    self.cache.clone(),
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
                        uri: url.clone(),
                    });
                }
            }
        }

        // If we don't have a URL or the saved one failed, discover servers
        if self.base_url.read().await.is_none() {
            // No URL saved, need to discover servers
            tracing::info!("No saved URL, discovering servers...");

            match PlexAuth::discover_servers(&token).await {
                Ok(servers) => {
                    // Find the right server based on our source's machine_id if available
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
                                    uri: best_conn.uri.clone(),
                                });

                                // Create and store the API client with cache
                                let api = PlexApi::with_cache(
                                    best_conn.uri.clone(),
                                    token.to_string(),
                                    self.cache.clone(),
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

                                // Update the source with the working URL if we have access to auth_manager
                                if let Some(ref auth_manager) = self.auth_manager {
                                    if let Some(ref source) = self.source {
                                        tracing::info!(
                                            "Updating source with working URL: {}",
                                            best_conn.uri
                                        );
                                        if let Err(e) = auth_manager
                                            .update_source_url(&source.id, &best_conn.uri)
                                            .await
                                        {
                                            tracing::warn!("Failed to update source URL: {}", e);
                                        }
                                    }
                                }
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
            _ => Err(anyhow::anyhow!("Plex only supports token authentication")),
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
        let season = seasons
            .iter()
            .find(|s| s.season_number == season_number)
            .ok_or_else(|| anyhow!("Season {} not found for show {}", season_number, show_id))?;

        // Now get the episodes for the correct season
        api.get_episodes(&season.id).await
    }

    async fn get_stream_url(&self, media_id: &str) -> Result<StreamInfo> {
        tracing::info!(
            "get_stream_url() called for media_id: {} on backend: {}",
            media_id,
            self.backend_id
        );
        let api = self.get_api().await?;
        tracing::info!("Got API client, fetching stream URL from Plex API");
        let result = api.get_stream_url(media_id).await;
        match &result {
            Ok(info) => tracing::info!("Successfully got stream URL: {}", info.url),
            Err(e) => tracing::error!("Failed to get stream URL: {}", e),
        }
        result
    }

    async fn update_progress(
        &self,
        media_id: &str,
        position: Duration,
        duration: Duration,
    ) -> Result<()> {
        let api = self.get_api().await?;
        api.update_progress(media_id, position, duration).await
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

        // Do a quick connectivity check
        let is_online = self.check_connectivity().await;

        if let Some(info) = server_info.as_ref() {
            let connection_type = if !is_online {
                // Server configured but not reachable
                super::traits::ConnectionType::Offline
            } else if info.is_local {
                super::traits::ConnectionType::Local
            } else if info.is_relay {
                super::traits::ConnectionType::Relay
            } else {
                super::traits::ConnectionType::Remote
            };

            super::traits::BackendInfo {
                name: self.backend_id.clone(),
                display_name: format!("Plex ({})", info.name),
                backend_type: super::traits::BackendType::Plex,
                server_name: Some(info.name.clone()),
                server_version: None,
                connection_type,
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

    async fn fetch_episode_markers(
        &self,
        episode_id: &str,
    ) -> Result<(Option<ChapterMarker>, Option<ChapterMarker>)> {
        let api = self.get_api().await?;
        api.fetch_episode_markers(episode_id).await
    }

    async fn fetch_media_markers(
        &self,
        media_id: &str,
    ) -> Result<(Option<ChapterMarker>, Option<ChapterMarker>)> {
        // Plex uses the same API endpoint for both movies and episodes
        let api = self.get_api().await?;
        api.fetch_episode_markers(media_id).await
    }

    async fn find_next_episode(&self, current_episode: &Episode) -> Result<Option<Episode>> {
        let api = self.get_api().await?;

        // For now, return None as we need to implement the logic to find the show
        // and get the next episode. This is a placeholder implementation.
        // TODO: Implement proper next episode finding logic

        info!(
            "Finding next episode after S{:02}E{:02} - {}",
            current_episode.season_number, current_episode.episode_number, current_episode.title
        );

        // For now, return None - this will show "No next episode available"
        // until we implement the full logic
        Ok(None)
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
