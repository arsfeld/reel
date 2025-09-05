use anyhow::{Result, anyhow};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::backends::plex::PlexAuth;
use crate::config::Config;
use crate::events::{
    event_bus::EventBus,
    types::{DatabaseEvent, EventPayload, EventType},
};
use crate::models::{AuthProvider, NetworkAuthType, NetworkCredentialData, Source, SourceType};

/// Manages authentication providers and their credentials
#[derive(Clone)]
pub struct AuthManager {
    providers: Arc<RwLock<HashMap<String, AuthProvider>>>,
    config: Arc<RwLock<Config>>,
    event_bus: Arc<EventBus>,
}

impl AuthManager {
    pub fn new(config: Arc<RwLock<Config>>, event_bus: Arc<EventBus>) -> Self {
        Self {
            providers: Arc::new(RwLock::new(HashMap::new())),
            config,
            event_bus,
        }
    }

    /// Load saved providers from config
    pub async fn load_providers(&self) -> Result<()> {
        let config = self.config.read().await;
        let mut providers = config.get_auth_providers();
        drop(config);

        if !providers.is_empty() {
            info!("Loading {} saved auth providers", providers.len());

            // Load tokens from keyring for each provider
            for (id, provider) in providers.iter_mut() {
                match provider {
                    AuthProvider::PlexAccount { token, .. } => {
                        // Load token from keyring
                        if let Ok(stored_token) = self.get_credentials(id, "token") {
                            *token = stored_token;
                            info!("Loaded token from keyring for Plex provider {}", id);
                        } else {
                            error!("Failed to load token from keyring for Plex provider {}", id);
                        }
                    }
                    AuthProvider::JellyfinAuth { access_token, .. } => {
                        // Load token from keyring
                        if let Ok(stored_token) = self.get_credentials(id, "token") {
                            *access_token = stored_token;
                            info!("Loaded token from keyring for Jellyfin provider {}", id);
                        } else {
                            error!(
                                "Failed to load token from keyring for Jellyfin provider {}",
                                id
                            );
                        }
                    }
                    _ => {}
                }
            }

            let mut auth_providers = self.providers.write().await;
            *auth_providers = providers;
        }

        Ok(())
    }

    /// Add a new Plex account and discover its servers
    pub async fn add_plex_account(&self, token: &str) -> Result<(String, Vec<Source>)> {
        info!("Adding Plex account with token");

        // Get user info from Plex
        let user = PlexAuth::get_user(token).await?;

        // Create the auth provider
        let provider_id = format!(
            "plex_{}",
            Uuid::new_v4().to_string().split('-').next().unwrap()
        );
        let provider = AuthProvider::PlexAccount {
            id: provider_id.clone(),
            username: user.username.clone(),
            email: user.email.clone(),
            token: token.to_string(),
            refresh_token: None, // Plex doesn't provide refresh tokens currently
            token_expiry: None,
        };

        // Store the provider
        {
            let mut providers = self.providers.write().await;
            providers.insert(provider_id.clone(), provider.clone());
        }

        // Save to config
        {
            let mut config = self.config.write().await;
            config.add_auth_provider(provider_id.clone(), provider.clone())?;
        }

        // Store token in keyring
        self.store_credentials(&provider_id, "token", token)?;

        // Emit UserAuthenticated event
        let event = DatabaseEvent::new(
            EventType::UserAuthenticated,
            EventPayload::User {
                user_id: user.username.clone(),
                action: "plex_account_added".to_string(),
            },
        );

        if let Err(e) = self.event_bus.publish(event).await {
            tracing::warn!("Failed to publish UserAuthenticated event: {}", e);
        }

        // Discover servers (this will also cache them)
        let sources = self.discover_plex_sources(&provider_id).await?;

        Ok((provider_id, sources))
    }

