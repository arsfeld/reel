//! Inline video player with a custom on-screen display.
//!
//! Wraps `gtk::Picture` driven by a hand-built GStreamer `playbin3`
//! pipeline (with `gtk4paintablesink` as the video sink) and lays a
//! libadwaita-flavoured OSD on top: a seek bar, transport buttons, time
//! labels, a volume slider, and a fullscreen toggle. Controls fade out
//! after a short period of pointer / keyboard inactivity, mpv-style.
//!
//! Standard mpv shortcuts honoured (when the player has focus):
//!   Space / k       toggle play/pause
//!   Left / Right    seek -5s / +5s
//!   Shift+L/R       seek -1s / +1s
//!   j / l           seek -10s / +10s
//!   Up / Down       seek +60s / -60s   (mpv default)
//!   Home / 0        seek to start
//!   End             seek to end
//!   m               mute toggle
//!   9 / 0           volume down / up   (mpv uses 9/0; we also use Up/Down for seek)
//!   f               toggle fullscreen
//!   Esc             exit fullscreen
//!
//! Position polling runs at 4 Hz via `glib::timeout_add_local`. We avoid a
//! feedback loop on the seek slider by suppressing our own programmatic
//! updates with an `Rc<Cell<bool>>` flag — the user-driven `value-changed`
//! emits while the flag is clear go through `UserSeek`, which seeks the
//! underlying stream immediately.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gst::prelude::*;
use gtk::glib;
use gtk::prelude::*;
use relm4::gtk;
use relm4::prelude::*;

use crate::player::PlayState;

const TICK_INTERVAL_MS: u32 = 250;
const HIDE_DELAY_MS: u32 = 2500;

/// Owned GStreamer pipeline driving a single stream.
///
/// Behaves like `gtk::MediaFile` did: build with a URL, set state
/// transitions via `play`/`pause`, query timing in microseconds, etc.
/// Bus messages drive event-flag transitions (`prepared`, `seeking`)
/// and post a `Tick` to the relm4 sender so the widget refreshes.
struct PlaybackPipeline {
    pipeline: gst::Pipeline,
    playbin: gst::Element,
    paintable: gtk::gdk::Paintable,
    bus_watch: Option<gst::bus::BusWatchGuard>,
    prepared: Rc<Cell<bool>>,
    seeking: Rc<Cell<bool>>,
    playing: Rc<Cell<bool>>,
    error: Rc<RefCell<Option<String>>>,
}

impl PlaybackPipeline {
    fn new(
        url: &str,
        autoplay: bool,
        sender: &ComponentSender<VideoPlayer>,
    ) -> Option<Self> {
        let playbin = make_element("playbin3")?;
        let sink = make_element("gtk4paintablesink")?;

        let paintable = sink.property::<gtk::gdk::Paintable>("paintable");
        playbin.set_property("video-sink", &sink);
        playbin.set_property("uri", url);

        let pipeline = gst::Pipeline::new();
        if let Err(e) = pipeline.add(&playbin) {
            tracing::warn!("failed to add playbin to pipeline: {e}");
            return None;
        }

        let prepared = Rc::new(Cell::new(false));
        let seeking = Rc::new(Cell::new(false));
        let playing = Rc::new(Cell::new(false));
        let error: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

        let bus_watch = install_bus_watch(
            &pipeline,
            sender.clone(),
            BusFlags {
                prepared: prepared.clone(),
                seeking: seeking.clone(),
                playing: playing.clone(),
                error: error.clone(),
            },
        );

        // Drive directly to the user-visible target state. Going Null →
        // Paused → Playing as two separate set_state calls races the bus
        // watch and leaves `self.playing` stuck at false on autoplay.
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
            error,
        })
    }

    fn paintable(&self) -> &gtk::gdk::Paintable {
        &self.paintable
    }

    fn play(&self) {
        let _ = self.pipeline.set_state(gst::State::Playing);
    }

    fn pause(&self) {
        let _ = self.pipeline.set_state(gst::State::Paused);
    }

    fn is_playing(&self) -> bool {
        self.playing.get()
    }

    fn is_prepared(&self) -> bool {
        self.prepared.get()
    }

    fn is_seeking(&self) -> bool {
        self.seeking.get()
    }

    fn error_message(&self) -> Option<String> {
        self.error.borrow().clone()
    }

    fn duration_us(&self) -> i64 {
        self.pipeline
            .query_duration::<gst::ClockTime>()
            .map(|t| t.useconds() as i64)
            .unwrap_or(0)
    }

    fn position_us(&self) -> i64 {
        self.pipeline
            .query_position::<gst::ClockTime>()
            .map(|t| t.useconds() as i64)
            .unwrap_or(0)
    }

    fn set_volume(&self, v: f64) {
        self.playbin.set_property("volume", v);
    }

    fn volume(&self) -> f64 {
        self.playbin.property::<f64>("volume")
    }

    fn set_muted(&self, m: bool) {
        self.playbin.set_property("mute", m);
    }

    fn is_muted(&self) -> bool {
        self.playbin.property::<bool>("mute")
    }

    fn set_speed(&self, speed: f64) {
        self.pipeline.set_property("speed", speed);
    }

    fn speed(&self) -> f64 {
        self.pipeline.property::<f64>("speed")
    }

    fn seek(&self, target_us: i64) {
        if !self.prepared.get() {
            tracing::debug!(target_us, "seek skipped: pipeline not prepared");
            return;
        }
        let pos = gst::ClockTime::from_useconds(target_us.max(0) as u64);
        let was_seeking = self.seeking.get();
        let pre_position_us = self
            .pipeline
            .query_position::<gst::ClockTime>()
            .map(|t| t.useconds() as i64)
            .unwrap_or(-1);
        // KEY_UNIT | SNAP_BEFORE keeps the HTTP range request cheap by
        // landing on the keyframe at-or-before the target (one fetch,
        // smooth playback resumption). ACCURATE then asks the decoder to
        // chase forward from that keyframe to the exact requested frame
        // — without it, several rapid "+5s" nudges between two sparse
        // keyframes all land on the same earlier keyframe and the
        // playhead visibly stalls while the slider keeps jumping.
        let result = self.pipeline.seek_simple(
            gst::SeekFlags::FLUSH
                | gst::SeekFlags::KEY_UNIT
                | gst::SeekFlags::SNAP_BEFORE
                | gst::SeekFlags::ACCURATE,
            pos,
        );
        match result {
            Ok(()) => {
                tracing::debug!(
                    target_us,
                    pre_position_us,
                    was_seeking,
                    "seek submitted"
                );
            }
            Err(e) => {
                tracing::warn!(target_us, "seek failed: {e}");
            }
        }
    }
}

