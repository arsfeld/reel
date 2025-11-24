use anyhow::Result;
use gtk4;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, trace};

use super::{Player, PlayerState};
use crate::config::Config;

use crate::player::{UpscalingMode, ZoomMode};

#[cfg(feature = "gstreamer")]
use crate::player::BufferingState;

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
    /// Cycle subtitle track
    CycleSubtitleTrack {
        respond_to: oneshot::Sender<Result<()>>,
    },
    /// Cycle audio track
    CycleAudioTrack {
        respond_to: oneshot::Sender<Result<()>>,
    },
    /// Set zoom mode
    SetZoomMode {
        mode: ZoomMode,
        respond_to: oneshot::Sender<Result<()>>,
    },
    /// Get buffering state (GStreamer only)
    #[cfg(feature = "gstreamer")]
    GetBufferingState {
        respond_to: oneshot::Sender<Option<BufferingState>>,
    },
    /// Wait for player backend to be ready for seeking operations
    WaitUntilReady {
        timeout: Duration,
        respond_to: oneshot::Sender<Result<()>>,
    },
}

/// Controller that owns the Player and processes commands
pub struct PlayerController {
    player: Player,
    receiver: mpsc::UnboundedReceiver<PlayerCommand>,
    error_sender: Option<mpsc::UnboundedSender<String>>,
}

impl PlayerController {
    /// Create a new player controller with the given config
    pub fn new(config: &Config) -> Result<(PlayerHandle, PlayerController)> {
        let player = Player::new(config)?;
        let (sender, receiver) = mpsc::unbounded_channel();
        let (error_tx, error_rx) = mpsc::unbounded_channel();

        let controller = PlayerController {
            player,
            receiver,
            error_sender: Some(error_tx.clone()),
        };
        let handle = PlayerHandle {
            sender,
            error_receiver: Arc::new(Mutex::new(Some(error_rx))),
        };

        Ok((handle, controller))
    }

    /// Set up error callback from player
    fn setup_error_callback(&mut self) {
        if let Some(ref error_sender) = self.error_sender {
            let sender = error_sender.clone();
            self.player.set_error_callback(move |error_msg| {
                let _ = sender.send(error_msg);
            });
        }
    }

