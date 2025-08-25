mod api;

pub use api::JellyfinApi;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dirs;
use reqwest::Client;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{error, info};

use super::traits::{
    BackendInfo, BackendType, ConnectionType, MediaBackend, SearchResults, WatchStatus,
};
use crate::models::{
    AuthProvider, Credentials, Episode, HomeSection, Library, MediaItem, Movie, Show, Source,
    SourceType, StreamInfo, User,
};
use crate::services::{auth_manager::AuthManager, cache::CacheManager};

pub struct JellyfinBackend {
    client: Client,
    base_url: Arc<RwLock<Option<String>>>,
    api_key: Arc<RwLock<Option<String>>>,
    user_id: Arc<RwLock<Option<String>>>,
    backend_id: String,
    last_sync_time: Arc<RwLock<Option<DateTime<Utc>>>>,
    api: Arc<RwLock<Option<JellyfinApi>>>,
    server_name: Arc<RwLock<Option<String>>>,
    cache: Option<Arc<CacheManager>>,
    auth_provider: Option<AuthProvider>,
    source: Option<Source>,
    auth_manager: Option<Arc<AuthManager>>,
}

impl fmt::Debug for JellyfinBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JellyfinBackend")
            .field("backend_id", &self.backend_id)
            .field("server_name", &self.server_name)
            .finish()
    }
}

