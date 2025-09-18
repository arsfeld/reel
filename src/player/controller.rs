use anyhow::Result;
use gtk4;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, info};

use super::{Player, PlayerState};
use crate::config::Config;

use crate::player::UpscalingMode;

/// Commands that can be sent to the player controller
#[derive(Debug)]
pub enum PlayerCommand {
    /// Create a video widget for rendering
    CreateVideoWidget {
        respond_to: oneshot::Sender<gtk4::Widget>,
    },
    /// Load media from URL
    LoadMedia {
        url: String,
        respond_to: oneshot::Sender<Result<()>>,
    },
    /// Start playback
    Play {
        respond_to: oneshot::Sender<Result<()>>,
    },
    /// Pause playback
    Pause {
        respond_to: oneshot::Sender<Result<()>>,
    },
    /// Stop playback
    Stop {
        respond_to: oneshot::Sender<Result<()>>,
    },
    /// Seek to position
    Seek {
        position: Duration,
        respond_to: oneshot::Sender<Result<()>>,
    },
    /// Get current position
    GetPosition {
        respond_to: oneshot::Sender<Option<Duration>>,
    },
    /// Get media duration
    GetDuration {
        respond_to: oneshot::Sender<Option<Duration>>,
    },
    /// Set volume (0.0 to 1.0)
    SetVolume {
        volume: f64,
        respond_to: oneshot::Sender<Result<()>>,
    },
    /// Get video dimensions
    GetVideoDimensions {
        respond_to: oneshot::Sender<Option<(i32, i32)>>,
    },
    /// Get player state
    GetState {
        respond_to: oneshot::Sender<PlayerState>,
    },
    /// Get audio tracks
    GetAudioTracks {
        respond_to: oneshot::Sender<Vec<(i32, String)>>,
    },
    /// Get subtitle tracks
    GetSubtitleTracks {
        respond_to: oneshot::Sender<Vec<(i32, String)>>,
    },
    /// Set audio track
    SetAudioTrack {
        track_index: i32,
        respond_to: oneshot::Sender<Result<()>>,
    },
    /// Set subtitle track
    SetSubtitleTrack {
        track_index: i32,
        respond_to: oneshot::Sender<Result<()>>,
    },
    /// Get current audio track
    GetCurrentAudioTrack { respond_to: oneshot::Sender<i32> },
    /// Get current subtitle track
    GetCurrentSubtitleTrack { respond_to: oneshot::Sender<i32> },
    /// Set upscaling mode (MPV only)
    SetUpscalingMode {
        mode: UpscalingMode,
        respond_to: oneshot::Sender<Result<()>>,
    },
    /// Set playback speed
    SetPlaybackSpeed {
        speed: f64,
        respond_to: oneshot::Sender<Result<()>>,
    },
    /// Get playback speed
    GetPlaybackSpeed { respond_to: oneshot::Sender<f64> },
    /// Frame step forward
    FrameStepForward {
        respond_to: oneshot::Sender<Result<()>>,
    },
    /// Frame step backward
    FrameStepBackward {
        respond_to: oneshot::Sender<Result<()>>,
    },
    /// Toggle mute
    ToggleMute {
        respond_to: oneshot::Sender<Result<()>>,
    },
    /// Check if muted
    IsMuted { respond_to: oneshot::Sender<bool> },
    /// Cycle subtitle track
    CycleSubtitleTrack {
        respond_to: oneshot::Sender<Result<()>>,
    },
    /// Cycle audio track
    CycleAudioTrack {
        respond_to: oneshot::Sender<Result<()>>,
    },
    /// Shutdown the player controller
    Shutdown,
}

/// Controller that owns the Player and processes commands
pub struct PlayerController {
    player: Player,
    receiver: mpsc::UnboundedReceiver<PlayerCommand>,
}

impl PlayerController {
    /// Create a new player controller with the given config
    pub fn new(config: &Config) -> Result<(PlayerHandle, PlayerController)> {
        let player = Player::new(config)?;
        let (sender, receiver) = mpsc::unbounded_channel();

        let controller = PlayerController { player, receiver };
        let handle = PlayerHandle { sender };

        Ok((handle, controller))
    }

