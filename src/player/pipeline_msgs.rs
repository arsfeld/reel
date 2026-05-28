//! Bus messages emitted by the GStreamer playback pipeline.

use gst::{Stream, StreamCollection};

#[derive(Debug)]
pub enum PipelineBusMsg {
    StreamCollection(StreamCollection),
    StreamsSelected {
        collection: StreamCollection,
        streams: Vec<Stream>,
    },
    /// Buffering progress (0-100) for the player's status-plate indicator.
    Buffering {
        percent: i32,
    },
}

/// Action to take in response to a `GST_MESSAGE_BUFFERING`, derived purely from
/// the buffering mode, percent, and the user's intended play state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferingAction {
    /// Live source — never pause/seek; ignore entirely (no indicator).
    Ignore,
    /// Report percent to the indicator only; do not touch playback state.
    Report,
    /// Report percent and pause (stream/queue2 underrun below 100%).
    ReportAndPause,
    /// Report percent and resume (buffer refilled and the user wants to play).
    ReportAndResume,
}

/// Decide how to respond to a buffering message. Pure — no pipeline access, so
/// it is unit-testable without a live pipeline (see `WatchStateTracker`).
///
/// - `Live`: ignored — never pause/seek a live source.
/// - `Download` (the mode this feature introduces): report for the indicator
///   only and never pause. `downloadbuffer` reads ahead while playback
///   continues, so a pause-below-100 protocol would stall the whole download.
/// - `Stream`/`Timeshift` (queue2): classic pause-below-100 / resume-at-100,
///   with the resume gated on `wants_play` so buffering never force-resumes a
///   user-initiated pause.
pub fn buffering_action(
    mode: gst::BufferingMode,
    percent: i32,
    wants_play: bool,
) -> BufferingAction {
    match mode {
        gst::BufferingMode::Live => BufferingAction::Ignore,
        gst::BufferingMode::Download => BufferingAction::Report,
        _ => {
            if percent < 100 {
                BufferingAction::ReportAndPause
            } else if wants_play {
                BufferingAction::ReportAndResume
            } else {
                BufferingAction::Report
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffering_decision_ignores_live_mode() {
        assert_eq!(
            buffering_action(gst::BufferingMode::Live, 40, true),
            BufferingAction::Ignore
        );
        assert_eq!(
            buffering_action(gst::BufferingMode::Live, 100, false),
            BufferingAction::Ignore
        );
    }

    #[test]
    fn buffering_decision_download_reports_without_pausing() {
        assert_eq!(
            buffering_action(gst::BufferingMode::Download, 40, true),
            BufferingAction::Report
        );
    }

    #[test]
    fn buffering_decision_download_at_100_no_action() {
        assert_eq!(
            buffering_action(gst::BufferingMode::Download, 100, true),
            BufferingAction::Report
        );
    }

    #[test]
    fn buffering_decision_stream_pauses_below_100() {
        assert_eq!(
            buffering_action(gst::BufferingMode::Stream, 40, true),
            BufferingAction::ReportAndPause
        );
    }

    #[test]
    fn buffering_decision_stream_resumes_at_100_when_user_wants_play() {
        assert_eq!(
            buffering_action(gst::BufferingMode::Stream, 100, true),
            BufferingAction::ReportAndResume
        );
    }

    #[test]
    fn buffering_decision_stream_respects_user_pause() {
        assert_eq!(
            buffering_action(gst::BufferingMode::Stream, 100, false),
            BufferingAction::Report
        );
    }
}
