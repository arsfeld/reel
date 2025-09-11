use super::{Property, PropertySubscriber, ViewModel};
use crate::config::Config;
use crate::events::{EventBus, EventPayload, EventType};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

#[derive(Clone, Debug)]
pub enum ThemeOption {
    System,
    Light,
    Dark,
}

impl ThemeOption {
    pub fn from_config_value(value: &str) -> Self {
        match value {
            "light" => Self::Light,
            "dark" => Self::Dark,
            _ => Self::System,
        }
    }

    pub fn to_config_value(&self) -> &'static str {
        match self {
            Self::System => "auto",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    pub fn index(&self) -> u32 {
        match self {
            Self::System => 0,
            Self::Light => 1,
            Self::Dark => 2,
        }
    }

    pub fn from_index(index: u32) -> Self {
        match index {
            1 => Self::Light,
            2 => Self::Dark,
            _ => Self::System,
        }
    }
}

#[derive(Clone, Debug)]
pub enum PlayerBackend {
    GStreamer,
    Mpv,
}

impl PlayerBackend {
    pub fn from_config_value(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "gstreamer" => Self::GStreamer,
            _ => Self::Mpv,
        }
    }

    pub fn to_config_value(&self) -> &'static str {
        match self {
            Self::GStreamer => "gstreamer",
            Self::Mpv => "mpv",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::GStreamer => "GStreamer",
            Self::Mpv => "MPV",
        }
    }

    pub fn index(&self) -> u32 {
        match self {
            Self::GStreamer => 0,
            Self::Mpv => 1,
        }
    }

    pub fn from_index(index: u32) -> Self {
        match index {
            0 => Self::GStreamer,
            _ => Self::Mpv,
        }
    }
}

pub struct PreferencesViewModel {
    // Properties
    theme: Property<ThemeOption>,
    player_backend: Property<PlayerBackend>,
    is_loading: Property<bool>,

    // Services
    config: Arc<RwLock<Config>>,
    event_bus: RwLock<Option<Arc<EventBus>>>,
}

impl std::fmt::Debug for PreferencesViewModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreferencesViewModel")
            .field("theme", &"Property<ThemeOption>")
            .field("player_backend", &"Property<PlayerBackend>")
            .field("is_loading", &"Property<bool>")
            .field("config", &"Arc<RwLock<Config>>")
            .field("event_bus", &"Option<Arc<EventBus>>")
            .finish()
    }
}

impl PreferencesViewModel {
    pub fn new(config: Arc<RwLock<Config>>) -> Self {
        Self {
            theme: Property::new(ThemeOption::System, "theme"),
            player_backend: Property::new(PlayerBackend::Mpv, "player_backend"),
            is_loading: Property::new(false, "is_loading"),
            config,
            event_bus: RwLock::new(None),
        }
    }

    /// Get the theme property for subscription
    pub fn theme(&self) -> &Property<ThemeOption> {
        &self.theme
    }

    /// Get the player backend property for subscription
    pub fn player_backend(&self) -> &Property<PlayerBackend> {
        &self.player_backend
    }

    /// Get the loading state property
    pub fn is_loading(&self) -> &Property<bool> {
        &self.is_loading
    }

    /// Load current configuration values into properties
    pub async fn load_current_config(&self) {
        self.is_loading.set(true).await;

        let config = self.config.read().await;

        let theme = ThemeOption::from_config_value(&config.general.theme);
        let backend = PlayerBackend::from_config_value(&config.playback.player_backend);

        info!("🔧 PreferencesViewModel: Loading current configuration");
        debug!(
            "🔧 PreferencesViewModel: Raw theme config: '{}'",
            config.general.theme
        );
        debug!(
            "🔧 PreferencesViewModel: Raw player backend config: '{}'",
            config.playback.player_backend
        );
        info!(
            "🔧 PreferencesViewModel: Parsed values - theme={:?}, backend={:?}",
            theme, backend
        );

        self.theme.set(theme).await;
        self.player_backend.set(backend).await;

        info!("✅ PreferencesViewModel: Configuration loaded successfully");
        self.is_loading.set(false).await;
    }