impl JellyfinBackend {
    pub fn new() -> Self {
        Self::with_id("jellyfin".to_string())
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
            api_key: Arc::new(RwLock::new(None)),
            user_id: Arc::new(RwLock::new(None)),
            backend_id: id,
            last_sync_time: Arc::new(RwLock::new(None)),
            api: Arc::new(RwLock::new(None)),
            server_name: Arc::new(RwLock::new(None)),
            cache,
            auth_provider: None,
            source: None,
            auth_manager: None,
        }
    }

    /// Create a new JellyfinBackend from an AuthProvider and Source
    pub fn from_auth(
        auth_provider: AuthProvider,
        source: Source,
        auth_manager: Arc<AuthManager>,
        cache: Option<Arc<CacheManager>>,
    ) -> Result<Self> {
        // Validate that this is a Jellyfin auth provider
        if !matches!(auth_provider, AuthProvider::JellyfinAuth { .. }) {
            return Err(anyhow!("Invalid auth provider type for Jellyfin backend"));
        }

        // Validate that this is a Jellyfin source
        if !matches!(source.source_type, SourceType::JellyfinServer) {
            return Err(anyhow!("Invalid source type for Jellyfin backend"));
        }

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        // Extract server URL from auth provider
        let base_url = if let AuthProvider::JellyfinAuth { server_url, .. } = &auth_provider {
            Some(server_url.clone())
        } else {
            source.connection_info.primary_url.clone()
        };

        Ok(Self {
            client,
            base_url: Arc::new(RwLock::new(base_url)),
            api_key: Arc::new(RwLock::new(None)), // Will be loaded from AuthProvider
            user_id: Arc::new(RwLock::new(None)), // Will be loaded from AuthProvider
            backend_id: source.id.clone(),
            last_sync_time: Arc::new(RwLock::new(None)),
            api: Arc::new(RwLock::new(None)),
            server_name: Arc::new(RwLock::new(Some(source.name.clone()))),
            cache,
            auth_provider: Some(auth_provider),
            source: Some(source),
            auth_manager: Some(auth_manager),
        })
    }

    pub fn set_cache(&mut self, cache: Arc<CacheManager>) {
        self.cache = Some(cache);
    }

    pub async fn set_base_url(&self, url: &str) {
        *self.base_url.write().await = Some(url.to_string());
    }

    pub async fn get_api_client(&self) -> Option<JellyfinApi> {
        self.api.read().await.clone()
    }

    async fn ensure_api_initialized(&self) -> Result<JellyfinApi> {
        // Check if already initialized
        if let Some(api) = self.api.read().await.clone() {
            return Ok(api);
        }

        // Try to initialize
        self.initialize().await?;

        // Get the API after initialization
        self.api
            .read()
            .await
            .clone()
            .ok_or_else(|| anyhow!("Failed to initialize Jellyfin API"))
    }

    pub async fn save_credentials(
        &self,
        base_url: &str,
        api_key: &str,
        user_id: &str,
    ) -> Result<()> {
        let service = "dev.arsfeld.Reel";
        let account = &format!("{}_jellyfin", self.backend_id);

        info!("Saving Jellyfin credentials for account: {}", account);

        let credentials = format!("{}|{}|{}", base_url, api_key, user_id);

        match keyring::Entry::new(service, account) {
            Ok(entry) => match entry.set_password(&credentials) {
                Ok(_) => {
                    info!("Credentials saved to keyring for {}", self.backend_id);
                    return Ok(());
                }
                Err(e) => {
                    error!("Failed to save to keyring: {}, using file fallback", e);
                }
            },
            Err(e) => {
                error!("Failed to create keyring entry: {}, using file fallback", e);
            }
        }

        let config_dir =
            dirs::config_dir().ok_or_else(|| anyhow!("Could not determine config directory"))?;
        let cred_file = config_dir
            .join("reel")
            .join(format!(".{}_jellyfin.cred", self.backend_id));

        if let Some(parent) = cred_file.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let obfuscated = credentials
            .bytes()
            .enumerate()
            .map(|(i, b)| b ^ ((i as u8) + 42))
            .collect::<Vec<u8>>();

        std::fs::write(&cred_file, obfuscated)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&cred_file)?.permissions();
            perms.set_mode(0o600);
            std::fs::set_permissions(&cred_file, perms)?;
        }

        info!("Credentials saved to file for {}", self.backend_id);
        Ok(())
    }

    pub async fn load_credentials(&self) -> Result<Option<(String, String, String)>> {
        let service = "dev.arsfeld.Reel";
        let account = &format!("{}_jellyfin", self.backend_id);

        if let Ok(entry) = keyring::Entry::new(service, account) {
            if let Ok(creds) = entry.get_password() {
                let parts: Vec<&str> = creds.split('|').collect();
                if parts.len() == 3 {
                    info!("Loaded credentials from keyring for {}", self.backend_id);
                    return Ok(Some((
                        parts[0].to_string(),
                        parts[1].to_string(),
                        parts[2].to_string(),
                    )));
                }
            }
        }

        let config_dir =
            dirs::config_dir().ok_or_else(|| anyhow!("Could not determine config directory"))?;
        let cred_file = config_dir
            .join("reel")
            .join(format!(".{}_jellyfin.cred", self.backend_id));

        if !cred_file.exists() {
            return Ok(None);
        }

        let obfuscated = std::fs::read(&cred_file)?;
        let credentials = obfuscated
            .into_iter()
            .enumerate()
            .map(|(i, b)| b ^ ((i as u8) + 42))
            .collect::<Vec<u8>>();

        let creds = String::from_utf8(credentials)?;
        let parts: Vec<&str> = creds.split('|').collect();

        if parts.len() == 3 {
            info!("Loaded credentials from file for {}", self.backend_id);
            Ok(Some((
                parts[0].to_string(),
                parts[1].to_string(),
                parts[2].to_string(),
            )))
        } else {
            Ok(None)
        }
    }

    pub async fn authenticate_with_credentials(
        &self,
        base_url: &str,
        username: &str,
        password: &str,
    ) -> Result<()> {
        let auth_response = JellyfinApi::authenticate(base_url, username, password).await?;

        *self.base_url.write().await = Some(base_url.to_string());
        *self.api_key.write().await = Some(auth_response.access_token.clone());
        *self.user_id.write().await = Some(auth_response.user.id.clone());

        let api = JellyfinApi::with_cache(
            base_url.to_string(),
            auth_response.access_token.clone(),
            auth_response.user.id.clone(),
            self.cache.clone(),
            self.backend_id.clone(),
        );

        if let Ok(server_info) = api.get_server_info().await {
            *self.server_name.write().await = Some(server_info.server_name.clone());
            info!("Connected to Jellyfin server: {}", server_info.server_name);
        }

        *self.api.write().await = Some(api);

        self.save_credentials(
            base_url,
            &auth_response.access_token,
            &auth_response.user.id,
        )
        .await?;

        Ok(())
    }

    pub async fn get_credentials(&self) -> Option<(String, String)> {
        let api_key = self.api_key.read().await.clone()?;
        let user_id = self.user_id.read().await.clone()?;
        Some((api_key, user_id))
    }
}

