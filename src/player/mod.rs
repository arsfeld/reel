// Player module — migrated to components::player::video_player.

pub mod gst_pipeline;
pub mod pipeline_msgs;
pub mod subtitles;
pub mod tracks;

/// Playback state for MPRIS and other external consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayState {
    Playing,
    Paused,
    Stopped,
}

/// Timestamp ranges that define where the intro and credits fall within
/// the current media. Set externally (e.g. from Plex chapter data).
#[derive(Debug, Clone, Default)]
pub struct SkipMarkers {
    pub intro_start_secs: f64,
    pub intro_end_secs: f64,
    pub credits_start_secs: f64,
}

/// Human-readable window title for a playback state.
pub fn window_title_for_state(state: PlayState) -> &'static str {
    match state {
        PlayState::Playing => "Reel - Playing",
        PlayState::Paused => "Reel - Paused",
        PlayState::Stopped => "Reel",
    }
}