    /// Run the controller event loop
    pub async fn run(mut self) {
        info!("🎮 PlayerController: Starting event loop");

        while let Some(command) = self.receiver.recv().await {
            match command {
                PlayerCommand::CreateVideoWidget { respond_to } => {
                    debug!("🎮 PlayerController: Creating video widget");
                    let widget = self.player.create_video_widget();
                    let _ = respond_to.send(widget);
                }
                PlayerCommand::LoadMedia { url, respond_to } => {
                    debug!("🎮 PlayerController: Loading media: {}", url);
                    let result = self.player.load_media(&url).await;
                    let _ = respond_to.send(result);
                }
                PlayerCommand::Play { respond_to } => {
                    debug!("🎮 PlayerController: Starting playback");
                    let result = self.player.play().await;
                    let _ = respond_to.send(result);
                }
                PlayerCommand::Pause { respond_to } => {
                    debug!("🎮 PlayerController: Pausing playback");
                    let result = self.player.pause().await;
                    let _ = respond_to.send(result);
                }
                PlayerCommand::Stop { respond_to } => {
                    debug!("🎮 PlayerController: Stopping playback");
                    let result = self.player.stop().await;
                    let _ = respond_to.send(result);
                }
                PlayerCommand::Seek {
                    position,
                    respond_to,
                } => {
                    debug!("🎮 PlayerController: Seeking to {:?}", position);
                    let result = self.player.seek(position).await;
                    let _ = respond_to.send(result);
                }
                PlayerCommand::GetPosition { respond_to } => {
                    let position = self.player.get_position().await;
                    let _ = respond_to.send(position);
                }
                PlayerCommand::GetDuration { respond_to } => {
                    let duration = self.player.get_duration().await;
                    let _ = respond_to.send(duration);
                }
                PlayerCommand::SetVolume { volume, respond_to } => {
                    debug!("🎮 PlayerController: Setting volume to {}", volume);
                    let result = self.player.set_volume(volume).await;
                    let _ = respond_to.send(result);
                }
                PlayerCommand::GetVideoDimensions { respond_to } => {
                    let dimensions = self.player.get_video_dimensions().await;
                    let _ = respond_to.send(dimensions);
                }
                PlayerCommand::GetState { respond_to } => {
                    let state = self.player.get_state().await;
                    let _ = respond_to.send(state);
                }
                PlayerCommand::GetAudioTracks { respond_to } => {
                    let tracks = self.player.get_audio_tracks().await;
                    let _ = respond_to.send(tracks);
                }
                PlayerCommand::GetSubtitleTracks { respond_to } => {
                    let tracks = self.player.get_subtitle_tracks().await;
                    let _ = respond_to.send(tracks);
                }
                PlayerCommand::SetAudioTrack {
                    track_index,
                    respond_to,
                } => {
                    debug!(
                        "🎮 PlayerController: Setting audio track to {}",
                        track_index
                    );
                    let result = self.player.set_audio_track(track_index).await;
                    let _ = respond_to.send(result);
                }
                PlayerCommand::SetSubtitleTrack {
                    track_index,
                    respond_to,
                } => {
                    debug!(
                        "🎮 PlayerController: Setting subtitle track to {}",
                        track_index
                    );
                    let result = self.player.set_subtitle_track(track_index).await;
                    let _ = respond_to.send(result);
                }
                PlayerCommand::GetCurrentAudioTrack { respond_to } => {
                    let track = self.player.get_current_audio_track().await;
                    let _ = respond_to.send(track);
                }
                PlayerCommand::GetCurrentSubtitleTrack { respond_to } => {
                    let track = self.player.get_current_subtitle_track().await;
                    let _ = respond_to.send(track);
                }
                PlayerCommand::SetUpscalingMode { mode, respond_to } => {
                    debug!("🎮 PlayerController: Setting upscaling mode to {:?}", mode);
                    let result = match &self.player {
                        Player::Mpv(mpv) => mpv.set_upscaling_mode(mode).await,
                        Player::GStreamer(_) => {
                            // GStreamer doesn't support upscaling modes
                            Err(anyhow::anyhow!(
                                "Upscaling mode not supported for GStreamer backend"
                            ))
                        }
                    };
                    let _ = respond_to.send(result);
                }
                PlayerCommand::SetPlaybackSpeed { speed, respond_to } => {
                    debug!("🎮 PlayerController: Setting playback speed to {}", speed);
                    let result = self.player.set_playback_speed(speed).await;
                    let _ = respond_to.send(result);
                }
                PlayerCommand::GetPlaybackSpeed { respond_to } => {
                    let speed = self.player.get_playback_speed().await;
                    let _ = respond_to.send(speed);
                }
                PlayerCommand::FrameStepForward { respond_to } => {
                    debug!("🎮 PlayerController: Frame stepping forward");
                    let result = self.player.frame_step_forward().await;
                    let _ = respond_to.send(result);
                }
                PlayerCommand::FrameStepBackward { respond_to } => {
                    debug!("🎮 PlayerController: Frame stepping backward");
                    let result = self.player.frame_step_backward().await;
                    let _ = respond_to.send(result);
                }
                PlayerCommand::ToggleMute { respond_to } => {
                    debug!("🎮 PlayerController: Toggling mute");
                    let result = self.player.toggle_mute().await;
                    let _ = respond_to.send(result);
                }
                PlayerCommand::IsMuted { respond_to } => {
                    let is_muted = self.player.is_muted().await;
                    let _ = respond_to.send(is_muted);
                }
                PlayerCommand::CycleSubtitleTrack { respond_to } => {
                    debug!("🎮 PlayerController: Cycling subtitle track");
                    let result = self.player.cycle_subtitle_track().await;
                    let _ = respond_to.send(result);
                }
                PlayerCommand::CycleAudioTrack { respond_to } => {
                    debug!("🎮 PlayerController: Cycling audio track");
                    let result = self.player.cycle_audio_track().await;
                    let _ = respond_to.send(result);
                }
                PlayerCommand::Shutdown => {
                    info!("🎮 PlayerController: Shutting down");
                    break;
                }
            }
        }

        info!("🎮 PlayerController: Event loop terminated");
    }
}