#[async_trait]
impl MediaBackend for JellyfinBackend {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn initialize(&self) -> Result<Option<User>> {
        // If we have an AuthProvider, use its credentials
        let (base_url, api_key, user_id) = if let Some(auth_provider) = &self.auth_provider {
            match auth_provider {
                AuthProvider::JellyfinAuth {
                    server_url,
                    access_token,
                    user_id,
                    username,
                    ..
                } => {
                    tracing::info!(
                        "JellyfinBackend::initialize - provider_id: {}, has_token: {}, has_auth_manager: {}",
                        auth_provider.id(),
                        !access_token.is_empty(),
                        self.auth_manager.is_some()
                    );

                    if !access_token.is_empty() {
                        (server_url.clone(), access_token.clone(), user_id.clone())
                    } else if let Some(auth_manager) = &self.auth_manager {
                        // Try to get token from keyring via AuthManager
                        match auth_manager.get_credentials(auth_provider.id(), "token") {
                            Ok(token) => {
                                tracing::info!(
                                    "Successfully retrieved token from keyring for {}",
                                    auth_provider.id()
                                );
                                (server_url.clone(), token, user_id.clone())
                            }
                            Err(e) => {
                                tracing::warn!("Failed to get token from keyring: {}", e);
                                // Try to get password and re-authenticate
                                match auth_manager.get_credentials(auth_provider.id(), "password") {
                                    Ok(password) => {
                                        tracing::info!(
                                            "Got password from keyring, re-authenticating..."
                                        );
                                        // Re-authenticate with username/password
                                        match JellyfinApi::authenticate(
                                            server_url, username, &password,
                                        )
                                        .await
                                        {
                                            Ok(auth_response) => {
                                                // Save the new token
                                                tracing::info!(
                                                    "Re-authentication successful, saving new token"
                                                );
                                                auth_manager
                                                    .store_credentials(
                                                        auth_provider.id(),
                                                        "token",
                                                        &auth_response.access_token,
                                                    )
                                                    .ok();
                                                (
                                                    server_url.clone(),
                                                    auth_response.access_token,
                                                    auth_response.user.id,
                                                )
                                            }
                                            Err(e) => {
                                                error!(
                                                    "Failed to re-authenticate with Jellyfin: {}",
                                                    e
                                                );
                                                return Ok(None);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            "No credentials found in AuthProvider or keyring: {}",
                                            e
                                        );
                                        return Ok(None);
                                    }
                                }
                            }
                        }
                    } else {
                        tracing::warn!("No token in AuthProvider and no AuthManager available");
                        return Ok(None);
                    }
                }
                _ => {
                    tracing::error!("Invalid AuthProvider type for Jellyfin backend");
                    return Ok(None);
                }
            }
        } else {
            // No AuthProvider available - can't initialize
            tracing::error!("No AuthProvider available for Jellyfin backend");
            return Ok(None);
        };

        // Store the credentials
        *self.base_url.write().await = Some(base_url.clone());
        *self.api_key.write().await = Some(api_key.clone());
        *self.user_id.write().await = Some(user_id.clone());

        let api = JellyfinApi::with_cache(
            base_url,
            api_key,
            user_id,
            self.cache.clone(),
            self.backend_id.clone(),
        );

        match api.get_user().await {
            Ok(user) => {
                if let Ok(server_info) = api.get_server_info().await {
                    *self.server_name.write().await = Some(server_info.server_name.clone());
                    info!("Connected to Jellyfin server: {}", server_info.server_name);
                }
                *self.api.write().await = Some(api);
                Ok(Some(user))
            }
            Err(e) => {
                error!("Failed to connect with saved credentials: {}", e);
                Ok(None)
            }
        }
    }

    async fn is_initialized(&self) -> bool {
        self.api.read().await.is_some()
    }

    async fn authenticate(&self, credentials: Credentials) -> Result<User> {
        match credentials {
            Credentials::UsernamePassword { username, password } => {
                let base_url = self
                    .base_url
                    .read()
                    .await
                    .clone()
                    .ok_or_else(|| anyhow!("Base URL not set"))?;
                self.authenticate_with_credentials(&base_url, &username, &password)
                    .await?;
            }
            Credentials::ApiKey { key } => {
                return Err(anyhow!("API key authentication not supported for Jellyfin"));
            }
            Credentials::Token { token } => {
                let parts: Vec<&str> = token.split('|').collect();
                if parts.len() == 3 {
                    let base_url = parts[0];
                    let api_key = parts[1];
                    let user_id = parts[2];

                    *self.base_url.write().await = Some(base_url.to_string());
                    *self.api_key.write().await = Some(api_key.to_string());
                    *self.user_id.write().await = Some(user_id.to_string());

                    let api = JellyfinApi::with_cache(
                        base_url.to_string(),
                        api_key.to_string(),
                        user_id.to_string(),
                        self.cache.clone(),
                        self.backend_id.clone(),
                    );

                    *self.api.write().await = Some(api.clone());

                    return api.get_user().await;
                }
                return Err(anyhow!("Invalid token format"));
            }
        }

        let api = self.ensure_api_initialized().await?;
        api.get_user().await
    }

