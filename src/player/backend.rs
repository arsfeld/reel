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

// --- Track types ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum TrackType {
    Audio,
    Video,
    Subtitle,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct TrackInfo {
    pub id: i64,
    pub track_type: TrackType,
    pub title: Option<String>,
    pub language: Option<String>,
    pub codec: Option<String>,
    pub selected: bool,
    pub default: bool,
    pub forced: bool,
    pub external: bool,
    // Audio-specific
    pub channels: Option<i64>,
    // Video-specific
    pub width: Option<i64>,
    pub height: Option<i64>,
}

impl TrackInfo {
    /// Create a minimal TrackInfo for testing.
    #[cfg(test)]
    pub fn test_audio(id: i64) -> Self {
        Self {
            id,
            track_type: TrackType::Audio,
            title: None,
            language: None,
            codec: None,
            selected: false,
            default: false,
            forced: false,
            external: false,
            channels: None,
            width: None,
            height: None,
        }
    }

    /// Create a minimal subtitle TrackInfo for testing.
    #[cfg(test)]
    pub fn test_subtitle(id: i64) -> Self {
        Self {
            track_type: TrackType::Subtitle,
            ..Self::test_audio(id)
        }
    }
}

/// Format a human-readable label for a track.
///
/// Examples: "English (AAC 5.1)", "Japanese (DTS-HD)", "Track 3 (Opus Stereo)"
#[allow(dead_code)]
pub fn format_track_label(track: &TrackInfo) -> String {
    let name = track
        .title
        .as_deref()
        .or(track.language.as_deref())
        .unwrap_or(match track.track_type {
            TrackType::Audio => "Audio",
            TrackType::Subtitle => "Subtitle",
            TrackType::Video => "Video",
        });

    let mut details = Vec::new();

    if let Some(codec) = &track.codec {
        details.push(codec.to_uppercase());
    }

    if track.track_type == TrackType::Audio
        && let Some(ch) = track.channels
    {
        details.push(format_channel_layout(ch));
    }

    if track.forced {
        details.push("Forced".to_string());
    }

    if track.external {
        details.push("External".to_string());
    }

    if details.is_empty() {
        format!("Track {}: {name}", track.id)
    } else {
        format!("{name} ({})", details.join(" "))
    }
}

/// Format audio channel count as a layout string.
#[allow(dead_code)]
fn format_channel_layout(channels: i64) -> String {
    match channels {
        1 => "Mono".to_string(),
        2 => "Stereo".to_string(),
        6 => "5.1".to_string(),
        8 => "7.1".to_string(),
        n => format!("{n}ch"),
    }
}

/// Format volume as a percentage string.
#[allow(dead_code)]
pub fn format_volume_label(volume: f64) -> String {
    format!("{}%", volume.round() as i64)
}

/// Format playback speed as a label.
#[allow(dead_code)]
pub fn format_speed_label(speed: f64) -> String {
    if (speed - speed.round()).abs() < 0.001 {
        format!("{}x", speed as i64)
    } else {
        format!("{speed:.2}x")
    }
}

/// Speed presets for stepping through with keyboard shortcuts.
#[allow(dead_code)]
const SPEED_PRESETS: &[f64] = &[0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 3.0, 4.0];

/// Get the next speed preset above the current speed.
#[allow(dead_code)]
pub fn next_speed(current: f64) -> f64 {
    for &preset in SPEED_PRESETS {
        if preset > current + 0.001 {
            return preset;
        }
    }
    *SPEED_PRESETS.last().unwrap()
}

/// Get the previous speed preset below the current speed.
#[allow(dead_code)]
pub fn prev_speed(current: f64) -> f64 {
    for &preset in SPEED_PRESETS.iter().rev() {
        if preset < current - 0.001 {
            return preset;
        }
    }
    *SPEED_PRESETS.first().unwrap()
}

/// Classify a file extension as video, subtitle, or unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum FileKind {
    Video,
    Subtitle,
    Unknown,
}