/// Handle to send commands to the player controller
#[derive(Clone)]
pub struct PlayerHandle {
    sender: mpsc::UnboundedSender<PlayerCommand>,
}

// PlayerHandle is safe to send between threads since the mpsc channel is Send-safe
// The underlying Player is accessed only through the PlayerController which runs on
// the main GTK thread via glib::spawn_future_local
unsafe impl Send for PlayerHandle {}
unsafe impl Sync for PlayerHandle {}

impl PlayerHandle {
    /// Create a video widget for rendering
    pub async fn create_video_widget(&self) -> Result<gtk4::Widget> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::CreateVideoWidget { respond_to })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))
    }

    /// Load media from URL
    pub async fn load_media(&self, url: &str) -> Result<()> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::LoadMedia {
                url: url.to_string(),
                respond_to,
            })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))?
    }

    /// Start playback
    pub async fn play(&self) -> Result<()> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::Play { respond_to })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))?
    }

    /// Pause playback
    pub async fn pause(&self) -> Result<()> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::Pause { respond_to })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))?
    }

    /// Stop playback
    pub async fn stop(&self) -> Result<()> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::Stop { respond_to })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))?
    }

    /// Seek to position
    pub async fn seek(&self, position: Duration) -> Result<()> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::Seek {
                position,
                respond_to,
            })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))?
    }

    /// Get current position
    pub async fn get_position(&self) -> Result<Option<Duration>> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::GetPosition { respond_to })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))
    }

    /// Get media duration
    pub async fn get_duration(&self) -> Result<Option<Duration>> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::GetDuration { respond_to })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))
    }

    /// Set volume (0.0 to 1.0)
    pub async fn set_volume(&self, volume: f64) -> Result<()> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::SetVolume { volume, respond_to })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))?
    }

    /// Get video dimensions
    pub async fn get_video_dimensions(&self) -> Result<Option<(i32, i32)>> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::GetVideoDimensions { respond_to })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))
    }

    /// Get player state
    pub async fn get_state(&self) -> Result<PlayerState> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::GetState { respond_to })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))
    }

    /// Get audio tracks
    pub async fn get_audio_tracks(&self) -> Result<Vec<(i32, String)>> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::GetAudioTracks { respond_to })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))
    }

    /// Get subtitle tracks
    pub async fn get_subtitle_tracks(&self) -> Result<Vec<(i32, String)>> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::GetSubtitleTracks { respond_to })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))
    }

    /// Set audio track
    pub async fn set_audio_track(&self, track_index: i32) -> Result<()> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::SetAudioTrack {
                track_index,
                respond_to,
            })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))?
    }

    /// Set subtitle track
    pub async fn set_subtitle_track(&self, track_index: i32) -> Result<()> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::SetSubtitleTrack {
                track_index,
                respond_to,
            })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))?
    }

    /// Get current audio track
    pub async fn get_current_audio_track(&self) -> Result<i32> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::GetCurrentAudioTrack { respond_to })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))
    }

    /// Get current subtitle track
    pub async fn get_current_subtitle_track(&self) -> Result<i32> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::GetCurrentSubtitleTrack { respond_to })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))
    }

    /// Set upscaling mode (MPV only)
    pub async fn set_upscaling_mode(&self, mode: UpscalingMode) -> Result<()> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::SetUpscalingMode { mode, respond_to })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))?
    }

    /// Set playback speed
    pub async fn set_playback_speed(&self, speed: f64) -> Result<()> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::SetPlaybackSpeed { speed, respond_to })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))?
    }

    /// Get playback speed
    pub async fn get_playback_speed(&self) -> Result<f64> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::GetPlaybackSpeed { respond_to })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))
    }

    /// Frame step forward
    pub async fn frame_step_forward(&self) -> Result<()> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::FrameStepForward { respond_to })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))?
    }

    /// Frame step backward
    pub async fn frame_step_backward(&self) -> Result<()> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::FrameStepBackward { respond_to })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))?
    }

    /// Toggle mute
    pub async fn toggle_mute(&self) -> Result<()> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::ToggleMute { respond_to })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))?
    }

    /// Check if muted
    pub async fn is_muted(&self) -> Result<bool> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::IsMuted { respond_to })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))
    }

    /// Cycle subtitle track
    pub async fn cycle_subtitle_track(&self) -> Result<()> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::CycleSubtitleTrack { respond_to })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))?
    }

    /// Cycle audio track
    pub async fn cycle_audio_track(&self) -> Result<()> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::CycleAudioTrack { respond_to })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))?
    }

    /// Shutdown the player controller
    pub fn shutdown(&self) -> Result<()> {
        self.sender
            .send(PlayerCommand::Shutdown)
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))
    }
}