    async fn get_libraries(&self) -> Result<Vec<Library>> {
        let api = self.ensure_api_initialized().await?;
        api.get_libraries().await
    }

    async fn get_movies(&self, library_id: &str) -> Result<Vec<Movie>> {
        let api = self.ensure_api_initialized().await?;
        api.get_movies(library_id).await
    }

    async fn get_shows(&self, library_id: &str) -> Result<Vec<Show>> {
        let api = self.ensure_api_initialized().await?;
        api.get_shows(library_id).await
    }

    async fn get_episodes(&self, show_id: &str, season: u32) -> Result<Vec<Episode>> {
        let api = self.ensure_api_initialized().await?;

        let shows = api.get_shows(show_id).await?;
        if let Some(show) = shows.first()
            && let Some(season_info) = show.seasons.iter().find(|s| s.season_number == season)
        {
            return api.get_episodes(&season_info.id).await;
        }

        Err(anyhow!("Season not found"))
    }

    async fn get_stream_url(&self, media_id: &str) -> Result<StreamInfo> {
        let api = self.ensure_api_initialized().await?;

        api.report_playback_start(media_id).await.ok();

        api.get_stream_url(media_id).await
    }

    async fn update_progress(
        &self,
        media_id: &str,
        position: Duration,
        duration: Duration,
    ) -> Result<()> {
        let api = self.ensure_api_initialized().await?;

        if position >= duration * 9 / 10 {
            api.report_playback_stopped(media_id, position).await?;
        } else {
            api.update_playback_progress(media_id, position).await?;
        }

        Ok(())
    }

    async fn mark_watched(&self, media_id: &str) -> Result<()> {
        let api = self.ensure_api_initialized().await?;
        api.mark_as_watched(media_id).await
    }

    async fn mark_unwatched(&self, media_id: &str) -> Result<()> {
        let api = self.ensure_api_initialized().await?;
        api.mark_as_unwatched(media_id).await
    }

    async fn get_watch_status(&self, media_id: &str) -> Result<WatchStatus> {
        let api = self.ensure_api_initialized().await?;

        api.get_watch_status(media_id).await
    }

    async fn search(&self, query: &str) -> Result<SearchResults> {
        let api = self.ensure_api_initialized().await?;

        let items = api.search(query).await?;

        let mut movies = Vec::new();
        let mut shows = Vec::new();
        let mut episodes = Vec::new();

        for item in items {
            match item {
                MediaItem::Movie(movie) => movies.push(movie),
                MediaItem::Show(show) => shows.push(show),
                MediaItem::Episode(episode) => episodes.push(episode),
                _ => {}
            }
        }

        Ok(SearchResults {
            movies,
            shows,
            episodes,
        })
    }

    async fn find_next_episode(&self, current_episode: &Episode) -> Result<Option<Episode>> {
        let api = self.ensure_api_initialized().await?;

        api.find_next_episode(current_episode).await
    }

    async fn get_backend_id(&self) -> String {
        self.backend_id.clone()
    }

    async fn get_last_sync_time(&self) -> Option<DateTime<Utc>> {
        *self.last_sync_time.read().await
    }

    async fn supports_offline(&self) -> bool {
        true
    }

    async fn get_home_sections(&self) -> Result<Vec<HomeSection>> {
        let api = self.ensure_api_initialized().await?;
        api.get_home_sections().await
    }

    async fn get_backend_info(&self) -> BackendInfo {
        let server_name = self.server_name.read().await.clone();
        BackendInfo {
            name: self.backend_id.clone(),
            display_name: server_name
                .clone()
                .unwrap_or_else(|| "Jellyfin".to_string()),
            backend_type: BackendType::Jellyfin,
            server_name,
            server_version: None,
            connection_type: ConnectionType::Remote,
            is_local: false,
            is_relay: false,
        }
    }
}