impl Drop for PlaybackPipeline {
    fn drop(&mut self) {
        if let Some(watch) = self.bus_watch.take() {
            // Dropping the guard removes the watch.
            drop(watch);
        }
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

/// Build a GStreamer element by factory name, logging on failure.
fn make_element(name: &str) -> Option<gst::Element> {
    match gst::ElementFactory::make(name).build() {
        Ok(el) => Some(el),
        Err(e) => {
            tracing::warn!("{name} unavailable: {e}");
            None
        }
    }
}

/// State flags shared between the bus watch and the owning `PlaybackPipeline`.
struct BusFlags {
    prepared: Rc<Cell<bool>>,
    seeking: Rc<Cell<bool>>,
    playing: Rc<Cell<bool>>,
    error: Rc<RefCell<Option<String>>>,
}

/// Wire up `pipeline`'s bus watch to drive the supplied flags and post a
/// `Tick` to the widget after every message.
fn install_bus_watch(
    pipeline: &gst::Pipeline,
    sender: ComponentSender<VideoPlayer>,
    flags: BusFlags,
) -> Option<gst::bus::BusWatchGuard> {
    let bus = pipeline.bus().expect("pipeline has a bus");
    let pipeline_weak = pipeline.downgrade();
    bus.add_watch_local(move |_, msg| {
        handle_bus_message(msg, &flags, &pipeline_weak);
        sender.input(VideoPlayerMsg::Tick);
        glib::ControlFlow::Continue
    })
    .ok()
}

fn handle_bus_message(
    msg: &gst::Message,
    flags: &BusFlags,
    pipeline_weak: &glib::WeakRef<gst::Pipeline>,
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
            // An error means we'll never finish preparing; clear the
            // seeking flag so the loading overlay logic in the widget
            // can switch to the error state.
            flags.seeking.set(false);
        }
        MessageView::AsyncDone(_) => {
            let landed_us = pipeline_weak
                .upgrade()
                .and_then(|p| p.query_position::<gst::ClockTime>())
                .map(|t| t.useconds() as i64)
                .unwrap_or(-1);
            tracing::debug!(landed_us, "bus: AsyncDone — seek complete, prepared");
            flags.prepared.set(true);
            flags.seeking.set(false);
        }
        MessageView::AsyncStart(_) => {
            tracing::debug!("bus: AsyncStart — seek/preroll begin");
            flags.seeking.set(true);
        }
        MessageView::StateChanged(sc) => apply_state_change(sc, flags, pipeline_weak),
        MessageView::DurationChanged(_) | MessageView::Eos(_) => {}
        _ => {}
    }
}

fn apply_state_change(
    sc: &gst::message::StateChanged,
    flags: &BusFlags,
    pipeline_weak: &glib::WeakRef<gst::Pipeline>,
) {
    // Only the pipeline's own state changes are load-bearing; child
    // elements emit their own.
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

#[derive(Debug)]
pub(crate) struct VideoPlayerInit {
    pub(crate) url: Option<String>,
    /// Start playing as soon as a stream is loaded (initial URL and any
    /// later `SetUrl`).
    pub(crate) autoplay: bool,
    /// Saved resume position (seconds) for the initial URL. Applied once
    /// the stream is prepared, then forgotten.
    pub(crate) resume_secs: Option<f64>,
    /// Initial volume in `0.0..=1.0`. Restored from app config so the
    /// user's preference survives across scenes and restarts.
    pub(crate) volume: f64,
    /// Initial mute state, paired with `volume` so unmuting returns to
    /// the previously chosen level.
    pub(crate) muted: bool,
}

impl Default for VideoPlayerInit {
    fn default() -> Self {
        Self {
            url: None,
            autoplay: false,
            resume_secs: None,
            volume: 1.0,
            muted: false,
        }
    }
}

/// What the player tells the parent about playback.
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) enum VideoPlayerOutput {
    FileLoaded {
        path: String,
        duration_secs: f64,
    },
    PositionChanged {
        position_secs: f64,
        duration_secs: f64,
    },
    StateChanged(PlayState),
    EndOfFile,
    VolumeChanged { volume: f64, muted: bool },
    SpeedChanged(f64),
    ToggleFullscreen,
    LoadSubtitleFile,
    Error(String),
    ControlsRevealedChanged(bool),
}

#[derive(Debug)]
#[allow(dead_code)]
pub(crate) enum VideoPlayerMsg {
    LoadFile(String),
    SetUrl {
        url: Option<String>,
        resume_secs: Option<f64>,
    },
    Clear,
    SetAutoplay(bool),
    /// 4 Hz timer poll: refresh slider + labels from the media stream.
    Tick,
    TogglePlay,
    /// Seek by N seconds, clamped to [0, duration].
    SeekRelative(i64),
    /// Seek to a fraction of the duration.
    SeekFraction(f64),
    UserSeek(i64),
    SeekAbsolute(f64),
    SetVolume(f64),
    AdjustVolume(f64),
    ToggleMute,
    ToggleFullscreen,
    ExitFullscreen,
    /// Pointer moved or a key was pressed — extend the controls' visible
    /// window.
    PointerActive,
    /// Inactivity timer fired — hide controls if not interacting.
    HideControls,
    KeyPressed(gtk::gdk::Key, gtk::gdk::ModifierType),
    SetSpeed(f64),
    LoadSubtitleFile,
    FullscreenChanged(bool),
    FilesDropped(String),
}

/// Independent playback flags grouped to keep `VideoPlayer`'s top-level
/// bool count under the `struct_excessive_bools` threshold.
#[derive(Debug, Default)]
struct PlaybackFlags {
    playing: bool,
    autoplay: bool,
    eos_reached: bool,
}

/// OSD reveal flags, similarly grouped.
#[derive(Debug, Default)]
struct OsdState {
    /// Most recent request to reveal the OSD (driven by inactivity timer).
    show_controls: bool,
    /// Latched copy of the most recently emitted reveal state. We diff
    /// against this in `update_with_view` so the parent only hears about
    /// real edge transitions, not every tick.
    controls_revealed: bool,
}