    /// Run the controller event loop
    pub async fn run(mut self) {
        debug!("PlayerController event loop started");

        // Set up error callback
        self.setup_error_callback();

        while let Some(command) = self.receiver.recv().await {
            match command {
                PlayerCommand::CreateVideoWidget { respond_to } => {
                    trace!("Creating video widget");
                    let widget = self.player.create_video_widget();
                    let _ = respond_to.send(widget);
                }
                PlayerCommand::LoadMedia { url, respond_to } => {
                    trace!("Loading media: {}", url);
                    let result = self.player.load_media(&url).await;

                    // If initial load succeeded, wait a moment and check if media actually loaded
                    if result.is_ok() {
                        // Give MPV time to process the file
                        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

                        // Check if we have a valid duration (indicates successful load)
                        if let Some(duration) = self.player.get_duration().await {
                            if duration.as_secs() == 0 {
                                // Media failed to load properly
                                let _ = respond_to.send(Err(anyhow::anyhow!(
                                    "Media failed to load - no valid duration detected"
                                )));
                                continue;
                            }
                        } else {
                            // Check if player is in error or idle state
                            let state = self.player.get_state().await;
                            if matches!(state, PlayerState::Error | PlayerState::Idle) {
                                let _ = respond_to.send(Err(anyhow::anyhow!(
                                    "Media failed to load - player is in {} state",
                                    match state {
                                        PlayerState::Error => "error",
                                        PlayerState::Idle => "idle",
                                        _ => "invalid",
                                    }
                                )));
                                continue;
                            }
                        }
                    }

                    let _ = respond_to.send(result);
                }
                PlayerCommand::Play { respond_to } => {
                    trace!("Starting playback");
                    let result = self.player.play().await;
                    let _ = respond_to.send(result);
                }
                PlayerCommand::Pause { respond_to } => {
                    trace!("Pausing playback");
                    let result = self.player.pause().await;
                    let _ = respond_to.send(result);
                }
                PlayerCommand::Stop { respond_to } => {
                    trace!("Stopping playback");
                    let result = self.player.stop().await;
                    let _ = respond_to.send(result);
                }
                PlayerCommand::Seek {
                    position,
                    respond_to,
                } => {
                    trace!("Seeking to {:?}", position);
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
                    trace!("Setting volume to {}", volume);
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
                    trace!("Setting audio track to {}", track_index);
                    let result = self.player.set_audio_track(track_index).await;
                    let _ = respond_to.send(result);
                }
                PlayerCommand::SetSubtitleTrack {
                    track_index,
                    respond_to,
                } => {
                    trace!("Setting subtitle track to {}", track_index);
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
                    trace!("Setting upscaling mode to {:?}", mode);
                    let result = match &self.player {
                        #[cfg(all(feature = "mpv", not(target_os = "macos")))]
                        Player::Mpv(mpv) => mpv.set_upscaling_mode(mode).await,
                        #[cfg(feature = "gstreamer")]
                        Player::GStreamer(_) => {
                            // GStreamer doesn't support upscaling modes
                            Err(anyhow::anyhow!(
                                "Upscaling mode not supported for GStreamer backend"
                            ))
                        }
                        #[cfg(not(any(feature = "mpv", feature = "gstreamer")))]
                        _ => Err(anyhow::anyhow!("No player backend available")),
                    };
                    let _ = respond_to.send(result);
                }
                PlayerCommand::SetPlaybackSpeed { speed, respond_to } => {
                    trace!("Setting playback speed to {}", speed);
                    let result = self.player.set_playback_speed(speed).await;
                    let _ = respond_to.send(result);
                }
                PlayerCommand::FrameStepForward { respond_to } => {
                    trace!("Frame stepping forward");
                    let result = self.player.frame_step_forward().await;
                    let _ = respond_to.send(result);
                }
                PlayerCommand::FrameStepBackward { respond_to } => {
                    trace!("Frame stepping backward");
                    let result = self.player.frame_step_backward().await;
                    let _ = respond_to.send(result);
                }
                PlayerCommand::ToggleMute { respond_to } => {
                    trace!("Toggling mute");
                    let result = self.player.toggle_mute().await;
                    let _ = respond_to.send(result);
                }
                PlayerCommand::CycleSubtitleTrack { respond_to } => {
                    trace!("Cycling subtitle track");
                    let result = self.player.cycle_subtitle_track().await;
                    let _ = respond_to.send(result);
                }
                PlayerCommand::CycleAudioTrack { respond_to } => {
                    trace!("Cycling audio track");
                    let result = self.player.cycle_audio_track().await;
                    let _ = respond_to.send(result);
                }
                PlayerCommand::SetZoomMode { mode, respond_to } => {
                    trace!("Setting zoom mode to {:?}", mode);
                    let result = self.player.set_zoom_mode(mode).await;
                    let _ = respond_to.send(result);
                }
                #[cfg(feature = "gstreamer")]
                PlayerCommand::GetBufferingState { respond_to } => {
                    let state = match &self.player {
                        #[cfg(feature = "gstreamer")]
                        Player::GStreamer(gst) => Some(gst.get_buffering_state().await),
                        #[cfg(all(feature = "mpv", not(target_os = "macos")))]
                        Player::Mpv(_) => None,
                        #[cfg(not(any(feature = "mpv", feature = "gstreamer")))]
                        _ => None,
                    };
                    let _ = respond_to.send(state);
                }
                PlayerCommand::WaitUntilReady {
                    timeout,
                    respond_to,
                } => {
                    trace!("Waiting for player to be ready (timeout: {:?})", timeout);
                    let result = self.player.wait_until_ready(timeout).await;
                    let _ = respond_to.send(result);
                }
            }
        }

        debug!("PlayerController event loop terminated");
    }
}

/// Handle to send commands to the player controller
pub struct PlayerHandle {
    sender: mpsc::UnboundedSender<PlayerCommand>,
    error_receiver: Arc<Mutex<Option<mpsc::UnboundedReceiver<String>>>>,
}

impl Clone for PlayerHandle {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            error_receiver: self.error_receiver.clone(),
        }
    }
}

// PlayerHandle is safe to send between threads since the mpsc channel is Send-safe
// The underlying Player is accessed only through the PlayerController which runs on
// the main GTK thread via glib::spawn_future_local
unsafe impl Send for PlayerHandle {}
unsafe impl Sync for PlayerHandle {}

impl PlayerHandle {
    /// Take the error receiver (can only be done once)
    pub fn take_error_receiver(&self) -> Option<mpsc::UnboundedReceiver<String>> {
        self.error_receiver.lock().unwrap().take()
    }
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

    /// Set zoom mode
    pub async fn set_zoom_mode(&self, mode: ZoomMode) -> Result<()> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::SetZoomMode { mode, respond_to })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))?
    }

    /// Get buffering state (GStreamer only, returns None for MPV)
    #[cfg(feature = "gstreamer")]
    pub async fn get_buffering_state(&self) -> Result<Option<BufferingState>> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::GetBufferingState { respond_to })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))
    }

    /// Wait for player backend to be ready for seeking operations.
    /// Each backend implements its own readiness check:
    /// - GStreamer: waits for ASYNC_DONE message (pipeline_ready flag)
    /// - MPV: waits for duration to be available (file loaded and parsed)
    pub async fn wait_until_ready(&self, timeout: Duration) -> Result<()> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::WaitUntilReady {
                timeout,
                respond_to,
            })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))?
    }
}