#[allow(dead_code)]
pub fn classify_file_extension(path: &str) -> FileKind {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "mkv" | "mp4" | "avi" | "mov" | "m4v" | "webm" | "flv" | "ts" | "m2ts" | "mpeg" | "mpg"
        | "ogm" | "wmv" | "3gp" | "vob" => FileKind::Video,
        "srt" | "ass" | "ssa" | "vtt" | "sub" | "idx" => FileKind::Subtitle,
        _ => FileKind::Unknown,
    }
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

    // --- format_track_label ---

    #[test]
    fn track_label_with_language_and_codec() {
        let track = TrackInfo {
            language: Some("English".into()),
            codec: Some("aac".into()),
            ..TrackInfo::test_audio(1)
        };
        assert_eq!(format_track_label(&track), "English (AAC)");
    }

    #[test]
    fn track_label_with_language_codec_and_channels() {
        let track = TrackInfo {
            language: Some("English".into()),
            codec: Some("aac".into()),
            channels: Some(6),
            ..TrackInfo::test_audio(1)
        };
        assert_eq!(format_track_label(&track), "English (AAC 5.1)");
    }

    #[test]
    fn track_label_stereo() {
        let track = TrackInfo {
            language: Some("Japanese".into()),
            codec: Some("dts".into()),
            channels: Some(2),
            ..TrackInfo::test_audio(1)
        };
        assert_eq!(format_track_label(&track), "Japanese (DTS Stereo)");
    }

    #[test]
    fn track_label_mono() {
        let track = TrackInfo {
            language: Some("Commentary".into()),
            codec: Some("aac".into()),
            channels: Some(1),
            ..TrackInfo::test_audio(1)
        };
        assert_eq!(format_track_label(&track), "Commentary (AAC Mono)");
    }

    #[test]
    fn track_label_7_1() {
        let track = TrackInfo {
            language: Some("English".into()),
            codec: Some("truehd".into()),
            channels: Some(8),
            ..TrackInfo::test_audio(1)
        };
        assert_eq!(format_track_label(&track), "English (TRUEHD 7.1)");
    }

    #[test]
    fn track_label_no_language_uses_title() {
        let track = TrackInfo {
            title: Some("Director Commentary".into()),
            codec: Some("aac".into()),
            ..TrackInfo::test_audio(1)
        };
        assert_eq!(format_track_label(&track), "Director Commentary (AAC)");
    }

    #[test]
    fn track_label_title_takes_precedence_over_language() {
        let track = TrackInfo {
            title: Some("SDH".into()),
            language: Some("English".into()),
            codec: Some("srt".into()),
            ..TrackInfo::test_subtitle(1)
        };
        assert_eq!(format_track_label(&track), "SDH (SRT)");
    }

    #[test]
    fn track_label_no_language_no_title_no_codec() {
        let track = TrackInfo::test_audio(3);
        assert_eq!(format_track_label(&track), "Track 3: Audio");
    }

    #[test]
    fn track_label_subtitle_no_info() {
        let track = TrackInfo::test_subtitle(2);
        assert_eq!(format_track_label(&track), "Track 2: Subtitle");
    }

    #[test]
    fn track_label_forced_subtitle() {
        let track = TrackInfo {
            language: Some("English".into()),
            codec: Some("pgs".into()),
            forced: true,
            ..TrackInfo::test_subtitle(1)
        };
        assert_eq!(format_track_label(&track), "English (PGS Forced)");
    }

    #[test]
    fn track_label_external_subtitle() {
        let track = TrackInfo {
            language: Some("English".into()),
            codec: Some("srt".into()),
            external: true,
            ..TrackInfo::test_subtitle(1)
        };
        assert_eq!(format_track_label(&track), "English (SRT External)");
    }

    #[test]
    fn track_label_forced_and_external() {
        let track = TrackInfo {
            language: Some("English".into()),
            codec: Some("srt".into()),
            forced: true,
            external: true,
            ..TrackInfo::test_subtitle(1)
        };
        assert_eq!(format_track_label(&track), "English (SRT Forced External)");
    }

    // --- format_volume_label ---

    #[test]
    fn volume_label_zero() {
        assert_eq!(format_volume_label(0.0), "0%");
    }

    #[test]
    fn volume_label_fifty() {
        assert_eq!(format_volume_label(50.0), "50%");
    }

    #[test]
    fn volume_label_hundred() {
        assert_eq!(format_volume_label(100.0), "100%");
    }

    #[test]
    fn volume_label_boost() {
        assert_eq!(format_volume_label(150.0), "150%");
    }

    #[test]
    fn volume_label_fractional_rounds() {
        assert_eq!(format_volume_label(73.6), "74%");
    }

    // --- format_speed_label ---

    #[test]
    fn speed_label_normal() {
        assert_eq!(format_speed_label(1.0), "1x");
    }

    #[test]
    fn speed_label_double() {
        assert_eq!(format_speed_label(2.0), "2x");
    }

    #[test]
    fn speed_label_half() {
        assert_eq!(format_speed_label(0.50), "0.50x");
    }

    #[test]
    fn speed_label_quarter() {
        assert_eq!(format_speed_label(0.25), "0.25x");
    }

    #[test]
    fn speed_label_one_point_five() {
        assert_eq!(format_speed_label(1.5), "1.50x");
    }

    #[test]
    fn speed_label_four() {
        assert_eq!(format_speed_label(4.0), "4x");
    }

    // --- next_speed / prev_speed ---

    #[test]
    fn next_speed_from_one() {
        assert!((next_speed(1.0) - 1.25).abs() < 0.001);
    }

    #[test]
    fn next_speed_from_two() {
        assert!((next_speed(2.0) - 3.0).abs() < 0.001);
    }

    #[test]
    fn next_speed_at_max_stays() {
        assert!((next_speed(4.0) - 4.0).abs() < 0.001);
    }

    #[test]
    fn next_speed_from_quarter() {
        assert!((next_speed(0.25) - 0.5).abs() < 0.001);
    }

    #[test]
    fn prev_speed_from_one() {
        assert!((prev_speed(1.0) - 0.75).abs() < 0.001);
    }

    #[test]
    fn prev_speed_from_half() {
        assert!((prev_speed(0.5) - 0.25).abs() < 0.001);
    }

    #[test]
    fn prev_speed_at_min_stays() {
        assert!((prev_speed(0.25) - 0.25).abs() < 0.001);
    }

    #[test]
    fn prev_speed_from_four() {
        assert!((prev_speed(4.0) - 3.0).abs() < 0.001);
    }

    // --- classify_file_extension ---

    #[test]
    fn classify_video_mkv() {
        assert_eq!(classify_file_extension("movie.mkv"), FileKind::Video);
    }

    #[test]
    fn classify_video_mp4() {
        assert_eq!(classify_file_extension("movie.mp4"), FileKind::Video);
    }

    #[test]
    fn classify_video_avi() {
        assert_eq!(classify_file_extension("movie.avi"), FileKind::Video);
    }

    #[test]
    fn classify_subtitle_srt() {
        assert_eq!(classify_file_extension("movie.srt"), FileKind::Subtitle);
    }

    #[test]
    fn classify_subtitle_ass() {
        assert_eq!(classify_file_extension("movie.ass"), FileKind::Subtitle);
    }

    #[test]
    fn classify_subtitle_vtt() {
        assert_eq!(classify_file_extension("movie.vtt"), FileKind::Subtitle);
    }

    #[test]
    fn classify_unknown_txt() {
        assert_eq!(classify_file_extension("readme.txt"), FileKind::Unknown);
    }

    #[test]
    fn classify_case_insensitive() {
        assert_eq!(classify_file_extension("movie.MKV"), FileKind::Video);
        assert_eq!(classify_file_extension("movie.SRT"), FileKind::Subtitle);
    }

    #[test]
    fn classify_no_extension() {
        assert_eq!(classify_file_extension("noext"), FileKind::Unknown);
    }

    // --- format_channel_layout ---

    #[test]
    fn channel_layout_unusual_count() {
        assert_eq!(format_channel_layout(4), "4ch");
    }
}
