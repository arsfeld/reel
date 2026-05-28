use mpris_server::{LoopStatus, Metadata, PlaybackStatus, Time, TrackId};

use crate::models::media::MediaItem;
use crate::player::PlayState;

// --- Pure conversion functions (fully testable, no I/O) ---

/// Convert internal PlayState to MPRIS PlaybackStatus.
pub fn playback_status_from_state(state: PlayState) -> PlaybackStatus {
    match state {
        PlayState::Playing => PlaybackStatus::Playing,
        PlayState::Paused => PlaybackStatus::Paused,
        PlayState::Stopped => PlaybackStatus::Stopped,
    }
}

/// Convert seconds (f64, from mpv) to microseconds (i64, for MPRIS).
pub fn seconds_to_micros(seconds: f64) -> i64 {
    (seconds * 1_000_000.0) as i64
}

/// Convert microseconds (i64, from MPRIS) to seconds (f64, for mpv).
#[allow(dead_code)]
pub fn micros_to_seconds(micros: i64) -> f64 {
    micros as f64 / 1_000_000.0
}

/// Data used to build MPRIS Metadata dict.
#[derive(Debug, Clone, Default)]
pub struct MprisMetadata {
    pub track_id: String,
    pub title: Option<String>,
    pub duration_secs: Option<f64>,
    pub art_url: Option<String>,
    pub url: Option<String>,
}

/// Build MPRIS Metadata from our struct.
pub fn build_metadata(meta: &MprisMetadata) -> Metadata {
    let track_id: TrackId = TrackId::try_from(meta.track_id.as_str()).unwrap_or(TrackId::NO_TRACK);

    let mut builder = Metadata::builder().trackid(track_id);

    if let Some(ref title) = meta.title {
        builder = builder.title(title.as_str());
    }
    if let Some(dur) = meta.duration_secs {
        builder = builder.length(Time::from_micros(seconds_to_micros(dur)));
    }
    if let Some(ref art) = meta.art_url {
        builder = builder.art_url(art.as_str());
    }
    if let Some(ref url) = meta.url {
        builder = builder.url(url.as_str());
    }

    builder.build()
}

/// Derive MprisMetadata from a MediaItem (Plex source).
#[allow(dead_code)]
pub fn metadata_from_media_item(
    item: &MediaItem,
    duration_secs: f64,
    art_path: Option<&str>,
) -> MprisMetadata {
    // Create a valid D-Bus object path from the media ID
    let safe_id: String = item
        .id
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let track_id = format!("/org/reel/track/{safe_id}");

    MprisMetadata {
        track_id,
        title: Some(item.title.clone()),
        duration_secs: Some(duration_secs),
        art_url: art_path.map(|p| format!("file://{p}")),
        url: None,
    }
}

/// Derive MprisMetadata from a local file (no MediaItem).
#[allow(dead_code)]
pub fn metadata_from_file(
    path: &str,
    media_title: Option<&str>,
    duration_secs: f64,
) -> MprisMetadata {
    let title = media_title.map(String::from).unwrap_or_else(|| {
        std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown")
            .to_string()
    });

    MprisMetadata {
        track_id: "/org/reel/track/local".to_string(),
        title: Some(title),
        duration_secs: Some(duration_secs),
        art_url: None,
        url: Some(format!("file://{path}")),
    }
}

/// Commands from MPRIS D-Bus methods back to the player.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum MprisCommand {
    Play,
    Pause,
    PlayPause,
    Stop,
    Seek(i64),
    SetPosition(i64),
    SetVolume(f64),
    OpenUri(String),
    Raise,
    Quit,
}

// --- MPRIS Server implementation ---

use mpris_server::zbus::{self, fdo};
use std::sync::Arc;
use tokio::sync::{mpsc, watch};

/// Shared state read by the MPRIS server.
#[allow(dead_code)]
pub struct MprisState {
    pub status_rx: watch::Receiver<PlayState>,
    pub metadata_rx: watch::Receiver<MprisMetadata>,
    pub volume_rx: watch::Receiver<f64>,
    pub position_rx: watch::Receiver<i64>,
    pub command_tx: mpsc::UnboundedSender<MprisCommand>,
}