    /// Add Jellyfin server credentials
    pub async fn add_jellyfin_auth(
        &self,
        server_url: &str,
        username: &str,
        password: &str,
        access_token: &str,
        user_id: &str,
    ) -> Result<(String, Source)> {
        info!("Adding Jellyfin auth for {}", server_url);

        // Generate stable provider ID based on server_url + username
        let provider_id = self.generate_stable_jellyfin_id(server_url, username);

        // Check if this provider already exists
        {
            let providers = self.providers.read().await;
            if providers.contains_key(&provider_id) {
                info!(
                    "Jellyfin provider {} already exists, updating credentials",
                    provider_id
                );
                // Provider exists, just update the credentials
                drop(providers);

                // Update stored credentials in keyring
                self.store_credentials(&provider_id, "password", password)?;
                self.store_credentials(&provider_id, "token", access_token)?;

                let source = Source::new(
                    format!("source_{}", provider_id),
                    format!("Jellyfin - {}", server_url),
                    SourceType::JellyfinServer,
                    Some(provider_id.clone()),
                );

                return Ok((provider_id, source));
            }
        }
        let provider = AuthProvider::JellyfinAuth {
            id: provider_id.clone(),
            server_url: server_url.to_string(),
            username: username.to_string(),
            user_id: user_id.to_string(),
            access_token: access_token.to_string(),
        };

        // Store the provider
        {
            let mut providers = self.providers.write().await;
            providers.insert(provider_id.clone(), provider.clone());
        }

        // Save to config
        {
            let mut config = self.config.write().await;
            config.add_auth_provider(provider_id.clone(), provider.clone())?;
        }

        // Store credentials in keyring
        self.store_credentials(&provider_id, "password", password)?;
        self.store_credentials(&provider_id, "token", access_token)?;

        // Emit UserAuthenticated event
        let event = DatabaseEvent::new(
            EventType::UserAuthenticated,
            EventPayload::User {
                user_id: username.to_string(),
                action: "jellyfin_auth_added".to_string(),
            },
        );

        if let Err(e) = self.event_bus.publish(event).await {
            tracing::warn!("Failed to publish UserAuthenticated event: {}", e);
        }

        // Create the source with stable ID based on provider ID
        let source = Source::new(
            format!("source_{}", provider_id),
            format!("Jellyfin - {}", server_url),
            SourceType::JellyfinServer,
            Some(provider_id.clone()),
        );

        Ok((provider_id, source))
    }

    /// Add network credentials
    pub async fn add_network_credentials(
        &self,
        display_name: &str,
        auth_type: NetworkAuthType,
        credentials: NetworkCredentialData,
    ) -> Result<String> {
        info!("Adding network credentials: {}", display_name);

        let provider_id = format!(
            "network_{}",
            Uuid::new_v4().to_string().split('-').next().unwrap()
        );

        // Store sensitive parts in keyring
        match &credentials {
            NetworkCredentialData::UsernamePassword { password, .. } => {
                self.store_credentials(&provider_id, "password", password)?;
            }
            NetworkCredentialData::SSHKey { passphrase, .. } => {
                if let Some(pass) = passphrase {
                    self.store_credentials(&provider_id, "passphrase", pass)?;
                }
            }
            NetworkCredentialData::Token(token) => {
                self.store_credentials(&provider_id, "token", token)?;
            }
        }

        let provider = AuthProvider::NetworkCredentials {
            id: provider_id.clone(),
            display_name: display_name.to_string(),
            auth_type,
            credentials,
        };

        let mut providers = self.providers.write().await;
        providers.insert(provider_id.clone(), provider);

        Ok(provider_id)
    }

    /// Add local files provider (no auth needed)
    pub async fn add_local_provider(&self) -> Result<String> {
        let provider_id = format!(
            "local_{}",
            Uuid::new_v4().to_string().split('-').next().unwrap()
        );
        let provider = AuthProvider::LocalFiles {
            id: provider_id.clone(),
        };

        let mut providers = self.providers.write().await;
        providers.insert(provider_id.clone(), provider);

        Ok(provider_id)
    }

