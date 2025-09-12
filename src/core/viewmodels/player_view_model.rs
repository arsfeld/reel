use super::{Property, PropertySubscriber, ViewModel};
use crate::core::AppState;
use crate::events::{DatabaseEvent, EventBus, EventFilter, EventPayload, EventType};
use crate::models::{ChapterMarker, Episode, MediaItem, StreamInfo};
use crate::services::DataService;
use anyhow::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{error, warn};

#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioTrack {
    pub id: i32,
    pub name: String,
    pub language: Option<String>,
    pub codec: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SubtitleTrack {
    pub id: i32,
    pub name: String,
    pub language: Option<String>,
    pub forced: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QualityOption {
    pub id: String,
    pub name: String,
    pub bitrate: u32,
    pub resolution: String,
}

#[derive(Debug)]
pub struct PlayerViewModel {
    data_service: Arc<DataService>,
    app_state: Arc<AppState>,
    current_media: Property<Option<MediaItem>>,
    playback_state: Property<PlaybackState>,
    position: Property<Duration>,
    duration: Property<Duration>,
    volume: Property<f64>,
    playback_rate: Property<f64>,
    is_muted: Property<bool>,
    is_fullscreen: Property<bool>,
    is_loading: Property<bool>,
    error: Property<Option<String>>,
    // Newly added data props
    stream_info: Property<Option<StreamInfo>>,
    markers: Property<(Option<ChapterMarker>, Option<ChapterMarker>)>,
    next_episode: Property<Option<Episode>>,
    auto_play_state: Property<AutoPlayState>,
    playlist: Property<Vec<MediaItem>>,
    playlist_index: Property<usize>,
    show_controls: Property<bool>,
    subtitles_enabled: Property<bool>,
    audio_track: Property<i32>,
    subtitle_track: Property<i32>,
    // Track management properties
    audio_tracks: Property<Vec<AudioTrack>>,
    subtitle_tracks: Property<Vec<SubtitleTrack>>,
    selected_audio_track: Property<Option<usize>>,
    selected_subtitle_track: Property<Option<usize>>,
    quality_options: Property<Vec<QualityOption>>,
    selected_quality: Property<Option<usize>>,
    // Enhanced next episode properties
    next_episode_thumbnail: Property<Option<Vec<u8>>>, // Raw image data
    auto_play_enabled: Property<bool>,
    auto_play_countdown_duration: Property<u32>, // Configurable countdown (5-30 seconds)
    next_episode_load_state: Property<LoadState>,
    event_bus: Arc<EventBus>,
    // Throttle state
    last_progress_save: Arc<Mutex<Option<Instant>>>,
    // Countdown timer handle
    countdown_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum AutoPlayState {
    Idle,
    Counting(u32), // Seconds remaining
    Disabled,
    Loading,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum LoadState {
    Idle,
    Loading,
    Ready,
    Error(String),
}

#[derive(Debug, Clone, Default, PartialEq)]
#[allow(dead_code)]
pub struct NextEpisodeInfo {
    pub title: String,
    pub show_title: String,
    pub season_episode: String,
    pub duration: String,
    pub summary: String,
}

impl PlayerViewModel {
    pub fn new(
        data_service: Arc<DataService>,
        app_state: Arc<AppState>,
        event_bus: Arc<EventBus>,
    ) -> Self {
        Self {
            data_service,
            app_state,
            event_bus,
            current_media: Property::new(None, "current_media"),
            playback_state: Property::new(PlaybackState::Stopped, "playback_state"),
            position: Property::new(Duration::ZERO, "position"),
            duration: Property::new(Duration::ZERO, "duration"),
            volume: Property::new(1.0, "volume"),
            playback_rate: Property::new(1.0, "playback_rate"),
            is_muted: Property::new(false, "is_muted"),
            is_fullscreen: Property::new(false, "is_fullscreen"),
            is_loading: Property::new(false, "is_loading"),
            error: Property::new(None, "error"),
            stream_info: Property::new(None, "stream_info"),
            markers: Property::new((None, None), "markers"),
            next_episode: Property::new(None, "next_episode"),
            auto_play_state: Property::new(AutoPlayState::Idle, "auto_play_state"),
            playlist: Property::new(Vec::new(), "playlist"),
            playlist_index: Property::new(0, "playlist_index"),
            show_controls: Property::new(true, "show_controls"),
            subtitles_enabled: Property::new(false, "subtitles_enabled"),
            audio_track: Property::new(0, "audio_track"),
            subtitle_track: Property::new(-1, "subtitle_track"),
            // Track management properties
            audio_tracks: Property::new(Vec::new(), "audio_tracks"),
            subtitle_tracks: Property::new(Vec::new(), "subtitle_tracks"),
            selected_audio_track: Property::new(None, "selected_audio_track"),
            selected_subtitle_track: Property::new(None, "selected_subtitle_track"),
            quality_options: Property::new(Vec::new(), "quality_options"),
            selected_quality: Property::new(None, "selected_quality"),
            // Enhanced next episode properties
            next_episode_thumbnail: Property::new(None, "next_episode_thumbnail"),
            auto_play_enabled: Property::new(true, "auto_play_enabled"), // Default to enabled
            auto_play_countdown_duration: Property::new(10, "auto_play_countdown_duration"), // Default 10 seconds
            next_episode_load_state: Property::new(LoadState::Idle, "next_episode_load_state"),
            last_progress_save: Arc::new(Mutex::new(None)),
            countdown_handle: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn set_media_item(&self, media: MediaItem) {
        self.current_media.set(Some(media)).await;
    }

    pub async fn load_stream_and_metadata(&self) -> Result<()> {
        self.is_loading.set(true).await;
        self.error.set(None).await;

        let media = match self.current_media.get().await {
            Some(m) => m,
            None => {
                let msg = "No current media set".to_string();
                self.error.set(Some(msg.clone())).await;
                self.is_loading.set(false).await;
                return Err(anyhow::anyhow!(msg));
            }
        };

        let backend_id = media.backend_id();
        let media_id = media.id().to_string();

        // Resolve backend
        let backend = self
            .app_state
            .source_coordinator
            .get_backend(backend_id)
            .await;

        let Some(backend) = backend else {
            let msg = format!("Backend not found for ID: {}", backend_id);
            self.error.set(Some(msg.clone())).await;
            self.is_loading.set(false).await;
            return Err(anyhow::anyhow!(msg));
        };

        // Fetch stream info
        match backend.get_stream_url(&media_id).await {
            Ok(info) => {
                self.stream_info.set(Some(info)).await;
            }
            Err(e) => {
                let es = e.to_string();
                let friendly = if es.contains("401") || es.to_lowercase().contains("unauthorized") {
                    "Authentication failed. Please re-add your account."
                } else if es.contains("404") {
                    "Media not found on server. It may have been deleted."
                } else if es.to_lowercase().contains("connection")
                    || es.to_lowercase().contains("timed out")
                {
                    "Cannot connect to server. Check if the server is running and accessible."
                } else {
                    "Failed to load media from server"
                };
                self.error.set(Some(friendly.to_string())).await;
                self.is_loading.set(false).await;
                return Err(e);
            }
        }

        // Fetch markers (optional; best-effort)
        match backend.fetch_media_markers(&media_id).await {
            Ok(tuple) => {
                self.markers.set(tuple).await;
            }
            Err(e) => {
                warn!("Failed to fetch markers for {}: {}", media_id, e);
                // Keep markers as (None, None)
                self.markers.set((None, None)).await;
            }
        }

        self.is_loading.set(false).await;
        Ok(())
    }

    pub async fn find_next_episode(&self) -> Result<Option<Episode>> {
        let Some(MediaItem::Episode(ref ep)) = self.current_media.get().await else {
            return Ok(None);
        };

        let backend_id = ep.backend_id.as_str();
        let backend = self
            .app_state
            .source_coordinator
            .get_backend(backend_id)
            .await;

        let Some(backend) = backend else {
            return Ok(None);
        };
        match backend.find_next_episode(ep).await {
            Ok(next) => {
                self.next_episode.set(next.clone()).await;
                Ok(next)
            }
            Err(e) => {
                warn!("Failed to resolve next episode for {}: {}", ep.id, e);
                Ok(None)
            }
        }
    }

    #[allow(dead_code)]
    pub async fn load_next_episode_metadata(&self) -> Result<()> {
        self.next_episode_load_state.set(LoadState::Loading).await;

        // Get next episode from current media context
        if let Some(MediaItem::Episode(ref _current)) = self.current_media.get().await {
            match self.find_next_episode().await {
                Ok(Some(next)) => {
                    // Pre-load thumbnail if available
                    if let Some(thumb_url) = &next.thumbnail_url {
                        if let Err(e) = self.load_episode_thumbnail(thumb_url).await {
                            warn!("Failed to load next episode thumbnail: {}", e);
                        }
                    }

                    self.next_episode_load_state.set(LoadState::Ready).await;
                }
                Ok(None) => {
                    self.next_episode_load_state.set(LoadState::Idle).await;
                }
                Err(e) => {
                    self.next_episode_load_state
                        .set(LoadState::Error(e.to_string()))
                        .await;
                }
            }
        }

        Ok(())
    }

    async fn load_episode_thumbnail(&self, url: &str) -> Result<()> {
        // Fetch thumbnail using reqwest
        let response = reqwest::get(url).await?;
        let bytes = response.bytes().await?;
        self.next_episode_thumbnail.set(Some(bytes.to_vec())).await;
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn play_next_episode_now(&self) {
        // Cancel any active countdown
        if let Some(handle) = self.countdown_handle.lock().await.take() {
            handle.abort();
        }
        self.auto_play_state.set(AutoPlayState::Idle).await;

        // Navigate to next episode
        if let Some(next) = self.next_episode.get().await {
            // Convert Episode to MediaItem::Episode
            let media_item = MediaItem::Episode(next);
            self.set_media_item(media_item).await;
            // The player page should handle reloading the stream
        }
    }

    #[allow(dead_code)]
    pub async fn cancel_auto_play(&self) {
        // Cancel countdown timer
        if let Some(handle) = self.countdown_handle.lock().await.take() {
            handle.abort();
        }
        self.auto_play_state.set(AutoPlayState::Idle).await;
    }

    #[allow(dead_code)]
    pub async fn toggle_auto_play(&self) {
        let enabled = !self.auto_play_enabled.get().await;
        self.auto_play_enabled.set(enabled).await;

        // TODO: Save preference to settings

        // Cancel current countdown if disabling
        if !enabled && matches!(self.auto_play_state.get().await, AutoPlayState::Counting(_)) {
            self.cancel_auto_play().await;
        }
    }

    #[allow(dead_code)]
    pub async fn start_auto_play_countdown(&self, duration_seconds: u32) {
        // Cancel any existing countdown
        if let Some(handle) = self.countdown_handle.lock().await.take() {
            handle.abort();
        }

        // Start countdown
        self.auto_play_state
            .set(AutoPlayState::Counting(duration_seconds))
            .await;

        let auto_play_state = self.auto_play_state.clone();
        let next_episode = self.next_episode.clone();
        let current_media = self.current_media.clone();

        let handle = tokio::spawn(async move {
            for remaining in (1..=duration_seconds).rev() {
                auto_play_state
                    .set(AutoPlayState::Counting(remaining))
                    .await;
                tokio::time::sleep(Duration::from_secs(1)).await;
            }

            // Countdown complete - play next episode
            if let Some(next) = next_episode.get().await {
                let media_item = MediaItem::Episode(next);
                current_media.set(Some(media_item)).await;
                auto_play_state.set(AutoPlayState::Idle).await;
            }
        });

        *self.countdown_handle.lock().await = Some(handle);
    }

    #[allow(dead_code)]
    pub async fn handle_playback_near_end(&self) {
        // Check if we're within last 30 seconds
        let position = self.position.get().await;
        let duration = self.duration.get().await;

        if duration > Duration::ZERO {
            let remaining = duration - position;

            if remaining <= Duration::from_secs(30)
                && self.auto_play_enabled.get().await
                && self.next_episode.get().await.is_some()
            {
                // Start showing overlay with countdown
                let countdown_duration = self.auto_play_countdown_duration.get().await;
                self.start_auto_play_countdown(countdown_duration).await;
            }
        }
    }

    #[allow(dead_code)]
    pub async fn handle_playback_completed(&self) {
        self.playback_state.set(PlaybackState::Stopped).await;

        // If auto-play is disabled but there's a next episode, show overlay without countdown
        if !self.auto_play_enabled.get().await && self.next_episode.get().await.is_some() {
            self.auto_play_state.set(AutoPlayState::Disabled).await;
        }
    }

    pub async fn save_progress_throttled(&self, id: &str, position: Duration, duration: Duration) {
        // Debounce to once every ~2s
        let mut last = self.last_progress_save.lock().await;
        let now = Instant::now();
        if last
            .map(|t| now.duration_since(t) < Duration::from_secs(2))
            .unwrap_or(false)
        {
            return;
        }
        *last = Some(now);

        let position_ms = position.as_millis() as i64;
        let duration_ms = duration.as_millis() as i64;
        let watched = if duration > Duration::ZERO {
            position.as_secs_f64() / duration.as_secs_f64() > 0.9
        } else {
            false
        };

        let _ = self
            .data_service
            .update_playback_progress(id, position_ms, duration_ms, watched)
            .await;
    }

    pub async fn load_media(&self, media_id: String) -> Result<()> {
        self.is_loading.set(true).await;
        self.error.set(None).await;

        match self.data_service.get_media_item(&media_id).await {
            Ok(Some(media)) => {
                self.current_media.set(Some(media.clone())).await;

                // Extract duration from media item based on type
                let duration = match &media {
                    MediaItem::Movie(m) => Some(m.duration),
                    MediaItem::Episode(e) => Some(e.duration),
                    MediaItem::MusicTrack(t) => Some(t.duration),
                    _ => None,
                };

                if let Some(duration) = duration {
                    self.duration.set(duration).await;
                }

                if let Ok(Some((position_ms, _duration_ms))) =
                    self.data_service.get_playback_progress(&media_id).await
                {
                    self.position.set(Duration::from_millis(position_ms)).await;
                }

                self.is_loading.set(false).await;
                Ok(())
            }
            Ok(None) => {
                let msg = format!("Media item {} not found", media_id);
                self.error.set(Some(msg.clone())).await;
                self.is_loading.set(false).await;
                Err(anyhow::anyhow!(msg))
            }
            Err(e) => {
                error!("Failed to load media: {}", e);
                self.error.set(Some(e.to_string())).await;
                self.is_loading.set(false).await;
                Err(e)
            }
        }
    }

    pub async fn play(&self) {
        if self.current_media.get().await.is_some() {
            let previous_state = self.playback_state.get().await;
            self.playback_state.set(PlaybackState::Playing).await;

            // Emit appropriate event based on previous state
            match previous_state {
                PlaybackState::Paused => {
                    self.emit_playback_event(EventType::PlaybackResumed).await;
                }
                _ => {
                    self.emit_playback_event(EventType::PlaybackStarted).await;
                }
            }
        }
    }

    pub async fn pause(&self) {
        if matches!(self.playback_state.get().await, PlaybackState::Playing) {
            self.playback_state.set(PlaybackState::Paused).await;
            self.emit_playback_event(EventType::PlaybackPaused).await;
            self.save_progress().await;
        }
    }

    pub async fn stop(&self) {
        self.playback_state.set(PlaybackState::Stopped).await;
        self.is_loading.set(false).await; // Clear loading state to allow new media
        self.emit_playback_event(EventType::PlaybackStopped).await;
        self.save_progress().await;
        self.position.set(Duration::ZERO).await;
    }

    async fn save_progress(&self) {
        if let Some(media) = self.current_media.get().await {
            let position = self.position.get().await;
            let duration = self.duration.get().await;

            let position_ms = position.as_millis() as i64;
            let duration_ms = duration.as_millis() as i64;
            let watched = position.as_secs_f64() / duration.as_secs_f64() > 0.9;

            let _ = self
                .data_service
                .update_playback_progress(media.id(), position_ms, duration_ms, watched)
                .await;
        }
    }

    async fn emit_playback_event(&self, event_type: EventType) {
        if let Some(media) = self.current_media.get().await {
            let position = self.position.get().await;
            let duration = self.duration.get().await;

            let event = DatabaseEvent::new(
                event_type,
                EventPayload::Playback {
                    media_id: media.id().to_string(),
                    position: Some(position),
                    duration: Some(duration),
                },
            );

            let _ = self.event_bus.publish(event).await;
        }
    }

    async fn handle_event(&self, event: DatabaseEvent) {
        match event.event_type {
            EventType::MediaUpdated | EventType::MediaDeleted => {
                if let Some(current) = self.current_media.get().await
                    && let EventPayload::Media { id, .. } = event.payload
                    && id == current.id()
                {
                    if event.event_type == EventType::MediaDeleted {
                        self.stop().await;
                        self.current_media.set(None).await;
                    } else {
                        let _ = self.load_media(id).await;
                    }
                }
            }
            EventType::SourceUpdated | EventType::SourceOnlineStatusChanged => {
                // If we have pending media to load and a source just came online, retry loading
                if let Some(current) = self.current_media.get().await {
                    let backend_id = current.backend_id();

                    // Check if this event is for the backend we're waiting for
                    if let EventPayload::Source { id, .. } = &event.payload {
                        if id == backend_id && self.is_loading.get().await {
                            // Check if the source is now connected
                            if let Some(status) = self
                                .app_state
                                .source_coordinator
                                .get_source_status(backend_id)
                                .await
                            {
                                match &status.connection_status {
                                    crate::services::source_coordinator::ConnectionStatus::Connected => {
                                        tracing::info!("PlayerViewModel: Backend {} is now connected, retrying media load", backend_id);
                                        // Retry loading the stream and metadata
                                        if let Err(e) = self.load_stream_and_metadata().await {
                                            self.error.set(Some(format!("Failed to load media after backend connected: {}", e))).await;
                                        }
                                    }
                                    _ => {
                                        // Backend still not ready or has errors - will be handled by UI
                                        tracing::debug!("PlayerViewModel: Backend {} status changed but not connected yet", backend_id);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    pub fn current_media(&self) -> &Property<Option<MediaItem>> {
        &self.current_media
    }

    pub fn playback_state(&self) -> &Property<PlaybackState> {
        &self.playback_state
    }

    pub fn position(&self) -> &Property<Duration> {
        &self.position
    }

    pub fn duration(&self) -> &Property<Duration> {
        &self.duration
    }

    pub fn volume(&self) -> &Property<f64> {
        &self.volume
    }

    pub fn is_muted(&self) -> &Property<bool> {
        &self.is_muted
    }

    pub fn is_loading(&self) -> &Property<bool> {
        &self.is_loading
    }

    pub fn error(&self) -> &Property<Option<String>> {
        &self.error
    }

    pub fn stream_info(&self) -> &Property<Option<StreamInfo>> {
        &self.stream_info
    }

    pub fn markers(&self) -> &Property<(Option<ChapterMarker>, Option<ChapterMarker>)> {
        &self.markers
    }

    pub fn next_episode(&self) -> &Property<Option<Episode>> {
        &self.next_episode
    }

    pub fn audio_tracks(&self) -> &Property<Vec<AudioTrack>> {
        &self.audio_tracks
    }

    pub fn subtitle_tracks(&self) -> &Property<Vec<SubtitleTrack>> {
        &self.subtitle_tracks
    }

    #[allow(dead_code)]
    pub fn selected_audio_track(&self) -> &Property<Option<usize>> {
        &self.selected_audio_track
    }

    #[allow(dead_code)]
    pub fn selected_subtitle_track(&self) -> &Property<Option<usize>> {
        &self.selected_subtitle_track
    }

    #[allow(dead_code)]
    pub fn quality_options(&self) -> &Property<Vec<QualityOption>> {
        &self.quality_options
    }

    #[allow(dead_code)]
    pub fn selected_quality(&self) -> &Property<Option<usize>> {
        &self.selected_quality
    }

    pub fn show_controls(&self) -> &Property<bool> {
        &self.show_controls
    }

    #[allow(dead_code)]
    pub fn auto_play_enabled(&self) -> &Property<bool> {
        &self.auto_play_enabled
    }

    #[allow(dead_code)]
    pub fn auto_play_state(&self) -> &Property<AutoPlayState> {
        &self.auto_play_state
    }

    #[allow(dead_code)]
    pub fn next_episode_thumbnail(&self) -> &Property<Option<Vec<u8>>> {
        &self.next_episode_thumbnail
    }

    #[allow(dead_code)]
    pub fn auto_play_countdown_duration(&self) -> &Property<u32> {
        &self.auto_play_countdown_duration
    }

    #[allow(dead_code)]
    pub fn next_episode_load_state(&self) -> &Property<LoadState> {
        &self.next_episode_load_state
    }

    pub async fn show_controls_temporarily(&self, delay_secs: u64) {
        // Show controls immediately
        self.show_controls.set(true).await;

        // Schedule auto-hide after delay
        let show_controls = self.show_controls.clone();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(delay_secs)).await;
            show_controls.set(false).await;
        });
    }

    #[allow(dead_code)]
    pub async fn toggle_controls_visibility(&self) {
        let visible = self.show_controls.get().await;
        if visible {
            self.show_controls.set(false).await;
        } else {
            // Show temporarily when toggling on
            self.show_controls_temporarily(crate::constants::PLAYER_CONTROLS_HIDE_DELAY_SECS)
                .await;
        }
    }

    #[allow(dead_code)]
    pub fn has_audio_tracks(&self) -> bool {
        // Simple sync check for computed property
        self.audio_tracks.get_sync().len() > 0
    }

    #[allow(dead_code)]
    pub fn has_subtitle_tracks(&self) -> bool {
        // Simple sync check for computed property
        self.subtitle_tracks.get_sync().len() > 0
    }

    pub async fn discover_tracks(&self, player: &crate::player::Player) -> Result<()> {
        // Get tracks from the player backend
        let audio_track_tuples = player.get_audio_tracks().await;
        let subtitle_track_tuples = player.get_subtitle_tracks().await;

        // Convert to our AudioTrack type
        let audio_tracks: Vec<AudioTrack> = audio_track_tuples
            .into_iter()
            .map(|(id, name)| {
                // Try to parse language from name (e.g., "English (5.1 AC3)")
                let (clean_name, language) = if name.contains('(') {
                    let parts: Vec<&str> = name.splitn(2, '(').collect();
                    (
                        parts[0].trim().to_string(),
                        Some(parts[0].trim().to_string()),
                    )
                } else {
                    (name.clone(), None)
                };
                AudioTrack {
                    id,
                    name: clean_name,
                    language,
                    codec: None, // Could be extracted from name if needed
                }
            })
            .collect();

        // Convert to our SubtitleTrack type
        let subtitle_tracks: Vec<SubtitleTrack> = subtitle_track_tuples
            .into_iter()
            .map(|(id, name)| {
                let forced = name.to_lowercase().contains("forced");
                let (clean_name, language) = if name.contains('(') {
                    let parts: Vec<&str> = name.splitn(2, '(').collect();
                    (
                        parts[0].trim().to_string(),
                        Some(parts[0].trim().to_string()),
                    )
                } else {
                    (name.clone(), None)
                };
                SubtitleTrack {
                    id,
                    name: clean_name,
                    language,
                    forced,
                }
            })
            .collect();

        // Update properties
        self.audio_tracks.set(audio_tracks).await;
        self.subtitle_tracks.set(subtitle_tracks).await;

        // Auto-select first audio track if available
        if self.audio_tracks.get().await.len() > 0 {
            self.selected_audio_track.set(Some(0)).await;
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn select_audio_track(
        &self,
        track_index: usize,
        player: &crate::player::Player,
    ) -> Result<()> {
        let tracks = self.audio_tracks.get().await;
        if let Some(track) = tracks.get(track_index) {
            // Update player backend
            player.set_audio_track(track.id).await?;

            // Update ViewModel state
            self.selected_audio_track.set(Some(track_index)).await;

            // Save preference (TODO: implement preference saving)
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn select_subtitle_track(
        &self,
        track_index: Option<usize>,
        player: &crate::player::Player,
    ) -> Result<()> {
        match track_index {
            Some(idx) => {
                let tracks = self.subtitle_tracks.get().await;
                if let Some(track) = tracks.get(idx) {
                    player.set_subtitle_track(track.id).await?;
                    self.selected_subtitle_track.set(Some(idx)).await;
                    self.subtitles_enabled.set(true).await;
                }
            }
            None => {
                // Disable subtitles
                player.set_subtitle_track(-1).await?;
                self.selected_subtitle_track.set(None).await;
                self.subtitles_enabled.set(false).await;
            }
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl ViewModel for PlayerViewModel {
    async fn initialize(&self, _event_bus: Arc<EventBus>) {
        // Subscribe using the non-optional event bus held by this VM
        let filter = EventFilter::new().with_types(vec![
            EventType::MediaUpdated,
            EventType::MediaDeleted,
            EventType::SourceUpdated,
            EventType::SourceOnlineStatusChanged,
        ]);

        let mut subscriber = self.event_bus.subscribe_filtered(filter);
        let self_clone = self.clone();

        tokio::spawn(async move {
            while let Ok(event) = subscriber.recv().await {
                self_clone.handle_event(event).await;
            }
        });
    }

    fn subscribe_to_property(&self, property_name: &str) -> Option<PropertySubscriber> {
        match property_name {
            "current_media" => Some(self.current_media.subscribe()),
            "playback_state" => Some(self.playback_state.subscribe()),
            "position" => Some(self.position.subscribe()),
            "duration" => Some(self.duration.subscribe()),
            "volume" => Some(self.volume.subscribe()),
            "is_fullscreen" => Some(self.is_fullscreen.subscribe()),
            "is_loading" => Some(self.is_loading.subscribe()),
            "error" => Some(self.error.subscribe()),
            "stream_info" => Some(self.stream_info.subscribe()),
            "markers" => Some(self.markers.subscribe()),
            "next_episode" => Some(self.next_episode.subscribe()),
            "auto_play_state" => Some(self.auto_play_state.subscribe()),
            "playlist" => Some(self.playlist.subscribe()),
            _ => None,
        }
    }

    async fn refresh(&self) {
        if let Some(media) = self.current_media.get().await {
            let _ = self.load_media(media.id().to_string()).await;
        }
    }
}

impl Clone for PlayerViewModel {
    fn clone(&self) -> Self {
        Self {
            data_service: self.data_service.clone(),
            app_state: self.app_state.clone(),
            current_media: self.current_media.clone(),
            playback_state: self.playback_state.clone(),
            position: self.position.clone(),
            duration: self.duration.clone(),
            volume: self.volume.clone(),
            playback_rate: self.playback_rate.clone(),
            is_muted: self.is_muted.clone(),
            is_fullscreen: self.is_fullscreen.clone(),
            is_loading: self.is_loading.clone(),
            error: self.error.clone(),
            stream_info: self.stream_info.clone(),
            markers: self.markers.clone(),
            next_episode: self.next_episode.clone(),
            auto_play_state: self.auto_play_state.clone(),
            playlist: self.playlist.clone(),
            playlist_index: self.playlist_index.clone(),
            show_controls: self.show_controls.clone(),
            subtitles_enabled: self.subtitles_enabled.clone(),
            audio_track: self.audio_track.clone(),
            subtitle_track: self.subtitle_track.clone(),
            audio_tracks: self.audio_tracks.clone(),
            subtitle_tracks: self.subtitle_tracks.clone(),
            selected_audio_track: self.selected_audio_track.clone(),
            selected_subtitle_track: self.selected_subtitle_track.clone(),
            quality_options: self.quality_options.clone(),
            selected_quality: self.selected_quality.clone(),
            // Enhanced next episode properties
            next_episode_thumbnail: self.next_episode_thumbnail.clone(),
            auto_play_enabled: self.auto_play_enabled.clone(),
            auto_play_countdown_duration: self.auto_play_countdown_duration.clone(),
            next_episode_load_state: self.next_episode_load_state.clone(),
            event_bus: self.event_bus.clone(),
            last_progress_save: self.last_progress_save.clone(),
            countdown_handle: self.countdown_handle.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_playback_state_equality() {
        // Test that PlaybackState variants work correctly for reactive comparison
        let playing = PlaybackState::Playing;
        let paused = PlaybackState::Paused;
        let stopped = PlaybackState::Stopped;

        assert_eq!(playing, PlaybackState::Playing);
        assert_ne!(playing, paused);
        assert_ne!(paused, stopped);

        // Test the icon mapping logic used in reactive binding
        let play_icon = match playing {
            PlaybackState::Playing => "media-playback-pause-symbolic",
            _ => "media-playback-start-symbolic",
        };

        let pause_icon = match paused {
            PlaybackState::Playing => "media-playback-pause-symbolic",
            _ => "media-playback-start-symbolic",
        };

        assert_eq!(play_icon, "media-playback-pause-symbolic");
        assert_eq!(pause_icon, "media-playback-start-symbolic");
    }

    #[test]
    fn test_playback_state_cloning() {
        // Test that PlaybackState can be cloned (needed for reactive properties)
        let original = PlaybackState::Playing;
        let cloned = original.clone();

        assert_eq!(original, cloned);
    }
}
