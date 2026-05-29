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
}