    /// Discover Plex servers for an account with offline-first approach
    pub async fn discover_plex_sources(&self, provider_id: &str) -> Result<Vec<Source>> {
        // Try to get cached sources first
        let config = self.config.read().await;
        let cached = config.get_cached_sources(provider_id);
        let is_stale = config.is_sources_cache_stale(provider_id, 300); // 5 minute cache
        drop(config);

        // If we have cached sources and they're fresh, return them immediately
        if let Some(ref sources) = cached
            && !is_stale
        {
            info!("Returning cached Plex sources for provider {}", provider_id);
            return Ok(sources.clone());
        }

        // If online, try to fetch fresh data
        let providers = self.providers.read().await;
        let provider = providers
            .get(provider_id)
            .ok_or_else(|| anyhow!("Provider not found"))?;

        if let AuthProvider::PlexAccount { token, .. } = provider {
            info!(
                "Discovering Plex servers for provider {} with token length {}",
                provider_id,
                token.len()
            );

            // Try to fetch from network
            match PlexAuth::discover_servers(token).await {
                Ok(servers) => {
                    let sources: Vec<Source> = servers
                        .into_iter()
                        .map(|server| {
                            let source_id = format!("plex_server_{}", server.client_identifier);
                            let mut source = Source::new(
                                source_id,
                                server.name.clone(),
                                SourceType::PlexServer {
                                    machine_id: server.client_identifier.clone(),
                                    owned: server.owned,
                                },
                                Some(provider_id.to_string()),
                            );

                            // Find the best connection URL (prefer local, then remote, then relay)
                            // Log all available connections for debugging
                            for conn in &server.connections {
                                info!(
                                    "Available connection for {}: {} (local: {}, relay: {})",
                                    server.name, conn.uri, conn.local, conn.relay
                                );
                            }

                            // Sort connections by preference
                            let mut sorted_connections = server.connections.clone();
                            sorted_connections.sort_by_key(|c| {
                                if c.local && !c.relay {
                                    0 // Best: local non-relay
                                } else if c.local {
                                    1 // Good: local (might be relay)
                                } else if !c.relay {
                                    2 // OK: remote direct
                                } else {
                                    3 // Last resort: relay
                                }
                            });

                            // For now, just pick the first one by preference
                            // TODO: In the future, we could test connections here too
                            if let Some(connection) = sorted_connections.first() {
                                source.connection_info.primary_url = Some(connection.uri.clone());
                                info!(
                                    "Selected primary URL for {}: {} (local: {}, relay: {})",
                                    server.name, connection.uri, connection.local, connection.relay
                                );
                            } else {
                                warn!("No connections found for Plex server {}", server.name);
                            }

                            source
                        })
                        .collect();

                    // Cache the fresh sources
                    let mut config = self.config.write().await;
                    let _ = config.set_cached_sources(provider_id.to_string(), sources.clone());
                    drop(config);

                    Ok(sources)
                }
                Err(e) => {
                    // If fetch failed but we have cached data, return it
                    if let Some(sources) = cached {
                        info!(
                            "Failed to fetch fresh Plex sources, returning cached: {}",
                            e
                        );
                        Ok(sources)
                    } else {
                        Err(e)
                    }
                }
            }
        } else {
            Err(anyhow!("Not a Plex provider"))
        }
    }

    /// Get cached sources without network fetch
    pub async fn get_cached_sources(&self, provider_id: &str) -> Option<Vec<Source>> {
        let config = self.config.read().await;
        config.get_cached_sources(provider_id)
    }

    /// Update library count for a specific source
    pub async fn update_source_library_count(
        &self,
        source_id: &str,
        library_count: usize,
    ) -> Result<()> {
        let mut config = self.config.write().await;

        // Find and update the source across all providers
        for (_provider_id, sources) in config.runtime.cached_sources.iter_mut() {
            if let Some(source) = sources.iter_mut().find(|s| s.id == source_id) {
                source.library_count = library_count;
                info!(
                    "Updated library count for source {}: {} libraries",
                    source_id, library_count
                );
                config.save()?;
                return Ok(());
            }
        }

        warn!("Source {} not found in cache", source_id);
        Ok(())
    }

    /// Update the URL for a source (when we find a better working URL)
    pub async fn update_source_url(&self, source_id: &str, new_url: &str) -> Result<()> {
        let mut config = self.config.write().await;

        // Find and update the source across all providers
        for (_provider_id, sources) in config.runtime.cached_sources.iter_mut() {
            if let Some(source) = sources.iter_mut().find(|s| s.id == source_id) {
                source.connection_info.primary_url = Some(new_url.to_string());
                info!("Updated URL for source {}: {}", source_id, new_url);
                config.save()?;
                return Ok(());
            }
        }

        warn!("Source {} not found in cache", source_id);
        Ok(())
    }

