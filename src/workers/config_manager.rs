use anyhow::Result;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use relm4::{ComponentSender, Sender, Worker};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::config::Config;
use crate::ui::shared::broker::{BROKER, BrokerMessage, ConfigMessage};

#[derive(Debug)]
pub enum ConfigManagerInput {
    /// Load the configuration from disk
    LoadConfig,
    /// Update a specific configuration value
    UpdateConfig(Box<Config>),
    /// Save current configuration to disk
    SaveConfig,
    /// Reload configuration from disk (triggered by file watcher)
    ReloadConfig,
    /// Shutdown the worker
    Shutdown,
}

#[derive(Debug)]
pub enum ConfigManagerOutput {
    /// Configuration loaded successfully
    ConfigLoaded(Arc<Config>),
    /// Configuration updated
    ConfigUpdated(Arc<Config>),
    /// Error occurred
    Error(String),
}

pub struct ConfigManager {
    config: Arc<RwLock<Config>>,
    config_path: PathBuf,
    file_watcher: Option<RecommendedWatcher>,
    last_reload_time: Arc<std::sync::Mutex<Option<std::time::Instant>>>,
    config_content_hash: Arc<std::sync::Mutex<Option<u64>>>,
}

impl std::fmt::Debug for ConfigManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigManager")
            .field("config", &"<config>")
            .field("config_path", &self.config_path)
            .field("file_watcher", &self.file_watcher.is_some())
            .finish()
    }
}