#[allow(dead_code)]
pub struct ReelMprisPlayer {
    state: Arc<MprisState>,
}

#[allow(dead_code)]
impl ReelMprisPlayer {
    pub fn new(state: Arc<MprisState>) -> Self {
        Self { state }
    }
}

impl mpris_server::RootInterface for ReelMprisPlayer {
    async fn identity(&self) -> fdo::Result<String> {
        Ok("Reel".into())
    }

    async fn can_quit(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_raise(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn has_track_list(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn raise(&self) -> fdo::Result<()> {
        let _ = self.state.command_tx.send(MprisCommand::Raise);
        Ok(())
    }

    async fn quit(&self) -> fdo::Result<()> {
        let _ = self.state.command_tx.send(MprisCommand::Quit);
        Ok(())
    }

    async fn can_set_fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn set_fullscreen(&self, _fullscreen: bool) -> zbus::Result<()> {
        Ok(())
    }

    async fn desktop_entry(&self) -> fdo::Result<String> {
        Ok("dev.arsfeld.Reel".into())
    }

    async fn supported_uri_schemes(&self) -> fdo::Result<Vec<String>> {
        Ok(vec!["file".into()])
    }

    async fn supported_mime_types(&self) -> fdo::Result<Vec<String>> {
        Ok(vec![
            "video/mp4".into(),
            "video/x-matroska".into(),
            "video/webm".into(),
            "video/x-msvideo".into(),
            "video/mpeg".into(),
            "video/ogg".into(),
            "video/quicktime".into(),
        ])
    }
}

impl mpris_server::PlayerInterface for ReelMprisPlayer {
    async fn play(&self) -> fdo::Result<()> {
        let _ = self.state.command_tx.send(MprisCommand::Play);
        Ok(())
    }

    async fn pause(&self) -> fdo::Result<()> {
        let _ = self.state.command_tx.send(MprisCommand::Pause);
        Ok(())
    }

    async fn play_pause(&self) -> fdo::Result<()> {
        let _ = self.state.command_tx.send(MprisCommand::PlayPause);
        Ok(())
    }

    async fn stop(&self) -> fdo::Result<()> {
        let _ = self.state.command_tx.send(MprisCommand::Stop);
        Ok(())
    }

    async fn next(&self) -> fdo::Result<()> {
        Ok(()) // No playlist
    }

    async fn previous(&self) -> fdo::Result<()> {
        Ok(()) // No playlist
    }

    async fn seek(&self, offset: Time) -> fdo::Result<()> {
        let _ = self
            .state
            .command_tx
            .send(MprisCommand::Seek(offset.as_micros()));
        Ok(())
    }

    async fn set_position(&self, _track_id: TrackId, position: Time) -> fdo::Result<()> {
        let _ = self
            .state
            .command_tx
            .send(MprisCommand::SetPosition(position.as_micros()));
        Ok(())
    }

    async fn open_uri(&self, uri: String) -> fdo::Result<()> {
        let _ = self.state.command_tx.send(MprisCommand::OpenUri(uri));
        Ok(())
    }

    async fn playback_status(&self) -> fdo::Result<PlaybackStatus> {
        Ok(playback_status_from_state(*self.state.status_rx.borrow()))
    }

    async fn loop_status(&self) -> fdo::Result<LoopStatus> {
        Ok(LoopStatus::None)
    }

    async fn set_loop_status(&self, _status: LoopStatus) -> zbus::Result<()> {
        Ok(())
    }

    async fn rate(&self) -> fdo::Result<f64> {
        Ok(1.0)
    }

    async fn set_rate(&self, _rate: mpris_server::PlaybackRate) -> zbus::Result<()> {
        Ok(())
    }

    async fn shuffle(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn set_shuffle(&self, _shuffle: bool) -> zbus::Result<()> {
        Ok(())
    }

    async fn metadata(&self) -> fdo::Result<Metadata> {
        let meta = self.state.metadata_rx.borrow().clone();
        Ok(build_metadata(&meta))
    }

    async fn volume(&self) -> fdo::Result<mpris_server::Volume> {
        // Convert 0-150 scale to 0.0-1.5
        Ok(*self.state.volume_rx.borrow() / 100.0)
    }

    async fn set_volume(&self, volume: mpris_server::Volume) -> zbus::Result<()> {
        let _ = self
            .state
            .command_tx
            .send(MprisCommand::SetVolume(volume * 100.0));
        Ok(())
    }

    async fn position(&self) -> fdo::Result<Time> {
        Ok(Time::from_micros(*self.state.position_rx.borrow()))
    }

    async fn minimum_rate(&self) -> fdo::Result<f64> {
        Ok(0.25)
    }

    async fn maximum_rate(&self) -> fdo::Result<f64> {
        Ok(4.0)
    }

    async fn can_go_next(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn can_go_previous(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn can_play(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_pause(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_seek(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_control(&self) -> fdo::Result<bool> {
        Ok(true)
    }
}

/// Channels for communicating between the App (GTK thread) and the MPRIS server (tokio).
#[allow(dead_code)]
pub struct MprisBridge {
    pub status_tx: watch::Sender<PlayState>,
    pub metadata_tx: watch::Sender<MprisMetadata>,
    pub volume_tx: watch::Sender<f64>,
    pub position_tx: watch::Sender<i64>,
    /// Take this receiver once to relay commands into the GTK main loop.
    pub command_rx: Option<mpsc::UnboundedReceiver<MprisCommand>>,
}

/// Spawn the MPRIS server on the tokio runtime and return the bridge channels.
#[allow(dead_code)]
pub fn spawn_mpris_server() -> MprisBridge {
    let (status_tx, status_rx) = watch::channel(PlayState::Stopped);
    let (metadata_tx, metadata_rx) = watch::channel(MprisMetadata::default());
    let (volume_tx, volume_rx) = watch::channel(100.0f64);
    let (position_tx, position_rx) = watch::channel(0i64);
    let (command_tx, command_rx) = mpsc::unbounded_channel();

    let state = Arc::new(MprisState {
        status_rx,
        metadata_rx,
        volume_rx,
        position_rx,
        command_tx,
    });

    let player = ReelMprisPlayer::new(state);

    tokio::spawn(async move {
        match mpris_server::Server::new("org.mpris.MediaPlayer2.Reel", player).await {
            Ok(server) => {
                tracing::info!("MPRIS server started on D-Bus");
                // Keep the server alive
                std::future::pending::<()>().await;
                drop(server);
            }
            Err(e) => {
                tracing::warn!("Failed to start MPRIS server: {e}");
            }
        }
    });

    MprisBridge {
        status_tx,
        metadata_tx,
        volume_tx,
        position_tx,
        command_rx: Some(command_rx),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playing_state_maps_to_playing() {
        assert!(matches!(
            playback_status_from_state(PlayState::Playing),
            PlaybackStatus::Playing
        ));
    }

    #[test]
    fn paused_state_maps_to_paused() {
        assert!(matches!(
            playback_status_from_state(PlayState::Paused),
            PlaybackStatus::Paused
        ));
    }

    #[test]
    fn stopped_state_maps_to_stopped() {
        assert!(matches!(
            playback_status_from_state(PlayState::Stopped),
            PlaybackStatus::Stopped
        ));
    }

    #[test]
    fn seconds_to_micros_zero() {
        assert_eq!(seconds_to_micros(0.0), 0);
    }

    #[test]
    fn seconds_to_micros_one_and_half() {
        assert_eq!(seconds_to_micros(1.5), 1_500_000);
    }

    #[test]
    fn seconds_to_micros_large_value() {
        assert_eq!(seconds_to_micros(3600.0), 3_600_000_000);
    }

    #[test]
    fn micros_to_seconds_zero() {
        assert!((micros_to_seconds(0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn micros_to_seconds_roundtrip() {
        let secs = 42.5;
        let micros = seconds_to_micros(secs);
        assert!((micros_to_seconds(micros) - secs).abs() < 0.001);
    }

    #[test]
    fn build_metadata_with_all_fields() {
        let meta = MprisMetadata {
            track_id: "/org/reel/track/42".into(),
            title: Some("Dune".into()),
            duration_secs: Some(9240.0),
            art_url: Some("file:///tmp/art.jpg".into()),
            url: Some("file:///movies/dune.mkv".into()),
        };
        // Should not panic
        let _m = build_metadata(&meta);
    }

    #[test]
    fn build_metadata_with_no_optional_fields() {
        let meta = MprisMetadata {
            track_id: "/org/reel/track/0".into(),
            ..Default::default()
        };
        // Should not panic
        let _m = build_metadata(&meta);
    }

    #[test]
    fn build_metadata_with_invalid_track_id_uses_notrack() {
        let meta = MprisMetadata {
            track_id: "not a valid dbus path".into(),
            ..Default::default()
        };
        // Should fall back to NoTrack, not panic
        let _m = build_metadata(&meta);
    }

    fn test_media_item(id: &str, title: &str) -> MediaItem {
        use crate::models::media::{MediaType, SourceType};
        MediaItem {
            id: id.into(),
            source_type: SourceType::Plex,
            source_id: "http://localhost:32400".into(),
            external_id: "123".into(),
            media_type: MediaType::Movie,
            title: title.into(),
            year: None,
            overview: None,
            content_rating: None,
            rating: None,
            runtime_minutes: None,
            poster_path: None,
            series_poster_path: None,
            backdrop_path: None,
            genres: vec![],
            parent_id: None,
            season_number: None,
            episode_number: None,
            air_date: None,
            file_path: None,
            video_resolution: None,
            hdr: None,
            added_at: String::new(),
            updated_at: String::new(),
            playback_position_ms: None,
            watched: false,
            library_section_id: None,
        }
    }

    #[test]
    fn metadata_from_media_item_creates_valid_path() {
        let item = test_media_item("plex:123:456", "Dune");
        let meta = metadata_from_media_item(&item, 9240.0, None);
        assert!(meta.track_id.starts_with("/org/reel/track/"));
        // Colons should be replaced with underscores
        assert!(!meta.track_id.contains(':'));
        assert_eq!(meta.title, Some("Dune".into()));
        assert_eq!(meta.duration_secs, Some(9240.0));
    }

    #[test]
    fn metadata_from_media_item_with_artwork() {
        let item = test_media_item("test", "Test");
        let meta = metadata_from_media_item(&item, 100.0, Some("/tmp/art.jpg"));
        assert_eq!(meta.art_url, Some("file:///tmp/art.jpg".into()));
    }

    #[test]
    fn metadata_from_file_uses_filename_as_title() {
        let meta = metadata_from_file("/home/user/movies/My Movie.mkv", None, 7200.0);
        assert_eq!(meta.title, Some("My Movie".into()));
        assert_eq!(meta.track_id, "/org/reel/track/local");
        assert_eq!(meta.duration_secs, Some(7200.0));
        assert_eq!(
            meta.url,
            Some("file:///home/user/movies/My Movie.mkv".into())
        );
    }

    #[test]
    fn metadata_from_file_uses_media_title_when_available() {
        let meta = metadata_from_file("/path/to/file.mkv", Some("Custom Title"), 100.0);
        assert_eq!(meta.title, Some("Custom Title".into()));
    }

    #[test]
    fn metadata_from_file_handles_no_extension() {
        let meta = metadata_from_file("/path/to/videofile", None, 100.0);
        assert_eq!(meta.title, Some("videofile".into()));
    }

    #[test]
    fn mpris_command_debug() {
        // Verify all variants are Debug-printable
        let cmds = vec![
            MprisCommand::Play,
            MprisCommand::Pause,
            MprisCommand::PlayPause,
            MprisCommand::Stop,
            MprisCommand::Seek(1_000_000),
            MprisCommand::SetPosition(5_000_000),
            MprisCommand::SetVolume(0.8),
            MprisCommand::OpenUri("file:///test.mkv".into()),
            MprisCommand::Raise,
            MprisCommand::Quit,
        ];
        for cmd in cmds {
            let _ = format!("{cmd:?}");
        }
    }
}