    /// Refresh sources in background
    pub async fn refresh_sources_background(&self, provider_id: &str) {
        let provider_id = provider_id.to_string();
        let self_clone = Arc::new(self.clone());

        tokio::spawn(async move {
            info!("Background refresh of sources for provider {}", provider_id);
            if let Err(e) = self_clone.discover_plex_sources(&provider_id).await {
                error!("Failed to refresh sources in background: {}", e);
            }
        });
    }

    /// Get a provider by ID
    pub async fn get_provider(&self, provider_id: &str) -> Option<AuthProvider> {
        let providers = self.providers.read().await;
        providers.get(provider_id).cloned()
    }

    /// Get all providers
    pub async fn get_all_providers(&self) -> Vec<AuthProvider> {
        let providers = self.providers.read().await;
        providers.values().cloned().collect()
    }

    /// Remove a provider and all associated sources
    pub async fn remove_provider(&self, provider_id: &str) -> Result<()> {
        // Get provider info before removing for event
        let provider = {
            let providers = self.providers.read().await;
            providers.get(provider_id).cloned()
        };

        let mut providers = self.providers.write().await;

        // Clean up keyring entries
        self.delete_credentials(provider_id)?;

        // Remove from in-memory providers
        providers.remove(provider_id);
        drop(providers);

        // Remove from config and persist to disk
        let mut config = self.config.write().await;
        config.remove_auth_provider(provider_id)?;
        drop(config);

        // Emit UserLoggedOut event if provider existed
        if let Some(auth_provider) = provider {
            let user_id = match auth_provider {
                AuthProvider::PlexAccount { username, .. } => username,
                AuthProvider::JellyfinAuth { username, .. } => username,
                AuthProvider::NetworkCredentials { display_name, .. } => display_name,
                AuthProvider::LocalFiles { id } => id,
            };

            let event = DatabaseEvent::new(
                EventType::UserLoggedOut,
                EventPayload::User {
                    user_id,
                    action: "provider_removed".to_string(),
                },
            );

            if let Err(e) = self.event_bus.publish(event).await {
                tracing::warn!("Failed to publish UserLoggedOut event: {}", e);
            }
        }

        Ok(())
    }

    /// Refresh token if needed (for providers that support it)
    pub async fn refresh_token(&self, provider_id: &str) -> Result<()> {
        // TODO: Implement token refresh for providers that support it
        // For now, Plex tokens don't expire, but this is where we'd handle it
        Ok(())
    }

    /// Store credentials in keyring
    pub fn store_credentials(&self, provider_id: &str, field: &str, value: &str) -> Result<()> {
        let key = format!("{}_{}", provider_id, field);
        match keyring::Entry::new("dev.arsfeld.Reel", &key) {
            Ok(entry) => {
                entry.set_password(value)?;
                Ok(())
            }
            Err(e) => {
                error!("Failed to create keyring entry: {}", e);
                Err(anyhow!("Failed to store credentials"))
            }
        }
    }

    /// Retrieve credentials from keyring
    pub fn get_credentials(&self, provider_id: &str, field: &str) -> Result<String> {
        let key = format!("{}_{}", provider_id, field);
        match keyring::Entry::new("dev.arsfeld.Reel", &key) {
            Ok(entry) => Ok(entry.get_password()?),
            Err(e) => {
                error!("Failed to get keyring entry: {}", e);
                Err(anyhow!("Failed to retrieve credentials"))
            }
        }
    }

    /// Delete all credentials for a provider
    fn delete_credentials(&self, provider_id: &str) -> Result<()> {
        // Try to delete common credential fields
        for field in &["token", "password", "passphrase", "refresh_token"] {
            let key = format!("{}_{}", provider_id, field);
            if let Ok(entry) = keyring::Entry::new("dev.arsfeld.Reel", &key) {
                let _ = entry.delete_credential(); // Ignore errors for non-existent entries
            }
        }
        Ok(())
    }

