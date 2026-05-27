// Player module — migrated to components::player::video_player.

/// Playback state for MPRIS and other external consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayState {
    Playing,
    Paused,
    Stopped,
}

/// Human-readable window title for a playback state.
pub fn window_title_for_state(state: PlayState) -> &'static str {
    match state {
        PlayState::Playing => "Reel - Playing",
        PlayState::Paused => "Reel - Paused",
        PlayState::Stopped => "Reel",
    }
}
