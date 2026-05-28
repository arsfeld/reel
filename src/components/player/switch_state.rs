//! Pure switch-epoch + buffering-watchdog logic for mid-playback quality/seek
//! reloads (U8).
//!
//! Owned by the App: the App issues the async re-decision (`resolve_playback`)
//! and must discard results from a switch that a newer switch has superseded,
//! so exactly one stream loads and overlapping switches never collide (KTD7).
//! Mirrors the `PlaybackTracker` / `watch_state` pure-state-machine pattern —
//! no GTK, GStreamer, async, or I/O, so it is fully unit-testable.
//!
//! NOTE: the buffering watchdog (auto-step-down on a stalled transcode) is
//! deferred — it needs pipeline `BUFFERING` plumbing in `gst_pipeline.rs`. The
//! manual quality menu (U9) covers stepping down for now; `QualityPreset::
//! step_down` is the building block when the watchdog lands.

/// Monotonic epoch guard for overlapping quality/seek switches.
#[derive(Debug, Default)]
pub struct SwitchState {
    epoch: u64,
}

/// Whether a resolved decision should be applied or discarded as superseded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchOutcome {
    /// The result is for the current switch — apply it.
    Apply,
    /// A newer switch superseded this one — discard it (and stop its session).
    DiscardStale,
}

impl SwitchState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Begin a new switch (or initial play); returns the epoch to tag its async
    /// resolve with. Each call supersedes any in-flight switch.
    pub fn begin(&mut self) -> u64 {
        self.epoch += 1;
        self.epoch
    }

    /// Evaluate a resolved result that was tagged with `epoch`.
    pub fn evaluate(&self, epoch: u64) -> SwitchOutcome {
        if epoch == self.epoch {
            SwitchOutcome::Apply
        } else {
            SwitchOutcome::DiscardStale
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begin_increments_and_returns_epoch() {
        let mut s = SwitchState::new();
        assert_eq!(s.begin(), 1);
        assert_eq!(s.begin(), 2);
    }

    #[test]
    fn current_epoch_applies() {
        let mut s = SwitchState::new();
        let e = s.begin();
        assert_eq!(s.evaluate(e), SwitchOutcome::Apply);
    }

    #[test]
    fn superseded_epoch_is_discarded() {
        // Two rapid switches: A's result must be discarded once B has begun, so
        // only B's stream loads (exactly one live session).
        let mut s = SwitchState::new();
        let a = s.begin();
        let b = s.begin();
        assert_eq!(s.evaluate(a), SwitchOutcome::DiscardStale);
        assert_eq!(s.evaluate(b), SwitchOutcome::Apply);
    }
}
