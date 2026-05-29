//! Render-capability helpers for the playback pipeline.
//!
//! 10-bit / BT.2020 frames cannot negotiate directly to `gtk4paintablesink`,
//! which is the root cause of HDR/10-bit content playing audio with a black
//! video area. Inserting a `glupload ! glcolorconvert` stage ahead of the sink
//! gives those frames a renderable path. For **SDR** content this conversion is
//! correct (matrix + bit-depth conversion, no tone curve needed); HDR tone-
//! mapping is a separate, hardware-conditional concern handled elsewhere.
//!
//! This module owns two things:
//! - [`build_color_convert_filter`] — constructs the GStreamer convert bin used
//!   as `playbin3`'s `video-filter` (touches GStreamer; not unit-tested beyond
//!   construction).
//! - [`can_direct_play`] — the pure decision of whether a given request should
//!   be advertised as direct-playable, fully unit-tested with no GStreamer.

use gst::prelude::*;

use crate::models::playback::{QualityPreset, QualitySelection};
use crate::player::gst_pipeline::make_element;

/// Build a `glupload ! glcolorconvert` bin to install as `playbin3`'s
/// `video-filter`. The bin exposes a sink ghost pad (on `glupload`) and a src
/// ghost pad (on `glcolorconvert`) so playbin3 can link it between the decoder
/// and the sink.
///
/// Returns `None` if either GL element is unavailable, so the caller installs
/// no filter and falls back gracefully rather than panicking. We deliberately
/// do **not** create or inject a GL context: `gtk4paintablesink` manages a real
/// shared context and GStreamer propagates it via the `GST_CONTEXT` query —
/// wrapping an external context here triggers the `active_thread` assertion.
pub(crate) fn build_color_convert_filter() -> Option<gst::Bin> {
    let upload = make_element("glupload")?;
    let convert = make_element("glcolorconvert")?;

    let bin = gst::Bin::builder().name("reel-color-convert").build();
    bin.add_many([&upload, &convert]).ok()?;
    gst::Element::link_many([&upload, &convert]).ok()?;

    // Sink ghost pad -> first element's sink; src ghost pad -> last element's
    // src. A video-filter is bidirectional, so both are required.
    let sink_target = upload.static_pad("sink")?;
    let ghost_sink = gst::GhostPad::with_target(&sink_target).ok()?;
    ghost_sink.set_active(true).ok()?;
    bin.add_pad(&ghost_sink).ok()?;

    let src_target = convert.static_pad("src")?;
    let ghost_src = gst::GhostPad::with_target(&src_target).ok()?;
    ghost_src.set_active(true).ok()?;
    bin.add_pad(&ghost_src).ok()?;

    Some(bin)
}

/// One-time startup capability probe: can the GL colorspace-convert elements be
/// instantiated in this environment? Cheap (element-factory creation only, no
/// pipeline run). When false, the convert stage can't be built, so 10-bit
/// direct-play is never advertised and content keeps transcoding server-side.
pub(crate) fn probe_can_render_10bit() -> bool {
    make_element("glupload").is_some() && make_element("glcolorconvert").is_some()
}

/// Whether to advertise 10-bit (SDR) direct-play for a request — a pure
/// decision combining the startup probe with the user's quality choice.
///
/// HDR-vs-SDR is **not** decided here: the transcode profile advertises
/// "10-bit SDR direct-play OK" and the server transcodes HDR per-file from that
/// directive. This function only gates on whether the client can render 10-bit
/// at all and whether the user's quality selection already implies a transcode:
/// an explicit `force_transcode`, or any capped `Manual` rung (a bitrate/
/// resolution cap means the server must re-encode anyway). `Auto` and
/// `Manual(Original)` permit direct-play when the probe is capable.
pub(crate) fn can_direct_play(
    probe_capable: bool,
    quality: QualitySelection,
    force_transcode: bool,
) -> bool {
    if !probe_capable || force_transcode {
        return false;
    }
    match quality {
        QualitySelection::Auto | QualitySelection::Manual(QualityPreset::Original) => true,
        QualitySelection::Manual(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construction smoke test. When the GL elements are present (the dev
    /// shell), the bin is built with both ghost pads; otherwise the builder
    /// degrades to `None`. Either outcome is correct — the assertion adapts to
    /// the environment so this passes on hosts without the GL plugins.
    #[test]
    fn build_color_convert_filter_constructs_bin_or_degrades() {
        let _ = gst::init();
        match build_color_convert_filter() {
            Some(bin) => {
                assert!(
                    bin.static_pad("sink").is_some(),
                    "filter bin must expose a sink ghost pad"
                );
                assert!(
                    bin.static_pad("src").is_some(),
                    "filter bin must expose a src ghost pad"
                );
            }
            None => {
                // GL elements unavailable in this environment — graceful degrade.
                assert!(make_element("glupload").is_none() || make_element("glcolorconvert").is_none());
            }
        }
    }

    #[test]
    fn can_direct_play_allows_auto_when_capable() {
        assert!(can_direct_play(true, QualitySelection::Auto, false));
    }

    #[test]
    fn can_direct_play_allows_manual_original_when_capable() {
        assert!(can_direct_play(
            true,
            QualitySelection::Manual(QualityPreset::Original),
            false
        ));
    }

    #[test]
    fn can_direct_play_blocks_when_incapable() {
        assert!(!can_direct_play(false, QualitySelection::Auto, false));
        assert!(!can_direct_play(
            false,
            QualitySelection::Manual(QualityPreset::Original),
            false
        ));
    }

    #[test]
    fn can_direct_play_blocks_on_capped_manual_rung() {
        // A bitrate/resolution cap implies a transcode — don't advertise direct.
        assert!(!can_direct_play(
            true,
            QualitySelection::Manual(QualityPreset::P1080Mbps8),
            false
        ));
    }

    #[test]
    fn force_transcode_always_blocks_direct_play() {
        assert!(!can_direct_play(true, QualitySelection::Auto, true));
        assert!(!can_direct_play(
            true,
            QualitySelection::Manual(QualityPreset::Original),
            true
        ));
    }
}