    /// Migrate legacy backends to AuthProvider model
    pub async fn migrate_legacy_backends(&self) -> Result<()> {
        info!("Migrating legacy backends to AuthProvider model");

        // First, clean up any non-legacy backends that were incorrectly added
        self.cleanup_non_legacy_backends().await?;

        let config = self.config.read().await;
        let legacy_backends = config.get_legacy_backends();
        drop(config);
        if legacy_backends.is_empty() {
            info!("No legacy backends to migrate");
            return Ok(());
        }

        let mut migrated_count = 0;

        for backend_id in legacy_backends {
            info!("Attempting to migrate backend: {}", backend_id);

            // Check if this backend has already been migrated
            let providers = self.providers.read().await;
            if providers.contains_key(&backend_id) {
                info!("Backend {} already migrated, skipping", backend_id);
                continue;
            }
            drop(providers);

            // Try to migrate based on backend type
            if backend_id.starts_with("plex") {
                // Try to get the token from keyring - Plex backend stores it directly with backend_id
                info!("Looking for Plex token for backend: {}", backend_id);

                // Try the format used by PlexBackend (service: "dev.arsfeld.Reel", account: backend_id)
                let token = if let Ok(entry) = keyring::Entry::new("dev.arsfeld.Reel", &backend_id)
                {
                    entry.get_password().ok()
                } else {
                    // Also try the new format just in case
                    keyring::Entry::new("reel", &format!("{}_token", backend_id))
                        .ok()
                        .and_then(|e| e.get_password().ok())
                };

                if let Some(token) = token {
                    info!("Found Plex token for {}, migrating...", backend_id);

                    // Try to get user info from token
                    match PlexAuth::get_user(&token).await {
                        Ok(user) => {
                            // Create AuthProvider for this Plex account
                            let provider = AuthProvider::PlexAccount {
                                id: backend_id.clone(),
                                username: user.username.clone(),
                                email: user.email.clone(),
                                token: token.clone(),
                                refresh_token: None,
                                token_expiry: None,
                            };

                            // Store the provider
                            let mut providers = self.providers.write().await;
                            providers.insert(backend_id.clone(), provider.clone());
                            drop(providers);

                            // Save to config
                            let mut config = self.config.write().await;
                            config.add_auth_provider(backend_id.clone(), provider)?;
                            drop(config);

                            // Also store token in the new keyring location
                            if let Err(e) = self.store_credentials(&backend_id, "token", &token) {
                                error!("Failed to store migrated token in keyring: {}", e);
                            }

                            info!(
                                "Successfully migrated Plex backend: {} (user: {})",
                                backend_id, user.username
                            );
                            migrated_count += 1;

                            // Remove from legacy_backends to prevent re-migration
                            let mut config = self.config.write().await;
                            if let Err(e) = config.remove_legacy_backend(&backend_id) {
                                error!("Failed to remove migrated backend from config: {}", e);
                            }
                            drop(config);
                        }
                        Err(e) => {
                            error!("Failed to get Plex user info for {}: {}", backend_id, e);
                            // Still create a minimal provider so the backend shows up
                            let provider = AuthProvider::PlexAccount {
                                id: backend_id.clone(),
                                username: backend_id.clone(),
                                email: format!("{}@migrated", backend_id),
                                token: token.clone(),
                                refresh_token: None,
                                token_expiry: None,
                            };

                            let mut providers = self.providers.write().await;
                            providers.insert(backend_id.clone(), provider.clone());
                            drop(providers);

                            // Save to config
                            let mut config = self.config.write().await;
                            config.add_auth_provider(backend_id.clone(), provider)?;
                            drop(config);

                            // Also store token in the new keyring location
                            if let Err(e) = self.store_credentials(&backend_id, "token", &token) {
                                error!("Failed to store migrated token in keyring: {}", e);
                            }

                            info!("Created minimal provider for {}", backend_id);
                            migrated_count += 1;

                            // Remove from legacy_backends to prevent re-migration
                            let mut config = self.config.write().await;
                            if let Err(e) = config.remove_legacy_backend(&backend_id) {
                                error!("Failed to remove migrated backend from config: {}", e);
                            }
                            drop(config);
                        }
                    }
                } else {
                    info!("No token found for Plex backend {}, skipping", backend_id);
                }
            } else if backend_id.starts_with("jellyfin") {
                // Try to migrate Jellyfin backend
                let url_key = format!("{}_url", backend_id);
                let username_key = format!("{}_username", backend_id);
                let token_key = format!("{}_access_token", backend_id);
                let user_id_key = format!("{}_user_id", backend_id);

                let url = keyring::Entry::new("reel", &url_key)
                    .ok()
                    .and_then(|e| e.get_password().ok());
                let username = keyring::Entry::new("reel", &username_key)
                    .ok()
                    .and_then(|e| e.get_password().ok());
                let token = keyring::Entry::new("reel", &token_key)
                    .ok()
                    .and_then(|e| e.get_password().ok())
                    .or_else(|| {
                        // Try old token key
                        keyring::Entry::new("reel", &format!("{}_token", backend_id))
                            .ok()
                            .and_then(|e| e.get_password().ok())
                    });
                let user_id = keyring::Entry::new("reel", &user_id_key)
                    .ok()
                    .and_then(|e| e.get_password().ok());

                if let (Some(url), Some(username), Some(token)) = (url, username, token) {
                    info!(
                        "Found Jellyfin credentials for {}, migrating...",
                        backend_id
                    );

                    let provider = AuthProvider::JellyfinAuth {
                        id: backend_id.clone(),
                        server_url: url.clone(),
                        username: username.clone(),
                        user_id: user_id.unwrap_or_else(|| username.clone()),
                        access_token: token.clone(),
                    };

                    let mut providers = self.providers.write().await;
                    providers.insert(backend_id.clone(), provider.clone());
                    drop(providers);

                    // Save to config
                    let mut config = self.config.write().await;
                    config.add_auth_provider(backend_id.clone(), provider)?;
                    drop(config);

                    info!(
                        "Successfully migrated Jellyfin backend: {} (user: {}@{})",
                        backend_id, username, url
                    );
                    migrated_count += 1;

                    // Remove from legacy_backends to prevent re-migration
                    let mut config = self.config.write().await;
                    if let Err(e) = config.remove_legacy_backend(&backend_id) {
                        error!("Failed to remove migrated backend from config: {}", e);
                    }
                    drop(config);
                } else {
                    info!(
                        "Incomplete credentials for Jellyfin backend {}, skipping",
                        backend_id
                    );
                }
            }
        }

        info!("Migration complete: {} backends migrated", migrated_count);
        Ok(())
    }

