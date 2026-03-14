use std::time::{Duration, Instant};

/// Threshold: consider watched at 90% of duration.
const SCROBBLE_THRESHOLD: f64 = 0.90;
/// Debounce interval for persisting progress to local DB.
const PERSIST_INTERVAL: Duration = Duration::from_secs(15);
/// Interval for reporting timeline to Plex.
const TIMELINE_INTERVAL: Duration = Duration::from_secs(10);

/// Events emitted by the WatchStateTracker.
/// These are data describing what should happen, not side effects.
#[derive(Debug, Clone, PartialEq)]
pub enum WatchStateEvent {
    /// Save position to local DB (debounced).
    PersistProgress {
        media_id: String,
        position: f64,
        duration: f64,
    },
    /// Report to Plex timeline API.
    ReportTimeline {
        rating_key: String,
        state: String,
        time_ms: i64,
        duration_ms: i64,
    },
    /// Mark as watched locally + Plex scrobble.
    Scrobble {
        media_id: String,
        rating_key: String,
    },
}

/// Playback state for timeline reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Playing,
    Paused,
    Stopped,
}

impl PlaybackState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Playing => "playing",
            Self::Paused => "paused",
            Self::Stopped => "stopped",
        }
    }
}

/// Pure watch state tracker. No I/O, no GTK, no mpv.
///
/// Processes position and state updates, emits events for persistence,
/// timeline reporting, and scrobble at configurable intervals and thresholds.
pub struct WatchStateTracker {
    media_id: Option<String>,
    rating_key: Option<String>,
    duration: f64,
    last_persisted_position: f64,
    last_persist_time: Instant,
    last_timeline_time: Instant,
    last_state: PlaybackState,
    scrobbled: bool,
    active: bool,
}

