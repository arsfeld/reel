//! GStreamer `playbin3` pipeline wrapper for the video player widget.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gst::prelude::*;
use gtk::glib;

use crate::player::pipeline_msgs::{BufferingAction, PipelineBusMsg, buffering_action};
use crate::player::subtitles::{best_matching_subtitle, find_matching_subtitles};
use crate::player::tracks::{
    MediaTrack, TrackKind, build_stream_selection, pick_preferred_subtitle, tracks_from_collection,
};

/// Owned GStreamer pipeline driving a single stream.
pub(crate) struct PlaybackPipeline {
    pipeline: gst::Pipeline,
    playbin: gst::Element,
    paintable: gtk::gdk::Paintable,
    bus_watch: Option<gst::bus::BusWatchGuard>,
    prepared: Rc<Cell<bool>>,
    seeking: Rc<Cell<bool>>,
    playing: Rc<Cell<bool>>,
    /// The user's *intended* play state, distinct from the actual pipeline
    /// state in `playing`. A buffering-induced pause flips `playing` to false
    /// but leaves this true, so the stream/queue2 buffering protocol knows to
    /// resume at 100% — while a real user pause clears it and is never
    /// overridden.
    wants_play: Rc<Cell<bool>>,
    error: Rc<RefCell<Option<String>>>,
    tracks: RefCell<Vec<MediaTrack>>,
    active_stream_ids: RefCell<Vec<String>>,
    subtitles_enabled: Cell<bool>,
    preferred_subtitle_lang: Option<String>,
    subtitle_pref_applied: Cell<bool>,
    user_subtitle_override: RefCell<Option<Option<String>>>,
}

impl PlaybackPipeline {
    pub(crate) fn new(
        url: &str,
        autoplay: bool,
        preferred_subtitle_lang: Option<String>,
        local_path: Option<&str>,
        install_color_convert: bool,
        sender: Rc<dyn Fn(PipelineBusMsg)>,
    ) -> Option<Self> {
        let playbin = make_element("playbin3")?;
        let paintable_sink = make_element("gtk4paintablesink")?;
        let paintable = paintable_sink.property::<gtk::gdk::Paintable>("paintable");

        // playbin3 renders subtitles via its `text-sink` (defaults to
        // subtitleoverlay). Wrapping the video sink in subtitleoverlay and
        // setting its removed `video-sink` property panics on GStreamer 1.26+.
        playbin.set_property("video-sink", &paintable_sink);

        // Insert a colorspace-convert stage (`glupload ! glcolorconvert`) ahead
        // of the sink via playbin3's `video-filter`. Without it, 10-bit /
        // BT.2020 frames fail to negotiate to gtk4paintablesink and play as a
        // black video area. `video-filter` composes inside playbin3 ahead of the
        // sink — unlike wrapping the sink in a bin, which panics on 1.26+ (see
        // the note above).
        //
        // Only for direct-play: a server transcode already outputs SDR h264 that
        // renders natively, so the convert stage is unnecessary there — and
        // inserting it into the HLS/adaptivedemux path is a needless risk. If the
        // GL elements are unavailable we install nothing and 10-bit direct-play
        // is simply not advertised by the capability gate.
        if install_color_convert
            && let Some(filter) = crate::player::capabilities::build_color_convert_filter()
        {
            playbin.set_property("video-filter", &filter);
        }

        playbin.set_property("uri", url);
        // Network streams (http/https) get GStreamer's `download` play flag:
        // the stream is routed through an internally-created `downloadbuffer`
        // that progressively writes the full file to a temp file on disk,
        // smoothing jitter and serving in-buffer seeks from the local copy.
        // Local files (`file://`) play directly. The flag is set via the typed
        // `FlagsClass` builder, preserving the existing defaults — setting a
        // raw u32 panics, the property expects GstPlayFlags.
        if crate::services::stream_cache::should_cache_stream(url) {
            enable_stream_cache(&playbin);
        }

        if let Some(path) = local_path {
            let matches = find_matching_subtitles(std::path::Path::new(path));
            if let Some(sub) = best_matching_subtitle(&matches, preferred_subtitle_lang.as_deref())
            {
                let sub_uri = format!("file://{}", sub.to_string_lossy());
                playbin.set_property("suburi", &sub_uri);
            }
        }

        let pipeline = gst::Pipeline::new();
        if let Err(e) = pipeline.add(&playbin) {
            tracing::warn!("failed to add playbin to pipeline: {e}");
            return None;
        }

        let prepared = Rc::new(Cell::new(false));
        let seeking = Rc::new(Cell::new(false));
        let playing = Rc::new(Cell::new(false));
        // Initial intent mirrors the requested target state.
        let wants_play = Rc::new(Cell::new(autoplay));
        let error: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

        let bus_watch = install_bus_watch(
            &pipeline,
            sender,
            BusFlags {
                prepared: prepared.clone(),
                seeking: seeking.clone(),
                playing: playing.clone(),
                wants_play: wants_play.clone(),
                error: error.clone(),
            },
        );

        let target = if autoplay {
            gst::State::Playing
        } else {
            gst::State::Paused
        };
        if let Err(e) = pipeline.set_state(target) {
            tracing::warn!("failed to set pipeline to {target:?}: {e}");
        }

        Some(Self {
            pipeline,
            playbin,
            paintable,
            bus_watch,
            prepared,
            seeking,
            playing,
            wants_play,
            error,
            tracks: RefCell::new(Vec::new()),
            active_stream_ids: RefCell::new(Vec::new()),
            subtitles_enabled: Cell::new(true),
            preferred_subtitle_lang,
            subtitle_pref_applied: Cell::new(false),
            user_subtitle_override: RefCell::new(None),
        })
    }

