use crate::player::backend::{EndReason, PlayState};

/// Polled property values from the video backend.
/// Pure data - no mpv or GTK dependencies.
#[derive(Debug, Clone, Default)]
pub struct PollData {
    pub path: Option<String>,
    pub duration: Option<f64>,
    pub position: Option<f64>,
    pub paused: Option<bool>,
    pub eof_reached: Option<bool>,
    pub hwdec_current: Option<String>,
    pub volume: Option<f64>,
    pub muted: Option<bool>,
    pub speed: Option<f64>,
}

/// Events produced by processing poll data.
/// These are data describing what happened, not side effects.
#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackEvent {
    FileLoaded {
        path: String,
        duration: f64,
        hwdec: Option<String>,
    },
    PositionChanged {
        position: f64,
        duration: f64,
    },
    StateChanged(PlayState),
    EndOfFile(EndReason),
    VolumeChanged {
        volume: f64,
        muted: bool,
    },
    SpeedChanged(f64),
}

/// Pure playback state tracker. No I/O, no GTK, no mpv.
///
/// Tracks transitions by comparing successive poll snapshots and
/// emitting events only when meaningful changes occur.
pub struct PlaybackTracker {
    pub file_loaded: bool,
    pub last_position: f64,
    pub last_paused: Option<bool>,
    pub last_volume: Option<f64>,
    pub last_muted: Option<bool>,
    pub last_speed: Option<f64>,
}

impl PlaybackTracker {
    pub fn new() -> Self {
        Self {
            file_loaded: false,
            last_position: -1.0,
            last_paused: None,
            last_volume: None,
            last_muted: None,
            last_speed: None,
        }
    }

    /// Process a polled backend state snapshot and return events to emit.
    pub fn process(&mut self, data: &PollData) -> Vec<PlaybackEvent> {
        let mut events = Vec::new();

        // Detect file loaded: path is non-empty and duration is positive
        if !self.file_loaded
            && let (Some(path), Some(dur)) = (&data.path, data.duration)
            && !path.is_empty()
            && dur > 0.0
        {
            self.file_loaded = true;
            events.push(PlaybackEvent::FileLoaded {
                path: path.clone(),
                duration: dur,
                hwdec: data.hwdec_current.clone(),
            });
        }

        // Detect position change (threshold: 50ms to avoid noise)
        if let Some(pos) = data.position
            && (pos - self.last_position).abs() > 0.05
        {
            self.last_position = pos;
            events.push(PlaybackEvent::PositionChanged {
                position: pos,
                duration: data.duration.unwrap_or(0.0),
            });
        }

        // Detect pause state change
        if let Some(paused) = data.paused
            && self.last_paused != Some(paused)
        {
            self.last_paused = Some(paused);
            let state = if paused {
                PlayState::Paused
            } else {
                PlayState::Playing
            };
            events.push(PlaybackEvent::StateChanged(state));
        }

        // Detect volume/mute change
        if let Some(vol) = data.volume {
            let muted = data.muted.unwrap_or(false);
            let vol_changed = self.last_volume.is_none_or(|lv| (lv - vol).abs() > 0.5);
            let mute_changed = self.last_muted != Some(muted);
            if vol_changed || mute_changed {
                self.last_volume = Some(vol);
                self.last_muted = Some(muted);
                events.push(PlaybackEvent::VolumeChanged { volume: vol, muted });
            }
        }

        // Detect speed change
        if let Some(speed) = data.speed
            && self.last_speed.is_none_or(|ls| (ls - speed).abs() > 0.001)
        {
            self.last_speed = Some(speed);
            events.push(PlaybackEvent::SpeedChanged(speed));
        }

        // Detect end of file
        if data.eof_reached == Some(true) && self.file_loaded {
            self.file_loaded = false;
            events.push(PlaybackEvent::EndOfFile(EndReason::Finished));
        }

        events
    }