impl ConfigManager {
    pub fn new() -> Self {
        let config = Config::load().unwrap_or_default();
        let config_path = Config::config_path().unwrap_or_else(|_| PathBuf::from("config.toml"));

        Self {
            config: Arc::new(RwLock::new(config)),
            config_path,
            file_watcher: None,
            last_reload_time: Arc::new(std::sync::Mutex::new(None)),
            config_content_hash: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Compute hash of config file content
    fn compute_file_hash(path: &PathBuf) -> Option<u64> {
        std::fs::read_to_string(path).ok().map(|content| {
            let mut hasher = DefaultHasher::new();
            content.hash(&mut hasher);
            hasher.finish()
        })
    }

    async fn notify_config_change(&self, config: Arc<Config>) {
        BROKER
            .broadcast(BrokerMessage::Config(ConfigMessage::Updated {
                config: config.clone(),
            }))
            .await;
    }

    async fn load_config(&mut self) -> Result<()> {
        info!("Loading configuration from disk");

        match Config::load() {
            Ok(new_config) => {
                let arc_config = Arc::new(new_config);
                *self.config.write().await = (*arc_config).clone();

                // Update stored hash after successful load
                if let Some(hash) = Self::compute_file_hash(&self.config_path) {
                    *self.config_content_hash.lock().unwrap() = Some(hash);
                }

                self.notify_config_change(arc_config.clone()).await;
                info!("Configuration loaded and broadcasted");
                Ok(())
            }
            Err(e) => {
                error!("Failed to load config: {}", e);
                Err(e)
            }
        }
    }

    async fn save_config(&self) -> Result<()> {
        info!("Saving configuration to disk");

        let config = self.config.read().await;
        match config.save() {
            Ok(_) => {
                info!("Configuration saved successfully");
                Ok(())
            }
            Err(e) => {
                error!("Failed to save config: {}", e);
                Err(e)
            }
        }
    }

    async fn update_config(&mut self, new_config: Config) -> Result<()> {
        info!("Updating configuration");

        // Update in-memory config
        *self.config.write().await = new_config.clone();

        // Save to disk
        match new_config.save() {
            Ok(_) => {
                let arc_config = Arc::new(new_config);
                self.notify_config_change(arc_config.clone()).await;
                info!("Configuration updated and saved");
                Ok(())
            }
            Err(e) => {
                error!("Failed to save updated config: {}", e);
                Err(e)
            }
        }
    }

    fn setup_file_watcher(&mut self, sender: Sender<ConfigManagerInput>) -> Result<()> {
        info!("Setting up configuration file watcher");

        let config_path = self.config_path.clone();
        let config_path_for_closure = config_path.clone();

        let mut watcher =
            notify::recommended_watcher(move |res: Result<notify::Event, _>| match res {
                Ok(event) => {
                    use notify::EventKind;

                    // Only process events for our specific config file
                    let is_config_file = event.paths.iter().any(|p| p == &config_path_for_closure);
                    if !is_config_file {
                        return;
                    }

                    match event.kind {
                        EventKind::Modify(_) | EventKind::Create(_) => {
                            // Don't log here - let the handler decide if content actually changed
                            let _ = sender.send(ConfigManagerInput::ReloadConfig);
                        }
                        _ => {}
                    }
                }
                Err(e) => error!("File watcher error: {:?}", e),
            })?;

        // Watch the config file
        if let Some(parent) = config_path.parent() {
            watcher.watch(parent, RecursiveMode::NonRecursive)?;
            info!("Watching config directory: {:?}", parent);
        }

        self.file_watcher = Some(watcher);
        Ok(())
    }

    pub fn get_config(&self) -> Arc<RwLock<Config>> {
        self.config.clone()
    }
}

impl Worker for ConfigManager {
    type Init = ();
    type Input = ConfigManagerInput;
    type Output = ConfigManagerOutput;

    fn init(_init: Self::Init, sender: ComponentSender<Self>) -> Self {
        let mut manager = Self::new();

        // Setup file watcher
        if let Err(e) = manager.setup_file_watcher(sender.input_sender().clone()) {
            warn!("Failed to setup config file watcher: {}", e);
        }

        // Load initial config
        sender.input(ConfigManagerInput::LoadConfig);

        manager
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            ConfigManagerInput::LoadConfig => {
                let mut manager = self.clone();
                let sender = sender.clone();
                relm4::spawn_local(async move {
                    if let Err(e) = manager.load_config().await {
                        sender
                            .output(ConfigManagerOutput::Error(e.to_string()))
                            .unwrap();
                    } else {
                        let config = manager.config.read().await;
                        sender
                            .output(ConfigManagerOutput::ConfigLoaded(Arc::new(config.clone())))
                            .unwrap();
                    }
                });
            }
            ConfigManagerInput::UpdateConfig(new_config) => {
                let mut manager = self.clone();
                let sender = sender.clone();
                relm4::spawn_local(async move {
                    if let Err(e) = manager.update_config(*new_config).await {
                        sender
                            .output(ConfigManagerOutput::Error(e.to_string()))
                            .unwrap();
                    } else {
                        let config = manager.config.read().await;
                        sender
                            .output(ConfigManagerOutput::ConfigUpdated(Arc::new(config.clone())))
                            .unwrap();
                    }
                });
            }
            ConfigManagerInput::SaveConfig => {
                let manager = self.clone();
                let sender = sender.clone();
                relm4::spawn_local(async move {
                    if let Err(e) = manager.save_config().await {
                        sender
                            .output(ConfigManagerOutput::Error(e.to_string()))
                            .unwrap();
                    }
                });
            }
            ConfigManagerInput::ReloadConfig => {
                // Debounce rapid file changes using timestamp comparison
                const DEBOUNCE_MS: u128 = 100;

                let now = std::time::Instant::now();
                let should_check_hash = {
                    let mut last_time = self.last_reload_time.lock().unwrap();
                    if let Some(last) = *last_time {
                        if now.duration_since(last).as_millis() < DEBOUNCE_MS {
                            false
                        } else {
                            *last_time = Some(now);
                            true
                        }
                    } else {
                        *last_time = Some(now);
                        true
                    }
                };

                if should_check_hash {
                    // Check if file content actually changed by comparing hashes
                    let current_hash = Self::compute_file_hash(&self.config_path);
                    let should_reload = {
                        let mut stored_hash = self.config_content_hash.lock().unwrap();
                        if current_hash != *stored_hash {
                            // Update hash immediately to prevent duplicate reloads from burst events
                            *stored_hash = current_hash;
                            true
                        } else {
                            false
                        }
                    };

                    if should_reload {
                        debug!("Config file content changed, reloading");
                        let mut manager = self.clone();
                        let sender = sender.clone();
                        relm4::spawn_local(async move {
                            if let Err(e) = manager.load_config().await {
                                sender
                                    .output(ConfigManagerOutput::Error(e.to_string()))
                                    .unwrap();
                            } else {
                                let config = manager.config.read().await;
                                sender
                                    .output(ConfigManagerOutput::ConfigLoaded(Arc::new(
                                        config.clone(),
                                    )))
                                    .unwrap();
                            }
                        });
                    } else {
                        debug!("Config file event detected but content unchanged, skipping reload");
                    }
                }
            }
            ConfigManagerInput::Shutdown => {
                info!("ConfigManager shutting down");
                self.file_watcher = None;
            }
        }
    }
}

impl Clone for ConfigManager {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            config_path: self.config_path.clone(),
            file_watcher: None, // Don't clone the file watcher
            last_reload_time: self.last_reload_time.clone(),
            config_content_hash: self.config_content_hash.clone(),
        }
    }
}
