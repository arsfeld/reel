pub mod api;
#[cfg(test)]
mod tests;

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

use super::traits::MediaBackend;
use crate::models::{
    AuthProvider, BackendId, Credentials, Episode, HomeSection, Library, LibraryId, MediaItemId,
    Movie, Season, Show, ShowId, Source, SourceType, StreamInfo, User,
};

#[allow(dead_code)] // Used via dynamic dispatch in BackendService
pub struct JellyfinBackend {
    base_url: Arc<RwLock<Option<String>>>,
    api_key: Arc<RwLock<Option<String>>>,
    user_id: Arc<RwLock<Option<String>>>,
    backend_id: String,
    last_sync_time: Arc<RwLock<Option<DateTime<Utc>>>>,
    api: Arc<RwLock<Option<JellyfinApi>>>,
    server_name: Arc<RwLock<Option<String>>>,
    auth_provider: Option<AuthProvider>,
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
        let _client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            base_url: Arc::new(RwLock::new(None)),
            api_key: Arc::new(RwLock::new(None)),
            user_id: Arc::new(RwLock::new(None)),
            backend_id: id,
            last_sync_time: Arc::new(RwLock::new(None)),
            api: Arc::new(RwLock::new(None)),
            server_name: Arc::new(RwLock::new(None)),
            auth_provider: None,
        }
    }

    /// Create a new JellyfinBackend from an AuthProvider and Source
    pub fn from_auth(auth_provider: AuthProvider, source: Source) -> Result<Self> {
        // Validate that this is a Jellyfin auth provider
        if !matches!(auth_provider, AuthProvider::JellyfinAuth { .. }) {
            return Err(anyhow!("Invalid auth provider type for Jellyfin backend"));
        }

        // Validate that this is a Jellyfin source
        if !matches!(source.source_type, SourceType::JellyfinServer) {
            return Err(anyhow!("Invalid source type for Jellyfin backend"));
        }

        let _client = Client::builder()
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
            base_url: Arc::new(RwLock::new(base_url)),
            api_key: Arc::new(RwLock::new(None)), // Will be loaded from AuthProvider
            user_id: Arc::new(RwLock::new(None)), // Will be loaded from AuthProvider
            backend_id: source.id.clone(),
            last_sync_time: Arc::new(RwLock::new(None)),
            api: Arc::new(RwLock::new(None)),
            server_name: Arc::new(RwLock::new(Some(source.name.clone()))),
            auth_provider: Some(auth_provider),
        })
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

    pub async fn set_base_url(&self, base_url: String) {
        *self.base_url.write().await = Some(base_url);
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

        let api = JellyfinApi::with_backend_id(
            base_url.to_string(),
            auth_response.access_token.clone(),
            auth_response.user.id.clone(),
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

    /// Extract the actual Jellyfin item ID from a composite media ID
    /// Format: "backend_id:library_id:type:item_id" or variations
    fn extract_jellyfin_item_id(&self, media_id: &MediaItemId) -> String {
        let media_id_str = media_id.as_str();
        if media_id_str.contains(':') {
            // Split and get the last part which should be the item ID
            media_id_str
                .split(':')
                .next_back()
                .unwrap_or(media_id_str)
                .to_string()
        } else {
            // If no separator, assume it's already just the item ID
            media_id_str.to_string()
        }
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
                    ..
                } => {
                    tracing::info!(
                        "JellyfinBackend::initialize - provider_id: {}, has_token: {}, user_id: '{}'",
                        auth_provider.id(),
                        !access_token.is_empty(),
                        user_id
                    );

                    if !access_token.is_empty() {
                        (server_url.clone(), access_token.clone(), user_id.clone())
                    } else {
                        // Token should be provided in AuthProvider
                        tracing::error!("No access token found in AuthProvider");
                        return Err(anyhow::anyhow!("No credentials available"));
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

        let api = JellyfinApi::with_backend_id(base_url, api_key, user_id, self.backend_id.clone());

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
            Credentials::ApiKey { key: _ } => {
                return Err(anyhow!("API key authentication not supported for Jellyfin"));
            }
            Credentials::Token { token } => {
                // Check format of the token
                let parts: Vec<&str> = token.split('|').collect();

                if parts.len() == 3 {
                    // Old format with base_url|api_key|user_id
                    let base_url = parts[0];
                    let api_key = parts[1];
                    let user_id = parts[2];

                    *self.base_url.write().await = Some(base_url.to_string());
                    *self.api_key.write().await = Some(api_key.to_string());
                    *self.user_id.write().await = Some(user_id.to_string());

                    let api = JellyfinApi::with_backend_id(
                        base_url.to_string(),
                        api_key.to_string(),
                        user_id.to_string(),
                        self.backend_id.clone(),
                    );

                    *self.api.write().await = Some(api.clone());

                    return api.get_user().await;
                } else if parts.len() == 2 {
                    // New format from Quick Connect: token|user_id
                    let access_token = parts[0];
                    let user_id = parts[1];

                    // We need the base_url to be already set
                    let base_url = self
                        .base_url
                        .read()
                        .await
                        .clone()
                        .ok_or_else(|| anyhow!("Base URL not set for token authentication"))?;

                    *self.api_key.write().await = Some(access_token.to_string());
                    *self.user_id.write().await = Some(user_id.to_string());

                    let api = JellyfinApi::with_backend_id(
                        base_url.clone(),
                        access_token.to_string(),
                        user_id.to_string(),
                        self.backend_id.clone(),
                    );

                    *self.api.write().await = Some(api.clone());

                    return api.get_user().await;
                } else {
                    // Just an access token - need to fetch user info
                    // We need the base_url to be already set
                    let base_url = self
                        .base_url
                        .read()
                        .await
                        .clone()
                        .ok_or_else(|| anyhow!("Base URL not set for token authentication"))?;

                    // Get user info first to extract user_id
                    let temp_api = JellyfinApi::with_backend_id(
                        base_url.clone(),
                        token.clone(),
                        String::new(), // We don't have user_id yet
                        self.backend_id.clone(),
                    );

                    // Get user info to extract user_id
                    match temp_api.get_user().await {
                        Ok(user) => {
                            *self.api_key.write().await = Some(token.clone());
                            *self.user_id.write().await = Some(user.id.clone());

                            let api = JellyfinApi::with_backend_id(
                                base_url,
                                token,
                                user.id.clone(),
                                self.backend_id.clone(),
                            );

                            *self.api.write().await = Some(api);

                            return Ok(user);
                        }
                        Err(e) => {
                            return Err(anyhow!("Failed to authenticate with token: {}", e));
                        }
                    }
                }
            }
        }

        let api = self.ensure_api_initialized().await?;
        api.get_user().await
    }

    async fn get_libraries(&self) -> Result<Vec<Library>> {
        let api = self.ensure_api_initialized().await?;
        api.get_libraries().await
    }

    async fn get_movies(&self, library_id: &LibraryId) -> Result<Vec<Movie>> {
        let api = self.ensure_api_initialized().await?;
        api.get_movies(library_id.as_ref()).await
    }

    async fn get_shows(&self, library_id: &LibraryId) -> Result<Vec<Show>> {
        let api = self.ensure_api_initialized().await?;
        api.get_shows(library_id.as_ref()).await
    }

    async fn get_seasons(&self, show_id: &ShowId) -> Result<Vec<Season>> {
        let api = self.ensure_api_initialized().await?;
        api.get_seasons(show_id.as_ref()).await
    }

    async fn get_episodes(&self, show_id: &ShowId, season: u32) -> Result<Vec<Episode>> {
        let api = self.ensure_api_initialized().await?;

        let seasons = api.get_seasons(show_id.as_ref()).await?;
        if let Some(season_info) = seasons.iter().find(|s| s.season_number == season) {
            let mut episodes = api.get_episodes(&season_info.id).await?;

            // Ensure show_id and season_number are set correctly for all episodes
            for episode in &mut episodes {
                if episode.show_id.is_none() {
                    episode.show_id = Some(show_id.to_string());
                }
                // Fix: Ensure season_number is set correctly
                episode.season_number = season;
            }

            return Ok(episodes);
        }

        Err(anyhow!("Failed to get seasons"))
    }

    async fn get_stream_url(&self, media_id: &MediaItemId) -> Result<StreamInfo> {
        let api = self.ensure_api_initialized().await?;
        let jellyfin_item_id = self.extract_jellyfin_item_id(media_id);

        tracing::info!(
            "Extracted Jellyfin item ID: {} from media_id: {}",
            jellyfin_item_id,
            media_id
        );

        let stream_info = api.get_stream_url(&jellyfin_item_id).await?;

        // Report playback start after successfully getting stream info
        api.report_playback_start(&jellyfin_item_id).await.ok();

        Ok(stream_info)
    }

    async fn update_progress(
        &self,
        media_id: &MediaItemId,
        position: Duration,
        duration: Duration,
    ) -> Result<()> {
        let api = self.ensure_api_initialized().await?;
        let jellyfin_item_id = self.extract_jellyfin_item_id(media_id);

        if position >= duration * 9 / 10 {
            api.report_playback_stopped(&jellyfin_item_id, position)
                .await?;
        } else {
            api.update_playback_progress(&jellyfin_item_id, position)
                .await?;
        }

        Ok(())
    }

    async fn get_backend_id(&self) -> BackendId {
        BackendId::new(&self.backend_id)
    }

    async fn get_home_sections(&self) -> Result<Vec<HomeSection>> {
        let api = self.ensure_api_initialized().await?;
        api.get_home_sections().await
    }
}
