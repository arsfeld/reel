use anyhow::Result;
use once_cell::sync::Lazy;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

use crate::config::{Config, PlaybackConfig};
use crate::ui::shared::broker::{BROKER, BrokerMessage, ConfigMessage};

/// Global configuration service instance
pub static CONFIG_SERVICE: Lazy<ConfigService> = Lazy::new(|| {
    info!("Initializing global ConfigService");
    ConfigService::new()
});

/// Service for managing application configuration
pub struct ConfigService {
    config: Arc<RwLock<Config>>,
}

impl ConfigService {
    pub fn new() -> Self {
        let config = Config::load().unwrap_or_default();

        Self {
            config: Arc::new(RwLock::new(config)),
        }
    }

    /// Setup file watcher for config changes (deprecated - now handled by ConfigManager worker)
    pub fn setup_file_watcher(&mut self) {
        info!("Config file watcher is now handled by ConfigManager worker in MainWindow");
    }

    /// Get the current configuration
    pub async fn get_config(&self) -> Config {
        self.config.read().await.clone()
    }

    /// Get a reference to the configuration
    pub fn get_config_arc(&self) -> Arc<RwLock<Config>> {
        self.config.clone()
    }

    /// Update the entire configuration
    pub async fn update_config(&self, config: Config) -> Result<()> {
        info!("Updating configuration");

        // Update local copy first
        {
            let mut current = self.config.write().await;
            *current = config.clone();
        }

        // Save and broadcast the change
        config.save()?;

        BROKER
            .broadcast(BrokerMessage::Config(ConfigMessage::Updated {
                config: Arc::new(config),
            }))
            .await;

        Ok(())
    }

    /// Update the player backend
    pub async fn set_player_backend(&self, backend: String) -> Result<()> {
        debug!("Setting player backend to: {}", backend);

        let mut config = self.get_config().await;
        let old_backend = config.playback.player_backend.clone();

        if old_backend != backend {
            config.playback.player_backend = backend.clone();
            self.update_config(config).await?;

            // Send specific notification for player backend change
            BROKER
                .broadcast(BrokerMessage::Config(ConfigMessage::PlayerBackendChanged {
                    backend,
                }))
                .await;
        }

        Ok(())
    }

    /// Update hardware acceleration setting
    pub async fn set_hardware_acceleration(&self, enabled: bool) -> Result<()> {
        debug!("Setting hardware acceleration to: {}", enabled);

        let mut config = self.get_config().await;
        if config.playback.hardware_acceleration != enabled {
            config.playback.hardware_acceleration = enabled;
            self.update_config(config).await?;
        }

        Ok(())
    }

    /// Update MPV cache settings
    pub async fn set_mpv_cache_settings(
        &self,
        cache_size_mb: u32,
        cache_backbuffer_mb: Option<u32>,
        cache_secs: Option<u32>,
    ) -> Result<()> {
        debug!("Updating MPV cache settings");

        let mut config = self.get_config().await;
        config.playback.mpv_cache_size_mb = cache_size_mb;

        if let Some(backbuffer) = cache_backbuffer_mb {
            config.playback.mpv_cache_backbuffer_mb = backbuffer;
        }

        if let Some(secs) = cache_secs {
            config.playback.mpv_cache_secs = secs;
        }

        self.update_config(config).await?;
        Ok(())
    }

    /// Update auto-resume settings
    pub async fn set_auto_resume(
        &self,
        enabled: bool,
        threshold_seconds: Option<u32>,
    ) -> Result<()> {
        debug!("Updating auto-resume settings");

        let mut config = self.get_config().await;
        config.playback.auto_resume = enabled;

        if let Some(threshold) = threshold_seconds {
            config.playback.resume_threshold_seconds = threshold;
        }

        self.update_config(config).await?;
        Ok(())
    }

    /// Update MPV upscaling mode
    pub async fn set_mpv_upscaling_mode(&self, mode: String) -> Result<()> {
        debug!("Setting MPV upscaling mode to: {}", mode);

        let mut config = self.get_config().await;
        if config.playback.mpv_upscaling_mode != mode {
            config.playback.mpv_upscaling_mode = mode;
            self.update_config(config).await?;
        }

        Ok(())
    }

    /// Get playback configuration
    pub async fn get_playback_config(&self) -> PlaybackConfig {
        self.config.read().await.playback.clone()
    }

    /// Reload configuration from disk
    pub async fn reload_from_disk(&self) -> Result<()> {
        info!("Reloading configuration from disk");

        let config = Config::load()?;
        self.update_config(config).await?;

        Ok(())
    }

    /// Save current configuration to disk
    pub async fn save_to_disk(&self) -> Result<()> {
        info!("Saving configuration to disk");

        let config = self.config.read().await;
        config.save()?;

        Ok(())
    }
}

impl Default for ConfigService {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper function to get the global config service
pub fn config_service() -> &'static ConfigService {
    &CONFIG_SERVICE
}
