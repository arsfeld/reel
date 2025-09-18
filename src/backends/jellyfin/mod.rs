pub mod api;

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
    Movie, Season, Show, ShowId, Source, SourceId, SourceType, StreamInfo, User,
};
use crate::services::core::auth::AuthService;

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
                    username,
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
                        // Try to get credentials from keyring via AuthService
                        match AuthService::load_credentials(&SourceId::new(auth_provider.id()))
                            .await
                        {
                            Ok(Some(Credentials::Token { token, .. })) => {
                                tracing::info!(
                                    "Successfully retrieved token from keyring for {}, token format check: {}",
                                    auth_provider.id(),
                                    if token.contains('|') {
                                        "contains pipe separator"
                                    } else {
                                        "no pipe separator"
                                    }
                                );
                                // Parse token to extract user_id if it's in format token|user_id
                                let parts: Vec<&str> = token.split('|').collect();
                                let (actual_token, actual_user_id) = if parts.len() == 2 {
                                    tracing::info!("Extracted user_id from token: {}", parts[1]);
                                    (parts[0].to_string(), parts[1].to_string())
                                } else {
                                    tracing::warn!(
                                        "Token doesn't contain user_id, falling back to AuthProvider user_id: '{}'",
                                        user_id
                                    );
                                    (token.clone(), user_id.clone())
                                };
                                (server_url.clone(), actual_token, actual_user_id)
                            }
                            Ok(Some(Credentials::UsernamePassword { ref password, .. })) => {
                                tracing::info!("Got password from keyring, re-authenticating...");
                                // Re-authenticate with username/password
                                match JellyfinApi::authenticate(server_url, username, &password)
                                    .await
                                {
                                    Ok(auth_response) => {
                                        // Save the new token
                                        tracing::info!(
                                            "Re-authentication successful, saving new token"
                                        );
                                        let token_creds = Credentials::Token {
                                            token: auth_response.access_token.clone(),
                                        };
                                        AuthService::save_credentials(
                                            &SourceId::new(auth_provider.id()),
                                            &token_creds,
                                        )
                                        .await
                                        .ok();
                                        (
                                            server_url.clone(),
                                            auth_response.access_token,
                                            auth_response.user.id,
                                        )
                                    }
                                    Err(e) => {
                                        error!("Failed to re-authenticate with Jellyfin: {}", e);
                                        return Ok(None);
                                    }
                                }
                            }
                            _ => {
                                tracing::warn!("No credentials found in keyring");
                                return Ok(None);
                            }
                        }
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
        api.get_movies(&library_id.to_string()).await
    }

    async fn get_shows(&self, library_id: &LibraryId) -> Result<Vec<Show>> {
        let api = self.ensure_api_initialized().await?;
        api.get_shows(&library_id.to_string()).await
    }

    async fn get_seasons(&self, show_id: &ShowId) -> Result<Vec<Season>> {
        let api = self.ensure_api_initialized().await?;
        api.get_seasons(&show_id.to_string()).await
    }

    async fn get_episodes(&self, show_id: &ShowId, season: u32) -> Result<Vec<Episode>> {
        let api = self.ensure_api_initialized().await?;

        let seasons = api.get_seasons(&show_id.to_string()).await?;
        if let Some(season_info) = seasons.iter().find(|s| s.season_number == season) {
            let mut episodes = api.get_episodes(&season_info.id).await?;

            // Ensure show_id is set correctly for all episodes
            for episode in &mut episodes {
                if episode.show_id.is_none() {
                    episode.show_id = Some(show_id.to_string());
                }
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

    // Removed unused methods: mark_watched, mark_unwatched, get_watch_status, search, find_next_episode

    async fn get_backend_id(&self) -> BackendId {
        BackendId::new(&self.backend_id)
    }

    // Removed unused methods: get_last_sync_time, supports_offline

    async fn get_home_sections(&self) -> Result<Vec<HomeSection>> {
        let api = self.ensure_api_initialized().await?;
        api.get_home_sections().await
    }

    // Removed unused methods: fetch_media_markers, get_backend_info
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::events::EventBus;
    use crate::models::{AuthProvider, Source, SourceType};

    #[test]
    fn test_new() {
        let backend = JellyfinBackend::new();
        assert_eq!(backend.backend_id, "jellyfin");
    }

    #[test]
    fn test_with_id() {
        let backend = JellyfinBackend::with_id("custom_jellyfin".to_string());
        assert_eq!(backend.backend_id, "custom_jellyfin");
    }

    #[test]
    fn test_from_auth_invalid_auth_provider() {
        let auth_provider = AuthProvider::PlexAccount {
            id: "plex".to_string(),
            username: "user".to_string(),
            email: "user@example.com".to_string(),
            token: "token123".to_string(),
            refresh_token: None,
            token_expiry: None,
        };

        let source = Source::new(
            "source1".to_string(),
            "Test Source".to_string(),
            SourceType::JellyfinServer,
            Some("plex".to_string()),
        );

        let config = Arc::new(RwLock::new(Config::default()));
        let event_bus = Arc::new(EventBus::new(1000));
        let auth_manager = Arc::new(AuthManager::new(config, event_bus));
        let result = JellyfinBackend::from_auth(auth_provider, source, auth_manager);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid auth provider type")
        );
    }

    #[test]
    fn test_from_auth_invalid_source_type() {
        let auth_provider = AuthProvider::JellyfinAuth {
            id: "jellyfin".to_string(),
            server_url: "http://jellyfin.local".to_string(),
            username: "user".to_string(),
            user_id: "user123".to_string(),
            access_token: "token123".to_string(),
        };

        let source = Source::new(
            "source1".to_string(),
            "Test Source".to_string(),
            SourceType::PlexServer {
                machine_id: "abc123".to_string(),
                owned: true,
            },
            Some("jellyfin".to_string()),
        );

        let config = Arc::new(RwLock::new(Config::default()));
        let event_bus = Arc::new(EventBus::new(1000));
        let auth_manager = Arc::new(AuthManager::new(config, event_bus));
        let result = JellyfinBackend::from_auth(auth_provider, source, auth_manager);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid source type")
        );
    }

    #[test]
    fn test_from_auth_valid() {
        let auth_provider = AuthProvider::JellyfinAuth {
            id: "jellyfin".to_string(),
            server_url: "http://jellyfin.local".to_string(),
            username: "user".to_string(),
            user_id: "user123".to_string(),
            access_token: "token123".to_string(),
        };

        let mut source = Source::new(
            "source1".to_string(),
            "Test Jellyfin Server".to_string(),
            SourceType::JellyfinServer,
            Some("jellyfin".to_string()),
        );
        source.connection_info.primary_url = Some("http://jellyfin.local".to_string());

        let config = Arc::new(RwLock::new(Config::default()));
        let event_bus = Arc::new(EventBus::new(1000));
        let auth_manager = Arc::new(AuthManager::new(config, event_bus));
        let result = JellyfinBackend::from_auth(auth_provider, source.clone(), auth_manager);

        assert!(result.is_ok());
        let backend = result.unwrap();
        assert_eq!(backend.backend_id, "source1");
    }

    #[tokio::test]
    async fn test_is_initialized_false() {
        let backend = JellyfinBackend::new();
        assert!(!backend.is_initialized().await);
    }

    #[tokio::test]
    async fn test_get_credentials_none() {
        let backend = JellyfinBackend::new();
        let creds = backend.get_credentials().await;
        assert!(creds.is_none());
    }

    #[tokio::test]
    async fn test_get_credentials_with_values() {
        let backend = JellyfinBackend::new();

        *backend.api_key.write().await = Some("api_key_123".to_string());
        *backend.user_id.write().await = Some("user_id_456".to_string());

        let creds = backend.get_credentials().await;
        assert!(creds.is_some());

        let (api_key, user_id) = creds.unwrap();
        assert_eq!(api_key, "api_key_123");
        assert_eq!(user_id, "user_id_456");
    }

    #[test]
    fn test_credentials_obfuscation() {
        let creds = "http://server|api_key|user_id";
        let obfuscated: Vec<u8> = creds
            .bytes()
            .enumerate()
            .map(|(i, b)| b ^ ((i as u8) + 42))
            .collect();

        let deobfuscated: Vec<u8> = obfuscated
            .iter()
            .enumerate()
            .map(|(i, &b)| b ^ ((i as u8) + 42))
            .collect();

        let recovered = String::from_utf8(deobfuscated).unwrap();
        assert_eq!(recovered, creds);
    }

    #[tokio::test]
    async fn test_authenticate_invalid_api_key() {
        let backend = JellyfinBackend::new();

        let result = backend
            .authenticate(Credentials::ApiKey {
                key: "some_key".to_string(),
            })
            .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("API key authentication not supported")
        );
    }

    #[tokio::test]
    async fn test_authenticate_invalid_token_format() {
        let backend = JellyfinBackend::new();

        let result = backend
            .authenticate(Credentials::Token {
                token: "invalid_token".to_string(),
            })
            .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid token format")
        );
    }

    #[tokio::test]
    async fn test_authenticate_valid_token_format() {
        let backend = JellyfinBackend::new();

        // This will fail since we don't have a real server, but it tests the token parsing
        let result = backend
            .authenticate(Credentials::Token {
                token: "http://server|api_key|user_id".to_string(),
            })
            .await;

        // Check that the values were parsed correctly
        assert_eq!(
            backend.base_url.read().await.clone(),
            Some("http://server".to_string())
        );
        assert_eq!(
            backend.api_key.read().await.clone(),
            Some("api_key".to_string())
        );
        assert_eq!(
            backend.user_id.read().await.clone(),
            Some("user_id".to_string())
        );
    }

    #[tokio::test]
    async fn test_get_backend_id() {
        let backend = JellyfinBackend::with_id("test_jellyfin_123".to_string());
        let id = backend.get_backend_id().await;
        assert_eq!(id, "test_jellyfin_123");
    }

    #[tokio::test]
    async fn test_get_last_sync_time_none() {
        let backend = JellyfinBackend::new();
        let time = backend.get_last_sync_time().await;
        assert!(time.is_none());
    }

    #[tokio::test]
    async fn test_get_last_sync_time_with_value() {
        let backend = JellyfinBackend::new();
        let now = Utc::now();

        *backend.last_sync_time.write().await = Some(now);

        let time = backend.get_last_sync_time().await;
        assert!(time.is_some());
        assert_eq!(time.unwrap(), now);
    }

    #[tokio::test]
    async fn test_supports_offline() {
        let backend = JellyfinBackend::new();
        assert!(backend.supports_offline().await);
    }

    #[tokio::test]
    async fn test_get_backend_info_default() {
        let backend = JellyfinBackend::new();
        let info = backend.get_backend_info().await;

        assert_eq!(info.name, "jellyfin");
        assert_eq!(info.display_name, "Jellyfin");
        assert!(matches!(info.backend_type, BackendType::Jellyfin));
        assert_eq!(info.connection_type, ConnectionType::Remote);
        assert!(!info.is_local);
        assert!(!info.is_relay);
        assert!(info.server_name.is_none());
        assert!(info.server_version.is_none());
    }

    #[tokio::test]
    async fn test_get_backend_info_with_server_name() {
        let backend = JellyfinBackend::with_id("my_jellyfin".to_string());

        *backend.server_name.write().await = Some("Home Media Server".to_string());

        let info = backend.get_backend_info().await;

        assert_eq!(info.name, "my_jellyfin");
        assert_eq!(info.display_name, "Home Media Server");
        assert!(matches!(info.backend_type, BackendType::Jellyfin));
        assert_eq!(info.server_name, Some("Home Media Server".to_string()));
    }

    #[test]
    fn test_debug_impl() {
        let backend = JellyfinBackend::new();
        let debug_str = format!("{:?}", backend);

        assert!(debug_str.contains("JellyfinBackend"));
        assert!(debug_str.contains("backend_id"));
        assert!(debug_str.contains("server_name"));
    }
}