    pub(crate) fn paintable(&self) -> &gtk::gdk::Paintable {
        &self.paintable
    }

    pub(crate) fn play(&self) {
        self.wants_play.set(true);
        let _ = self.pipeline.set_state(gst::State::Playing);
    }

    pub(crate) fn pause(&self) {
        self.wants_play.set(false);
        let _ = self.pipeline.set_state(gst::State::Paused);
    }

    pub(crate) fn is_playing(&self) -> bool {
        self.playing.get()
    }

    /// The user's intended play state, which survives a buffering-induced
    /// auto-pause (unlike `is_playing`, which reflects the actual pipeline
    /// state). Use this to decide play/pause toggles so a user press during a
    /// buffering stall isn't inverted.
    pub(crate) fn wants_play(&self) -> bool {
        self.wants_play.get()
    }

    pub(crate) fn is_prepared(&self) -> bool {
        self.prepared.get()
    }

    pub(crate) fn is_seeking(&self) -> bool {
        self.seeking.get()
    }

    pub(crate) fn error_message(&self) -> Option<String> {
        self.error.borrow().clone()
    }

    pub(crate) fn tracks(&self) -> Vec<MediaTrack> {
        self.tracks.borrow().clone()
    }

    pub(crate) fn subtitles_enabled(&self) -> bool {
        self.subtitles_enabled.get()
    }

    pub(crate) fn select_audio(&self, stream_id: &str) {
        self.send_selection(Some(stream_id), None, self.subtitles_enabled.get());
    }

    pub(crate) fn select_subtitle(&self, stream_id: Option<&str>) {
        let enabled = stream_id.is_some();
        self.subtitles_enabled.set(enabled);
        *self.user_subtitle_override.borrow_mut() = Some(stream_id.map(str::to_string));
        self.send_selection(None, stream_id, enabled);
    }

    pub(crate) fn load_external_subtitle(&self, uri: &str) {
        self.playbin.set_property("suburi", uri);
        self.subtitles_enabled.set(true);
        *self.user_subtitle_override.borrow_mut() = Some(None);
    }

    pub(crate) fn handle_stream_collection(&self, collection: &gst::StreamCollection) {
        let active = self.active_stream_ids.borrow().clone();
        let mut tracks = tracks_from_collection(collection, &active);
        self.apply_selection_labels(&mut tracks);
        *self.tracks.borrow_mut() = tracks.clone();

        if !self.subtitle_pref_applied.get() {
            self.subtitle_pref_applied.set(true);
            if self.user_subtitle_override.borrow().is_none()
                && let Some(id) =
                    pick_preferred_subtitle(&tracks, self.preferred_subtitle_lang.as_deref())
            {
                self.subtitles_enabled.set(true);
                self.send_selection(None, Some(&id), true);
            }
        }
    }

