use super::{Property, PropertySubscriber, ViewModel};
use crate::events::{DatabaseEvent, EventBus, EventFilter, EventPayload, EventType};
use crate::models::MediaItem;
use crate::services::DataService;
use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info};

#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
    Buffering,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct PlaybackInfo {
    pub media_item: MediaItem,
    pub position: Duration,
    pub duration: Duration,
    pub volume: f64,
    pub playback_rate: f64,
    pub is_muted: bool,
}

pub struct PlayerViewModel {
    data_service: Arc<DataService>,
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
    playlist: Property<Vec<MediaItem>>,
    playlist_index: Property<usize>,
    show_controls: Property<bool>,
    subtitles_enabled: Property<bool>,
    audio_track: Property<i32>,
    subtitle_track: Property<i32>,
    event_bus: Option<Arc<EventBus>>,
}

impl PlayerViewModel {
    pub fn new(data_service: Arc<DataService>) -> Self {
        Self {
            data_service,
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
            playlist: Property::new(Vec::new(), "playlist"),
            playlist_index: Property::new(0, "playlist_index"),
            show_controls: Property::new(true, "show_controls"),
            subtitles_enabled: Property::new(false, "subtitles_enabled"),
            audio_track: Property::new(0, "audio_track"),
            subtitle_track: Property::new(-1, "subtitle_track"),
            event_bus: None,
        }
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

                if let Ok(Some((position_ms, duration_ms))) =
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
        self.emit_playback_event(EventType::PlaybackStopped).await;
        self.save_progress().await;
        self.position.set(Duration::ZERO).await;
    }

    pub async fn toggle_play_pause(&self) {
        match self.playback_state.get().await {
            PlaybackState::Playing => self.pause().await,
            PlaybackState::Paused | PlaybackState::Stopped => self.play().await,
            _ => {}
        }
    }

    pub async fn seek(&self, position: Duration) {
        self.position.set(position).await;
        self.emit_playback_event(EventType::PlaybackPositionUpdated)
            .await;
    }

    pub async fn set_volume(&self, volume: f64) {
        let clamped = volume.clamp(0.0, 1.0);
        self.volume.set(clamped).await;
        if clamped == 0.0 {
            self.is_muted.set(true).await;
        } else if self.is_muted.get().await {
            self.is_muted.set(false).await;
        }
    }

    pub async fn toggle_mute(&self) {
        let is_muted = !self.is_muted.get().await;
        self.is_muted.set(is_muted).await;
    }

    pub async fn set_playback_rate(&self, rate: f64) {
        let clamped = rate.clamp(0.25, 4.0);
        self.playback_rate.set(clamped).await;
    }

    pub async fn toggle_fullscreen(&self) {
        let fullscreen = !self.is_fullscreen.get().await;
        self.is_fullscreen.set(fullscreen).await;
    }

    pub async fn next(&self) {
        let playlist = self.playlist.get().await;
        let current_index = self.playlist_index.get().await;

        if current_index + 1 < playlist.len() {
            let next_index = current_index + 1;
            self.playlist_index.set(next_index).await;

            if let Some(next_item) = playlist.get(next_index) {
                let _ = self.load_media(next_item.id().to_string()).await;
                self.play().await;
            }
        }
    }

    pub async fn previous(&self) {
        let position = self.position.get().await;

        if position > Duration::from_secs(3) {
            self.seek(Duration::ZERO).await;
        } else {
            let current_index = self.playlist_index.get().await;

            if current_index > 0 {
                let prev_index = current_index - 1;
                self.playlist_index.set(prev_index).await;

                let playlist = self.playlist.get().await;
                if let Some(prev_item) = playlist.get(prev_index) {
                    let _ = self.load_media(prev_item.id().to_string()).await;
                    self.play().await;
                }
            } else {
                self.seek(Duration::ZERO).await;
            }
        }
    }

    pub async fn set_playlist(&self, items: Vec<MediaItem>, start_index: usize) {
        self.playlist.set(items.clone()).await;
        self.playlist_index.set(start_index).await;

        if let Some(item) = items.get(start_index) {
            let _ = self.load_media(item.id().to_string()).await;
        }
    }

    pub async fn update_position(&self, position: Duration) {
        self.position.set(position).await;

        let duration = self.duration.get().await;
        if duration > Duration::ZERO && position >= duration {
            self.playback_state.set(PlaybackState::Stopped).await;
            self.emit_playback_event(EventType::PlaybackCompleted).await;
            self.next().await;
        }
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
        if let (Some(event_bus), Some(media)) =
            (self.event_bus.as_ref(), self.current_media.get().await)
        {
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

            let _ = event_bus.publish(event).await;
        }
    }

    async fn handle_event(&self, event: DatabaseEvent) {
        match event.event_type {
            EventType::MediaUpdated | EventType::MediaDeleted => {
                if let Some(current) = self.current_media.get().await {
                    if let EventPayload::Media { id, .. } = event.payload {
                        if id == current.id() {
                            if event.event_type == EventType::MediaDeleted {
                                self.stop().await;
                                self.current_media.set(None).await;
                            } else {
                                let _ = self.load_media(id).await;
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

    pub fn is_fullscreen(&self) -> &Property<bool> {
        &self.is_fullscreen
    }

    pub fn playlist(&self) -> &Property<Vec<MediaItem>> {
        &self.playlist
    }
}

#[async_trait::async_trait]
impl ViewModel for PlayerViewModel {
    async fn initialize(&self, event_bus: Arc<EventBus>) {
        let filter =
            EventFilter::new().with_types(vec![EventType::MediaUpdated, EventType::MediaDeleted]);

        let mut subscriber = event_bus.subscribe_filtered(filter);
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
            "playlist" => Some(self.playlist.subscribe()),
            _ => None,
        }
    }

    async fn refresh(&self) {
        if let Some(media) = self.current_media.get().await {
            let _ = self.load_media(media.id().to_string()).await;
        }
    }

    fn dispose(&self) {}
}

impl Clone for PlayerViewModel {
    fn clone(&self) -> Self {
        Self {
            data_service: self.data_service.clone(),
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
            playlist: self.playlist.clone(),
            playlist_index: self.playlist_index.clone(),
            show_controls: self.show_controls.clone(),
            subtitles_enabled: self.subtitles_enabled.clone(),
            audio_track: self.audio_track.clone(),
            subtitle_track: self.subtitle_track.clone(),
            event_bus: self.event_bus.clone(),
        }
    }
}
