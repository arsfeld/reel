use anyhow::Result;
use gtk4;
use std::time::Duration;
use tracing::{debug, error, info, warn};

#[cfg(feature = "gstreamer")]
use super::GStreamerPlayer;
#[cfg(feature = "mpv")]
use super::MpvPlayer;
#[cfg(feature = "gstreamer")]
use super::gstreamer_player::PlayerState as GstPlayerState;
#[cfg(feature = "mpv")]
use super::mpv_player::PlayerState as MpvPlayerState;
use crate::config::Config;

#[derive(Debug)]
pub enum PlayerBackend {
    #[cfg(feature = "gstreamer")]
    GStreamer,
    #[cfg(feature = "mpv")]
    Mpv,
}

impl From<&str> for PlayerBackend {
    fn from(s: &str) -> Self {
        // On macOS, always return GStreamer
        if cfg!(target_os = "macos") {
            #[cfg(feature = "gstreamer")]
            return PlayerBackend::GStreamer;
            #[cfg(not(feature = "gstreamer"))]
            panic!("GStreamer backend is required on macOS but not enabled");
        }

        match s.to_lowercase().as_str() {
            #[cfg(feature = "gstreamer")]
            "gstreamer" => PlayerBackend::GStreamer,
            #[cfg(feature = "mpv")]
            "mpv" => PlayerBackend::Mpv,
            _ => {
                // Default to available backend
                #[cfg(all(feature = "mpv", not(feature = "gstreamer")))]
                return PlayerBackend::Mpv;
                #[cfg(all(feature = "gstreamer", not(feature = "mpv")))]
                return PlayerBackend::GStreamer;
                #[cfg(all(feature = "mpv", feature = "gstreamer"))]
                return PlayerBackend::Mpv; // Default to MPV when both available
                #[cfg(not(any(feature = "mpv", feature = "gstreamer")))]
                compile_error!("At least one player backend (mpv or gstreamer) must be enabled");
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlayerState {
    Idle,
    Loading,
    Playing,
    Paused,
    Stopped,
    Error,
}

pub enum Player {
    #[cfg(feature = "gstreamer")]
    GStreamer(GStreamerPlayer),
    #[cfg(feature = "mpv")]
    Mpv(MpvPlayer),
}

impl Player {
    pub fn new(config: &Config) -> Result<Self> {
        #[cfg(not(any(feature = "mpv", feature = "gstreamer")))]
        compile_error!("At least one player backend (mpv or gstreamer) must be enabled");

        // On macOS, always use GStreamer regardless of configuration
        // MPV has critical OpenGL issues on macOS
        let backend = if cfg!(target_os = "macos") {
            info!("🎬 Player Factory: macOS detected, forcing GStreamer backend");
            PlayerBackend::GStreamer
        } else {
            PlayerBackend::from(config.playback.player_backend.as_str())
        };

        info!("🎬 Player Factory: Creating new player instance");
        debug!(
            "🎬 Player Factory: Requested backend: {}",
            config.playback.player_backend
        );
        debug!("🎬 Player Factory: Parsed backend: {:?}", backend);
        debug!("🎬 Player Factory: Target OS: {}", std::env::consts::OS);

        match backend {
            #[cfg(feature = "mpv")]
            PlayerBackend::Mpv => {
                // This should never be reached on macOS due to the check above,
                // but add an extra safety check
                if cfg!(target_os = "macos") {
                    error!(
                        "❌ Player Factory: MPV backend requested on macOS, falling back to GStreamer"
                    );
                    #[cfg(feature = "gstreamer")]
                    return match GStreamerPlayer::new() {
                        Ok(player) => {
                            info!(
                                "✅ Player Factory: Successfully created GStreamer player (macOS fallback)"
                            );
                            Ok(Player::GStreamer(player))
                        }
                        Err(e) => {
                            error!(
                                "❌ Player Factory: Failed to create GStreamer player: {}",
                                e
                            );
                            Err(e)
                        }
                    };
                    #[cfg(not(feature = "gstreamer"))]
                    return Err(anyhow::anyhow!(
                        "MPV is not supported on macOS and GStreamer is not available"
                    ));
                }

                info!("🎬 Player Factory: Creating MPV player backend");
                debug!(
                    "🎬 Player Factory: MPV config - hardware_accel={}, cache_size={}MB",
                    config.playback.hardware_acceleration, config.playback.mpv_cache_size_mb
                );
                match MpvPlayer::new(config) {
                    Ok(player) => {
                        info!("✅ Player Factory: Successfully created MPV player");
                        Ok(Player::Mpv(player))
                    }
                    Err(e) => {
                        error!("❌ Player Factory: Failed to create MPV player: {}", e);

                        // Try fallback to GStreamer if available
                        #[cfg(feature = "gstreamer")]
                        {
                            warn!("🔄 Player Factory: Attempting fallback to GStreamer");
                            match GStreamerPlayer::new() {
                                Ok(gst_player) => {
                                    warn!(
                                        "✅ Player Factory: Successfully created GStreamer fallback player"
                                    );
                                    return Ok(Player::GStreamer(gst_player));
                                }
                                Err(gst_e) => {
                                    error!(
                                        "❌ Player Factory: Fallback to GStreamer also failed: {}",
                                        gst_e
                                    );
                                    return Err(e); // Return original MPV error
                                }
                            }
                        }

                        #[cfg(not(feature = "gstreamer"))]
                        Err(e)
                    }
                }
            }
            #[cfg(feature = "gstreamer")]
            PlayerBackend::GStreamer => {
                info!("🎬 Player Factory: Creating GStreamer player backend");
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

                        // Try fallback to MPV if available
                        #[cfg(feature = "mpv")]
                        {
                            warn!("🔄 Player Factory: Attempting fallback to MPV");
                            match MpvPlayer::new(config) {
                                Ok(mpv_player) => {
                                    warn!(
                                        "✅ Player Factory: Successfully created MPV fallback player"
                                    );
                                    return Ok(Player::Mpv(mpv_player));
                                }
                                Err(mpv_e) => {
                                    error!(
                                        "❌ Player Factory: Fallback to MPV also failed: {}",
                                        mpv_e
                                    );
                                    return Err(e); // Return original GStreamer error
                                }
                            }
                        }

                        #[cfg(not(feature = "mpv"))]
                        Err(e)
                    }
                }
            }
        }
    }

    pub fn set_error_callback<F>(&self, callback: F)
    where
        F: Fn(String) + Send + 'static,
    {
        match self {
            #[cfg(feature = "mpv")]
            Player::Mpv(mpv) => mpv.set_error_callback(callback),
            #[cfg(feature = "gstreamer")]
            Player::GStreamer(_) => {
                // GStreamer doesn't have this callback mechanism yet
                // TODO: Implement error callbacks for GStreamer
            }
        }
    }

    pub fn create_video_widget(&self) -> gtk4::Widget {
        match self {
            #[cfg(feature = "gstreamer")]
            Player::GStreamer(p) => p.create_video_widget(),
            #[cfg(feature = "mpv")]
            Player::Mpv(p) => p.create_video_widget(),
        }
    }

    pub async fn load_media(&self, url: &str) -> Result<()> {
        let backend_name = match self {
            #[cfg(feature = "gstreamer")]
            Player::GStreamer(_) => "GStreamer",
            #[cfg(feature = "mpv")]
            Player::Mpv(_) => "MPV",
        };
        info!("🎥 Player ({}): Loading media: {}", backend_name, url);
        debug!(
            "🎥 Player ({}): Media URL length: {} chars",
            backend_name,
            url.len()
        );

        let result = match self {
            #[cfg(feature = "gstreamer")]
            Player::GStreamer(p) => p.load_media(url, None).await,
            #[cfg(feature = "mpv")]
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
            #[cfg(feature = "gstreamer")]
            Player::GStreamer(_) => "GStreamer",
            #[cfg(feature = "mpv")]
            Player::Mpv(_) => "MPV",
        };
        debug!("▶️ Player ({}): Starting playback", backend_name);

        let result = match self {
            #[cfg(feature = "gstreamer")]
            Player::GStreamer(p) => p.play().await,
            #[cfg(feature = "mpv")]
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
            #[cfg(feature = "gstreamer")]
            Player::GStreamer(_) => "GStreamer",
            #[cfg(feature = "mpv")]
            Player::Mpv(_) => "MPV",
        };
        debug!("⏸️ Player ({}): Pausing playback", backend_name);

        let result = match self {
            #[cfg(feature = "gstreamer")]
            Player::GStreamer(p) => p.pause().await,
            #[cfg(feature = "mpv")]
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
            #[cfg(feature = "gstreamer")]
            Player::GStreamer(p) => p.stop().await,
            #[cfg(feature = "mpv")]
            Player::Mpv(p) => p.stop().await,
        }
    }

    pub async fn seek(&self, position: Duration) -> Result<()> {
        match self {
            #[cfg(feature = "gstreamer")]
            Player::GStreamer(p) => p.seek(position).await,
            #[cfg(feature = "mpv")]
            Player::Mpv(p) => p.seek(position).await,
        }
    }

    pub async fn get_position(&self) -> Option<Duration> {
        match self {
            #[cfg(feature = "gstreamer")]
            Player::GStreamer(p) => p.get_position().await,
            #[cfg(feature = "mpv")]
            Player::Mpv(p) => p.get_position().await,
        }
    }

    pub async fn get_duration(&self) -> Option<Duration> {
        match self {
            #[cfg(feature = "gstreamer")]
            Player::GStreamer(p) => p.get_duration().await,
            #[cfg(feature = "mpv")]
            Player::Mpv(p) => p.get_duration().await,
        }
    }

    pub async fn set_volume(&self, volume: f64) -> Result<()> {
        match self {
            #[cfg(feature = "gstreamer")]
            Player::GStreamer(p) => p.set_volume(volume).await,
            #[cfg(feature = "mpv")]
            Player::Mpv(p) => p.set_volume(volume).await,
        }
    }

    pub async fn get_video_dimensions(&self) -> Option<(i32, i32)> {
        match self {
            #[cfg(feature = "gstreamer")]
            Player::GStreamer(p) => p.get_video_dimensions().await,
            #[cfg(feature = "mpv")]
            Player::Mpv(p) => p.get_video_dimensions().await,
        }
    }

    pub async fn get_state(&self) -> PlayerState {
        match self {
            #[cfg(feature = "gstreamer")]
            Player::GStreamer(p) => match p.get_state().await {
                GstPlayerState::Idle => PlayerState::Idle,
                GstPlayerState::Loading => PlayerState::Loading,
                GstPlayerState::Playing => PlayerState::Playing,
                GstPlayerState::Paused => PlayerState::Paused,
                GstPlayerState::Stopped => PlayerState::Stopped,
                GstPlayerState::Error => PlayerState::Error,
            },
            #[cfg(feature = "mpv")]
            Player::Mpv(p) => match p.get_state().await {
                MpvPlayerState::Idle => PlayerState::Idle,
                MpvPlayerState::Loading => PlayerState::Loading,
                MpvPlayerState::Playing => PlayerState::Playing,
                MpvPlayerState::Paused => PlayerState::Paused,
                MpvPlayerState::Stopped => PlayerState::Stopped,
                MpvPlayerState::Error => PlayerState::Error,
            },
        }
    }

    pub async fn get_audio_tracks(&self) -> Vec<(i32, String)> {
        match self {
            #[cfg(feature = "gstreamer")]
            Player::GStreamer(p) => p.get_audio_tracks().await,
            #[cfg(feature = "mpv")]
            Player::Mpv(p) => p.get_audio_tracks().await,
        }
    }

    pub async fn get_subtitle_tracks(&self) -> Vec<(i32, String)> {
        match self {
            #[cfg(feature = "gstreamer")]
            Player::GStreamer(p) => p.get_subtitle_tracks().await,
            #[cfg(feature = "mpv")]
            Player::Mpv(p) => p.get_subtitle_tracks().await,
        }
    }

    pub async fn set_audio_track(&self, track_index: i32) -> Result<()> {
        match self {
            #[cfg(feature = "gstreamer")]
            Player::GStreamer(p) => p.set_audio_track(track_index).await,
            #[cfg(feature = "mpv")]
            Player::Mpv(p) => p.set_audio_track(track_index).await,
        }
    }

    pub async fn set_subtitle_track(&self, track_index: i32) -> Result<()> {
        match self {
            #[cfg(feature = "gstreamer")]
            Player::GStreamer(p) => p.set_subtitle_track(track_index).await,
            #[cfg(feature = "mpv")]
            Player::Mpv(p) => p.set_subtitle_track(track_index).await,
        }
    }

    pub async fn get_current_audio_track(&self) -> i32 {
        match self {
            #[cfg(feature = "gstreamer")]
            Player::GStreamer(p) => p.get_current_audio_track().await,
            #[cfg(feature = "mpv")]
            Player::Mpv(p) => p.get_current_audio_track().await,
        }
    }

    pub async fn get_current_subtitle_track(&self) -> i32 {
        match self {
            #[cfg(feature = "gstreamer")]
            Player::GStreamer(p) => p.get_current_subtitle_track().await,
            #[cfg(feature = "mpv")]
            Player::Mpv(p) => p.get_current_subtitle_track().await,
        }
    }

    pub async fn set_playback_speed(&self, speed: f64) -> Result<()> {
        match self {
            #[cfg(feature = "gstreamer")]
            Player::GStreamer(p) => p.set_playback_speed(speed).await,
            #[cfg(feature = "mpv")]
            Player::Mpv(p) => p.set_playback_speed(speed).await,
        }
    }

    pub async fn get_playback_speed(&self) -> f64 {
        match self {
            #[cfg(feature = "gstreamer")]
            Player::GStreamer(p) => p.get_playback_speed().await,
            #[cfg(feature = "mpv")]
            Player::Mpv(p) => p.get_playback_speed().await,
        }
    }

    pub async fn frame_step_forward(&self) -> Result<()> {
        match self {
            #[cfg(feature = "gstreamer")]
            Player::GStreamer(p) => p.frame_step_forward().await,
            #[cfg(feature = "mpv")]
            Player::Mpv(p) => p.frame_step_forward().await,
        }
    }

    pub async fn frame_step_backward(&self) -> Result<()> {
        match self {
            #[cfg(feature = "gstreamer")]
            Player::GStreamer(p) => p.frame_step_backward().await,
            #[cfg(feature = "mpv")]
            Player::Mpv(p) => p.frame_step_backward().await,
        }
    }

    pub async fn toggle_mute(&self) -> Result<()> {
        match self {
            #[cfg(feature = "gstreamer")]
            Player::GStreamer(p) => p.toggle_mute().await,
            #[cfg(feature = "mpv")]
            Player::Mpv(p) => p.toggle_mute().await,
        }
    }

    pub async fn is_muted(&self) -> bool {
        match self {
            #[cfg(feature = "gstreamer")]
            Player::GStreamer(p) => p.is_muted().await,
            #[cfg(feature = "mpv")]
            Player::Mpv(p) => p.is_muted().await,
        }
    }

    pub async fn cycle_subtitle_track(&self) -> Result<()> {
        match self {
            #[cfg(feature = "gstreamer")]
            Player::GStreamer(p) => p.cycle_subtitle_track().await,
            #[cfg(feature = "mpv")]
            Player::Mpv(p) => p.cycle_subtitle_track().await,
        }
    }

    pub async fn cycle_audio_track(&self) -> Result<()> {
        match self {
            #[cfg(feature = "gstreamer")]
            Player::GStreamer(p) => p.cycle_audio_track().await,
            #[cfg(feature = "mpv")]
            Player::Mpv(p) => p.cycle_audio_track().await,
        }
    }

    pub async fn set_zoom_mode(&self, mode: crate::player::ZoomMode) -> Result<()> {
        match self {
            #[cfg(feature = "gstreamer")]
            Player::GStreamer(p) => p.set_zoom_mode(mode).await,
            #[cfg(feature = "mpv")]
            Player::Mpv(p) => p.set_zoom_mode(mode).await,
        }
    }

    pub async fn get_zoom_mode(&self) -> crate::player::ZoomMode {
        match self {
            #[cfg(feature = "gstreamer")]
            Player::GStreamer(p) => p.get_zoom_mode().await,
            #[cfg(feature = "mpv")]
            Player::Mpv(p) => p.get_zoom_mode().await,
        }
    }
}
