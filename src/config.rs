use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::{debug, info};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub playback: PlaybackConfig,

    #[serde(default)]
    pub cache: crate::cache::FileCacheConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlaybackConfig {
    #[serde(default)]
    pub player_backend: String,

    #[serde(default)]
    pub hardware_acceleration: bool,

    #[serde(default)]
    pub mpv_verbose_logging: bool,

    #[serde(default)]
    pub mpv_cache_size_mb: u32,

    #[serde(default)]
    pub mpv_cache_backbuffer_mb: u32,

    #[serde(default)]
    pub mpv_cache_secs: u32,

    #[serde(default)]
    pub auto_resume: bool,

    #[serde(default)]
    pub resume_threshold_seconds: u32,

    #[serde(default)]
    pub progress_update_interval_seconds: u32,

    #[serde(default)]
    pub mpv_upscaling_mode: String,
}

impl Default for PlaybackConfig {
    fn default() -> Self {
        Self {
            player_backend: "mpv".to_string(),
            hardware_acceleration: true,
            mpv_verbose_logging: true,
            mpv_cache_size_mb: 150,
            mpv_cache_backbuffer_mb: 50,
            mpv_cache_secs: 30,
            auto_resume: true,
            resume_threshold_seconds: 10,
            progress_update_interval_seconds: 10,
            mpv_upscaling_mode: "bilinear".to_string(),
        }
    }
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
            Ok(Config::default())
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

    pub fn config_path() -> Result<PathBuf> {
        #[cfg(target_os = "macos")]
        {
            // On macOS, use ~/Library/Application Support/Reel/
            let config_dir = dirs::config_dir()
                .or_else(|| dirs::home_dir().map(|h| h.join("Library/Application Support")))
                .context("Failed to get config directory")?;
            Ok(config_dir.join("Reel").join("config.toml"))
        }
        #[cfg(not(target_os = "macos"))]
        {
            // On Linux and other platforms, use ~/.config/reel/
            let config_dir = dirs::config_dir().context("Failed to get config directory")?;
            Ok(config_dir.join("reel").join("config.toml"))
        }
    }
}
