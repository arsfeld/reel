use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::{debug, info};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default, skip_serializing_if = "GeneralConfig::is_default")]
    pub general: GeneralConfig,

    #[serde(default, skip_serializing_if = "PlaybackConfig::is_default")]
    pub playback: PlaybackConfig,

    #[serde(default, skip_serializing_if = "NetworkConfig::is_default")]
    pub network: NetworkConfig,

    #[serde(default, skip_serializing_if = "BackendsConfig::is_default")]
    pub backends: BackendsConfig,

    #[serde(default, skip_serializing_if = "RuntimeConfig::is_default")]
    pub runtime: RuntimeConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GeneralConfig {
    #[serde(default = "default_theme", skip_serializing_if = "is_default_theme")]
    pub theme: String,

    #[serde(
        default = "default_language",
        skip_serializing_if = "is_default_language"
    )]
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlaybackConfig {
    #[serde(
        default = "default_player_backend",
        skip_serializing_if = "is_default_player_backend"
    )]
    pub player_backend: String,

    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub hardware_acceleration: bool,

    #[serde(
        default = "default_subtitle",
        skip_serializing_if = "is_default_subtitle"
    )]
    pub default_subtitle: String,

    #[serde(default = "default_audio", skip_serializing_if = "is_default_audio")]
    pub default_audio: String,

    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub skip_intro: bool,

    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub skip_credits: bool,

    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub auto_play_next: bool,

    #[serde(
        default = "default_auto_play_delay",
        skip_serializing_if = "is_default_auto_play_delay"
    )]
    pub auto_play_delay: u64,

    #[serde(default = "default_false", skip_serializing_if = "is_false")]
    pub mpv_verbose_logging: bool,

    #[serde(
        default = "default_cache_size_mb",
        skip_serializing_if = "is_default_cache_size_mb"
    )]
    pub mpv_cache_size_mb: u32,

    #[serde(
        default = "default_cache_backbuffer_mb",
        skip_serializing_if = "is_default_cache_backbuffer_mb"
    )]
    pub mpv_cache_backbuffer_mb: u32,

    #[serde(
        default = "default_cache_secs",
        skip_serializing_if = "is_default_cache_secs"
    )]
    pub mpv_cache_secs: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NetworkConfig {
    #[serde(
        default = "default_timeout",
        skip_serializing_if = "is_default_timeout"
    )]
    pub connection_timeout: u64,

    #[serde(
        default = "default_retries",
        skip_serializing_if = "is_default_retries"
    )]
    pub max_retries: u32,

    #[serde(
        default = "default_cache_size",
        skip_serializing_if = "is_default_cache_size"
    )]
    pub cache_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct BackendsConfig {
    #[serde(default, skip_serializing_if = "PlexConfig::is_default")]
    pub plex: PlexConfig,

    #[serde(default, skip_serializing_if = "JellyfinConfig::is_default")]
    pub jellyfin: JellyfinConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlexConfig {
    #[serde(
        default = "default_plex_url",
        skip_serializing_if = "is_default_plex_url"
    )]
    pub server_url: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct JellyfinConfig {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub server_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct RuntimeConfig {
    /// List of all legacy backend IDs that need migration
    #[serde(
        default,
        alias = "configured_backends",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub legacy_backends: Vec<String>,

    /// Last sync timestamp for each backend
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub last_sync_times: std::collections::HashMap<String, String>,

    /// Library visibility settings (library_id -> is_visible)
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub library_visibility: std::collections::HashMap<String, bool>,

    /// Persisted auth providers
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub auth_providers: std::collections::HashMap<String, crate::models::AuthProvider>,

    /// Cached sources for each provider (provider_id -> sources)
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub cached_sources: std::collections::HashMap<String, Vec<crate::models::Source>>,

    /// Last time sources were fetched for each provider
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub sources_last_fetched: std::collections::HashMap<String, chrono::DateTime<chrono::Utc>>,
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

    pub async fn set_plex_token(&mut self, token: &str) -> Result<()> {
        self.backends.plex.auth_token = Some(token.to_string());
        self.save()
    }

    pub fn get_legacy_backends(&self) -> Vec<String> {
        self.runtime.legacy_backends.clone()
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

    pub fn get_auth_providers(
        &self,
    ) -> std::collections::HashMap<String, crate::models::AuthProvider> {
        self.runtime.auth_providers.clone()
    }

    pub fn set_auth_providers(
        &mut self,
        providers: std::collections::HashMap<String, crate::models::AuthProvider>,
    ) -> Result<()> {
        self.runtime.auth_providers = providers;
        self.save()
    }

    pub fn add_auth_provider(
        &mut self,
        id: String,
        provider: crate::models::AuthProvider,
    ) -> Result<()> {
        self.runtime.auth_providers.insert(id, provider);
        self.save()
    }

    pub fn remove_auth_provider(&mut self, id: &str) -> Result<()> {
        self.runtime.auth_providers.remove(id);
        self.runtime.cached_sources.remove(id);
        self.runtime.sources_last_fetched.remove(id);
        self.save()
    }

    pub fn remove_legacy_backend(&mut self, backend_id: &str) -> Result<()> {
        self.runtime.legacy_backends.retain(|b| b != backend_id);
        self.save()
    }

    pub fn get_cached_sources(&self, provider_id: &str) -> Option<Vec<crate::models::Source>> {
        self.runtime.cached_sources.get(provider_id).cloned()
    }

    pub fn set_cached_sources(
        &mut self,
        provider_id: String,
        sources: Vec<crate::models::Source>,
    ) -> Result<()> {
        self.runtime
            .cached_sources
            .insert(provider_id.clone(), sources);
        self.runtime
            .sources_last_fetched
            .insert(provider_id, chrono::Utc::now());
        self.save()
    }

    pub fn get_sources_last_fetched(
        &self,
        provider_id: &str,
    ) -> Option<chrono::DateTime<chrono::Utc>> {
        self.runtime.sources_last_fetched.get(provider_id).copied()
    }

    pub fn is_sources_cache_stale(&self, provider_id: &str, max_age_secs: i64) -> bool {
        if let Some(last_fetched) = self.get_sources_last_fetched(provider_id) {
            let age = chrono::Utc::now() - last_fetched;
            age.num_seconds() > max_age_secs
        } else {
            true // No cached data means it's stale
        }
    }

    fn config_path() -> Result<PathBuf> {
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
            skip_credits: default_true(),
            auto_play_next: default_true(),
            auto_play_delay: default_auto_play_delay(),
            mpv_verbose_logging: default_false(),
            mpv_cache_size_mb: default_cache_size_mb(),
            mpv_cache_backbuffer_mb: default_cache_backbuffer_mb(),
            mpv_cache_secs: default_cache_secs(),
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
fn default_auto_play_delay() -> u64 {
    10 // 10 seconds countdown
}
fn default_false() -> bool {
    false
}

fn default_cache_size_mb() -> u32 {
    1500 // 1.5GB for ~15-30 min of 1080p/4K content
}

fn default_cache_backbuffer_mb() -> u32 {
    500 // 500MB for backward seeking
}

fn default_cache_secs() -> u32 {
    1800 // 30 minutes default
}

// Skip serializing helper functions
fn is_default_theme(value: &str) -> bool {
    value == default_theme()
}

fn is_default_language(value: &str) -> bool {
    value == default_language()
}

fn is_default_player_backend(value: &str) -> bool {
    value == default_player_backend()
}

fn is_true(value: &bool) -> bool {
    *value
}

fn is_false(value: &bool) -> bool {
    !(*value)
}

fn is_default_subtitle(value: &str) -> bool {
    value == default_subtitle()
}

fn is_default_audio(value: &str) -> bool {
    value == default_audio()
}

fn is_default_timeout(value: &u64) -> bool {
    *value == default_timeout()
}

fn is_default_retries(value: &u32) -> bool {
    *value == default_retries()
}

fn is_default_cache_size(value: &u64) -> bool {
    *value == default_cache_size()
}

fn is_default_plex_url(value: &str) -> bool {
    value == default_plex_url()
}

fn is_default_auto_play_delay(value: &u64) -> bool {
    *value == default_auto_play_delay()
}

fn is_default_cache_size_mb(value: &u32) -> bool {
    *value == default_cache_size_mb()
}

fn is_default_cache_backbuffer_mb(value: &u32) -> bool {
    *value == default_cache_backbuffer_mb()
}

fn is_default_cache_secs(value: &u32) -> bool {
    *value == default_cache_secs()
}

// is_default implementations for structs
impl GeneralConfig {
    fn is_default(value: &Self) -> bool {
        value == &Self::default()
    }
}

impl PlaybackConfig {
    fn is_default(value: &Self) -> bool {
        value == &Self::default()
    }
}

impl NetworkConfig {
    fn is_default(value: &Self) -> bool {
        value == &Self::default()
    }
}

impl BackendsConfig {
    fn is_default(value: &Self) -> bool {
        value == &Self::default()
    }
}

impl RuntimeConfig {
    fn is_default(value: &Self) -> bool {
        value == &Self::default()
    }
}

impl Default for PlexConfig {
    fn default() -> Self {
        Self {
            server_url: default_plex_url(),
            auth_token: None,
        }
    }
}

impl PlexConfig {
    fn is_default(value: &Self) -> bool {
        value == &Self::default()
    }
}

impl JellyfinConfig {
    fn is_default(value: &Self) -> bool {
        value == &Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_serializes_minimal() {
        let config = Config::default();
        let toml = toml::to_string_pretty(&config).unwrap();

        // Default config should serialize to empty or minimal TOML
        assert!(
            toml.is_empty() || toml == "\n",
            "Default config should produce empty TOML, got: {}",
            toml
        );
    }

    #[test]
    fn test_modified_field_serializes() {
        let mut config = Config::default();
        config.general.theme = "dark".to_string();

        let toml = toml::to_string_pretty(&config).unwrap();

        // Should only contain the general section with theme
        assert!(toml.contains("[general]"), "Should contain general section");
        assert!(
            toml.contains("theme = \"dark\""),
            "Should contain modified theme"
        );
        assert!(
            !toml.contains("language"),
            "Should not contain default language"
        );
        assert!(
            !toml.contains("[playback]"),
            "Should not contain default playback section"
        );
        assert!(
            !toml.contains("[network]"),
            "Should not contain default network section"
        );
    }

    #[test]
    fn test_multiple_modifications_serialize() {
        let mut config = Config::default();
        config.playback.mpv_verbose_logging = true;
        config.playback.mpv_cache_size_mb = 2000;
        config.network.connection_timeout = 60;

        let toml = toml::to_string_pretty(&config).unwrap();

        // Should contain modified sections and fields
        assert!(
            toml.contains("[playback]"),
            "Should contain playback section"
        );
        assert!(
            toml.contains("mpv_verbose_logging = true"),
            "Should contain modified logging"
        );
        assert!(
            toml.contains("mpv_cache_size_mb = 2000"),
            "Should contain modified cache size"
        );

        assert!(toml.contains("[network]"), "Should contain network section");
        assert!(
            toml.contains("connection_timeout = 60"),
            "Should contain modified timeout"
        );

        // Should not contain default values
        assert!(
            !toml.contains("hardware_acceleration"),
            "Should not contain default hardware_acceleration"
        );
        assert!(
            !toml.contains("skip_intro"),
            "Should not contain default skip_intro"
        );
        assert!(
            !toml.contains("max_retries"),
            "Should not contain default max_retries"
        );
        assert!(
            !toml.contains("[general]"),
            "Should not contain unmodified general section"
        );
    }

    #[test]
    fn test_runtime_config_with_data() {
        let mut config = Config::default();
        config
            .runtime
            .library_visibility
            .insert("lib1".to_string(), false);
        config.runtime.legacy_backends.push("backend1".to_string());

        let toml = toml::to_string_pretty(&config).unwrap();

        // Should contain runtime section with data
        assert!(toml.contains("[runtime]"), "Should contain runtime section");
        assert!(
            toml.contains("legacy_backends"),
            "Should contain legacy_backends"
        );
        assert!(
            toml.contains("library_visibility"),
            "Should contain library_visibility"
        );

        // Should not contain empty collections
        assert!(
            !toml.contains("last_sync_times"),
            "Should not contain empty last_sync_times"
        );
        assert!(
            !toml.contains("auth_providers"),
            "Should not contain empty auth_providers"
        );
    }

    #[test]
    fn test_backend_config_with_token() {
        let mut config = Config::default();
        config.backends.plex.auth_token = Some("test_token".to_string());

        let toml = toml::to_string_pretty(&config).unwrap();

        // Should contain backends section with plex token
        assert!(
            toml.contains("[backends.plex]"),
            "Should contain plex backend section"
        );
        assert!(
            toml.contains("auth_token = \"test_token\""),
            "Should contain auth token"
        );

        // Server URL is not serialized because it has the default value
        assert!(
            !toml.contains("server_url"),
            "Should not contain default server_url"
        );
        assert!(
            !toml.contains("[backends.jellyfin]"),
            "Should not contain empty jellyfin section"
        );
    }

    #[test]
    fn test_partial_playback_config() {
        let mut config = Config::default();
        config.playback.hardware_acceleration = false; // Changed from default true
        config.playback.auto_play_delay = 30; // Changed from default 10

        let toml = toml::to_string_pretty(&config).unwrap();

        assert!(
            toml.contains("[playback]"),
            "Should contain playback section"
        );
        assert!(
            toml.contains("hardware_acceleration = false"),
            "Should contain modified acceleration"
        );
        assert!(
            toml.contains("auto_play_delay = 30"),
            "Should contain modified delay"
        );

        // Default true values should not appear
        assert!(
            !toml.contains("skip_intro"),
            "Should not contain default skip_intro=true"
        );
        assert!(
            !toml.contains("skip_credits"),
            "Should not contain default skip_credits=true"
        );
        assert!(
            !toml.contains("auto_play_next"),
            "Should not contain default auto_play_next=true"
        );
    }

    #[test]
    fn test_config_roundtrip() {
        let mut original = Config::default();
        original.general.theme = "dark".to_string();
        original.playback.mpv_cache_size_mb = 2000;
        original.network.connection_timeout = 45;

        // Serialize to TOML
        let toml_str = toml::to_string_pretty(&original).unwrap();

        // Deserialize back
        let deserialized: Config = toml::from_str(&toml_str).unwrap();

        // Check modified values are preserved
        assert_eq!(deserialized.general.theme, "dark");
        assert_eq!(deserialized.playback.mpv_cache_size_mb, 2000);
        assert_eq!(deserialized.network.connection_timeout, 45);

        // Check defaults are still applied to non-serialized fields
        assert_eq!(deserialized.general.language, "system");
        assert_eq!(deserialized.playback.hardware_acceleration, true);
        assert_eq!(deserialized.network.max_retries, 3);
    }
}
