use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum PlayState {
    Playing,
    Paused,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum EndReason {
    Finished,
    Stopped,
    Error,
}

/// Derive the window title from the current play state.
pub fn window_title_for_state(state: PlayState) -> &'static str {
    match state {
        PlayState::Playing => "Reel - Playing",
        PlayState::Paused => "Reel - Paused",
        PlayState::Stopped => "Reel",
    }
}

/// Format a duration as "H:MM:SS" or "M:SS".
#[allow(dead_code)]
pub fn format_duration(d: Duration) -> String {
    let total_secs = d.as_secs();
    let hours = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    if hours > 0 {
        format!("{hours}:{mins:02}:{secs:02}")
    } else {
        format!("{mins}:{secs:02}")
    }
}

/// Format a position in seconds as "H:MM:SS" or "M:SS".
#[allow(dead_code)]
pub fn format_position(seconds: f64) -> String {
    if seconds < 0.0 {
        return "0:00".to_string();
    }
    format_duration(Duration::from_secs(seconds as u64))
}

/// Format remaining time as "-H:MM:SS" or "-M:SS".
#[allow(dead_code)]
pub fn format_remaining(position: f64, duration: f64) -> String {
    let remaining = (duration - position).max(0.0);
    format!(
        "-{}",
        format_duration(Duration::from_secs(remaining as u64))
    )
}

/// Calculate progress as a fraction (0.0 to 1.0).
#[allow(dead_code)]
pub fn progress_fraction(position: f64, duration: f64) -> f64 {
    if duration <= 0.0 {
        return 0.0;
    }
    (position / duration).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- window_title_for_state ---

    #[test]
    fn title_for_playing() {
        assert_eq!(window_title_for_state(PlayState::Playing), "Reel - Playing");
    }

    #[test]
    fn title_for_paused() {
        assert_eq!(window_title_for_state(PlayState::Paused), "Reel - Paused");
    }

    #[test]
    fn title_for_stopped() {
        assert_eq!(window_title_for_state(PlayState::Stopped), "Reel");
    }

    // --- format_duration ---

    #[test]
    fn format_duration_zero() {
        assert_eq!(format_duration(Duration::from_secs(0)), "0:00");
    }

    #[test]
    fn format_duration_seconds_only() {
        assert_eq!(format_duration(Duration::from_secs(5)), "0:05");
    }

    #[test]
    fn format_duration_minutes_and_seconds() {
        assert_eq!(format_duration(Duration::from_secs(125)), "2:05");
    }

    #[test]
    fn format_duration_exact_minute() {
        assert_eq!(format_duration(Duration::from_secs(60)), "1:00");
    }

    #[test]
    fn format_duration_with_hours() {
        assert_eq!(format_duration(Duration::from_secs(3661)), "1:01:01");
    }

    #[test]
    fn format_duration_hours_zero_padded() {
        assert_eq!(format_duration(Duration::from_secs(3600)), "1:00:00");
    }

    #[test]
    fn format_duration_long_movie() {
        // 2h 30m 45s = 9045s
        assert_eq!(format_duration(Duration::from_secs(9045)), "2:30:45");
    }

    // --- format_position ---

    #[test]
    fn format_position_normal() {
        assert_eq!(format_position(125.7), "2:05");
    }

    #[test]
    fn format_position_zero() {
        assert_eq!(format_position(0.0), "0:00");
    }

    #[test]
    fn format_position_negative_clamps_to_zero() {
        assert_eq!(format_position(-5.0), "0:00");
    }

    // --- format_remaining ---

    #[test]
    fn format_remaining_normal() {
        assert_eq!(format_remaining(50.0, 60.0), "-0:10");
    }

    #[test]
    fn format_remaining_at_start() {
        assert_eq!(format_remaining(0.0, 120.0), "-2:00");
    }

    #[test]
    fn format_remaining_at_end() {
        assert_eq!(format_remaining(120.0, 120.0), "-0:00");
    }

    #[test]
    fn format_remaining_past_end_clamps() {
        assert_eq!(format_remaining(130.0, 120.0), "-0:00");
    }

    // --- progress_fraction ---

    #[test]
    fn progress_at_start() {
        assert_eq!(progress_fraction(0.0, 100.0), 0.0);
    }

    #[test]
    fn progress_at_midpoint() {
        let p = progress_fraction(50.0, 100.0);
        assert!((p - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn progress_at_end() {
        let p = progress_fraction(100.0, 100.0);
        assert!((p - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn progress_with_zero_duration() {
        assert_eq!(progress_fraction(50.0, 0.0), 0.0);
    }

    #[test]
    fn progress_with_negative_duration() {
        assert_eq!(progress_fraction(50.0, -10.0), 0.0);
    }

    #[test]
    fn progress_clamps_above_one() {
        assert_eq!(progress_fraction(150.0, 100.0), 1.0);
    }

    #[test]
    fn progress_clamps_below_zero() {
        assert_eq!(progress_fraction(-10.0, 100.0), 0.0);
    }

    // --- PlayState / EndReason exhaustiveness ---

    #[test]
    fn all_play_states_have_titles() {
        let states = [PlayState::Playing, PlayState::Paused, PlayState::Stopped];
        for state in &states {
            let title = window_title_for_state(*state);
            assert!(!title.is_empty());
        }
    }

    #[test]
    fn play_state_equality() {
        assert_eq!(PlayState::Playing, PlayState::Playing);
        assert_ne!(PlayState::Playing, PlayState::Paused);
    }

    #[test]
    fn end_reason_equality() {
        assert_eq!(EndReason::Finished, EndReason::Finished);
        assert_ne!(EndReason::Finished, EndReason::Error);
    }
}