    /// Update theme setting
    pub async fn set_theme(&self, theme: ThemeOption) {
        info!("Setting theme to: {:?}", theme);

        let event_bus = self.event_bus.read().await.clone();
        let config_arc = self.config.clone();
        let theme_value = theme.to_config_value().to_string();
        let theme_prop = self.theme.clone();

        // Update property immediately for reactive UI
        theme_prop.set(theme).await;

        // Save to config in background
        tokio::spawn(async move {
            let mut config = config_arc.write().await;
            let old_theme = config.general.theme.clone();
            config.general.theme = theme_value.clone();

            if let Err(e) = config.save() {
                error!("Failed to save theme preference: {}", e);
            } else if old_theme != theme_value {
                info!("Theme saved successfully: {} -> {}", old_theme, theme_value);

                // Emit UserPreferencesChanged event
                if let Some(bus) = event_bus {
                    let event = crate::events::types::DatabaseEvent::new(
                        EventType::UserPreferencesChanged,
                        EventPayload::User {
                            user_id: "local_user".to_string(),
                            action: format!("theme_changed_to_{}", theme_value),
                        },
                    );

                    if let Err(e) = bus.publish(event).await {
                        tracing::warn!("Failed to publish UserPreferencesChanged event: {}", e);
                    }
                }
            }
        });
    }

    /// Update player backend setting
    pub async fn set_player_backend(&self, backend: PlayerBackend) {
        info!("🎬 PreferencesViewModel: Player backend change requested");
        debug!(
            "🎬 PreferencesViewModel: New backend: {:?} ({})",
            backend,
            backend.to_config_value()
        );
        debug!(
            "🎬 PreferencesViewModel: Display name: {}",
            backend.display_name()
        );
        debug!("🎬 PreferencesViewModel: UI index: {}", backend.index());

        let event_bus = self.event_bus.read().await.clone();
        let config_arc = self.config.clone();
        let backend_value = backend.to_config_value().to_string();
        let backend_prop = self.player_backend.clone();
        let backend_clone = backend.clone();

        // Update property immediately for reactive UI
        debug!("🎬 PreferencesViewModel: Updating reactive property immediately");
        backend_prop.set(backend).await;
        info!(
            "✅ PreferencesViewModel: UI property updated to {:?}",
            backend_clone
        );

        // Log the updated state
        debug!("🎬 PreferencesViewModel: State after property update:");
        self.log_current_state().await;

        // Save to config in background
        info!("🎬 PreferencesViewModel: Starting background config save task");
        tokio::spawn(async move {
            debug!("🎬 PreferencesViewModel: Acquiring config write lock");
            let mut config = config_arc.write().await;
            let old_backend = config.playback.player_backend.clone();

            info!(
                "🎬 PreferencesViewModel: Config change - '{}' -> '{}'",
                old_backend, backend_value
            );

            if old_backend == backend_value {
                info!("⚠️ PreferencesViewModel: Backend unchanged, skipping save");
                return;
            }

            config.playback.player_backend = backend_value.clone();
            debug!("🎬 PreferencesViewModel: Config updated in memory, attempting to save to disk");

            if let Err(e) = config.save() {
                error!(
                    "❌ PreferencesViewModel: Failed to save player backend preference: {}",
                    e
                );
                warn!(
                    "❌ PreferencesViewModel: Backend change not persisted! Old: '{}', Attempted: '{}'",
                    old_backend, backend_value
                );
            } else {
                info!("✅ PreferencesViewModel: Player backend config saved to disk successfully");
                info!(
                    "🎬 PreferencesViewModel: Backend transition complete: {} -> {}",
                    old_backend, backend_value
                );

                // Additional logging for troubleshooting
                debug!(
                    "🎬 PreferencesViewModel: Config file should now contain: player_backend = '{}'",
                    backend_value
                );
                if backend_value == "mpv" {
                    info!(
                        "🎬 PreferencesViewModel: MPV selected - should provide better subtitle rendering"
                    );
                } else if backend_value == "gstreamer" {
                    info!(
                        "🎬 PreferencesViewModel: GStreamer selected - may have subtitle color artifacts"
                    );
                }

                // Emit UserPreferencesChanged event
                if let Some(bus) = event_bus {
                    debug!("🎬 PreferencesViewModel: Publishing UserPreferencesChanged event");
                    let event = crate::events::types::DatabaseEvent::new(
                        EventType::UserPreferencesChanged,
                        EventPayload::User {
                            user_id: "local_user".to_string(),
                            action: format!("player_backend_changed_to_{}", backend_value),
                        },
                    );

                    if let Err(e) = bus.publish(event).await {
                        warn!(
                            "⚠️ PreferencesViewModel: Failed to publish UserPreferencesChanged event: {}",
                            e
                        );
                    } else {
                        info!(
                            "✅ PreferencesViewModel: UserPreferencesChanged event published successfully"
                        );
                        debug!(
                            "🎬 PreferencesViewModel: Other components should now be notified of the backend change"
                        );
                    }
                } else {
                    warn!(
                        "⚠️ PreferencesViewModel: No event bus available, components won't be notified of change"
                    );
                }
            }
        });
    }

