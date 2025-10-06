use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tracing::{debug, info};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub playback: PlaybackConfig,

    #[serde(default)]
    pub cache: crate::cache::FileCacheConfig,

    #[serde(default)]
    pub ui: UiPreferences,

    #[serde(default)]
    pub updates: UpdateConfig,
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

    #[serde(default = "default_true")]
    pub skip_intro_enabled: bool,

    #[serde(default = "default_true")]
    pub skip_credits_enabled: bool,

    #[serde(default)]
    pub auto_skip_intro: bool,

    #[serde(default)]
    pub auto_skip_credits: bool,

    #[serde(default = "default_minimum_marker_duration")]
    pub minimum_marker_duration_seconds: u32,
}

fn default_true() -> bool {
    true
}

fn default_minimum_marker_duration() -> u32 {
    5
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
            skip_intro_enabled: true,
            skip_credits_enabled: true,
            auto_skip_intro: false,
            auto_skip_credits: false,
            minimum_marker_duration_seconds: 5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct UiPreferences {
    /// Map of library_id -> filter_tab (e.g., "All", "Unwatched", "RecentlyAdded")
    #[serde(default)]
    pub library_filter_tabs: HashMap<String, String>,

    /// Map of library_id -> serialized FilterState JSON
    #[serde(default)]
    pub library_filter_states: HashMap<String, String>,

    /// Map of preset_name -> serialized FilterState JSON
    #[serde(default)]
    pub filter_presets: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UpdateConfig {
    /// Update behavior: "auto", "manual", or "disabled"
    #[serde(default = "default_update_behavior")]
    pub behavior: String,

    /// Check for updates on startup
    #[serde(default = "default_true")]
    pub check_on_startup: bool,

    /// Automatically download updates
    #[serde(default)]
    pub auto_download: bool,

    /// Automatically install updates
    #[serde(default)]
    pub auto_install: bool,

    /// Check for pre-release versions
    #[serde(default)]
    pub check_prerelease: bool,
}

fn default_update_behavior() -> String {
    "manual".to_string()
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            behavior: "manual".to_string(),
            check_on_startup: true,
            auto_download: false,
            auto_install: false,
            check_prerelease: false,
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