pub(crate) struct VideoPlayer {
    media: Option<PlaybackPipeline>,
    url: Option<String>,
    duration_us: i64,
    position_us: i64,
    volume: f64,
    muted: bool,
    playback: PlaybackFlags,
    is_fullscreen: bool,
    /// Transient borderless window holding the player while in fullscreen.
    /// `None` outside fullscreen.
    fs_window: Option<gtk::Window>,
    /// Original parent of `root_box` to reparent back to on exit. Stored as
    /// the inline overlay we were embedded in.
    fs_original_parent: Option<gtk::Widget>,
    /// OSD reveal-state flags (current request + most recently emitted edge).
    osd: OsdState,
    hide_source: Option<glib::SourceId>,
    tick_source: Option<glib::SourceId>,
    /// Flag set while we update the seek scale programmatically so the
    /// `value-changed` handler can ignore our own writes.
    suppress_scale: Rc<Cell<bool>>,
    suppress_volume: Rc<Cell<bool>>,
    /// Timestamp of the most recent user-driven seek slider change. Used
    /// by `refresh_widgets` to suppress pushing polled positions onto the
    /// thumb for a brief window after the user touches it — without this
    /// the thumb visibly skips backward to GStreamer's SNAP_BEFORE
    /// keyframe in between drag updates.
    last_user_seek: Rc<Cell<Option<Instant>>>,
    resume_pending: Option<f64>,
}

#[relm4::component(pub(crate))]
impl Component for VideoPlayer {
    type Init = VideoPlayerInit;
    type Input = VideoPlayerMsg;
    type Output = VideoPlayerOutput;
    type CommandOutput = ();