    /// Reset tracker state (e.g., when loading a new file).
    pub fn reset(&mut self) {
        self.file_loaded = false;
        self.last_position = -1.0;
        self.last_paused = None;
        self.last_volume = None;
        self.last_muted = None;
        self.last_speed = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn playing_state(position: f64) -> PollData {
        PollData {
            path: Some("/test/movie.mkv".into()),
            duration: Some(100.0),
            position: Some(position),
            paused: Some(false),
            eof_reached: Some(false),
            ..Default::default()
        }
    }

    fn paused_state(position: f64) -> PollData {
        PollData {
            paused: Some(true),
            ..playing_state(position)
        }
    }

    // --- File loaded detection ---

    #[test]
    fn detects_file_loaded_on_first_valid_poll() {
        let mut tracker = PlaybackTracker::new();
        let events = tracker.process(&playing_state(0.0));

        assert!(
            events
                .iter()
                .any(|e| matches!(e, PlaybackEvent::FileLoaded { .. }))
        );
        assert!(tracker.file_loaded);
    }

    #[test]
    fn file_loaded_includes_path_and_duration() {
        let mut tracker = PlaybackTracker::new();
        let events = tracker.process(&playing_state(0.0));

        let loaded = events
            .iter()
            .find(|e| matches!(e, PlaybackEvent::FileLoaded { .. }));
        assert!(matches!(
            loaded,
            Some(PlaybackEvent::FileLoaded {
                path,
                duration,
                ..
            }) if path == "/test/movie.mkv" && (*duration - 100.0).abs() < f64::EPSILON
        ));
    }

    #[test]
    fn file_loaded_includes_hwdec_info() {
        let mut tracker = PlaybackTracker::new();
        let data = PollData {
            hwdec_current: Some("vaapi".into()),
            ..playing_state(0.0)
        };
        let events = tracker.process(&data);

        let loaded = events
            .iter()
            .find(|e| matches!(e, PlaybackEvent::FileLoaded { .. }));
        assert!(matches!(
            loaded,
            Some(PlaybackEvent::FileLoaded { hwdec: Some(h), .. }) if h == "vaapi"
        ));
    }

    #[test]
    fn no_duplicate_file_loaded_on_subsequent_polls() {
        let mut tracker = PlaybackTracker::new();
        tracker.process(&playing_state(0.0));
        let events = tracker.process(&playing_state(1.0));

        assert!(
            !events
                .iter()
                .any(|e| matches!(e, PlaybackEvent::FileLoaded { .. }))
        );
    }

    #[test]
    fn no_file_loaded_when_path_is_empty() {
        let mut tracker = PlaybackTracker::new();
        let data = PollData {
            path: Some(String::new()),
            duration: Some(100.0),
            ..Default::default()
        };
        let events = tracker.process(&data);

        assert!(
            !events
                .iter()
                .any(|e| matches!(e, PlaybackEvent::FileLoaded { .. }))
        );
        assert!(!tracker.file_loaded);
    }

    #[test]
    fn no_file_loaded_when_path_is_none() {
        let mut tracker = PlaybackTracker::new();
        let data = PollData {
            path: None,
            duration: Some(100.0),
            ..Default::default()
        };
        let events = tracker.process(&data);

        assert!(
            !events
                .iter()
                .any(|e| matches!(e, PlaybackEvent::FileLoaded { .. }))
        );
    }

    #[test]
    fn no_file_loaded_when_duration_is_zero() {
        let mut tracker = PlaybackTracker::new();
        let data = PollData {
            path: Some("/test.mkv".into()),
            duration: Some(0.0),
            ..Default::default()
        };
        let events = tracker.process(&data);

        assert!(
            !events
                .iter()
                .any(|e| matches!(e, PlaybackEvent::FileLoaded { .. }))
        );
    }

    #[test]
    fn no_file_loaded_when_duration_is_none() {
        let mut tracker = PlaybackTracker::new();
        let data = PollData {
            path: Some("/test.mkv".into()),
            duration: None,
            ..Default::default()
        };
        let events = tracker.process(&data);

        assert!(
            !events
                .iter()
                .any(|e| matches!(e, PlaybackEvent::FileLoaded { .. }))
        );
    }

    // --- Position change detection ---

    #[test]
    fn detects_position_change_above_threshold() {
        let mut tracker = PlaybackTracker::new();
        tracker.process(&playing_state(10.0));
        let events = tracker.process(&playing_state(10.1));

        assert!(events
            .iter()
            .any(|e| matches!(e, PlaybackEvent::PositionChanged { position, .. } if (*position - 10.1).abs() < f64::EPSILON)));
    }

    #[test]
    fn position_change_includes_duration() {
        let mut tracker = PlaybackTracker::new();
        tracker.process(&playing_state(0.0));
        let events = tracker.process(&playing_state(1.0));

        let pos_event = events
            .iter()
            .find(|e| matches!(e, PlaybackEvent::PositionChanged { .. }));
        assert!(matches!(
            pos_event,
            Some(PlaybackEvent::PositionChanged { duration, .. }) if (*duration - 100.0).abs() < f64::EPSILON
        ));
    }

    #[test]
    fn ignores_position_change_below_threshold() {
        let mut tracker = PlaybackTracker::new();
        tracker.process(&playing_state(10.0));
        let events = tracker.process(&playing_state(10.03)); // 0.03 < 0.05 threshold

        // Filter out non-position events (FileLoaded, StateChanged may be there)
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, PlaybackEvent::PositionChanged { .. }))
        );
    }

    #[test]
    fn no_position_event_when_position_is_none() {
        let mut tracker = PlaybackTracker::new();
        let data = PollData {
            position: None,
            ..playing_state(0.0)
        };
        // First poll with position None
        let events = tracker.process(&data);

        assert!(
            !events
                .iter()
                .any(|e| matches!(e, PlaybackEvent::PositionChanged { .. }))
        );
    }

    #[test]
    fn position_uses_zero_duration_when_duration_is_none() {
        let mut tracker = PlaybackTracker::new();
        let data = PollData {
            path: Some("/test.mkv".into()),
            duration: None,
            position: Some(5.0),
            paused: Some(false),
            ..Default::default()
        };
        let events = tracker.process(&data);

        let pos_event = events
            .iter()
            .find(|e| matches!(e, PlaybackEvent::PositionChanged { .. }));
        assert!(matches!(
            pos_event,
            Some(PlaybackEvent::PositionChanged { duration, .. }) if *duration == 0.0
        ));
    }

    // --- Pause state change detection ---

    #[test]
    fn detects_initial_playing_state() {
        let mut tracker = PlaybackTracker::new();
        let events = tracker.process(&playing_state(0.0));

        assert!(events.contains(&PlaybackEvent::StateChanged(PlayState::Playing)));
    }

    #[test]
    fn detects_transition_from_playing_to_paused() {
        let mut tracker = PlaybackTracker::new();
        tracker.process(&playing_state(0.0));
        let events = tracker.process(&paused_state(1.0));

        assert!(events.contains(&PlaybackEvent::StateChanged(PlayState::Paused)));
    }

    #[test]
    fn detects_transition_from_paused_to_playing() {
        let mut tracker = PlaybackTracker::new();
        tracker.process(&paused_state(0.0));
        let events = tracker.process(&playing_state(0.1));

        assert!(events.contains(&PlaybackEvent::StateChanged(PlayState::Playing)));
    }

    #[test]
    fn no_state_change_when_pause_unchanged() {
        let mut tracker = PlaybackTracker::new();
        tracker.process(&playing_state(0.0));
        let events = tracker.process(&playing_state(1.0));

        assert!(
            !events
                .iter()
                .any(|e| matches!(e, PlaybackEvent::StateChanged(_)))
        );
    }

    #[test]
    fn no_state_change_when_paused_is_none() {
        let mut tracker = PlaybackTracker::new();
        let data = PollData {
            paused: None,
            ..playing_state(0.0)
        };
        let events = tracker.process(&data);

        assert!(
            !events
                .iter()
                .any(|e| matches!(e, PlaybackEvent::StateChanged(_)))
        );
    }

    // --- End of file detection ---

    #[test]
    fn detects_eof_when_file_loaded() {
        let mut tracker = PlaybackTracker::new();
        tracker.process(&playing_state(0.0)); // loads file
        assert!(tracker.file_loaded);

        let eof_data = PollData {
            eof_reached: Some(true),
            ..playing_state(100.0)
        };
        let events = tracker.process(&eof_data);

        assert!(events.contains(&PlaybackEvent::EndOfFile(EndReason::Finished)));
    }

    #[test]
    fn eof_resets_file_loaded_flag() {
        let mut tracker = PlaybackTracker::new();
        tracker.process(&playing_state(0.0));

        let eof_data = PollData {
            eof_reached: Some(true),
            ..playing_state(100.0)
        };
        tracker.process(&eof_data);

        assert!(!tracker.file_loaded);
    }

    #[test]
    fn no_eof_when_file_not_loaded() {
        let mut tracker = PlaybackTracker::new();
        assert!(!tracker.file_loaded);

        let data = PollData {
            eof_reached: Some(true),
            ..Default::default()
        };
        let events = tracker.process(&data);

        assert!(
            !events
                .iter()
                .any(|e| matches!(e, PlaybackEvent::EndOfFile(_)))
        );
    }

    #[test]
    fn no_eof_when_eof_reached_is_false() {
        let mut tracker = PlaybackTracker::new();
        tracker.process(&playing_state(0.0));

        let data = PollData {
            eof_reached: Some(false),
            ..playing_state(50.0)
        };
        let events = tracker.process(&data);

        assert!(
            !events
                .iter()
                .any(|e| matches!(e, PlaybackEvent::EndOfFile(_)))
        );
    }

    #[test]
    fn no_eof_when_eof_reached_is_none() {
        let mut tracker = PlaybackTracker::new();
        tracker.process(&playing_state(0.0));

        let data = PollData {
            eof_reached: None,
            ..playing_state(50.0)
        };
        let events = tracker.process(&data);

        assert!(
            !events
                .iter()
                .any(|e| matches!(e, PlaybackEvent::EndOfFile(_)))
        );
    }

    // --- Reset ---

    #[test]
    fn reset_clears_all_state() {
        let mut tracker = PlaybackTracker::new();
        tracker.process(&playing_state(50.0));
        assert!(tracker.file_loaded);
        assert!(tracker.last_paused.is_some());
        assert!(tracker.last_position > 0.0);

        tracker.reset();

        assert!(!tracker.file_loaded);
        assert_eq!(tracker.last_position, -1.0);
        assert!(tracker.last_paused.is_none());
    }

    #[test]
    fn file_loaded_fires_again_after_reset() {
        let mut tracker = PlaybackTracker::new();
        tracker.process(&playing_state(0.0));
        tracker.reset();

        let events = tracker.process(&playing_state(0.0));
        assert!(
            events
                .iter()
                .any(|e| matches!(e, PlaybackEvent::FileLoaded { .. }))
        );
    }

    // --- Full lifecycle ---

    #[test]
    fn full_playback_lifecycle() {
        let mut tracker = PlaybackTracker::new();

        // 1. File loads and starts playing
        let events = tracker.process(&playing_state(0.0));
        assert!(
            events
                .iter()
                .any(|e| matches!(e, PlaybackEvent::FileLoaded { .. }))
        );
        assert!(events.contains(&PlaybackEvent::StateChanged(PlayState::Playing)));

        // 2. Position advances
        let events = tracker.process(&playing_state(50.0));
        assert!(
            events
                .iter()
                .any(|e| matches!(e, PlaybackEvent::PositionChanged { .. }))
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, PlaybackEvent::StateChanged(_)))
        );

        // 3. User pauses
        let events = tracker.process(&paused_state(50.0));
        assert!(events.contains(&PlaybackEvent::StateChanged(PlayState::Paused)));

        // 4. User resumes
        let events = tracker.process(&playing_state(50.1));
        assert!(events.contains(&PlaybackEvent::StateChanged(PlayState::Playing)));

        // 5. Playback reaches end
        let eof_data = PollData {
            eof_reached: Some(true),
            ..playing_state(100.0)
        };
        let events = tracker.process(&eof_data);
        assert!(events.contains(&PlaybackEvent::EndOfFile(EndReason::Finished)));
        assert!(!tracker.file_loaded);
    }

    #[test]
    fn multiple_files_lifecycle() {
        let mut tracker = PlaybackTracker::new();

        // Play first file
        tracker.process(&playing_state(0.0));
        assert!(tracker.file_loaded);

        // EOF on first file
        let eof_data = PollData {
            eof_reached: Some(true),
            ..playing_state(100.0)
        };
        tracker.process(&eof_data);
        assert!(!tracker.file_loaded);

        // Reset for next file
        tracker.reset();

        // Second file loads
        let data = PollData {
            path: Some("/test/movie2.mkv".into()),
            ..playing_state(0.0)
        };
        let events = tracker.process(&data);
        assert!(events.iter().any(
            |e| matches!(e, PlaybackEvent::FileLoaded { path, .. } if path == "/test/movie2.mkv")
        ));
    }

    // --- Edge cases ---

    #[test]
    fn all_none_poll_data_produces_no_events() {
        let mut tracker = PlaybackTracker::new();
        let events = tracker.process(&PollData::default());

        assert!(events.is_empty());
    }

    #[test]
    fn negative_duration_does_not_trigger_file_loaded() {
        let mut tracker = PlaybackTracker::new();
        let data = PollData {
            path: Some("/test.mkv".into()),
            duration: Some(-1.0),
            ..Default::default()
        };
        let events = tracker.process(&data);

        assert!(
            !events
                .iter()
                .any(|e| matches!(e, PlaybackEvent::FileLoaded { .. }))
        );
    }

    #[test]
    fn rapid_pause_toggle_emits_all_transitions() {
        let mut tracker = PlaybackTracker::new();
        tracker.process(&playing_state(0.0)); // Playing

        let events = tracker.process(&paused_state(0.0)); // Paused
        assert!(events.contains(&PlaybackEvent::StateChanged(PlayState::Paused)));

        let events = tracker.process(&playing_state(0.1)); // Playing
        assert!(events.contains(&PlaybackEvent::StateChanged(PlayState::Playing)));

        let events = tracker.process(&paused_state(0.1)); // Paused
        assert!(events.contains(&PlaybackEvent::StateChanged(PlayState::Paused)));
    }

    // --- Volume change detection ---

    #[test]
    fn detects_initial_volume() {
        let mut tracker = PlaybackTracker::new();
        let data = PollData {
            volume: Some(100.0),
            muted: Some(false),
            ..playing_state(0.0)
        };
        let events = tracker.process(&data);

        assert!(events.iter().any(|e| matches!(
            e,
            PlaybackEvent::VolumeChanged { volume, muted } if (*volume - 100.0).abs() < 0.01 && !muted
        )));
    }

    #[test]
    fn detects_volume_change() {
        let mut tracker = PlaybackTracker::new();
        let data1 = PollData {
            volume: Some(100.0),
            muted: Some(false),
            ..playing_state(0.0)
        };
        tracker.process(&data1);

        let data2 = PollData {
            volume: Some(75.0),
            muted: Some(false),
            ..playing_state(1.0)
        };
        let events = tracker.process(&data2);

        assert!(events.iter().any(|e| matches!(
            e,
            PlaybackEvent::VolumeChanged { volume, .. } if (*volume - 75.0).abs() < 0.01
        )));
    }

    #[test]
    fn ignores_small_volume_change() {
        let mut tracker = PlaybackTracker::new();
        let data1 = PollData {
            volume: Some(100.0),
            muted: Some(false),
            ..playing_state(0.0)
        };
        tracker.process(&data1);

        let data2 = PollData {
            volume: Some(100.3), // < 0.5 threshold
            muted: Some(false),
            ..playing_state(1.0)
        };
        let events = tracker.process(&data2);

        assert!(
            !events
                .iter()
                .any(|e| matches!(e, PlaybackEvent::VolumeChanged { .. }))
        );
    }

    #[test]
    fn detects_mute_toggle() {
        let mut tracker = PlaybackTracker::new();
        let data1 = PollData {
            volume: Some(100.0),
            muted: Some(false),
            ..playing_state(0.0)
        };
        tracker.process(&data1);

        let data2 = PollData {
            volume: Some(100.0),
            muted: Some(true),
            ..playing_state(1.0)
        };
        let events = tracker.process(&data2);

        assert!(
            events
                .iter()
                .any(|e| matches!(e, PlaybackEvent::VolumeChanged { muted: true, .. }))
        );
    }

    #[test]
    fn no_volume_event_when_volume_is_none() {
        let mut tracker = PlaybackTracker::new();
        let data = PollData {
            volume: None,
            ..playing_state(0.0)
        };
        let events = tracker.process(&data);

        assert!(
            !events
                .iter()
                .any(|e| matches!(e, PlaybackEvent::VolumeChanged { .. }))
        );
    }

    // --- Speed change detection ---

    #[test]
    fn detects_initial_speed() {
        let mut tracker = PlaybackTracker::new();
        let data = PollData {
            speed: Some(1.0),
            ..playing_state(0.0)
        };
        let events = tracker.process(&data);

        assert!(events.contains(&PlaybackEvent::SpeedChanged(1.0)));
    }

    #[test]
    fn detects_speed_change() {
        let mut tracker = PlaybackTracker::new();
        let data1 = PollData {
            speed: Some(1.0),
            ..playing_state(0.0)
        };
        tracker.process(&data1);

        let data2 = PollData {
            speed: Some(2.0),
            ..playing_state(1.0)
        };
        let events = tracker.process(&data2);

        assert!(events.contains(&PlaybackEvent::SpeedChanged(2.0)));
    }

    #[test]
    fn no_speed_event_when_unchanged() {
        let mut tracker = PlaybackTracker::new();
        let data1 = PollData {
            speed: Some(1.0),
            ..playing_state(0.0)
        };
        tracker.process(&data1);

        let data2 = PollData {
            speed: Some(1.0),
            ..playing_state(1.0)
        };
        let events = tracker.process(&data2);

        assert!(
            !events
                .iter()
                .any(|e| matches!(e, PlaybackEvent::SpeedChanged(_)))
        );
    }

    #[test]
    fn no_speed_event_when_speed_is_none() {
        let mut tracker = PlaybackTracker::new();
        let data = PollData {
            speed: None,
            ..playing_state(0.0)
        };
        let events = tracker.process(&data);

        assert!(
            !events
                .iter()
                .any(|e| matches!(e, PlaybackEvent::SpeedChanged(_)))
        );
    }

    #[test]
    fn reset_clears_volume_and_speed() {
        let mut tracker = PlaybackTracker::new();
        let data = PollData {
            volume: Some(80.0),
            muted: Some(true),
            speed: Some(1.5),
            ..playing_state(0.0)
        };
        tracker.process(&data);

        tracker.reset();
        assert!(tracker.last_volume.is_none());
        assert!(tracker.last_muted.is_none());
        assert!(tracker.last_speed.is_none());
    }
}