    /// Remove backends from legacy_backends that are already in auth_providers
    /// This cleans up backends that were incorrectly added as "legacy" backends
    async fn cleanup_non_legacy_backends(&self) -> Result<()> {
        let providers = self.providers.read().await;
        let provider_ids: Vec<String> = providers.keys().cloned().collect();
        drop(providers);

        let mut config = self.config.write().await;
        let original_count = config.runtime.legacy_backends.len();

        // Remove any backend that already exists as an AuthProvider
        config.runtime.legacy_backends.retain(|backend_id| {
            let should_keep = !provider_ids.contains(backend_id);
            if !should_keep {
                info!(
                    "Removing non-legacy backend {} from legacy_backends",
                    backend_id
                );
            }
            should_keep
        });

        let removed_count = original_count - config.runtime.legacy_backends.len();
        if removed_count > 0 {
            info!(
                "Cleaned up {} non-legacy backends from legacy_backends",
                removed_count
            );
            config.save()?;
        }

        Ok(())
    }

    /// Generate a stable, deterministic ID for Jellyfin providers
    /// Based on server_url and username to ensure the same combination always gets the same ID
    fn generate_stable_jellyfin_id(&self, server_url: &str, username: &str) -> String {
        let input = format!("{}:{}", server_url.trim_end_matches('/'), username);
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        let result = hasher.finalize();
        // Convert first 8 bytes to hex string manually
        let hash = result[..8]
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();
        format!("jellyfin_{}", hash)
    }
}

impl Default for AuthManager {
    fn default() -> Self {
        // Create a default config wrapped in Arc<RwLock>
        let config = Arc::new(RwLock::new(Config::default()));
        let event_bus = Arc::new(EventBus::new(1000));
        Self::new(config, event_bus)
    }
}