    view! {
        #[name = "root_box"]
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_focusable: true,
            set_can_focus: true,
            add_css_class: "video-player",
            set_overflow: gtk::Overflow::Hidden,

            #[name = "stack_overlay"]
            gtk::Overlay {
                set_hexpand: true,
                set_vexpand: true,

                // Wrap the GtkPicture in a GtkGraphicsOffload (4.14+) so
                // the video paintable is composited directly by the
                // Wayland compositor instead of going through GSK every
                // frame — same trick GtkVideo uses internally. Without
                // this, decoded frames hit GTK's normal render pass and
                // playback stutters under load.
                #[wrap(Some)]
                set_child = &gtk::GraphicsOffload {
                    set_enabled: gtk::GraphicsOffloadEnabled::Enabled,

                    #[wrap(Some)]
                    #[name = "picture"]
                    set_child = &gtk::Picture {
                        set_hexpand: true,
                        set_vexpand: true,
                        set_height_request: 540,
                        set_content_fit: gtk::ContentFit::Contain,
                        add_css_class: "video-surface",
                    },
                },

                // Centred big play / pause / replay glyph that flashes on
                // state change. Hidden when controls fade out.
                add_overlay = &gtk::Box {
                    set_halign: gtk::Align::Center,
                    set_valign: gtk::Align::Center,
                    set_can_target: false,

                    #[name = "center_indicator"]
                    gtk::Image {
                        add_css_class: "video-center-indicator",
                        set_pixel_size: 96,
                        set_visible: false,
                        set_icon_name: Some("media-playback-start-symbolic"),
                    },
                },

                // Loading / error plate. Shown while the pipeline is
                // preparing (so opening a stream is visible feedback) and
                // swapped to an error message if the pipeline fails. Not
                // shown during seeks — playbin3 prepares fast enough that
                // a flashing spinner during scrubbing was just noise.
                add_overlay = &gtk::Box {
                    set_halign: gtk::Align::Center,
                    set_valign: gtk::Align::Center,
                    set_can_target: false,

                    #[name = "status_plate"]
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 12,
                        set_halign: gtk::Align::Center,
                        set_valign: gtk::Align::Center,
                        add_css_class: "video-status-plate",
                        set_visible: false,

                        #[name = "status_spinner"]
                        gtk::Spinner {
                            set_spinning: true,
                            set_width_request: 48,
                            set_height_request: 48,
                            set_halign: gtk::Align::Center,
                        },

                        #[name = "status_icon"]
                        gtk::Image {
                            set_icon_name: Some("dialog-error-symbolic"),
                            set_pixel_size: 48,
                            set_halign: gtk::Align::Center,
                            set_visible: false,
                        },

                        #[name = "status_title"]
                        gtk::Label {
                            set_label: "Loading video…",
                            add_css_class: "video-status-title",
                            set_halign: gtk::Align::Center,
                        },

                        #[name = "status_detail"]
                        gtk::Label {
                            add_css_class: "video-status-detail",
                            set_halign: gtk::Align::Center,
                            set_wrap: true,
                            set_justify: gtk::Justification::Center,
                            set_max_width_chars: 48,
                            set_visible: false,
                        },
                    },
                },

                // The OSD bar lives in a Revealer so we can fade it
                // smoothly. `set_can_target: false` on the surrounding box
                // would block clicks on the controls — we want them
                // clickable, so leave targeting on for the inner content.
                add_overlay = &gtk::Box {
                    set_valign: gtk::Align::End,
                    set_halign: gtk::Align::Fill,

                    #[name = "controls_revealer"]
                    gtk::Revealer {
                        set_transition_type: gtk::RevealerTransitionType::Crossfade,
                        set_transition_duration: 180,
                        set_reveal_child: true,
                        set_hexpand: true,

                        #[name = "controls_box"]
                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            add_css_class: "video-osd",
                            set_hexpand: true,

                            // Seek bar row.
                            gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 10,
                                set_margin_start: 16,
                                set_margin_end: 16,
                                set_margin_top: 8,

                                #[name = "position_label"]
                                gtk::Label {
                                    set_label: "0:00",
                                    add_css_class: "video-osd-time",
                                    set_width_chars: 5,
                                    set_xalign: 1.0,
                                },

                                #[name = "seek_scale"]
                                gtk::Scale {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_hexpand: true,
                                    set_draw_value: false,
                                    set_range: (0.0, 1.0),
                                    add_css_class: "video-osd-seek",
                                },

                                #[name = "duration_label"]
                                gtk::Label {
                                    set_label: "--:--",
                                    add_css_class: "video-osd-time",
                                    set_width_chars: 5,
                                    set_xalign: 0.0,
                                },
                            },

                            // Transport row.
                            gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 6,
                                set_margin_start: 12,
                                set_margin_end: 12,
                                set_margin_top: 4,
                                set_margin_bottom: 12,

                                #[name = "play_button"]
                                gtk::Button {
                                    set_icon_name: "media-playback-start-symbolic",
                                    add_css_class: "circular",
                                    add_css_class: "video-osd-primary",
                                    set_tooltip_text: Some("Play / pause (Space)"),
                                    connect_clicked[sender] => move |_| {
                                        sender.input(VideoPlayerMsg::TogglePlay);
                                    },
                                },

                                gtk::Button {
                                    set_icon_name: "media-seek-backward-symbolic",
                                    add_css_class: "flat",
                                    add_css_class: "circular",
                                    set_tooltip_text: Some("Back 10s (j)"),
                                    connect_clicked[sender] => move |_| {
                                        sender.input(VideoPlayerMsg::SeekRelative(-10));
                                    },
                                },

                                gtk::Button {
                                    set_icon_name: "media-seek-forward-symbolic",
                                    add_css_class: "flat",
                                    add_css_class: "circular",
                                    set_tooltip_text: Some("Forward 10s (l)"),
                                    connect_clicked[sender] => move |_| {
                                        sender.input(VideoPlayerMsg::SeekRelative(10));
                                    },
                                },

                                gtk::Box { set_hexpand: true },

                                #[name = "volume_button"]
                                gtk::Button {
                                    set_icon_name: "audio-volume-high-symbolic",
                                    add_css_class: "flat",
                                    add_css_class: "circular",
                                    set_tooltip_text: Some("Mute (m)"),
                                    connect_clicked[sender] => move |_| {
                                        sender.input(VideoPlayerMsg::ToggleMute);
                                    },
                                },

                                #[name = "volume_scale"]
                                gtk::Scale {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_draw_value: false,
                                    set_range: (0.0, 1.0),
                                    set_value: 1.0,
                                    set_width_request: 110,
                                    add_css_class: "video-osd-volume",
                                },

                                #[name = "fullscreen_button"]
                                gtk::Button {
                                    set_icon_name: "view-fullscreen-symbolic",
                                    add_css_class: "flat",
                                    add_css_class: "circular",
                                    set_tooltip_text: Some("Fullscreen (f)"),
                                    connect_clicked[sender] => move |_| {
                                        sender.input(VideoPlayerMsg::ToggleFullscreen);
                                    },
                                },
                            },
                        },
                    },
                },
            },
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let suppress_scale = Rc::new(Cell::new(false));
        let suppress_volume = Rc::new(Cell::new(false));
        let last_user_seek: Rc<Cell<Option<Instant>>> = Rc::new(Cell::new(None));

        let mut model = VideoPlayer::new_model(
            &init,
            suppress_scale.clone(),
            suppress_volume.clone(),
            last_user_seek.clone(),
        );

        let widgets = view_output!();

        wire_slider_handlers(
            &widgets,
            &sender,
            &suppress_scale,
            &suppress_volume,
            &last_user_seek,
        );
        wire_pointer_handlers(&widgets, &sender);
        wire_keyboard_handlers(&widgets, &sender);

        // Kick off the position-polling timer.
        let tick = glib::timeout_add_local(
            std::time::Duration::from_millis(TICK_INTERVAL_MS as u64),
            {
                let sender = sender.clone();
                move || {
                    sender.input(VideoPlayerMsg::Tick);
                    glib::ControlFlow::Continue
                }
            },
        );

        // Start the inactivity timer immediately so controls hide on a
        // still scene. They'll re-reveal on the next pointer/keypress.
        sender.input(VideoPlayerMsg::PointerActive);

        model.tick_source = Some(tick);

        if let Some(url) = init.url {
            sender.input(VideoPlayerMsg::SetUrl {
                url: Some(url),
                resume_secs: init.resume_secs,
            });
        }

        ComponentParts { model, widgets }
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        msg: VideoPlayerMsg,
        sender: ComponentSender<Self>,
        root: &Self::Root,
    ) {
        match msg {
            VideoPlayerMsg::LoadFile(path) => {
                let url = format!("file://{}", path);
                self.handle_set_url(widgets, &sender, Some(url), None);
            }
            VideoPlayerMsg::SetUrl { url, resume_secs } => {
                self.handle_set_url(widgets, &sender, url, resume_secs);
            }
            VideoPlayerMsg::Clear => {
                self.handle_set_url(widgets, &sender, None, None);
            }
            VideoPlayerMsg::SetAutoplay(on) => self.playback.autoplay = on,
            VideoPlayerMsg::Tick => self.handle_tick(widgets, &sender),
            VideoPlayerMsg::TogglePlay => self.handle_toggle_play(widgets, &sender),
            VideoPlayerMsg::SeekRelative(secs) => self.handle_seek_relative(&sender, secs),
            VideoPlayerMsg::SeekFraction(f) => self.handle_seek_fraction(&sender, f),
            VideoPlayerMsg::UserSeek(us) => self.handle_user_seek(&sender, us),
            VideoPlayerMsg::SeekAbsolute(secs) => {
                let us = (secs * 1_000_000.0) as i64;
                self.handle_user_seek(&sender, us);
            }
            VideoPlayerMsg::SetVolume(v) => self.handle_set_volume(&sender, v),
            VideoPlayerMsg::AdjustVolume(delta) => {
                let v = (self.volume + delta).clamp(0.0, 1.0);
                sender.input(VideoPlayerMsg::SetVolume(v));
                sender.input(VideoPlayerMsg::PointerActive);
            }
            VideoPlayerMsg::ToggleMute => self.handle_toggle_mute(&sender),
            VideoPlayerMsg::ToggleFullscreen => self.handle_toggle_fullscreen(widgets, &sender),
            VideoPlayerMsg::ExitFullscreen => self.handle_exit_fullscreen(widgets),
            VideoPlayerMsg::PointerActive => self.handle_pointer_active(&sender),
            VideoPlayerMsg::HideControls => {
                self.hide_source = None;
                self.osd.show_controls = false;
            }
            VideoPlayerMsg::KeyPressed(key, mods) => {
                if self.handle_key(&sender, key, mods) {
                    sender.input(VideoPlayerMsg::PointerActive);
                }
            }
            VideoPlayerMsg::SetSpeed(speed) => {
                if let Some(media) = self.media.as_ref() {
                    media.set_speed(speed);
                }
                let _ = sender.output(VideoPlayerOutput::SpeedChanged(speed));
            }
            VideoPlayerMsg::LoadSubtitleFile => {
                let _ = sender.output(VideoPlayerOutput::LoadSubtitleFile);
            }
            VideoPlayerMsg::FullscreenChanged(fs) => {
                if fs && !self.is_fullscreen {
                    self.handle_toggle_fullscreen(widgets, &sender);
                } else if !fs && self.is_fullscreen {
                    self.handle_exit_fullscreen(widgets);
                }
            }
            VideoPlayerMsg::FilesDropped(uri) => {
                let url = if uri.starts_with("file://") {
                    uri
                } else {
                    format!("file://{}", uri)
                };
                self.handle_set_url(widgets, &sender, Some(url), None);
            }
        }

        // Notify the parent on reveal-state edges so it can fade the
        // floating header bar together with the OSD. Computed here (not
        // in `refresh_widgets`) because we need `&mut self` to latch the
        // last value, and emitting only on changes avoids spamming the
        // parent on every tick.
        let force_visible = self.media.is_none() || self.duration_us == 0;
        let revealed = self.osd.show_controls || force_visible;
        if revealed != self.osd.controls_revealed {
            self.osd.controls_revealed = revealed;
            let _ = sender.output(VideoPlayerOutput::ControlsRevealedChanged(revealed));
        }

        // Re-render derived widget state. We keep this manual because
        // many of these properties depend on multiple model fields and a
        // few need to skip our own value-changed handlers.
        self.refresh_widgets(widgets, root);
        let _ = root;
    }

    fn shutdown(&mut self, _widgets: &mut Self::Widgets, _output: relm4::Sender<Self::Output>) {
        if let Some(id) = self.tick_source.take() {
            id.remove();
        }
        if let Some(id) = self.hide_source.take() {
            id.remove();
        }
        if let Some(media) = &self.media {
            media.pause();
        }
        // Drop the pipeline so its bus watch and any audio output are
        // torn down before the widget tree is finalized.
        self.media = None;
        if let Some(fs_window) = self.fs_window.take() {
            fs_window.set_child(gtk::Widget::NONE);
            fs_window.destroy();
        }
    }
}

