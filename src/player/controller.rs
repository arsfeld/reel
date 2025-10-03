use anyhow::Result;
use gtk4;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, info, trace};

use super::{Player, PlayerState};
use crate::config::Config;
use crate::models::QualityOption;
use crate::player::adaptive_quality::{AdaptiveMode, AdaptiveQualityManager, QualityDecision};
use crate::player::{UpscalingMode, ZoomMode};

/// Handle for communicating with the adaptive quality manager
struct AdaptiveQualityHandle {
    state_tx: mpsc::UnboundedSender<PlayerState>,
    bandwidth_tx: mpsc::UnboundedSender<(u64, Duration)>,
    decision_rx: mpsc::UnboundedReceiver<QualityDecision>,
}

impl AdaptiveQualityHandle {
    fn broadcast_state(&self, state: PlayerState) {
        let _ = self.state_tx.send(state);
    }

    fn report_bandwidth(&self, bytes: u64, duration: Duration) {
        let _ = self.bandwidth_tx.send((bytes, duration));
    }

    async fn recv_decision(&mut self) -> Option<QualityDecision> {
        self.decision_rx.recv().await
    }
}

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
    /// Enable adaptive quality with quality options
    EnableAdaptiveQuality {
        quality_options: Vec<QualityOption>,
        current_quality_index: usize,
        respond_to: oneshot::Sender<Result<()>>,
    },
    /// Disable adaptive quality
    DisableAdaptiveQuality {
        respond_to: oneshot::Sender<Result<()>>,
    },
    /// Set adaptive quality mode (Auto/Manual)
    SetAdaptiveMode {
        mode: AdaptiveMode,
        respond_to: oneshot::Sender<Result<()>>,
    },
    /// Manually select quality (disables auto mode)
    SetQuality {
        quality_index: usize,
        respond_to: oneshot::Sender<Result<()>>,
    },
    /// Report chunk download for bandwidth monitoring
    ReportChunkDownload { bytes: u64, duration: Duration },
}

/// Controller that owns the Player and processes commands
pub struct PlayerController {
    player: Player,
    receiver: mpsc::UnboundedReceiver<PlayerCommand>,
    error_sender: Option<mpsc::UnboundedSender<String>>,

    // Adaptive quality components
    adaptive_quality: Option<AdaptiveQualityHandle>,
    current_state: PlayerState,
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
            adaptive_quality: None,
            current_state: PlayerState::Idle,
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

    /// Enable adaptive quality management
    fn enable_adaptive_quality(
        &mut self,
        quality_options: Vec<QualityOption>,
        current_quality_index: usize,
    ) -> Result<()> {
        info!(
            "Enabling adaptive quality with {} quality options",
            quality_options.len()
        );

        // Create state broadcaster channel
        let (state_tx, state_rx) = mpsc::unbounded_channel();

        // Create bandwidth update channel
        let (bandwidth_tx, bandwidth_rx) = mpsc::unbounded_channel();

        // Create quality decision channel
        let (decision_tx, decision_rx) = mpsc::unbounded_channel();

        // Create adaptive quality manager and spawn it
        let manager = AdaptiveQualityManager::new(
            quality_options,
            current_quality_index,
            state_rx,
            bandwidth_rx,
            decision_tx,
        );

        // Spawn manager task to run independently
        tokio::spawn(async move {
            manager.run().await;
        });

        // Store handle for communication
        self.adaptive_quality = Some(AdaptiveQualityHandle {
            state_tx,
            bandwidth_tx,
            decision_rx,
        });

        // Broadcast current state to initialize the manager
        self.broadcast_state(self.current_state.clone());

        Ok(())
    }

    /// Disable adaptive quality management
    fn disable_adaptive_quality(&mut self) -> Result<()> {
        info!("Disabling adaptive quality management");
        self.adaptive_quality = None;
        Ok(())
    }