impl WatchStateTracker {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            media_id: None,
            rating_key: None,
            duration: 0.0,
            last_persisted_position: -1.0,
            last_persist_time: now,
            last_timeline_time: now,
            last_state: PlaybackState::Stopped,
            scrobbled: false,
            active: false,
        }
    }

    /// Start tracking a new media item.
    pub fn start(&mut self, media_id: &str, rating_key: Option<&str>, duration: f64, now: Instant) {
        self.media_id = Some(media_id.to_string());
        self.rating_key = rating_key.map(|k| k.to_string());
        self.duration = duration;
        self.last_persisted_position = -1.0;
        self.last_persist_time = now;
        self.last_timeline_time = now;
        self.last_state = PlaybackState::Playing;
        self.scrobbled = false;
        self.active = true;
    }

    /// Process a position update. Returns events to dispatch.
    pub fn process_position(&mut self, position: f64, now: Instant) -> Vec<WatchStateEvent> {
        if !self.active {
            return vec![];
        }

        let media_id = match &self.media_id {
            Some(id) => id.clone(),
            None => return vec![],
        };

        let mut events = Vec::new();

        // Check scrobble threshold (once per session)
        if !self.scrobbled && self.duration > 0.0 {
            let progress = position / self.duration;
            if progress >= SCROBBLE_THRESHOLD {
                self.scrobbled = true;
                events.push(WatchStateEvent::Scrobble {
                    media_id: media_id.clone(),
                    rating_key: self.rating_key.clone().unwrap_or_default(),
                });
            }
        }

        // Debounced persist to local DB
        if now.duration_since(self.last_persist_time) >= PERSIST_INTERVAL {
            self.last_persist_time = now;
            self.last_persisted_position = position;
            events.push(WatchStateEvent::PersistProgress {
                media_id: media_id.clone(),
                position,
                duration: self.duration,
            });
        }

        // Timeline reporting to Plex
        if self.rating_key.is_some()
            && now.duration_since(self.last_timeline_time) >= TIMELINE_INTERVAL
        {
            self.last_timeline_time = now;
            events.push(WatchStateEvent::ReportTimeline {
                rating_key: self.rating_key.clone().unwrap_or_default(),
                state: self.last_state.as_str().to_string(),
                time_ms: (position * 1000.0) as i64,
                duration_ms: (self.duration * 1000.0) as i64,
            });
        }

        events
    }

    /// Process a playback state change (play/pause).
    pub fn process_state_change(
        &mut self,
        state: PlaybackState,
        position: f64,
        now: Instant,
    ) -> Vec<WatchStateEvent> {
        if !self.active {
            return vec![];
        }

        let media_id = match &self.media_id {
            Some(id) => id.clone(),
            None => return vec![],
        };

        self.last_state = state;
        let mut events = Vec::new();

        // Persist on pause
        if state == PlaybackState::Paused {
            self.last_persist_time = now;
            self.last_persisted_position = position;
            events.push(WatchStateEvent::PersistProgress {
                media_id: media_id.clone(),
                position,
                duration: self.duration,
            });
        }

        // Report timeline on state change (if Plex item)
        if self.rating_key.is_some() {
            self.last_timeline_time = now;
            events.push(WatchStateEvent::ReportTimeline {
                rating_key: self.rating_key.clone().unwrap_or_default(),
                state: state.as_str().to_string(),
                time_ms: (position * 1000.0) as i64,
                duration_ms: (self.duration * 1000.0) as i64,
            });
        }

        events
    }

    /// Stop tracking. Returns final events (persist + stopped timeline).
    pub fn stop(&mut self, position: f64) -> Vec<WatchStateEvent> {
        if !self.active {
            return vec![];
        }

        let media_id = match &self.media_id {
            Some(id) => id.clone(),
            None => return vec![],
        };

        self.active = false;
        let mut events = Vec::new();

        // Final persist
        events.push(WatchStateEvent::PersistProgress {
            media_id: media_id.clone(),
            position,
            duration: self.duration,
        });

        // Check scrobble one last time
        if !self.scrobbled && self.duration > 0.0 && (position / self.duration) >= SCROBBLE_THRESHOLD
        {
            self.scrobbled = true;
            events.push(WatchStateEvent::Scrobble {
                media_id: media_id.clone(),
                rating_key: self.rating_key.clone().unwrap_or_default(),
            });
        }

        // Stopped timeline
        if self.rating_key.is_some() {
            events.push(WatchStateEvent::ReportTimeline {
                rating_key: self.rating_key.clone().unwrap_or_default(),
                state: "stopped".to_string(),
                time_ms: (position * 1000.0) as i64,
                duration_ms: (self.duration * 1000.0) as i64,
            });
        }

        events
    }

    /// Whether tracking is currently active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// The current media_id being tracked, if any.
    pub fn media_id(&self) -> Option<&str> {
        self.media_id.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DURATION: f64 = 7200.0; // 2 hours

    fn tracker_with_plex() -> (WatchStateTracker, Instant) {
        let mut t = WatchStateTracker::new();
        let now = Instant::now();
        t.start("media:1", Some("123"), DURATION, now);
        (t, now)
    }

    fn tracker_local_file() -> (WatchStateTracker, Instant) {
        let mut t = WatchStateTracker::new();
        let now = Instant::now();
        t.start("local:file:test.mkv", None, DURATION, now);
        (t, now)
    }

    #[test]
    fn new_tracker_is_inactive() {
        let t = WatchStateTracker::new();
        assert!(!t.is_active());
        assert!(t.media_id().is_none());
    }

    #[test]
    fn start_activates_tracker() {
        let (t, _) = tracker_with_plex();
        assert!(t.is_active());
        assert_eq!(t.media_id(), Some("media:1"));
    }

    #[test]
    fn process_position_no_events_before_intervals() {
        let (mut t, now) = tracker_with_plex();
        let events = t.process_position(100.0, now + Duration::from_secs(5));
        assert!(events.is_empty());
    }

    #[test]
    fn persist_after_interval() {
        let (mut t, now) = tracker_with_plex();
        let events = t.process_position(300.0, now + PERSIST_INTERVAL);
        assert!(events.iter().any(|e| matches!(
            e,
            WatchStateEvent::PersistProgress { position, .. } if (*position - 300.0).abs() < f64::EPSILON
        )));
    }

    #[test]
    fn timeline_after_interval() {
        let (mut t, now) = tracker_with_plex();
        let events = t.process_position(300.0, now + TIMELINE_INTERVAL);
        assert!(events.iter().any(|e| matches!(
            e,
            WatchStateEvent::ReportTimeline { rating_key, state, .. }
                if rating_key == "123" && state == "playing"
        )));
    }

    #[test]
    fn no_timeline_for_local_file() {
        let (mut t, now) = tracker_local_file();
        let events = t.process_position(300.0, now + TIMELINE_INTERVAL);
        assert!(!events.iter().any(|e| matches!(e, WatchStateEvent::ReportTimeline { .. })));
    }

    #[test]
    fn persist_still_fires_for_local_file() {
        let (mut t, now) = tracker_local_file();
        let events = t.process_position(300.0, now + PERSIST_INTERVAL);
        assert!(events.iter().any(|e| matches!(e, WatchStateEvent::PersistProgress { .. })));
    }

    #[test]
    fn scrobble_at_ninety_percent() {
        let (mut t, now) = tracker_with_plex();
        let position = DURATION * 0.91;
        let events = t.process_position(position, now + PERSIST_INTERVAL);
        assert!(events.iter().any(|e| matches!(
            e,
            WatchStateEvent::Scrobble { media_id, rating_key }
                if media_id == "media:1" && rating_key == "123"
        )));
    }

    #[test]
    fn scrobble_fires_once_per_session() {
        let (mut t, now) = tracker_with_plex();
        let events1 = t.process_position(DURATION * 0.91, now + PERSIST_INTERVAL);
        assert!(events1.iter().any(|e| matches!(e, WatchStateEvent::Scrobble { .. })));

        let events2 = t.process_position(DURATION * 0.95, now + PERSIST_INTERVAL * 2);
        assert!(!events2.iter().any(|e| matches!(e, WatchStateEvent::Scrobble { .. })));
    }

    #[test]
    fn no_scrobble_below_threshold() {
        let (mut t, now) = tracker_with_plex();
        let position = DURATION * 0.89;
        let events = t.process_position(position, now + PERSIST_INTERVAL);
        assert!(!events.iter().any(|e| matches!(e, WatchStateEvent::Scrobble { .. })));
    }

    #[test]
    fn scrobble_exactly_at_threshold() {
        let (mut t, now) = tracker_with_plex();
        let position = DURATION * SCROBBLE_THRESHOLD;
        let events = t.process_position(position, now + PERSIST_INTERVAL);
        assert!(events.iter().any(|e| matches!(e, WatchStateEvent::Scrobble { .. })));
    }

    #[test]
    fn pause_persists_immediately() {
        let (mut t, now) = tracker_with_plex();
        let events = t.process_state_change(PlaybackState::Paused, 500.0, now + Duration::from_secs(1));
        assert!(events.iter().any(|e| matches!(
            e,
            WatchStateEvent::PersistProgress { position, .. } if (*position - 500.0).abs() < f64::EPSILON
        )));
    }

    #[test]
    fn pause_reports_timeline() {
        let (mut t, now) = tracker_with_plex();
        let events = t.process_state_change(PlaybackState::Paused, 500.0, now + Duration::from_secs(1));
        assert!(events.iter().any(|e| matches!(
            e,
            WatchStateEvent::ReportTimeline { state, .. } if state == "paused"
        )));
    }

    #[test]
    fn resume_reports_timeline_playing() {
        let (mut t, now) = tracker_with_plex();
        t.process_state_change(PlaybackState::Paused, 500.0, now + Duration::from_secs(1));
        let events = t.process_state_change(PlaybackState::Playing, 500.0, now + Duration::from_secs(5));
        assert!(events.iter().any(|e| matches!(
            e,
            WatchStateEvent::ReportTimeline { state, .. } if state == "playing"
        )));
    }

    #[test]
    fn resume_does_not_persist() {
        let (mut t, now) = tracker_with_plex();
        t.process_state_change(PlaybackState::Paused, 500.0, now + Duration::from_secs(1));
        let events = t.process_state_change(PlaybackState::Playing, 500.0, now + Duration::from_secs(5));
        assert!(!events.iter().any(|e| matches!(e, WatchStateEvent::PersistProgress { .. })));
    }

    #[test]
    fn stop_emits_final_persist() {
        let (mut t, _now) = tracker_with_plex();
        let events = t.stop(1000.0);
        assert!(events.iter().any(|e| matches!(
            e,
            WatchStateEvent::PersistProgress { position, .. } if (*position - 1000.0).abs() < f64::EPSILON
        )));
    }

    #[test]
    fn stop_emits_stopped_timeline() {
        let (mut t, _now) = tracker_with_plex();
        let events = t.stop(1000.0);
        assert!(events.iter().any(|e| matches!(
            e,
            WatchStateEvent::ReportTimeline { state, .. } if state == "stopped"
        )));
    }

    #[test]
    fn stop_no_timeline_for_local() {
        let (mut t, _now) = tracker_local_file();
        let events = t.stop(1000.0);
        assert!(!events.iter().any(|e| matches!(e, WatchStateEvent::ReportTimeline { .. })));
    }

    #[test]
    fn stop_triggers_scrobble_if_past_threshold() {
        let (mut t, _now) = tracker_with_plex();
        let events = t.stop(DURATION * 0.95);
        assert!(events.iter().any(|e| matches!(e, WatchStateEvent::Scrobble { .. })));
    }

    #[test]
    fn stop_no_scrobble_below_threshold() {
        let (mut t, _now) = tracker_with_plex();
        let events = t.stop(DURATION * 0.5);
        assert!(!events.iter().any(|e| matches!(e, WatchStateEvent::Scrobble { .. })));
    }

    #[test]
    fn stop_no_duplicate_scrobble() {
        let (mut t, now) = tracker_with_plex();
        // Scrobble during playback
        t.process_position(DURATION * 0.91, now + PERSIST_INTERVAL);
        // Stop should NOT scrobble again
        let events = t.stop(DURATION * 0.95);
        assert!(!events.iter().any(|e| matches!(e, WatchStateEvent::Scrobble { .. })));
    }

    #[test]
    fn stop_deactivates_tracker() {
        let (mut t, _now) = tracker_with_plex();
        t.stop(1000.0);
        assert!(!t.is_active());
    }

    #[test]
    fn inactive_tracker_produces_no_events() {
        let mut t = WatchStateTracker::new();
        let now = Instant::now();
        let events = t.process_position(100.0, now + PERSIST_INTERVAL);
        assert!(events.is_empty());
    }

    #[test]
    fn inactive_tracker_state_change_no_events() {
        let mut t = WatchStateTracker::new();
        let now = Instant::now();
        let events = t.process_state_change(PlaybackState::Paused, 100.0, now);
        assert!(events.is_empty());
    }

    #[test]
    fn stop_on_inactive_tracker_no_events() {
        let mut t = WatchStateTracker::new();
        let events = t.stop(100.0);
        assert!(events.is_empty());
    }

    #[test]
    fn timeline_time_ms_conversion() {
        let (mut t, now) = tracker_with_plex();
        let events = t.process_position(100.5, now + TIMELINE_INTERVAL);
        let timeline = events.iter().find(|e| matches!(e, WatchStateEvent::ReportTimeline { .. }));
        if let Some(WatchStateEvent::ReportTimeline { time_ms, duration_ms, .. }) = timeline {
            assert_eq!(*time_ms, 100500);
            assert_eq!(*duration_ms, 7200000);
        } else {
            panic!("Expected ReportTimeline event");
        }
    }

    #[test]
    fn persist_resets_timer() {
        let (mut t, now) = tracker_with_plex();
        // First persist at 15s
        let events1 = t.process_position(100.0, now + PERSIST_INTERVAL);
        assert!(events1.iter().any(|e| matches!(e, WatchStateEvent::PersistProgress { .. })));

        // No persist at 20s (only 5s since last persist)
        let events2 = t.process_position(200.0, now + PERSIST_INTERVAL + Duration::from_secs(5));
        assert!(!events2.iter().any(|e| matches!(e, WatchStateEvent::PersistProgress { .. })));

        // Persist again at 30s
        let events3 = t.process_position(300.0, now + PERSIST_INTERVAL * 2);
        assert!(events3.iter().any(|e| matches!(e, WatchStateEvent::PersistProgress { .. })));
    }

    #[test]
    fn pause_resets_persist_timer() {
        let (mut t, now) = tracker_with_plex();
        // Pause at 5s -- persists immediately and resets timer
        t.process_state_change(PlaybackState::Paused, 500.0, now + Duration::from_secs(5));
        t.process_state_change(PlaybackState::Playing, 500.0, now + Duration::from_secs(6));

        // At 15s from start (10s since pause-persist), should NOT persist yet
        let events = t.process_position(600.0, now + Duration::from_secs(15));
        assert!(!events.iter().any(|e| matches!(e, WatchStateEvent::PersistProgress { .. })));

        // At 20s from start (15s since pause-persist), SHOULD persist
        let events = t.process_position(700.0, now + Duration::from_secs(20));
        assert!(events.iter().any(|e| matches!(e, WatchStateEvent::PersistProgress { .. })));
    }

    #[test]
    fn zero_duration_no_scrobble() {
        let mut t = WatchStateTracker::new();
        let now = Instant::now();
        t.start("media:1", Some("123"), 0.0, now);
        let events = t.process_position(100.0, now + PERSIST_INTERVAL);
        assert!(!events.iter().any(|e| matches!(e, WatchStateEvent::Scrobble { .. })));
    }

    #[test]
    fn start_resets_scrobble_flag() {
        let (mut t, now) = tracker_with_plex();
        t.process_position(DURATION * 0.91, now + PERSIST_INTERVAL);
        assert!(t.scrobbled);

        // Start a new item
        let now2 = now + Duration::from_secs(100);
        t.start("media:2", Some("456"), DURATION, now2);
        assert!(!t.scrobbled);
    }
}
