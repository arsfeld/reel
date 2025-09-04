use super::{Property, PropertySubscriber, ViewModel};
use crate::config::Config;
use crate::events::{DatabaseEvent, EventBus, EventPayload, EventType};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

#[derive(Debug, Clone, PartialEq)]
pub enum Theme {
    System,
    Light,
    Dark,
}

impl Theme {
    pub fn from_string(s: &str) -> Self {
        match s {
            "light" => Theme::Light,
            "dark" => Theme::Dark,
            _ => Theme::System,
        }
    }

    pub fn to_string(&self) -> &'static str {
        match self {
            Theme::System => "auto",
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }

    pub fn to_index(&self) -> u32 {
        match self {
            Theme::System => 0,
            Theme::Light => 1,
            Theme::Dark => 2,
        }
    }

    pub fn from_index(index: u32) -> Self {
        match index {
            1 => Theme::Light,
            2 => Theme::Dark,
            _ => Theme::System,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Theme::System => "System",
            Theme::Light => "Light",
            Theme::Dark => "Dark",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlayerBackend {
    GStreamer,
    Mpv,
}

impl PlayerBackend {
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "gstreamer" => PlayerBackend::GStreamer,
            _ => PlayerBackend::Mpv,
        }
    }

    pub fn to_string(&self) -> &'static str {
        match self {
            PlayerBackend::GStreamer => "gstreamer",
            PlayerBackend::Mpv => "mpv",
        }
    }

    pub fn to_index(&self) -> u32 {
        match self {
            PlayerBackend::GStreamer => 0,
            PlayerBackend::Mpv => 1,
        }
    }

    pub fn from_index(index: u32) -> Self {
        match index {
            0 => PlayerBackend::GStreamer,
            _ => PlayerBackend::Mpv,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            PlayerBackend::GStreamer => "GStreamer",
            PlayerBackend::Mpv => "MPV",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum VideoOutput {
    Embedded,
    ExternalHdr,
}

impl VideoOutput {
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "gpu-next" => VideoOutput::ExternalHdr,
            _ => VideoOutput::Embedded,
        }
    }

    pub fn to_string(&self) -> &'static str {
        match self {
            VideoOutput::Embedded => "libmpv",
            VideoOutput::ExternalHdr => "gpu-next",
        }
    }

    pub fn to_index(&self) -> u32 {
        match self {
            VideoOutput::Embedded => 0,
            VideoOutput::ExternalHdr => 1,
        }
    }

    pub fn from_index(index: u32) -> Self {
        match index {
            1 => VideoOutput::ExternalHdr,
            _ => VideoOutput::Embedded,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            VideoOutput::Embedded => "Embedded",
            VideoOutput::ExternalHdr => "External HDR",
        }
    }
}

pub struct PreferencesViewModel {
    // Properties
    theme: Property<Theme>,
    player_backend: Property<PlayerBackend>,
    video_output: Property<VideoOutput>,
    is_loading: Property<bool>,

    // Services
    config: Arc<RwLock<Config>>,
    event_bus: RwLock<Option<Arc<EventBus>>>,
}

impl std::fmt::Debug for PreferencesViewModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreferencesViewModel")
            .field("theme", &self.theme)
            .field("player_backend", &self.player_backend)
            .field("video_output", &self.video_output)
            .field("is_loading", &self.is_loading)
            .finish()
    }
}

impl PreferencesViewModel {
    pub fn new(config: Arc<RwLock<Config>>) -> Self {
        Self {
            theme: Property::new(Theme::System, "theme"),
            player_backend: Property::new(PlayerBackend::Mpv, "player_backend"),
            video_output: Property::new(VideoOutput::Embedded, "video_output"),
            is_loading: Property::new(false, "is_loading"),
            config,
            event_bus: RwLock::new(None),
        }
    }

    /// Load current preferences from config
    pub async fn load_preferences(&self) {
        self.is_loading.set(true).await;

        let config = self.config.read().await;

        let theme = Theme::from_string(&config.general.theme);
        let player_backend = PlayerBackend::from_string(&config.playback.player_backend);
        let video_output = VideoOutput::from_string(&config.playback.mpv_video_output);

        debug!(
            "Loading preferences: theme={:?}, player_backend={:?}, video_output={:?}",
            theme, player_backend, video_output
        );

        self.theme.set(theme).await;
        self.player_backend.set(player_backend).await;
        self.video_output.set(video_output).await;

        self.is_loading.set(false).await;
    }

