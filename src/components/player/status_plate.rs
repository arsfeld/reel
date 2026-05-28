//! Pure presentation logic for the player's central status plate.
//!
//! The plate is a centred overlay used for blocking/transient playback states
//! (initial load, network buffering, terminal errors). Extracting the
//! *decision* into a pure function keeps the priority ordering unit-testable
//! without GTK (the actual widget mutation stays in `video_player.rs`).

/// What the status plate should display, derived from playback state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StatusPlate {
    /// Hidden — playback is prepared and healthy.
    Hidden,
    /// Spinner + "Loading video…" (initial load, not yet prepared).
    Loading,
    /// Spinner + "Buffering… N%" (network fill / rebuffer, percent < 100).
    Buffering(i32),
    /// Error icon + message (terminal playback error).
    Error(String),
}

/// Decide what the status plate shows. Priority, highest first:
/// `error` > `buffering (< 100)` > not-prepared (initial load) > hidden.
///
/// The buffering tier sits above not-prepared so a mid-playback rebuffer still
/// surfaces even though the stream is already prepared; `error` always wins so
/// a failure is never masked by a stale buffering percent.
pub(crate) fn status_plate(
    error_msg: Option<&str>,
    is_prepared: bool,
    buffering: Option<i32>,
) -> StatusPlate {
    if let Some(msg) = error_msg {
        StatusPlate::Error(msg.to_string())
    } else if let Some(pct) = buffering.filter(|p| *p < 100) {
        StatusPlate::Buffering(pct)
    } else if !is_prepared {
        StatusPlate::Loading
    } else {
        StatusPlate::Hidden
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_outranks_buffering_and_loading() {
        assert_eq!(
            status_plate(Some("boom"), false, Some(40)),
            StatusPlate::Error("boom".to_string())
        );
    }

    #[test]
    fn buffering_below_100_outranks_prepared_state() {
        // Mid-playback rebuffer: prepared but buffering — plate must show.
        assert_eq!(
            status_plate(None, true, Some(40)),
            StatusPlate::Buffering(40)
        );
    }

    #[test]
    fn buffering_at_100_does_not_show() {
        assert_eq!(status_plate(None, true, Some(100)), StatusPlate::Hidden);
    }

    #[test]
    fn not_prepared_shows_loading() {
        assert_eq!(status_plate(None, false, None), StatusPlate::Loading);
    }

    #[test]
    fn prepared_and_healthy_is_hidden() {
        assert_eq!(status_plate(None, true, None), StatusPlate::Hidden);
    }
}