    /// Broadcast state change to adaptive quality manager
    fn broadcast_state(&self, state: PlayerState) {
        if let Some(ref handle) = self.adaptive_quality {
            handle.broadcast_state(state);
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
                PlayerCommand::EnableAdaptiveQuality {
                    quality_options,
                    current_quality_index,
                    respond_to,
                } => {
                    trace!("Enabling adaptive quality");
                    let result =
                        self.enable_adaptive_quality(quality_options, current_quality_index);
                    let _ = respond_to.send(result);
                }
                PlayerCommand::DisableAdaptiveQuality { respond_to } => {
                    trace!("Disabling adaptive quality");
                    let result = self.disable_adaptive_quality();
                    let _ = respond_to.send(result);
                }
                PlayerCommand::SetAdaptiveMode { mode, respond_to } => {
                    trace!("Setting adaptive mode to {:?}", mode);
                    // TODO: Implement mode change - need to send to manager
                    let _ = respond_to.send(Ok(()));
                }
                PlayerCommand::SetQuality {
                    quality_index,
                    respond_to,
                } => {
                    trace!("Setting quality to index {}", quality_index);
                    // TODO: Implement quality change and disable auto mode
                    let _ = respond_to.send(Ok(()));
                }
                PlayerCommand::ReportChunkDownload { bytes, duration } => {
                    trace!("Chunk download reported: {} bytes in {:?}", bytes, duration);
                    // Forward to adaptive quality manager via bandwidth channel
                    if let Some(ref handle) = self.adaptive_quality {
                        handle.report_bandwidth(bytes, duration);
                    }
                }
            }

            // Check for quality decisions from adaptive quality manager
            if let Some(ref mut handle) = self.adaptive_quality {
                if let Ok(decision) = handle.decision_rx.try_recv() {
                    info!("Received quality decision: {:?}", decision);
                    // TODO: Handle quality decision - trigger quality change
                    match decision {
                        QualityDecision::Maintain => {}
                        QualityDecision::Decrease(quality) => {
                            info!("Adaptive: Decreasing quality to {}", quality.name);
                            // TODO: Trigger quality change
                        }
                        QualityDecision::Increase(quality) => {
                            info!("Adaptive: Increasing quality to {}", quality.name);
                            // TODO: Trigger quality change
                        }
                        QualityDecision::Recover(quality) => {
                            info!("Adaptive: Recovering with quality {}", quality.name);
                            // TODO: Trigger quality change with recovery flag
                        }
                    }
                }
            }

            // Broadcast state changes
            let new_state = self.player.get_state().await;
            if new_state != self.current_state {
                self.current_state = new_state.clone();
                self.broadcast_state(new_state);
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

impl std::fmt::Debug for PlayerHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlayerHandle")
            .field("sender", &"<UnboundedSender>")
            .field("error_receiver", &"<Arc<Mutex<...>>>")
            .finish()
    }
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

    /// Enable adaptive quality management
    pub async fn enable_adaptive_quality(
        &self,
        quality_options: Vec<QualityOption>,
        current_quality_index: usize,
    ) -> Result<()> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::EnableAdaptiveQuality {
                quality_options,
                current_quality_index,
                respond_to,
            })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))?
    }

    /// Disable adaptive quality management
    pub async fn disable_adaptive_quality(&self) -> Result<()> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::DisableAdaptiveQuality { respond_to })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))?
    }

    /// Set adaptive quality mode
    pub async fn set_adaptive_mode(&self, mode: AdaptiveMode) -> Result<()> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::SetAdaptiveMode { mode, respond_to })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))?
    }

    /// Set quality manually (disables auto mode)
    pub async fn set_quality(&self, quality_index: usize) -> Result<()> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(PlayerCommand::SetQuality {
                quality_index,
                respond_to,
            })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive response from player controller"))?
    }

    /// Report chunk download for bandwidth monitoring
    pub fn report_chunk_download(&self, bytes: u64, duration: Duration) -> Result<()> {
        self.sender
            .send(PlayerCommand::ReportChunkDownload { bytes, duration })
            .map_err(|_| anyhow::anyhow!("Player controller disconnected"))?;
        Ok(())
    }
}