    /// Update theme preference
    pub async fn set_theme(&self, theme: Theme) -> anyhow::Result<()> {
        info!("Setting theme to: {:?}", theme);

        self.theme.set(theme.clone()).await;

        // Update config
        {
            let mut config = self.config.write().await;
            config.general.theme = theme.to_string().to_string();
            config.save()?;
        }

        // Emit event
        self.emit_preferences_changed_event("theme", theme.to_string())
            .await;

        Ok(())
    }

    /// Update player backend preference
    pub async fn set_player_backend(&self, backend: PlayerBackend) -> anyhow::Result<()> {
        info!("Setting player backend to: {:?}", backend);

        self.player_backend.set(backend.clone()).await;

        // Update config
        {
            let mut config = self.config.write().await;
            config.playback.player_backend = backend.to_string().to_string();
            config.save()?;
        }

        // Emit event
        self.emit_preferences_changed_event("player_backend", backend.to_string())
            .await;

        Ok(())
    }

    /// Update video output preference
    pub async fn set_video_output(&self, video_output: VideoOutput) -> anyhow::Result<()> {
        info!("Setting video output to: {:?}", video_output);

        self.video_output.set(video_output.clone()).await;

        // Update config
        {
            let mut config = self.config.write().await;
            config.playback.mpv_video_output = video_output.to_string().to_string();
            config.save()?;
        }

        // Emit event
        self.emit_preferences_changed_event("video_output", video_output.to_string())
            .await;

        Ok(())
    }

    /// Get current theme
    pub async fn get_theme(&self) -> Theme {
        self.theme.get().await
    }

    /// Get current player backend
    pub async fn get_player_backend(&self) -> PlayerBackend {
        self.player_backend.get().await
    }

    /// Get current video output
    pub async fn get_video_output(&self) -> VideoOutput {
        self.video_output.get().await
    }

    /// Get loading state
    pub async fn is_loading(&self) -> bool {
        self.is_loading.get().await
    }

    /// Subscribe to theme property
    pub fn subscribe_theme(&self) -> PropertySubscriber {
        self.theme.subscribe()
    }

    /// Subscribe to player backend property
    pub fn subscribe_player_backend(&self) -> PropertySubscriber {
        self.player_backend.subscribe()
    }

    /// Subscribe to video output property
    pub fn subscribe_video_output(&self) -> PropertySubscriber {
        self.video_output.subscribe()
    }

    /// Subscribe to loading property
    pub fn subscribe_loading(&self) -> PropertySubscriber {
        self.is_loading.subscribe()
    }

    async fn emit_preferences_changed_event(&self, setting: &str, value: &str) {
        if let Some(event_bus) = self.event_bus.read().await.as_ref() {
            let event = DatabaseEvent::new(
                EventType::UserPreferencesChanged,
                EventPayload::User {
                    user_id: "local_user".to_string(),
                    action: format!("{}_changed_to_{}", setting, value),
                },
            );

            if let Err(e) = event_bus.publish(event).await {
                error!("Failed to publish UserPreferencesChanged event: {}", e);
            }
        }
    }
}

#[async_trait::async_trait]
impl ViewModel for PreferencesViewModel {
    async fn initialize(&self, event_bus: Arc<EventBus>) {
        *self.event_bus.write().await = Some(event_bus);

        // Load initial preferences
        self.load_preferences().await;

        info!("PreferencesViewModel initialized");
    }

    fn subscribe_to_property(&self, property_name: &str) -> Option<PropertySubscriber> {
        match property_name {
            "theme" => Some(self.theme.subscribe()),
            "player_backend" => Some(self.player_backend.subscribe()),
            "video_output" => Some(self.video_output.subscribe()),
            "is_loading" => Some(self.is_loading.subscribe()),
            _ => None,
        }
    }

    async fn refresh(&self) {
        self.load_preferences().await;
    }

    fn dispose(&self) {
        debug!("PreferencesViewModel disposed");
    }
}
