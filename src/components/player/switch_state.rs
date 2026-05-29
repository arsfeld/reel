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

/// Per-item render-failure fallback state (U4).
///
/// When direct-play renders fail (a stream/negotiation bus error), the App
/// re-resolves the same item forcing a server transcode. This tracks two things
/// so that behavior is safe:
/// - **Stickiness**: once an item has fallen back to transcode, every later
///   re-resolve for that item this session must also force transcode, so a
///   quality/track change doesn't retry direct-play and re-trigger the black
///   screen.
/// - **A bounded retry**: a render failure triggers exactly one transcode retry;
///   if the transcode *also* fails to render, give up with a terminal error
///   rather than looping.
///
/// Pure — no GTK/GStreamer/async — so it is fully unit-testable.
#[derive(Debug, Default)]
pub struct RenderFallback {
    /// The item the current state applies to; switching items clears it.
    item_key: Option<String>,
    /// Whether re-resolves for the current item must force a transcode.
    sticky_transcode: bool,
    /// Render-failure count for the current item this session.
    failures: u32,
}

/// What to do in response to a render failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackAction {
    /// Re-resolve the current item forcing a server transcode.
    RetryWithTranscode,
    /// Already retried once — surface a terminal error; do not loop.
    GiveUp,
}

impl RenderFallback {
    /// One transcode retry after a direct-play render failure. A second failure
    /// (the transcode also failed to render) gives up.
    const MAX_FAILURES: u32 = 1;

    pub fn new() -> Self {
        Self::default()
    }

    /// Call when playback of an item begins. Resets fallback state when the item
    /// changed, so stickiness and the retry count are per-item.
    pub fn begin_item(&mut self, item_key: &str) {
        if self.item_key.as_deref() != Some(item_key) {
            self.item_key = Some(item_key.to_string());
            self.sticky_transcode = false;
            self.failures = 0;
        }
    }

    /// Whether re-resolves for the current item must force a transcode (sticky
    /// after a render failure). The App ORs this into every `resolve_playback`.
    pub fn force_transcode(&self) -> bool {
        self.sticky_transcode
    }

    /// Record a render failure for the current item and decide what to do.
    pub fn on_render_failure(&mut self) -> FallbackAction {
        if self.failures >= Self::MAX_FAILURES {
            return FallbackAction::GiveUp;
        }
        self.failures += 1;
        self.sticky_transcode = true;
        FallbackAction::RetryWithTranscode
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

    #[test]
    fn render_failure_marks_item_sticky_transcode() {
        let mut f = RenderFallback::new();
        f.begin_item("itemA");
        assert!(!f.force_transcode());
        assert_eq!(f.on_render_failure(), FallbackAction::RetryWithTranscode);
        assert!(f.force_transcode());
    }

    #[test]
    fn second_failure_gives_up_no_loop() {
        // direct fails -> transcode retry; transcode also fails -> give up.
        let mut f = RenderFallback::new();
        f.begin_item("itemA");
        assert_eq!(f.on_render_failure(), FallbackAction::RetryWithTranscode);
        assert_eq!(f.on_render_failure(), FallbackAction::GiveUp);
        // Still sticky so a manual retry stays on transcode, but no auto-loop.
        assert!(f.force_transcode());
    }

    #[test]
    fn begin_new_item_clears_sticky_state() {
        let mut f = RenderFallback::new();
        f.begin_item("itemA");
        f.on_render_failure();
        assert!(f.force_transcode());
        f.begin_item("itemB");
        assert!(!f.force_transcode());
        assert_eq!(f.on_render_failure(), FallbackAction::RetryWithTranscode);
    }

    #[test]
    fn begin_same_item_preserves_sticky_state() {
        // Re-entering begin_item for the SAME item (e.g. a re-resolve) must not
        // reset stickiness, or the failure would re-trigger every switch.
        let mut f = RenderFallback::new();
        f.begin_item("itemA");
        f.on_render_failure();
        f.begin_item("itemA");
        assert!(f.force_transcode());
    }
}