/// Hook the seek + volume scales: anything that isn't our own programmatic
/// write becomes a UserSeek/SetVolume; the seek slider also stamps an
/// `Instant` so `refresh_widgets` doesn't yank the thumb back during a drag.
fn wire_slider_handlers(
    widgets: &VideoPlayerWidgets,
    sender: &ComponentSender<VideoPlayer>,
    suppress_scale: &Rc<Cell<bool>>,
    suppress_volume: &Rc<Cell<bool>>,
    last_user_seek: &Rc<Cell<Option<Instant>>>,
) {
    {
        let sender = sender.clone();
        let suppress = suppress_scale.clone();
        let stamp = last_user_seek.clone();
        widgets.seek_scale.connect_value_changed(move |s| {
            if suppress.get() {
                return;
            }
            stamp.set(Some(Instant::now()));
            sender.input(VideoPlayerMsg::UserSeek(s.value() as i64));
        });
    }
    let sender = sender.clone();
    let suppress = suppress_volume.clone();
    widgets.volume_scale.connect_value_changed(move |s| {
        if suppress.get() {
            return;
        }
        sender.input(VideoPlayerMsg::SetVolume(s.value()));
    });
}

/// Pointer motion → wake controls. Single click anywhere on the surface
/// toggles play/pause; double click toggles fullscreen. Both paths also
/// keep the OSD visible (PointerActive).
fn wire_pointer_handlers(
    widgets: &VideoPlayerWidgets,
    sender: &ComponentSender<VideoPlayer>,
) {
    let motion = gtk::EventControllerMotion::new();
    let sender_m = sender.clone();
    motion.connect_motion(move |_, _, _| {
        sender_m.input(VideoPlayerMsg::PointerActive);
    });
    widgets.stack_overlay.add_controller(motion);

    let click = gtk::GestureClick::new();
    click.set_button(gtk::gdk::BUTTON_PRIMARY);
    let sender_c = sender.clone();
    let root_for_focus = widgets.root_box.clone();
    click.connect_pressed(move |_, n, _, _| {
        root_for_focus.grab_focus();
        if n == 1 {
            sender_c.input(VideoPlayerMsg::TogglePlay);
        } else if n == 2 {
            sender_c.input(VideoPlayerMsg::ToggleFullscreen);
        }
        sender_c.input(VideoPlayerMsg::PointerActive);
    });
    // Attach to the picture (not the controls bar) so clicks on the OSD
    // don't steal play/pause toggles.
    widgets.picture.add_controller(click);
}

