//! The player's central status plate: a centred overlay for blocking/transient
//! playback states (initial load, network buffering, terminal errors).
//!
//! The *decision* ([`status_plate`]) is a pure function so the priority
//! ordering is unit-testable without GTK; [`render`] applies that decision to
//! the live widgets.

use relm4::gtk;

use gtk::prelude::*;

use super::video_player::VideoPlayerWidgets;

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

/// Drive the central status plate from the pure [`status_plate`] decision.
/// Shown for initial loading, network buffering, and terminal errors —
/// seeking intentionally doesn't trigger it.
///
/// Note (v1 limitation): on a fast connection the buffering percent can jump
/// 0 → 100 in well under a frame, so the "Buffering… N%" plate may flicker
/// briefly. Accepted for v1 rather than adding a debounce timer.
pub(crate) fn render(
    widgets: &VideoPlayerWidgets,
    error_msg: Option<&str>,
    is_prepared: bool,
    buffering: Option<i32>,
) {
    // Loading and Buffering share the spinner layout; only the title differs.
    let show_spinner = |title: &str| {
        widgets.status_icon.set_visible(false);
        widgets.status_spinner.set_visible(true);
        widgets.status_spinner.set_spinning(true);
        widgets.status_title.set_label(title);
        widgets.status_detail.set_visible(false);
        widgets.status_plate.set_visible(true);
    };

    match status_plate(error_msg, is_prepared, buffering) {
        StatusPlate::Error(msg) => {
            widgets.status_spinner.set_spinning(false);
            widgets.status_spinner.set_visible(false);
            widgets.status_icon.set_visible(true);
            widgets.status_title.set_label("Couldn't play video");
            widgets.status_detail.set_label(&msg);
            widgets.status_detail.set_visible(true);
            widgets.status_plate.set_visible(true);
        }
        StatusPlate::Loading => show_spinner("Loading video…"),
        StatusPlate::Buffering(pct) => show_spinner(&format!("Buffering… {pct}%")),
        StatusPlate::Hidden => {
            widgets.status_plate.set_visible(false);
            widgets.status_spinner.set_spinning(false);
        }
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
