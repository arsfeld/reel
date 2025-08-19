use anyhow::{Context, Result};
use dirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::{debug, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    
    #[serde(default)]
    pub playback: PlaybackConfig,
    
    #[serde(default)]
    pub network: NetworkConfig,
    
    #[serde(default)]
    pub backends: BackendsConfig,
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

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;
        
        if config_path.exists() {
            debug!("Loading config from {:?}", config_path);
            let contents = fs::read_to_string(&config_path)
                .context("Failed to read config file")?;
            let config: Config = toml::from_str(&contents)
                .context("Failed to parse config file")?;
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
            fs::create_dir_all(parent)
                .context("Failed to create config directory")?;
        }
        
        let contents = toml::to_string_pretty(self)
            .context("Failed to serialize config")?;
        
        fs::write(&config_path, contents)
            .context("Failed to write config file")?;
        
        debug!("Config saved to {:?}", config_path);
        Ok(())
    }
    
    pub async fn set_plex_token(&mut self, token: &str) -> Result<()> {
        self.backends.plex.auth_token = Some(token.to_string());
        self.save()
    }
    
    fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Failed to get config directory")?;
        Ok(config_dir.join("reel").join("config.toml"))
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            playback: PlaybackConfig::default(),
            network: NetworkConfig::default(),
            backends: BackendsConfig::default(),
        }
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
fn default_theme() -> String { "auto".to_string() }
fn default_language() -> String { "system".to_string() }
fn default_true() -> bool { true }
fn default_subtitle() -> String { "none".to_string() }
fn default_audio() -> String { "original".to_string() }
fn default_timeout() -> u64 { 30 }
fn default_retries() -> u32 { 3 }
fn default_cache_size() -> u64 { 1000 }
fn default_plex_url() -> String { "https://plex.tv".to_string() }