/// Keyboard shortcuts: capture phase so the slider (and any other focused
/// descendant) doesn't eat arrow keys before we see them. Return `Stop`
/// for keys we recognise so we don't double-handle (e.g. focused seek
/// slider + our own seek-by-5s).
fn wire_keyboard_handlers(
    widgets: &VideoPlayerWidgets,
    sender: &ComponentSender<VideoPlayer>,
) {
    let key = gtk::EventControllerKey::new();
    key.set_propagation_phase(gtk::PropagationPhase::Capture);
    let sender_k = sender.clone();
    key.connect_key_pressed(move |_, keyval, _, mods| {
        if is_player_shortcut(keyval) {
            sender_k.input(VideoPlayerMsg::KeyPressed(keyval, mods));
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    widgets.root_box.add_controller(key);
}

impl VideoPlayer {
    /// Build the model struct with the right initial flags and shared
    /// suppression cells. Pulled out of `init` to keep that function
    /// short.
    fn new_model(
        init: &VideoPlayerInit,
        suppress_scale: Rc<Cell<bool>>,
        suppress_volume: Rc<Cell<bool>>,
        last_user_seek: Rc<Cell<Option<Instant>>>,
    ) -> Self {
        VideoPlayer {
            media: None,
            url: init.url.clone(),
            duration_us: 0,
            position_us: 0,
            volume: init.volume.clamp(0.0, 1.0),
            muted: init.muted,
            playback: PlaybackFlags {
                playing: false,
                autoplay: init.autoplay,
                eos_reached: false,
            },
            is_fullscreen: false,
            fs_window: None,
            fs_original_parent: None,
            osd: OsdState {
                show_controls: true,
                controls_revealed: true,
            },
            hide_source: None,
            tick_source: None,
            suppress_scale,
            suppress_volume,
            last_user_seek,
            resume_pending: None,
        }
    }

    fn refresh_widgets(
        &self,
        widgets: &<Self as Component>::Widgets,
        _root: &<Self as Component>::Root,
    ) {
        // Seek slider: max = duration, value = position. Clamp to a tiny
        // positive max to avoid GtkRange complaining when duration is 0.
        // Skip pushing polled values onto the thumb when (a) the stream
        // is mid-seek or (b) the user touched the slider very recently —
        // the polled timestamp lags GStreamer's SNAP_BEFORE keyframe and
        // would visibly snap the thumb backward in between drag updates.
        // 400 ms covers the gap between consecutive value-changed events
        // during a continuous drag without leaving the thumb desynced for
        // long after the user lets go.
        let media_seeking = self
            .media
            .as_ref()
            .is_some_and(|m| m.is_seeking());
        let user_holding = self
            .last_user_seek
            .get()
            .is_some_and(|t| t.elapsed() < Duration::from_millis(400));
        let max = (self.duration_us.max(1)) as f64;
        widgets.seek_scale.set_range(0.0, max);
        if !media_seeking && !user_holding {
            let pos = (self.position_us.clamp(0, self.duration_us.max(0))) as f64;
            self.suppress_scale.set(true);
            widgets.seek_scale.set_value(pos);
            self.suppress_scale.set(false);
        }
        widgets.seek_scale.set_sensitive(self.duration_us > 0);

        widgets
            .position_label
            .set_label(&format_us(self.position_us));
        let duration_text = if self.duration_us > 0 {
            format_us(self.duration_us)
        } else {
            "--:--".into()
        };
        widgets.duration_label.set_label(&duration_text);

        // Play / pause icon.
        widgets.play_button.set_icon_name(if self.playback.playing {
            "media-playback-pause-symbolic"
        } else {
            "media-playback-start-symbolic"
        });

        // Volume slider + button icon.
        let vol = if self.muted { 0.0 } else { self.volume };
        self.suppress_volume.set(true);
        widgets.volume_scale.set_value(vol);
        self.suppress_volume.set(false);
        widgets
            .volume_button
            .set_icon_name(volume_icon(self.muted, self.volume));

        // Fullscreen icon.
        widgets
            .fullscreen_button
            .set_icon_name(if self.is_fullscreen {
                "view-restore-symbolic"
            } else {
                "view-fullscreen-symbolic"
            });

        // OSD visibility: stay up only when no media is loaded yet so the
        // user has something to look at; otherwise let the inactivity timer
        // hide it whether playing or paused. `controls_revealed` was
        // computed (and emitted upward) in `update_with_view`.
        widgets.controls_revealer.set_reveal_child(self.osd.controls_revealed);

        // Cursor: hide on the player widget when controls are hidden so
        // the OSD "gets out of the way". Scoped to the widget (not the
        // toplevel surface) so the rest of the page keeps a normal
        // pointer.
        let cursor = if self.osd.controls_revealed {
            None
        } else {
            gtk::gdk::Cursor::from_name("none", None)
        };
        widgets.root_box.set_cursor(cursor.as_ref());
    }

    fn handle_key(
        &mut self,
        sender: &ComponentSender<Self>,
        key: gtk::gdk::Key,
        mods: gtk::gdk::ModifierType,
    ) -> bool {
        use gtk::gdk::Key;
        let shift = mods.contains(gtk::gdk::ModifierType::SHIFT_MASK);
        match key {
            Key::space | Key::k | Key::K => {
                sender.input(VideoPlayerMsg::TogglePlay);
                true
            }
            Key::Left => {
                let step = if shift { -1 } else { -5 };
                sender.input(VideoPlayerMsg::SeekRelative(step));
                true
            }
            Key::Right => {
                let step = if shift { 1 } else { 5 };
                sender.input(VideoPlayerMsg::SeekRelative(step));
                true
            }
            Key::j | Key::J => {
                sender.input(VideoPlayerMsg::SeekRelative(-10));
                true
            }
            Key::l | Key::L => {
                sender.input(VideoPlayerMsg::SeekRelative(10));
                true
            }
            Key::Up => {
                sender.input(VideoPlayerMsg::SeekRelative(60));
                true
            }
            Key::Down => {
                sender.input(VideoPlayerMsg::SeekRelative(-60));
                true
            }
            Key::Home => {
                sender.input(VideoPlayerMsg::SeekFraction(0.0));
                true
            }
            Key::End => {
                sender.input(VideoPlayerMsg::SeekFraction(1.0));
                true
            }
            Key::m | Key::M => {
                sender.input(VideoPlayerMsg::ToggleMute);
                true
            }
            Key::_9 => {
                sender.input(VideoPlayerMsg::AdjustVolume(-0.05));
                true
            }
            Key::_0 => {
                sender.input(VideoPlayerMsg::AdjustVolume(0.05));
                true
            }
            Key::f | Key::F => {
                sender.input(VideoPlayerMsg::ToggleFullscreen);
                true
            }
            Key::bracketleft | Key::bracketright => {
                let current = self
                    .media
                    .as_ref()
                    .map(PlaybackPipeline::speed)
                    .unwrap_or(1.0);
                let new = if key == Key::bracketleft {
                    (current - 0.1).max(0.1)
                } else {
                    (current + 0.1).min(4.0)
                };
                sender.input(VideoPlayerMsg::SetSpeed(new));
                true
            }
            Key::Escape if self.is_fullscreen => {
                sender.input(VideoPlayerMsg::ExitFullscreen);
                true
            }
            _ => false,
        }
    }

    fn handle_set_url(
        &mut self,
        widgets: &mut <Self as Component>::Widgets,
        sender: &ComponentSender<Self>,
        url: Option<String>,
        resume_secs: Option<f64>,
    ) {
        self.url = url.clone();
        self.duration_us = 0;
        self.position_us = 0;
        self.playback.playing = false;
        self.playback.eos_reached = false;
        self.resume_pending = resume_secs.filter(|s| *s > 0.0);

        self.media = None;

        let Some(url) = url else {
            widgets.picture.set_paintable(gtk::gdk::Paintable::NONE);
            let _ = sender.output(VideoPlayerOutput::StateChanged(PlayState::Stopped));
            return;
        };

        let Some(pipeline) = PlaybackPipeline::new(&url, self.playback.autoplay, sender) else {
            widgets.picture.set_paintable(gtk::gdk::Paintable::NONE);
            let msg = "Video playback unavailable (missing GStreamer plugins)".to_string();
            let _ = sender.output(VideoPlayerOutput::Error(msg));
            return;
        };

        pipeline.set_volume(self.volume);
        pipeline.set_muted(self.muted);
        widgets.picture.set_paintable(Some(pipeline.paintable()));
        if self.playback.autoplay {
            self.playback.playing = true;
        }
        self.media = Some(pipeline);
    }

    fn handle_tick(
        &mut self,
        widgets: &mut <Self as Component>::Widgets,
        sender: &ComponentSender<Self>,
    ) {
        let Some(media) = self.media.as_ref() else {
            return;
        };
        let was_playing = self.playback.playing;
        let snapshot = TickSnapshot {
            status: PipelineStatus {
                now_playing: media.is_playing(),
                is_prepared: media.is_prepared(),
                media_seeking: media.is_seeking(),
            },
            duration_us: media.duration_us().max(0),
            position_us: media.position_us().max(0),
            volume: media.volume(),
            muted: media.is_muted(),
            error_msg: media.error_message(),
        };
        let status = snapshot.status;

        let eos = !status.now_playing
            && was_playing
            && snapshot.duration_us > 0
            && snapshot.position_us >= snapshot.duration_us - 500_000;

        if eos && !self.playback.eos_reached {
            self.playback.eos_reached = true;
            self.playback.playing = false;
            let _ = sender.output(VideoPlayerOutput::EndOfFile);
            let _ = sender.output(VideoPlayerOutput::StateChanged(PlayState::Stopped));
            update_status_plate(widgets, snapshot.error_msg.as_deref(), status.is_prepared);
            return;
        }

        if snapshot.duration_us > 0 {
            let first_load = self.duration_us == 0;
            self.duration_us = snapshot.duration_us;
            if first_load
                && status.is_prepared
                && !self.playback.eos_reached
                && let Some(path) = self.url.clone()
            {
                let duration_secs = self.duration_us as f64 / 1_000_000.0;
                let _ = sender.output(VideoPlayerOutput::FileLoaded {
                    path,
                    duration_secs,
                });
            }
        }

        self.volume = snapshot.volume;
        self.muted = snapshot.muted;

        let prev_position = self.position_us;
        self.update_position_from_poll(snapshot.position_us, status.media_seeking);
        if self.position_us != prev_position {
            let _ = sender.output(VideoPlayerOutput::PositionChanged {
                position_secs: self.position_us as f64 / 1_000_000.0,
                duration_secs: self.duration_us as f64 / 1_000_000.0,
            });
        }

        if was_playing != status.now_playing {
            self.playback.playing = status.now_playing;
            let state = if status.now_playing {
                PlayState::Playing
            } else {
                PlayState::Paused
            };
            let _ = sender.output(VideoPlayerOutput::StateChanged(state));
            flash_center(&widgets.center_indicator, status.now_playing);
        } else {
            self.playback.playing = status.now_playing;
        }

        self.apply_pending_resume(status.is_prepared);
        update_status_plate(widgets, snapshot.error_msg.as_deref(), status.is_prepared);
    }

    /// Trust the polled position only when there isn't a user-initiated
    /// seek in flight. The seek handlers stamp `last_user_seek` before
    /// they call `media.seek()`; that timestamp, plus the bus-driven
    /// `is_seeking` flag, defines the window we ignore polled values in.
    fn update_position_from_poll(&mut self, position_us: i64, media_seeking: bool) {
        let user_seek_recent = self
            .last_user_seek
            .get()
            .is_some_and(|t| t.elapsed() < Duration::from_millis(400));
        if media_seeking || user_seek_recent {
            tracing::trace!(
                polled_us = position_us,
                held_us = self.position_us,
                media_seeking,
                user_seek_recent,
                "Tick: holding position_us (seek window)"
            );
            return;
        }
        if position_us != self.position_us {
            tracing::trace!(
                old_us = self.position_us,
                new_us = position_us,
                "Tick: position_us updated from poll"
            );
        }
        self.position_us = position_us;
    }

    /// Apply the pending resume seek once the stream is ready enough
    /// that a clamp against duration is meaningful.
    fn apply_pending_resume(&mut self, is_prepared: bool) {
        if !is_prepared || self.duration_us <= 0 {
            return;
        }
        let Some(resume) = self.resume_pending.take() else {
            return;
        };
        let target_us = (resume * 1_000_000.0) as i64;
        let target = target_us.clamp(0, self.duration_us);
        if let Some(media) = self.media.as_ref() {
            media.seek(target);
        }
        self.position_us = target;
    }

    fn handle_toggle_play(
        &mut self,
        widgets: &mut <Self as Component>::Widgets,
        sender: &ComponentSender<Self>,
    ) {
        let was_playing = self.media.as_ref().is_some_and(|m| m.is_playing());
        let Some(media) = self.media.as_ref() else {
            return;
        };
        if was_playing {
            media.pause();
            let _ = sender.output(VideoPlayerOutput::StateChanged(PlayState::Paused));
        } else {
            if self.playback.eos_reached {
                media.seek(0);
                self.position_us = 0;
                self.playback.eos_reached = false;
            }
            media.play();
            let _ = sender.output(VideoPlayerOutput::StateChanged(PlayState::Playing));
        }
        let now_playing = self.media.as_ref().is_some_and(|m| m.is_playing());
        // playbin3 state changes are async — assume the requested state
        // until the bus confirms.
        self.playback.playing = if was_playing { now_playing } else { true };
        flash_center(&widgets.center_indicator, self.playback.playing);
        sender.input(VideoPlayerMsg::PointerActive);
    }

    fn handle_seek_relative(&mut self, sender: &ComponentSender<Self>, secs: i64) {
        let Some(media) = self.media.as_ref() else {
            return;
        };
        if !media.is_prepared() {
            return;
        }
        let delta = secs.saturating_mul(1_000_000);
        // Anchor on our local position and our cached duration, not the
        // live `media.*()` queries. While a seek is in flight playbin3
        // reports the pre-seek (or 0) position and a 0 duration, so two
        // quick presses of "+10s" would both compute their target from
        // the same base and the clamp would collapse the target to 0.
        let base_us = self.position_us;
        let cached_dur_us = self.duration_us.max(0);
        let target = base_us.saturating_add(delta).clamp(0, cached_dur_us);
        tracing::debug!(
            secs,
            delta_us = delta,
            base_us,
            cached_dur_us,
            target_us = target,
            live_pos_us = media.position_us(),
            live_dur_us = media.duration_us(),
            "SeekRelative"
        );
        self.last_user_seek.set(Some(Instant::now()));
        media.seek(target);
        self.position_us = target;
        sender.input(VideoPlayerMsg::PointerActive);
    }

    fn handle_seek_fraction(&mut self, sender: &ComponentSender<Self>, fraction: f64) {
        let Some(media) = self.media.as_ref() else {
            return;
        };
        if !media.is_prepared() {
            return;
        }
        let dur = self.duration_us.max(0);
        let target = ((dur as f64) * fraction.clamp(0.0, 1.0)) as i64;
        tracing::debug!(
            fraction,
            cached_dur_us = dur,
            target_us = target,
            "SeekFraction"
        );
        self.last_user_seek.set(Some(Instant::now()));
        media.seek(target);
        self.position_us = target;
        sender.input(VideoPlayerMsg::PointerActive);
    }

    fn handle_user_seek(&mut self, _sender: &ComponentSender<Self>, us: i64) {
        let Some(media) = self.media.as_ref() else {
            return;
        };
        if !media.is_prepared() {
            return;
        }
        let target = us.clamp(0, self.duration_us.max(0));
        tracing::debug!(
            slider_us = us,
            cached_dur_us = self.duration_us,
            target_us = target,
            "UserSeek"
        );
        self.last_user_seek.set(Some(Instant::now()));
        media.seek(target);
        self.position_us = target;
    }

    fn handle_set_volume(&mut self, sender: &ComponentSender<Self>, v: f64) {
        let v = v.clamp(0.0, 1.0);
        self.volume = v;
        if let Some(media) = self.media.as_ref() {
            media.set_volume(v);
            if v > 0.0 && media.is_muted() {
                media.set_muted(false);
                self.muted = false;
            }
        }
        let _ = sender.output(VideoPlayerOutput::VolumeChanged {
            volume: self.volume,
            muted: self.muted,
        });
    }

    fn handle_toggle_mute(&mut self, sender: &ComponentSender<Self>) {
        let Some(media) = self.media.as_ref() else {
            return;
        };
        let new_muted = !media.is_muted();
        media.set_muted(new_muted);
        self.muted = new_muted;
        sender.input(VideoPlayerMsg::PointerActive);
        let _ = sender.output(VideoPlayerOutput::VolumeChanged {
            volume: self.volume,
            muted: self.muted,
        });
    }

    fn handle_toggle_fullscreen(
        &mut self,
        widgets: &mut <Self as Component>::Widgets,
        sender: &ComponentSender<Self>,
    ) {
        if self.is_fullscreen {
            sender.input(VideoPlayerMsg::ExitFullscreen);
            return;
        }
        // Reparent the player into a transient borderless window so
        // fullscreen covers only the video, not the rest of the app
        // chrome. The root_box is removed from its current parent (an
        // Overlay in the scene page) and reattached on exit.
        let Some(parent) = widgets.root_box.parent() else {
            return;
        };
        let Some(overlay) = parent.downcast_ref::<gtk::Overlay>() else {
            return;
        };
        overlay.set_child(gtk::Widget::NONE);

        let app_window = widgets.root_box.root().and_downcast::<gtk::Window>();
        let fs_window = gtk::Window::builder()
            .decorated(false)
            .child(&widgets.root_box)
            .build();
        if let Some(app) = app_window.as_ref() {
            fs_window.set_transient_for(Some(app));
        }
        fs_window.fullscreen();

        let sender_close = sender.clone();
        fs_window.connect_close_request(move |_| {
            sender_close.input(VideoPlayerMsg::ExitFullscreen);
            glib::Propagation::Stop
        });

        fs_window.present();
        widgets.root_box.grab_focus();

        self.fs_window = Some(fs_window);
        self.fs_original_parent = Some(parent);
        self.is_fullscreen = true;
        sender.input(VideoPlayerMsg::PointerActive);
    }

    fn handle_exit_fullscreen(&mut self, widgets: &mut <Self as Component>::Widgets) {
        if !self.is_fullscreen {
            return;
        }
        let Some(fs_window) = self.fs_window.take() else {
            return;
        };
        // Detach from the fullscreen window before destroying so the
        // widget survives, then reparent into the original overlay slot.
        fs_window.set_child(gtk::Widget::NONE);
        if let Some(overlay) = self
            .fs_original_parent
            .take()
            .and_then(|p| p.downcast::<gtk::Overlay>().ok())
        {
            overlay.set_child(Some(&widgets.root_box));
        }
        fs_window.destroy();
        self.is_fullscreen = false;
    }

    fn handle_pointer_active(&mut self, sender: &ComponentSender<Self>) {
        self.osd.show_controls = true;
        if let Some(id) = self.hide_source.take() {
            id.remove();
        }
        let sender = sender.clone();
        let id = glib::timeout_add_local_once(
            std::time::Duration::from_millis(HIDE_DELAY_MS as u64),
            move || sender.input(VideoPlayerMsg::HideControls),
        );
        self.hide_source = Some(id);
    }
}

/// Pipeline-state flags captured at the start of a tick.
#[derive(Clone, Copy)]
struct PipelineStatus {
    now_playing: bool,
    is_prepared: bool,
    media_seeking: bool,
}

/// Snapshot of pipeline state captured once per tick so the rest of the
/// handler can read consistent values without re-querying GStreamer.
struct TickSnapshot {
    status: PipelineStatus,
    duration_us: i64,
    position_us: i64,
    volume: f64,
    muted: bool,
    error_msg: Option<String>,
}

/// Drive the central status plate. We only show it for initial loading
/// and for terminal errors — seeking intentionally doesn't trigger it.
fn update_status_plate(
    widgets: &VideoPlayerWidgets,
    error_msg: Option<&str>,
    is_prepared: bool,
) {
    if let Some(msg) = error_msg {
        widgets.status_spinner.set_spinning(false);
        widgets.status_spinner.set_visible(false);
        widgets.status_icon.set_visible(true);
        widgets.status_title.set_label("Couldn't play video");
        widgets.status_detail.set_label(msg);
        widgets.status_detail.set_visible(true);
        widgets.status_plate.set_visible(true);
    } else if !is_prepared {
        widgets.status_icon.set_visible(false);
        widgets.status_spinner.set_visible(true);
        widgets.status_spinner.set_spinning(true);
        widgets.status_title.set_label("Loading video…");
        widgets.status_detail.set_visible(false);
        widgets.status_plate.set_visible(true);
    } else {
        widgets.status_plate.set_visible(false);
        widgets.status_spinner.set_spinning(false);
    }
}

fn is_player_shortcut(key: gtk::gdk::Key) -> bool {
    use gtk::gdk::Key;
    matches!(
        key,
        Key::space
            | Key::k
            | Key::K
            | Key::Left
            | Key::Right
            | Key::Up
            | Key::Down
            | Key::j
            | Key::J
            | Key::l
            | Key::L
            | Key::Home
            | Key::End
            | Key::m
            | Key::M
            | Key::_9
            | Key::_0
            | Key::f
            | Key::F
            | Key::Escape
    )
}

fn flash_center(image: &gtk::Image, playing: bool) {
    image.set_icon_name(Some(if playing {
        "media-playback-start-symbolic"
    } else {
        "media-playback-pause-symbolic"
    }));
    image.set_visible(true);
    image.remove_css_class("video-center-indicator-flash");
    image.add_css_class("video-center-indicator-flash");
    let weak = image.downgrade();
    glib::timeout_add_local_once(std::time::Duration::from_millis(550), move || {
        if let Some(img) = weak.upgrade() {
            img.set_visible(false);
            img.remove_css_class("video-center-indicator-flash");
        }
    });
}

fn format_us(us: i64) -> String {
    let total = (us.max(0) / 1_000_000) as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

fn volume_icon(muted: bool, volume: f64) -> &'static str {
    if muted || volume <= 0.001 {
        "audio-volume-muted-symbolic"
    } else if volume < 0.34 {
        "audio-volume-low-symbolic"
    } else if volume < 0.67 {
        "audio-volume-medium-symbolic"
    } else {
        "audio-volume-high-symbolic"
    }
}
