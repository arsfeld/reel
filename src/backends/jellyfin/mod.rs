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
use crate::services::auth_manager::AuthManager;

pub struct JellyfinBackend {
    client: Client,
    base_url: Arc<RwLock<Option<String>>>,
    api_key: Arc<RwLock<Option<String>>>,
    user_id: Arc<RwLock<Option<String>>>,
    backend_id: String,
    last_sync_time: Arc<RwLock<Option<DateTime<Utc>>>>,
    api: Arc<RwLock<Option<JellyfinApi>>>,
    server_name: Arc<RwLock<Option<String>>>,
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
            auth_provider: Some(auth_provider),
            source: Some(source),
            auth_manager: Some(auth_manager),
        })
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

                    let api = JellyfinApi::with_backend_id(
                        base_url.to_string(),
                        api_key.to_string(),
                        user_id.to_string(),
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

        let seasons = api.get_seasons(show_id).await?;
        if let Some(season_info) = seasons.iter().find(|s| s.season_number == season) {
            return api.get_episodes(&season_info.id).await;
        }

        Err(anyhow!("Failed to get seasons"))
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

    async fn fetch_media_markers(
        &self,
        media_id: &str,
    ) -> Result<(
        Option<crate::models::ChapterMarker>,
        Option<crate::models::ChapterMarker>,
    )> {
        let api = self.ensure_api_initialized().await?;

        if let Ok(segments) = api.get_media_segments(media_id).await {
            let mut intro = None;
            let mut credits = None;

            for segment in segments {
                match segment.segment_type {
                    crate::backends::jellyfin::api::MediaSegmentType::Intro => {
                        intro = Some(crate::models::ChapterMarker {
                            start_time: Duration::from_secs(segment.start_ticks / 10_000_000),
                            end_time: Duration::from_secs(segment.end_ticks / 10_000_000),
                            marker_type: crate::models::ChapterType::Intro,
                        });
                    }
                    crate::backends::jellyfin::api::MediaSegmentType::Credits
                    | crate::backends::jellyfin::api::MediaSegmentType::Outro => {
                        credits = Some(crate::models::ChapterMarker {
                            start_time: Duration::from_secs(segment.start_ticks / 10_000_000),
                            end_time: Duration::from_secs(segment.end_ticks / 10_000_000),
                            marker_type: crate::models::ChapterType::Credits,
                        });
                    }
                    _ => {}
                }
            }

            Ok((intro, credits))
        } else {
            Ok((None, None))
        }
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
    async fn test_set_base_url() {
        let backend = JellyfinBackend::new();
        assert!(backend.base_url.read().await.is_none());

        backend.set_base_url("http://test.jellyfin.local").await;

        let url = backend.base_url.read().await.clone();
        assert_eq!(url, Some("http://test.jellyfin.local".to_string()));
    }

    #[tokio::test]
    async fn test_get_api_client_none() {
        let backend = JellyfinBackend::new();
        let api = backend.get_api_client().await;
        assert!(api.is_none());
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
