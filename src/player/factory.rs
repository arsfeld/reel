use anyhow::Result;
use gtk4;
use std::time::Duration;
use tracing::{debug, error, info};

use super::gstreamer_player::PlayerState as GstPlayerState;
use super::mpv_player::PlayerState as MpvPlayerState;
use super::{GStreamerPlayer, MpvPlayer};
use crate::config::Config;

#[derive(Debug)]
pub enum PlayerBackend {
    GStreamer,
    Mpv,
}

impl From<&str> for PlayerBackend {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "gstreamer" => PlayerBackend::GStreamer,
            _ => PlayerBackend::Mpv, // Default to MPV
        }
    }
}

#[derive(Debug, Clone)]
pub enum PlayerState {
    Idle,
    Loading,
    Playing,
    Paused,
    Stopped,
    Error,
}

pub enum Player {
    GStreamer(GStreamerPlayer),
    Mpv(MpvPlayer),
}

impl Player {
    pub fn new(config: &Config) -> Result<Self> {
        let backend = PlayerBackend::from(config.playback.player_backend.as_str());

        info!("🎬 Player Factory: Creating new player instance");
        debug!(
            "🎬 Player Factory: Requested backend: {}",
            config.playback.player_backend
        );
        debug!("🎬 Player Factory: Parsed backend: {:?}", backend);
        debug!("🎬 Player Factory: Target OS: {}", std::env::consts::OS);

        // Platform-specific backend selection and fallback
        #[cfg(target_os = "macos")]
        {
            // On macOS, try MPV first as it has better compatibility
            match backend {
                PlayerBackend::Mpv => {
                    info!("🎬 Player Factory: Creating MPV player backend for macOS");
                    debug!(
                        "🎬 Player Factory: Attempting to initialize MPV with config: hardware_accel={}, cache_size={}MB",
                        config.playback.hardware_acceleration, config.playback.mpv_cache_size_mb
                    );
                    match MpvPlayer::new(config) {
                        Ok(player) => {
                            info!("✅ Player Factory: Successfully created MPV player for macOS");
                            return Ok(Player::Mpv(player));
                        }
                        Err(e) => {
                            warn!(
                                "⚠️ Player Factory: Failed to create MPV player on macOS: {}",
                                e
                            );
                            info!("🔄 Player Factory: Falling back to GStreamer");
                            match GStreamerPlayer::new() {
                                Ok(gst_player) => {
                                    warn!(
                                        "✅ Player Factory: Successfully created GStreamer fallback player"
                                    );
                                    return Ok(Player::GStreamer(gst_player));
                                }
                                Err(gst_e) => {
                                    error!(
                                        "❌ Player Factory: Both MPV and GStreamer failed on macOS. MPV: {}, GStreamer: {}",
                                        e, gst_e
                                    );
                                    return Err(e); // Return original MPV error
                                }
                            }
                        }
                    }
                }
                PlayerBackend::GStreamer => {
                    info!("🎬 Player Factory: Creating GStreamer player backend for macOS");
                    debug!(
                        "🎬 Player Factory: GStreamer on macOS may have compatibility issues, fallback available"
                    );
                    match GStreamerPlayer::new() {
                        Ok(player) => {
                            info!(
                                "✅ Player Factory: Successfully created GStreamer player for macOS"
                            );
                            return Ok(Player::GStreamer(player));
                        }
                        Err(e) => {
                            warn!(
                                "⚠️ Player Factory: Failed to create GStreamer player on macOS: {}",
                                e
                            );
                            info!("🔄 Player Factory: Falling back to MPV");
                            match MpvPlayer::new(config) {
                                Ok(mpv_player) => {
                                    warn!(
                                        "✅ Player Factory: Successfully created MPV fallback player"
                                    );
                                    return Ok(Player::Mpv(mpv_player));
                                }
                                Err(mpv_e) => {
                                    error!(
                                        "❌ Player Factory: Both GStreamer and MPV failed on macOS. GStreamer: {}, MPV: {}",
                                        e, mpv_e
                                    );
                                    return Err(e); // Return original GStreamer error
                                }
                            }
                        }
                    }
                }
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            match backend {
                PlayerBackend::GStreamer => {
                    info!("🎬 Player Factory: Creating GStreamer player backend for Linux/Other");
                    debug!(
                        "🎬 Player Factory: GStreamer should have good compatibility on this platform"
                    );
                    match GStreamerPlayer::new() {
                        Ok(player) => {
                            info!("✅ Player Factory: Successfully created GStreamer player");
                            Ok(Player::GStreamer(player))
                        }
                        Err(e) => {
                            error!(
                                "❌ Player Factory: Failed to create GStreamer player: {}",
                                e
                            );
                            Err(e)
                        }
                    }
                }
                PlayerBackend::Mpv => {
                    info!("🎬 Player Factory: Creating MPV player backend for Linux/Other");
                    debug!(
                        "🎬 Player Factory: MPV config - hardware_accel={}, verbose_logging={}, cache_size={}MB",
                        config.playback.hardware_acceleration,
                        config.playback.mpv_verbose_logging,
                        config.playback.mpv_cache_size_mb
                    );
                    match MpvPlayer::new(config) {
                        Ok(player) => {
                            info!("✅ Player Factory: Successfully created MPV player");
                            Ok(Player::Mpv(player))
                        }
                        Err(e) => {
                            error!("❌ Player Factory: Failed to create MPV player: {}", e);
                            Err(e)
                        }
                    }
                }
            }
        }
    }

    pub fn create_video_widget(&self) -> gtk4::Widget {
        match self {
            Player::GStreamer(p) => p.create_video_widget(),
            Player::Mpv(p) => p.create_video_widget(),
        }
    }

    pub async fn load_media(&self, url: &str) -> Result<()> {
        let backend_name = match self {
            Player::GStreamer(_) => "GStreamer",
            Player::Mpv(_) => "MPV",
        };
        info!("🎥 Player ({}): Loading media: {}", backend_name, url);
        debug!(
            "🎥 Player ({}): Media URL length: {} chars",
            backend_name,
            url.len()
        );

        let result = match self {
            Player::GStreamer(p) => p.load_media(url, None).await,
            Player::Mpv(p) => p.load_media(url, None).await,
        };

        match &result {
            Ok(_) => info!("✅ Player ({}): Successfully loaded media", backend_name),
            Err(e) => error!("❌ Player ({}): Failed to load media: {}", backend_name, e),
        }

        result
    }

    pub async fn play(&self) -> Result<()> {
        let backend_name = match self {
            Player::GStreamer(_) => "GStreamer",
            Player::Mpv(_) => "MPV",
        };
        debug!("▶️ Player ({}): Starting playback", backend_name);

        let result = match self {
            Player::GStreamer(p) => p.play().await,
            Player::Mpv(p) => p.play().await,
        };

        match &result {
            Ok(_) => info!(
                "✅ Player ({}): Playback started successfully",
                backend_name
            ),
            Err(e) => error!(
                "❌ Player ({}): Failed to start playback: {}",
                backend_name, e
            ),
        }

        result
    }

    pub async fn pause(&self) -> Result<()> {
        let backend_name = match self {
            Player::GStreamer(_) => "GStreamer",
            Player::Mpv(_) => "MPV",
        };
        debug!("⏸️ Player ({}): Pausing playback", backend_name);

        let result = match self {
            Player::GStreamer(p) => p.pause().await,
            Player::Mpv(p) => p.pause().await,
        };

        match &result {
            Ok(_) => info!("✅ Player ({}): Playback paused successfully", backend_name),
            Err(e) => error!(
                "❌ Player ({}): Failed to pause playback: {}",
                backend_name, e
            ),
        }

        result
    }

    pub async fn stop(&self) -> Result<()> {
        match self {
            Player::GStreamer(p) => p.stop().await,
            Player::Mpv(p) => p.stop().await,
        }
    }

    pub async fn seek(&self, position: Duration) -> Result<()> {
        match self {
            Player::GStreamer(p) => p.seek(position).await,
            Player::Mpv(p) => p.seek(position).await,
        }
    }

    pub async fn get_position(&self) -> Option<Duration> {
        match self {
            Player::GStreamer(p) => p.get_position().await,
            Player::Mpv(p) => p.get_position().await,
        }
    }

    pub async fn get_duration(&self) -> Option<Duration> {
        match self {
            Player::GStreamer(p) => p.get_duration().await,
            Player::Mpv(p) => p.get_duration().await,
        }
    }

    pub async fn set_volume(&self, volume: f64) -> Result<()> {
        match self {
            Player::GStreamer(p) => p.set_volume(volume).await,
            Player::Mpv(p) => p.set_volume(volume).await,
        }
    }

    pub async fn get_video_dimensions(&self) -> Option<(i32, i32)> {
        match self {
            Player::GStreamer(p) => p.get_video_dimensions().await,
            Player::Mpv(p) => p.get_video_dimensions().await,
        }
    }

    pub async fn get_state(&self) -> PlayerState {
        match self {
            Player::GStreamer(p) => match p.get_state().await {
                GstPlayerState::Idle => PlayerState::Idle,
                GstPlayerState::Loading => PlayerState::Loading,
                GstPlayerState::Playing => PlayerState::Playing,
                GstPlayerState::Paused => PlayerState::Paused,
                GstPlayerState::Stopped => PlayerState::Stopped,
                GstPlayerState::Error => PlayerState::Error,
            },
            Player::Mpv(p) => match p.get_state().await {
                MpvPlayerState::Idle => PlayerState::Idle,
                MpvPlayerState::Loading => PlayerState::Loading,
                MpvPlayerState::Playing => PlayerState::Playing,
                MpvPlayerState::Paused => PlayerState::Paused,
                MpvPlayerState::Stopped => PlayerState::Stopped,
                // MpvPlayerState::Error => PlayerState::Error, // Removed unused Error variant
            },
        }
    }

    pub async fn get_audio_tracks(&self) -> Vec<(i32, String)> {
        match self {
            Player::GStreamer(p) => p.get_audio_tracks().await,
            Player::Mpv(p) => p.get_audio_tracks().await,
        }
    }

    pub async fn get_subtitle_tracks(&self) -> Vec<(i32, String)> {
        match self {
            Player::GStreamer(p) => p.get_subtitle_tracks().await,
            Player::Mpv(p) => p.get_subtitle_tracks().await,
        }
    }

    #[allow(dead_code)]
    pub async fn set_audio_track(&self, track_index: i32) -> Result<()> {
        match self {
            Player::GStreamer(p) => p.set_audio_track(track_index).await,
            Player::Mpv(p) => p.set_audio_track(track_index).await,
        }
    }

    #[allow(dead_code)]
    pub async fn set_subtitle_track(&self, track_index: i32) -> Result<()> {
        match self {
            Player::GStreamer(p) => p.set_subtitle_track(track_index).await,
            Player::Mpv(p) => p.set_subtitle_track(track_index).await,
        }
    }

    #[allow(dead_code)]
    pub async fn get_current_audio_track(&self) -> i32 {
        match self {
            Player::GStreamer(p) => p.get_current_audio_track().await,
            Player::Mpv(p) => p.get_current_audio_track().await,
        }
    }

    #[allow(dead_code)]
    pub async fn get_current_subtitle_track(&self) -> i32 {
        match self {
            Player::GStreamer(p) => p.get_current_subtitle_track().await,
            Player::Mpv(p) => p.get_current_subtitle_track().await,
        }
    }
}