    /// Get available theme options
    pub fn get_theme_options(&self) -> Vec<ThemeOption> {
        vec![ThemeOption::System, ThemeOption::Light, ThemeOption::Dark]
    }

    /// Get available player backend options
    pub fn get_player_backend_options(&self) -> Vec<PlayerBackend> {
        vec![PlayerBackend::GStreamer, PlayerBackend::Mpv]
    }

    /// Debug helper to log current preferences state
    pub async fn log_current_state(&self) {
        let theme = self.theme.get().await;
        let backend = self.player_backend.get().await;
        let is_loading = self.is_loading.get().await;

        info!("🔍 PreferencesViewModel State:");
        info!(
            "  Theme: {:?} (config: '{}', index: {})",
            theme,
            theme.to_config_value(),
            theme.index()
        );
        info!(
            "  Player Backend: {:?} (config: '{}', index: {})",
            backend,
            backend.to_config_value(),
            backend.index()
        );
        info!("  Display Name: {}", backend.display_name());
        info!("  Is Loading: {}", is_loading);

        // Also log the actual config values for comparison
        let config = self.config.read().await;
        debug!("🔍 Raw Config Values:");
        debug!("  config.general.theme: '{}'", config.general.theme);
        debug!(
            "  config.playback.player_backend: '{}'",
            config.playback.player_backend
        );
    }
}

#[async_trait::async_trait]
impl ViewModel for PreferencesViewModel {
    async fn initialize(&self, event_bus: Arc<EventBus>) {
        info!("🚀 PreferencesViewModel: Starting initialization");

        // Store event bus reference
        *self.event_bus.write().await = Some(event_bus);
        debug!("✅ PreferencesViewModel: Event bus reference stored");

        // Load current configuration
        self.load_current_config().await;

        // Log the loaded state for debugging
        self.log_current_state().await;

        info!("✅ PreferencesViewModel: Initialization completed");

        // Note: We don't need to subscribe to events for preferences
        // since we're the ones generating the UserPreferencesChanged events
    }

    fn subscribe_to_property(&self, property_name: &str) -> Option<PropertySubscriber> {
        match property_name {
            "theme" => Some(self.theme.subscribe()),
            "player_backend" => Some(self.player_backend.subscribe()),
            "is_loading" => Some(self.is_loading.subscribe()),
            _ => None,
        }
    }

    async fn refresh(&self) {
        self.load_current_config().await;
    }
}