    pub(crate) fn handle_streams_selected(
        &self,
        collection: &gst::StreamCollection,
        streams: &[gst::Stream],
    ) {
        let ids: Vec<String> = streams
            .iter()
            .filter_map(|s| s.stream_id().map(|id| id.to_string()))
            .collect();
        *self.active_stream_ids.borrow_mut() = ids.clone();
        let mut tracks = tracks_from_collection(collection, &ids);
        self.apply_selection_labels(&mut tracks);
        *self.tracks.borrow_mut() = tracks;
    }

    fn apply_selection_labels(&self, tracks: &mut [MediaTrack]) {
        for track in tracks {
            if track.kind == TrackKind::Audio {
                track.selected = self
                    .active_stream_ids
                    .borrow()
                    .iter()
                    .any(|id| id == &track.stream_id);
            }
            if track.kind == TrackKind::Subtitle {
                track.selected = self.subtitles_enabled.get()
                    && self
                        .active_stream_ids
                        .borrow()
                        .iter()
                        .any(|id| id == &track.stream_id);
            }
        }
    }

    fn send_selection(
        &self,
        audio_id: Option<&str>,
        subtitle_id: Option<&str>,
        subtitles_enabled: bool,
    ) {
        let tracks = self.tracks.borrow().clone();
        let active = self.active_stream_ids.borrow().clone();
        let ids =
            build_stream_selection(&tracks, &active, audio_id, subtitle_id, subtitles_enabled);
        if ids.is_empty() {
            return;
        }
        let event = gst::event::SelectStreams::new(ids.iter().map(String::as_str));
        if !self.pipeline.send_event(event) {
            tracing::warn!("failed to send SelectStreams event");
        }
    }

    pub(crate) fn duration_us(&self) -> i64 {
        self.pipeline
            .query_duration::<gst::ClockTime>()
            .map(|t| t.useconds() as i64)
            .unwrap_or(0)
    }

    /// Fraction (0.0..=1.0) of the stream cached on disk, from a
    /// `GST_QUERY_BUFFERING` range query in percent format. `downloadbuffer`
    /// fills sequentially from the start, so this is how far the read-ahead has
    /// cached ahead of the playhead. Returns 0.0 when the query is unsupported
    /// (local files, or a queue2 stream that reports no range), so the seek bar
    /// simply shows no buffered region.
    pub(crate) fn buffered_fraction(&self) -> f64 {
        // Buffering ranges in percent format run 0..=GST_FORMAT_PERCENT_MAX.
        const PERCENT_MAX: f64 = 1_000_000.0;
        let mut query = gst::query::Buffering::new(gst::Format::Percent);
        if !self.pipeline.query(&mut query) {
            return 0.0;
        }
        let (_start, stop, _estimated_total) = query.range();
        (stop.value() as f64 / PERCENT_MAX).clamp(0.0, 1.0)
    }

    pub(crate) fn position_us(&self) -> i64 {
        self.pipeline
            .query_position::<gst::ClockTime>()
            .map(|t| t.useconds() as i64)
            .unwrap_or(0)
    }

    pub(crate) fn set_volume(&self, v: f64) {
        self.playbin.set_property("volume", v);
    }

    pub(crate) fn volume(&self) -> f64 {
        self.playbin.property::<f64>("volume")
    }

    pub(crate) fn set_muted(&self, m: bool) {
        self.playbin.set_property("mute", m);
    }

    pub(crate) fn is_muted(&self) -> bool {
        self.playbin.property::<bool>("mute")
    }

    pub(crate) fn set_speed(&self, speed: f64) {
        self.pipeline.set_property("speed", speed);
    }

    pub(crate) fn speed(&self) -> f64 {
        self.pipeline.property::<f64>("speed")
    }

    pub(crate) fn seek(&self, target_us: i64) {
        if !self.prepared.get() {
            tracing::debug!(target_us, "seek skipped: pipeline not prepared");
            return;
        }
        let pos = gst::ClockTime::from_useconds(target_us.max(0) as u64);
        let result = self.pipeline.seek_simple(
            gst::SeekFlags::FLUSH
                | gst::SeekFlags::KEY_UNIT
                | gst::SeekFlags::SNAP_BEFORE
                | gst::SeekFlags::ACCURATE,
            pos,
        );
        if let Err(e) = result {
            tracing::warn!(target_us, "seek failed: {e}");
        }
    }
}

