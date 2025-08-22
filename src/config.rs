use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::{debug, info};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,

    #[serde(default)]
    pub playback: PlaybackConfig,

    #[serde(default)]
    pub network: NetworkConfig,

    #[serde(default)]
    pub backends: BackendsConfig,

    #[serde(default)]
    pub runtime: RuntimeConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default = "default_theme")]
    pub theme: String,

    #[serde(default = "default_language")]
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackConfig {
    #[serde(default = "default_player_backend")]
    pub player_backend: String,

    #[serde(default = "default_true")]
    pub hardware_acceleration: bool,

    #[serde(default = "default_subtitle")]
    pub default_subtitle: String,

    #[serde(default = "default_audio")]
    pub default_audio: String,

    #[serde(default = "default_true")]
    pub skip_intro: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    #[serde(default = "default_timeout")]
    pub connection_timeout: u64,

    #[serde(default = "default_retries")]
    pub max_retries: u32,

    #[serde(default = "default_cache_size")]
    pub cache_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BackendsConfig {
    #[serde(default)]
    pub plex: PlexConfig,

    #[serde(default)]
    pub jellyfin: JellyfinConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlexConfig {
    #[serde(default = "default_plex_url")]
    pub server_url: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JellyfinConfig {
    #[serde(default)]
    pub server_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuntimeConfig {
    /// The ID of the last active backend
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_active_backend: Option<String>,

    /// List of all configured backend IDs
    #[serde(default)]
    pub configured_backends: Vec<String>,

    /// Last sync timestamp for each backend
    #[serde(default)]
    pub last_sync_times: std::collections::HashMap<String, String>,

    /// Library visibility settings (library_id -> is_visible)
    #[serde(default)]
    pub library_visibility: std::collections::HashMap<String, bool>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            debug!("Loading config from {:?}", config_path);
            let contents =
                fs::read_to_string(&config_path).context("Failed to read config file")?;
            let config: Config =
                toml::from_str(&contents).context("Failed to parse config file")?;
            info!("Config loaded successfully");
            Ok(config)
        } else {
            info!("No config file found, using defaults");
            let config = Config::default();
            config.save()?;
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        // Ensure config directory exists
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).context("Failed to create config directory")?;
        }

        let contents = toml::to_string_pretty(self).context("Failed to serialize config")?;

        fs::write(&config_path, contents).context("Failed to write config file")?;

        debug!("Config saved to {:?}", config_path);
        Ok(())
    }

    pub async fn set_plex_token(&mut self, token: &str) -> Result<()> {
        self.backends.plex.auth_token = Some(token.to_string());
        self.save()
    }

    pub fn set_last_active_backend(&mut self, backend_id: &str) -> Result<()> {
        self.runtime.last_active_backend = Some(backend_id.to_string());

        // Add to configured backends if not already there
        if !self
            .runtime
            .configured_backends
            .contains(&backend_id.to_string())
        {
            self.runtime
                .configured_backends
                .push(backend_id.to_string());
        }

        self.save()
    }

    pub fn get_last_active_backend(&self) -> Option<String> {
        self.runtime.last_active_backend.clone()
    }

    pub fn get_configured_backends(&self) -> Vec<String> {
        self.runtime.configured_backends.clone()
    }

    pub fn set_library_visibility(&mut self, library_id: &str, visible: bool) -> Result<()> {
        self.runtime
            .library_visibility
            .insert(library_id.to_string(), visible);
        self.save()
    }

    pub fn get_library_visibility(&self, library_id: &str) -> bool {
        self.runtime
            .library_visibility
            .get(library_id)
            .copied()
            .unwrap_or(true)
    }

    pub fn get_all_library_visibility(&self) -> std::collections::HashMap<String, bool> {
        self.runtime.library_visibility.clone()
    }

    pub fn set_all_library_visibility(
        &mut self,
        visibility: std::collections::HashMap<String, bool>,
    ) -> Result<()> {
        self.runtime.library_visibility = visibility;
        self.save()
    }

    fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir().context("Failed to get config directory")?;
        Ok(config_dir.join("reel").join("config.toml"))
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            language: default_language(),
        }
    }
}

impl Default for PlaybackConfig {
    fn default() -> Self {
        Self {
            player_backend: default_player_backend(),
            hardware_acceleration: default_true(),
            default_subtitle: default_subtitle(),
            default_audio: default_audio(),
            skip_intro: default_true(),
        }
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            connection_timeout: default_timeout(),
            max_retries: default_retries(),
            cache_size: default_cache_size(),
        }
    }
}

// Default value functions
fn default_theme() -> String {
    "auto".to_string()
}
fn default_language() -> String {
    "system".to_string()
}
fn default_player_backend() -> String {
    "mpv".to_string()
}
fn default_true() -> bool {
    true
}
fn default_subtitle() -> String {
    "none".to_string()
}
fn default_audio() -> String {
    "original".to_string()
}
fn default_timeout() -> u64 {
    30
}
fn default_retries() -> u32 {
    3
}
fn default_cache_size() -> u64 {
    1000
}
fn default_plex_url() -> String {
    "https://plex.tv".to_string()
}