impl Drop for PlaybackPipeline {
    fn drop(&mut self) {
        if let Some(watch) = self.bus_watch.take() {
            drop(watch);
        }
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

/// Enable disk read-ahead for a network stream on `playbin`: prepare the cache
/// directory, OR-in the `download` play flag, and configure the internally
/// created `downloadbuffer`'s `temp-template` via `element-setup`.
///
/// If the cache directory can't be prepared (read-only / full partition), skip
/// both the flag and the template and fall back to plain streaming — pointing
/// `temp-template` at an unwritable dir would make `downloadbuffer` error and
/// break otherwise-fine playback.
fn enable_stream_cache(playbin: &gst::Element) {
    use crate::services::stream_cache;

    let cache_dir = crate::config::stream_cache_dir();
    if let Err(e) = stream_cache::prepare(&cache_dir) {
        tracing::warn!(
            "stream cache disabled: cannot prepare {}: {e}",
            cache_dir.display()
        );
        return;
    }

    if !set_download_flag(playbin) {
        return;
    }

    // `playbin3` doesn't expose temp-template directly — it creates the
    // `downloadbuffer` internally. Match it by factory name in `element-setup`
    // and set the template. We do NOT set `temp-remove`: its `true` default
    // removes the temp file on the element's READY transition, which a clean
    // stop/teardown passes through (Drop → Null). Crash orphans are reclaimed
    // by the startup sweep instead.
    let template = stream_cache::temp_template(&cache_dir);
    playbin.connect("element-setup", false, move |values| {
        if let Ok(element) = values[1].get::<gst::Element>()
            && element.factory().map(|f| f.name()).as_deref() == Some("downloadbuffer")
        {
            element.set_property("temp-template", &template);
        }
        None
    });
}

/// OR-in the `download` bit on `playbin`'s `flags` property, preserving the
/// existing defaults. Returns `false` (and logs) if the flags type or builder
/// is unavailable, so the caller can fall back to plain streaming.
fn set_download_flag(playbin: &gst::Element) -> bool {
    let flags = playbin.property_value("flags");
    let Some(flags_class) = glib::FlagsClass::with_type(flags.type_()) else {
        tracing::warn!("playbin flags type unavailable; streaming without disk cache");
        return false;
    };
    let Some(new_flags) = flags_class
        .builder_with_value(flags)
        .and_then(|b| b.set_by_nick("download").build())
    else {
        tracing::warn!("failed to set download flag; streaming without disk cache");
        return false;
    };
    playbin.set_property_from_value("flags", &new_flags);
    true
}

/// Classify a pipeline error as a *render* failure (the stream can't be decoded
/// or negotiated to the sink) versus a transport failure (network/resource).
/// Render failures drive the transcode fallback (U4); transport errors don't.
fn is_render_failure(err: &glib::Error) -> bool {
    err.kind::<gst::StreamError>().is_some()
        || matches!(
            err.kind::<gst::CoreError>(),
            Some(gst::CoreError::Negotiation)
        )
}

pub(crate) fn make_element(name: &str) -> Option<gst::Element> {
    match gst::ElementFactory::make(name).build() {
        Ok(el) => Some(el),
        Err(e) => {
            tracing::warn!("{name} unavailable: {e}");
            None
        }
    }
}

struct BusFlags {
    prepared: Rc<Cell<bool>>,
    seeking: Rc<Cell<bool>>,
    playing: Rc<Cell<bool>>,
    wants_play: Rc<Cell<bool>>,
    error: Rc<RefCell<Option<String>>>,
}

fn install_bus_watch(
    pipeline: &gst::Pipeline,
    sender: Rc<dyn Fn(PipelineBusMsg)>,
    flags: BusFlags,
) -> Option<gst::bus::BusWatchGuard> {
    let bus = pipeline.bus().expect("pipeline has a bus");
    let pipeline_weak = pipeline.downgrade();
    bus.add_watch_local(move |_, msg| {
        handle_bus_message(msg, &flags, &pipeline_weak, &sender);
        glib::ControlFlow::Continue
    })
    .ok()
}

fn handle_bus_message(
    msg: &gst::Message,
    flags: &BusFlags,
    pipeline_weak: &glib::WeakRef<gst::Pipeline>,
    sender: &Rc<dyn Fn(PipelineBusMsg)>,
) {
    use gst::MessageView;
    match msg.view() {
        MessageView::Error(err) => {
            let glib_err = err.error();
            tracing::warn!(
                "gstreamer error from {:?}: {} ({:?})",
                err.src().map(|s| s.path_string()),
                glib_err,
                err.debug()
            );
            *flags.error.borrow_mut() = Some(glib_err.to_string());
            flags.seeking.set(false);
            // A stream/negotiation/decode error means the current stream can't
            // render (e.g. the color-convert stage couldn't negotiate). Signal a
            // render failure so the App can fall back to a server transcode (U4).
            // Transport errors (GstResourceError) are left to surface as the
            // status-plate error string, not a transcode fallback.
            if is_render_failure(&glib_err) {
                sender(PipelineBusMsg::RenderFailed);
            }
        }
        MessageView::AsyncDone(_) => {
            flags.prepared.set(true);
            flags.seeking.set(false);
        }
        MessageView::AsyncStart(_) => {
            flags.seeking.set(true);
        }
        MessageView::StateChanged(sc) => apply_state_change(sc, flags, pipeline_weak),
        MessageView::StreamCollection(collection_msg) => {
            let collection = collection_msg.stream_collection();
            sender(PipelineBusMsg::StreamCollection(collection));
        }
        MessageView::StreamsSelected(selected) => {
            let collection = selected.stream_collection();
            let streams: Vec<gst::Stream> = selected.streams().collect();
            sender(PipelineBusMsg::StreamsSelected {
                collection,
                streams,
            });
        }
        MessageView::Buffering(b) => handle_buffering(b, flags, pipeline_weak, sender),
        MessageView::DurationChanged(_) | MessageView::Eos(_) => {}
        _ => {}
    }
}

/// Mode-aware buffering handling (KTD5). The pure `buffering_action` decides
/// the response; this only emits the indicator percent and performs the
/// stream/queue2 pause/resume `set_state` calls. Download mode never pauses —
/// `downloadbuffer` reads ahead while playback continues.
fn handle_buffering(
    b: &gst::message::Buffering,
    flags: &BusFlags,
    pipeline_weak: &glib::WeakRef<gst::Pipeline>,
    sender: &Rc<dyn Fn(PipelineBusMsg)>,
) {
    let percent = b.percent();
    let (mode, ..) = b.buffering_stats();
    let action = buffering_action(mode, percent, flags.wants_play.get());

    if action != BufferingAction::Ignore {
        sender(PipelineBusMsg::Buffering { percent });
    }

    let new_state = match action {
        BufferingAction::ReportAndPause => Some(gst::State::Paused),
        BufferingAction::ReportAndResume => Some(gst::State::Playing),
        BufferingAction::Ignore | BufferingAction::Report => None,
    };
    if let Some(state) = new_state
        && let Some(pipe) = pipeline_weak.upgrade()
        && let Err(e) = pipe.set_state(state)
    {
        // A discarded failure here can strand playback paused (a failed resume
        // leaves wants_play true with no retry); surface it like the
        // construction-time set_state does.
        tracing::warn!("buffering set_state({state:?}) failed: {e}");
    }
}

fn apply_state_change(
    sc: &gst::message::StateChanged,
    flags: &BusFlags,
    pipeline_weak: &glib::WeakRef<gst::Pipeline>,
) {
    let Some(pipe) = pipeline_weak.upgrade() else {
        return;
    };
    if !sc
        .src()
        .map(|s| s == pipe.upcast_ref::<gst::Object>())
        .unwrap_or(false)
    {
        return;
    }
    let current = sc.current();
    flags.playing.set(current == gst::State::Playing);
    if current == gst::State::Paused || current == gst::State::Playing {
        flags.prepared.set(true);
    } else if current == gst::State::Null || current == gst::State::Ready {
        flags.prepared.set(false);
    }
}
