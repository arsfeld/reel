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

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gst::prelude::*;
use gtk::glib;
use gtk::prelude::*;
use relm4::gtk;
use relm4::prelude::*;

use crate::components::player::status_plate;
use crate::components::player::video_player_chrome::{
    flash_center, format_us, is_player_shortcut, rebuild_track_popovers, volume_icon,
};
use crate::player::PlayState;
use crate::player::SkipMarkers;
use crate::player::gst_pipeline::PlaybackPipeline;
use crate::player::pipeline_msgs::PipelineBusMsg;
use crate::player::subtitles::is_subtitle_extension;
use crate::player::tracks::{MediaTrack, TrackKind};

const TICK_INTERVAL_MS: u32 = 250;
const HIDE_DELAY_MS: u32 = 2500;

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
    /// Preferred subtitle language from settings (ISO 639-1).
    pub(crate) preferred_subtitle_lang: Option<String>,
}

impl Default for VideoPlayerInit {
    fn default() -> Self {
        Self {
            url: None,
            autoplay: false,
            resume_secs: None,
            volume: 1.0,
            muted: false,
            preferred_subtitle_lang: None,
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
    /// User picked a quality selection (U8): re-resolve at this selection +
    /// position.
    SelectQuality {
        selection: crate::models::playback::QualitySelection,
        position_secs: f64,
    },
    /// A seek during an active transcode (U8/KTD2): the parent re-resolves at the
    /// new offset rather than seeking in-pipeline.
    SeekReload {
        position_secs: f64,
    },
    VolumeChanged {
        volume: f64,
        muted: bool,
    },
    SpeedChanged(f64),
    ToggleFullscreen,
    LoadSubtitleFile,
    Leave,
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
        /// Content-time offset the (transcode) stream was built at; added back to
        /// playbin3's 0-based position for display (U8). 0 for direct-play.
        base_offset_secs: f64,
        /// Whether this is a server transcode — seeks reload at a new offset.
        is_transcode: bool,
    },
    /// Switch quality mid-playback (U8): emitted by the quality menu (U9). The
    /// player captures its current content position and asks the parent to
    /// re-resolve at the chosen selection (Auto or a manual rung).
    SelectQuality(crate::models::playback::QualitySelection),
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
    LoadExternalSubtitle(String),
    FullscreenChanged(bool),
    FilesDropped(String),
    StreamCollection(gst::StreamCollection),
    StreamsSelected {
        collection: gst::StreamCollection,
        streams: Vec<gst::Stream>,
    },
    SelectAudio(String),
    SelectSubtitle(Option<String>),
    SetTitle(Option<String>),
    /// Update the quality menu (U9): whether quality control applies to this
    /// source, the active selection (Auto / a manual rung), and the decision
    /// indicator text ("Direct Play" / "Transcoding 1080p · 8 Mbps", R16).
    SetDecisionInfo {
        available: bool,
        selection: crate::models::playback::QualitySelection,
        indicator: String,
    },
    PopoverVisibilityChanged(bool),
    ClosePopovers,
    /// Set intro/credits skip markers for the current media.
    SetSkipMarkers(SkipMarkers),
    /// Seek past the intro (to intro_end_secs).
    SkipIntro,
    /// Seek past the credits (to end of media).
    SkipCredits,
    /// Context-aware skip: intro if in intro range, credits if in
    /// credits range, no-op otherwise. Used by the overlay button
    /// and the `s` keyboard shortcut.
    SkipCurrent,
    /// Buffering progress (0-100) from the pipeline bus, for the status-plate
    /// indicator.
    Buffering(i32),
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
    /// Track selector popover is open — keep chrome visible.
    popover_open: bool,
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
    /// Content-time offset of the current transcode stream (U8). playbin3
    /// reports a 0-based position on an offset-built transcode while reporting
    /// the full duration, so this is added back for the displayed position /
    /// seek bar / scrobble. 0 for direct-play.
    transcode_base_offset_us: i64,
    /// Whether the current stream is a server transcode: seeks reload at a new
    /// offset (via the parent) rather than seeking in-pipeline (KTD2).
    is_transcode: bool,
    preferred_subtitle_lang: Option<String>,
    tracks: Vec<MediaTrack>,
    title: Option<String>,
    track_ui_signature: String,
    /// Skip intro / credits markers for the current media. `None` until
    /// set by the parent (e.g. from Plex metadata).
    skip_markers: Option<SkipMarkers>,
    /// Buffering percent (0-100) while a network rebuffer/fill is active;
    /// `None` once filled (>= 100) or never buffering. Drives the status
    /// plate and suppresses poll-derived state flicker during buffering.
    buffering_percent: Option<i32>,
    /// Quality menu state (U9). `quality_available` gates the menu's
    /// sensitivity (only Plex sources can re-decide); `current_selection` is
    /// the active radio item; `decision_indicator` is the R16 indicator text.
    quality_available: bool,
    current_selection: crate::models::playback::QualitySelection,
    decision_indicator: String,
    quality_ui_signature: String,
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

                // Top chrome for fullscreen (back + title). Windowed mode
                // uses the app-level titlebar synced to the same reveal flag.
                add_overlay = &gtk::Box {
                    set_valign: gtk::Align::Start,
                    set_halign: gtk::Align::Fill,

                    #[name = "top_chrome_revealer"]
                    gtk::Revealer {
                        set_transition_type: gtk::RevealerTransitionType::Crossfade,
                        set_transition_duration: 180,
                        set_reveal_child: true,
                        set_hexpand: true,

                        #[name = "top_chrome_box"]
                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            add_css_class: "video-chrome",
                            set_spacing: 8,
                            set_margin_start: 8,
                            set_margin_end: 8,
                            set_margin_top: 8,
                            set_margin_bottom: 24,

                            #[name = "back_button"]
                            gtk::Button {
                                set_icon_name: "go-back-symbolic",
                                add_css_class: "flat",
                                add_css_class: "circular",
                                set_tooltip_text: Some("Back to library (Esc)"),
                                connect_clicked[sender] => move |_| {
                                    let _ = sender.output(VideoPlayerOutput::Leave);
                                },
                            },

                            #[name = "title_label"]
                            gtk::Label {
                                set_hexpand: true,
                                set_xalign: 0.0,
                                add_css_class: "video-chrome-title",
                                set_ellipsize: gtk::pango::EllipsizeMode::End,
                            },
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

                                #[name = "audio_menu"]
                                gtk::MenuButton {
                                    set_icon_name: "audio-x-generic-symbolic",
                                    add_css_class: "flat",
                                    add_css_class: "circular",
                                    set_tooltip_text: Some("Audio track"),
                                    #[wrap(Some)]
                                    set_popover = &gtk::Popover {
                                        gtk::Box {
                                            set_orientation: gtk::Orientation::Vertical,
                                            set_spacing: 4,
                                            set_margin_top: 8,
                                            set_margin_bottom: 8,
                                            set_margin_start: 8,
                                            set_margin_end: 8,
                                            #[name = "audio_tracks_box"]
                                            gtk::Box {
                                                set_orientation: gtk::Orientation::Vertical,
                                                set_spacing: 4,
                                            },
                                        },
                                    },
                                },

                                #[name = "subtitle_menu"]
                                gtk::MenuButton {
                                    set_icon_name: "media-view-subtitles-symbolic",
                                    add_css_class: "flat",
                                    add_css_class: "circular",
                                    set_tooltip_text: Some("Subtitles"),
                                    #[wrap(Some)]
                                    set_popover = &gtk::Popover {
                                        gtk::Box {
                                            set_orientation: gtk::Orientation::Vertical,
                                            set_spacing: 4,
                                            set_margin_top: 8,
                                            set_margin_bottom: 8,
                                            set_margin_start: 8,
                                            set_margin_end: 8,
                                            #[name = "subtitle_tracks_box"]
                                            gtk::Box {
                                                set_orientation: gtk::Orientation::Vertical,
                                                set_spacing: 4,
                                            },
                                        },
                                    },
                                },

                                #[name = "quality_menu"]
                                gtk::MenuButton {
                                    set_icon_name: "video-display-symbolic",
                                    add_css_class: "flat",
                                    add_css_class: "circular",
                                    set_tooltip_text: Some("Quality"),
                                    #[wrap(Some)]
                                    set_popover = &gtk::Popover {
                                        gtk::Box {
                                            set_orientation: gtk::Orientation::Vertical,
                                            set_spacing: 4,
                                            set_margin_top: 8,
                                            set_margin_bottom: 8,
                                            set_margin_start: 8,
                                            set_margin_end: 8,
                                            #[name = "quality_indicator"]
                                            gtk::Label {
                                                add_css_class: "dim-label",
                                                set_xalign: 0.0,
                                                set_visible: false,
                                            },
                                            #[name = "quality_box"]
                                            gtk::Box {
                                                set_orientation: gtk::Orientation::Vertical,
                                                set_spacing: 4,
                                            },
                                        },
                                    },
                                },

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

                // Skip intro / credits button. Positioned near the
                // bottom-end corner so it doesn't overlap transport
                // controls. Visibility is driven by refresh_widgets
                // based on the current position versus SkipMarkers.
                add_overlay = &gtk::Box {
                    set_valign: gtk::Align::End,
                    set_halign: gtk::Align::End,
                    set_margin_bottom: 90,
                    set_margin_end: 16,

                    #[name = "skip_button"]
                    gtk::Button {
                        set_label: "Skip Intro",
                        add_css_class: "suggested-action",
                        add_css_class: "circular",
                        set_visible: false,
                        connect_clicked[sender] => move |_| {
                            sender.input(VideoPlayerMsg::SkipCurrent);
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
        wire_popover_handlers(&widgets, &sender);

        // Kick off the position-polling timer.
        let tick =
            glib::timeout_add_local(std::time::Duration::from_millis(TICK_INTERVAL_MS as u64), {
                let sender = sender.clone();
                move || {
                    sender.input(VideoPlayerMsg::Tick);
                    glib::ControlFlow::Continue
                }
            });

        // Start the inactivity timer immediately so controls hide on a
        // still scene. They'll re-reveal on the next pointer/keypress.
        sender.input(VideoPlayerMsg::PointerActive);

        model.tick_source = Some(tick);

        if let Some(url) = init.url {
            sender.input(VideoPlayerMsg::SetUrl {
                url: Some(url),
                resume_secs: init.resume_secs,
                base_offset_secs: 0.0,
                is_transcode: false,
            });
        }

        ComponentParts { model, widgets }
    }

    // Message dispatcher; one arm per VideoPlayerMsg (same allow as the App's
    // init / update_cmd / handle_video_output dispatchers).
    #[allow(clippy::too_many_lines)]
    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        msg: VideoPlayerMsg,
        sender: ComponentSender<Self>,
        root: &Self::Root,
    ) {
        if self.dispatch_msg(widgets, sender.clone(), msg) {
            return;
        }

        // Notify the parent on reveal-state edges so it can fade the
        // floating header bar together with the OSD. Computed here (not
        // in `refresh_widgets`) because we need `&mut self` to latch the
        // last value, and emitting only on changes avoids spamming the
        // parent on every tick.
        let force_visible = self.media.is_none() || self.duration_us == 0;
        let revealed = self.osd.show_controls || force_visible || self.osd.popover_open;
        if revealed != self.osd.controls_revealed {
            self.osd.controls_revealed = revealed;
            let _ = sender.output(VideoPlayerOutput::ControlsRevealedChanged(revealed));
        }

        // Re-render derived widget state. We keep this manual because
        // many of these properties depend on multiple model fields and a
        // few need to skip our own value-changed handlers.
        rebuild_track_popovers(
            widgets,
            &self.tracks,
            self.media.as_ref(),
            &sender,
            &mut self.track_ui_signature,
        );
        crate::components::player::quality_menu::rebuild_quality_popover(
            widgets,
            self.current_selection,
            &self.decision_indicator,
            &mut self.quality_ui_signature,
            &sender,
        );
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

impl VideoPlayer {
    /// Dispatch a single input message to its handler. Returns `true` if the
    /// caller should return early without running the post-dispatch widget
    /// refresh (used by messages that bail out, e.g. dropped subtitle files).
    // Message dispatcher; one arm per VideoPlayerMsg (same allow as the App's
    // dispatchers).
    #[allow(clippy::too_many_lines)]
    fn dispatch_msg(
        &mut self,
        widgets: &mut <Self as relm4::Component>::Widgets,
        sender: ComponentSender<Self>,
        msg: VideoPlayerMsg,
    ) -> bool {
        match msg {
            VideoPlayerMsg::LoadFile(path) => {
                let url = format!("file://{}", path);
                self.handle_set_url(widgets, &sender, Some(url), None, 0.0, false, Some(path));
            }
            VideoPlayerMsg::SetUrl {
                url,
                resume_secs,
                base_offset_secs,
                is_transcode,
            } => {
                self.handle_set_url(
                    widgets,
                    &sender,
                    url,
                    resume_secs,
                    base_offset_secs,
                    is_transcode,
                    None,
                );
            }
            VideoPlayerMsg::SelectQuality(preset) => self.handle_select_quality(&sender, preset),
            VideoPlayerMsg::Clear => {
                self.tracks.clear();
                self.track_ui_signature.clear();
                self.handle_set_url(widgets, &sender, None, None, 0.0, false, None);
            }
            VideoPlayerMsg::SetAutoplay(on) => self.playback.autoplay = on,
            VideoPlayerMsg::Tick => self.handle_tick(widgets, &sender),
            VideoPlayerMsg::Buffering(percent) => self.handle_buffering(widgets, percent),
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
                if !self.osd.popover_open {
                    self.osd.show_controls = false;
                }
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
                let path = uri.strip_prefix("file://").unwrap_or(&uri);
                if is_subtitle_extension(std::path::Path::new(path)) {
                    if self.media.is_some() {
                        let sub_uri = if uri.starts_with("file://") {
                            uri.clone()
                        } else {
                            format!("file://{uri}")
                        };
                        sender.input(VideoPlayerMsg::LoadExternalSubtitle(sub_uri));
                    }
                    return true;
                }
                let url = if uri.starts_with("file://") {
                    uri.clone()
                } else {
                    format!("file://{}", uri)
                };
                self.handle_set_url(
                    widgets,
                    &sender,
                    Some(url),
                    None,
                    0.0,
                    false,
                    Some(path.to_string()),
                );
            }
            other => self.dispatch_track_msg(widgets, &sender, other),
        }

        false
    }

    /// Handles the track-selection, subtitle, popover, and skip-marker messages.
    /// Split out of `dispatch_msg` to keep each handler under the line cap.
    fn dispatch_track_msg(
        &mut self,
        widgets: &mut <Self as relm4::Component>::Widgets,
        sender: &ComponentSender<Self>,
        msg: VideoPlayerMsg,
    ) {
        match msg {
            VideoPlayerMsg::StreamCollection(collection) => {
                if let Some(media) = self.media.as_ref() {
                    media.handle_stream_collection(&collection);
                    self.tracks = media.tracks();
                }
            }
            VideoPlayerMsg::StreamsSelected {
                collection,
                streams,
            } => {
                if let Some(media) = self.media.as_ref() {
                    media.handle_streams_selected(&collection, &streams);
                    self.tracks = media.tracks();
                }
            }
            VideoPlayerMsg::SelectAudio(id) => {
                if let Some(media) = self.media.as_ref() {
                    media.select_audio(&id);
                }
                sender.input(VideoPlayerMsg::PointerActive);
            }
            VideoPlayerMsg::SelectSubtitle(id) => {
                if let Some(media) = self.media.as_ref() {
                    media.select_subtitle(id.as_deref());
                    self.tracks = media.tracks();
                }
                sender.input(VideoPlayerMsg::PointerActive);
            }
            VideoPlayerMsg::LoadExternalSubtitle(uri) => {
                if let Some(media) = self.media.as_ref() {
                    media.load_external_subtitle(&uri);
                }
                sender.input(VideoPlayerMsg::PointerActive);
            }
            VideoPlayerMsg::SetTitle(title) => {
                self.title = title;
            }
            VideoPlayerMsg::SetDecisionInfo {
                available,
                selection,
                indicator,
            } => {
                self.quality_available = available;
                self.current_selection = selection;
                self.decision_indicator = indicator;
            }
            VideoPlayerMsg::PopoverVisibilityChanged(open) => {
                self.osd.popover_open = open;
                if open {
                    sender.input(VideoPlayerMsg::PointerActive);
                }
            }
            VideoPlayerMsg::ClosePopovers => {
                widgets.audio_menu.popdown();
                widgets.subtitle_menu.popdown();
                self.osd.popover_open = false;
            }
            VideoPlayerMsg::SetSkipMarkers(markers) => {
                self.skip_markers = Some(markers);
            }
            VideoPlayerMsg::SkipIntro => {
                self.handle_skip_intro(sender);
            }
            VideoPlayerMsg::SkipCredits => {
                self.handle_skip_credits(sender);
            }
            VideoPlayerMsg::SkipCurrent => {
                self.handle_skip_current(sender);
            }
            _ => {}
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
fn wire_pointer_handlers(widgets: &VideoPlayerWidgets, sender: &ComponentSender<VideoPlayer>) {
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
fn wire_keyboard_handlers(widgets: &VideoPlayerWidgets, sender: &ComponentSender<VideoPlayer>) {
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
                popover_open: false,
            },
            hide_source: None,
            tick_source: None,
            suppress_scale,
            suppress_volume,
            last_user_seek,
            resume_pending: None,
            transcode_base_offset_us: 0,
            is_transcode: false,
            preferred_subtitle_lang: init.preferred_subtitle_lang.clone(),
            tracks: Vec::new(),
            title: None,
            track_ui_signature: String::new(),
            skip_markers: None,
            buffering_percent: None,
            quality_available: false,
            current_selection: crate::models::playback::QualitySelection::Auto,
            decision_indicator: String::new(),
            quality_ui_signature: String::new(),
        }
    }

    /// The content position to display/report: playbin3's 0-based position plus
    /// the transcode base offset (U8). Identity for direct-play (offset 0).
    fn display_position_us(&self) -> i64 {
        self.position_us + self.transcode_base_offset_us
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
        let media_seeking = self.media.as_ref().is_some_and(|m| m.is_seeking());
        let user_holding = self
            .last_user_seek
            .get()
            .is_some_and(|t| t.elapsed() < Duration::from_millis(400));
        let max = (self.duration_us.max(1)) as f64;
        widgets.seek_scale.set_range(0.0, max);
        if !media_seeking && !user_holding {
            let pos = (self.display_position_us().clamp(0, self.duration_us.max(0))) as f64;
            self.suppress_scale.set(true);
            widgets.seek_scale.set_value(pos);
            self.suppress_scale.set(false);
        }
        widgets.seek_scale.set_sensitive(self.duration_us > 0);

        widgets
            .position_label
            .set_label(&format_us(self.display_position_us()));
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
        widgets
            .controls_revealer
            .set_reveal_child(self.osd.controls_revealed);
        widgets
            .top_chrome_revealer
            .set_reveal_child(self.osd.controls_revealed && self.is_fullscreen);

        if let Some(title) = &self.title {
            widgets.title_label.set_label(title);
        }

        let has_audio = self.tracks.iter().any(|t| t.kind == TrackKind::Audio);
        let has_subs = self.tracks.iter().any(|t| t.kind == TrackKind::Subtitle);
        widgets.audio_menu.set_sensitive(has_audio);
        widgets
            .subtitle_menu
            .set_sensitive(has_audio || has_subs || self.media.is_some());
        // Quality control only applies to sources that can re-decide (Plex).
        widgets.quality_menu.set_visible(self.quality_available);

        // Skip intro / credits button visibility. Only show when the OSD
        // is revealed and the playhead is inside a marked range.
        {
            let pos_secs = self.position_us as f64 / 1_000_000.0;
            let (visible, label) = self
                .skip_markers
                .as_ref()
                .filter(|_| self.osd.controls_revealed)
                .and_then(|m| {
                    if m.credits_start_secs > 0.0 && pos_secs >= m.credits_start_secs {
                        Some((true, "Skip Credits"))
                    } else if m.intro_end_secs > 0.0
                        && pos_secs >= m.intro_start_secs
                        && pos_secs < m.intro_end_secs
                    {
                        Some((true, "Skip Intro"))
                    } else {
                        None
                    }
                })
                .unwrap_or((false, "Skip Intro"));
            widgets.skip_button.set_visible(visible);
            widgets.skip_button.set_label(label);
        }

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
            Key::s | Key::S => {
                sender.input(VideoPlayerMsg::SkipCurrent);
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
            Key::Escape => {
                if self.osd.popover_open {
                    sender.input(VideoPlayerMsg::ClosePopovers);
                } else if self.is_fullscreen {
                    sender.input(VideoPlayerMsg::ExitFullscreen);
                } else {
                    let _ = sender.output(VideoPlayerOutput::Leave);
                }
                true
            }
            _ => false,
        }
    }

    /// Seek past the intro: jump to `intro_end_secs`.
    fn handle_skip_intro(&mut self, _sender: &ComponentSender<Self>) {
        let Some(ref markers) = self.skip_markers else {
            return;
        };
        if markers.intro_end_secs <= 0.0 || self.duration_us <= 0 {
            return;
        }
        let pos_secs = self.position_us as f64 / 1_000_000.0;
        if pos_secs >= markers.intro_start_secs && pos_secs < markers.intro_end_secs {
            let target = (markers.intro_end_secs * 1_000_000.0) as i64;
            let target = target.clamp(0, self.duration_us);
            if let Some(media) = self.media.as_ref() {
                media.seek(target);
            }
            self.position_us = target;
            self.last_user_seek.set(Some(Instant::now()));
        }
    }

    /// Seek past the credits: jump to end of media.
    fn handle_skip_credits(&mut self, sender: &ComponentSender<Self>) {
        let Some(ref markers) = self.skip_markers else {
            return;
        };
        if markers.credits_start_secs <= 0.0 || self.duration_us <= 0 {
            return;
        }
        let pos_secs = self.position_us as f64 / 1_000_000.0;
        if pos_secs >= markers.credits_start_secs {
            sender.input(VideoPlayerMsg::SeekFraction(1.0));
        }
    }

    /// Context-aware skip: dispatches to skip-intro or skip-credits
    /// depending on where the playhead currently sits.
    fn handle_skip_current(&mut self, sender: &ComponentSender<Self>) {
        let Some(ref markers) = self.skip_markers else {
            return;
        };
        let pos_secs = self.position_us as f64 / 1_000_000.0;
        // Check credits first — if both ranges somehow overlap, prefer
        // skipping credits since the user is closer to the end.
        if markers.credits_start_secs > 0.0 && pos_secs >= markers.credits_start_secs {
            self.handle_skip_credits(sender);
        } else if markers.intro_end_secs > 0.0
            && pos_secs >= markers.intro_start_secs
            && pos_secs < markers.intro_end_secs
        {
            self.handle_skip_intro(sender);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_set_url(
        &mut self,
        widgets: &mut <Self as Component>::Widgets,
        sender: &ComponentSender<Self>,
        url: Option<String>,
        resume_secs: Option<f64>,
        base_offset_secs: f64,
        is_transcode: bool,
        local_path: Option<String>,
    ) {
        self.url = url.clone();
        self.duration_us = 0;
        self.position_us = 0;
        self.transcode_base_offset_us = (base_offset_secs * 1_000_000.0) as i64;
        self.is_transcode = is_transcode;
        self.playback.playing = false;
        self.playback.eos_reached = false;
        self.resume_pending = resume_secs.filter(|s| *s > 0.0);
        self.tracks.clear();
        self.track_ui_signature.clear();
        self.skip_markers = None;
        // Clear any in-flight buffering state from the previous media, else a
        // stale percent both shows a phantom "Buffering…" plate on the new
        // stream and suppresses its play/pause state edges in handle_tick —
        // permanently for a local file that emits no buffering messages.
        self.buffering_percent = None;

        self.media = None;

        let Some(url) = url else {
            widgets.picture.set_paintable(gtk::gdk::Paintable::NONE);
            // Clearing the stream (stop / leave) resets the quality menu so a
            // stale indicator doesn't linger into the next title.
            self.quality_available = false;
            self.current_selection = crate::models::playback::QualitySelection::Auto;
            self.decision_indicator.clear();
            let _ = sender.output(VideoPlayerOutput::StateChanged(PlayState::Stopped));
            return;
        };

        let sender_bus = sender.clone();
        let on_bus = Rc::new(move |msg: PipelineBusMsg| match msg {
            PipelineBusMsg::StreamCollection(c) => {
                sender_bus.input(VideoPlayerMsg::StreamCollection(c));
            }
            PipelineBusMsg::StreamsSelected {
                collection,
                streams,
            } => {
                sender_bus.input(VideoPlayerMsg::StreamsSelected {
                    collection,
                    streams,
                });
            }
            PipelineBusMsg::Buffering { percent } => {
                sender_bus.input(VideoPlayerMsg::Buffering(percent));
            }
        });

        let local_path_ref = local_path
            .as_deref()
            .or_else(|| url.strip_prefix("file://"));
        let Some(pipeline) = PlaybackPipeline::new(
            &url,
            self.playback.autoplay,
            self.preferred_subtitle_lang.clone(),
            local_path_ref,
            on_bus,
        ) else {
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

    /// Store buffering progress and refresh the status plate immediately so
    /// the indicator doesn't wait for the next 4 Hz tick. `percent >= 100`
    /// clears the buffering state (filled). The pause/resume side effects are
    /// handled pipeline-side (mode-aware) before this message is emitted.
    fn handle_buffering(&mut self, widgets: &mut <Self as Component>::Widgets, percent: i32) {
        let new_percent = if percent < 100 { Some(percent) } else { None };
        // downloadbuffer emits a buffering message roughly per percent (~100
        // per stream) and can repeat the same value; skip the redundant
        // allocation + widget writes when nothing the plate cares about moved.
        if new_percent == self.buffering_percent {
            return;
        }
        self.buffering_percent = new_percent;
        let (error_msg, is_prepared) = self
            .media
            .as_ref()
            .map(|m| (m.error_message(), m.is_prepared()))
            .unwrap_or((None, false));
        status_plate::render(
            widgets,
            error_msg.as_deref(),
            is_prepared,
            self.buffering_percent,
        );
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

        // A pipeline error means buffering can never reach 100% to clear
        // itself, so drop the buffering state — otherwise it would suppress
        // state edges for the rest of the session (e.g. a connection that dies
        // mid-fill). The error then surfaces on the plate (error outranks
        // buffering), and play/pause edges flow again.
        if snapshot.error_msg.is_some() {
            self.buffering_percent = None;
        }

        // A buffering-induced pause near the end of a queue2 stream looks
        // exactly like EOS (not playing, was playing, position within 500ms of
        // the end). Gate on buffering so a rebuffer doesn't fire a false EOF.
        let eos = self.buffering_percent.is_none()
            && !status.now_playing
            && was_playing
            && snapshot.duration_us > 0
            && snapshot.position_us >= snapshot.duration_us - 500_000;

        if eos && !self.playback.eos_reached {
            self.playback.eos_reached = true;
            self.playback.playing = false;
            let _ = sender.output(VideoPlayerOutput::EndOfFile);
            let _ = sender.output(VideoPlayerOutput::StateChanged(PlayState::Stopped));
            status_plate::render(
                widgets,
                snapshot.error_msg.as_deref(),
                status.is_prepared,
                self.buffering_percent,
            );
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
            // Report the content position (raw + transcode base offset, U8) so
            // watch-progress/scrobble and MPRIS see absolute content time.
            let _ = sender.output(VideoPlayerOutput::PositionChanged {
                position_secs: self.display_position_us() as f64 / 1_000_000.0,
                duration_secs: self.duration_us as f64 / 1_000_000.0,
            });
        }

        // Emit a state transition only on a genuine play/pause edge while not
        // buffering. While buffering is active the pipeline may dip into Paused
        // on a queue2 underrun; suppressing the poll-derived transition keeps
        // that automatic pause from leaking into `playback.playing` ownership
        // or flickering the window title / MPRIS state — the pipeline resumes
        // on its own once the buffer refills. (When there is no edge,
        // `playback.playing` already equals `now_playing`, so no update is
        // needed.)
        if self.buffering_percent.is_none() && was_playing != status.now_playing {
            self.playback.playing = status.now_playing;
            let state = if status.now_playing {
                PlayState::Playing
            } else {
                PlayState::Paused
            };
            let _ = sender.output(VideoPlayerOutput::StateChanged(state));
            flash_center(&widgets.center_indicator, status.now_playing);
        }

        self.apply_pending_resume(status.is_prepared);
        status_plate::render(
            widgets,
            snapshot.error_msg.as_deref(),
            status.is_prepared,
            self.buffering_percent,
        );
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
        // Decide from the user's intent, not the actual pipeline state: a
        // queue2 buffering stall may have auto-paused the pipeline while the
        // user still intends to play, and reading is_playing() there would
        // invert the toggle (a pause press would resume).
        let was_playing = self.media.as_ref().is_some_and(|m| m.wants_play());
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

    /// User picked a quality preset (U8): capture the current content position
    /// and ask the parent to re-resolve. The existing `!is_prepared` status
    /// plate provides the "switching" spinner once the reload starts (R13).
    fn handle_select_quality(
        &self,
        sender: &ComponentSender<Self>,
        selection: crate::models::playback::QualitySelection,
    ) {
        let position_secs = self.display_position_us() as f64 / 1_000_000.0;
        let _ = sender.output(VideoPlayerOutput::SelectQuality {
            selection,
            position_secs,
        });
    }

    /// During an active transcode, route a seek to an absolute content position
    /// through a reload at the new offset (KTD2) and return `true`; otherwise the
    /// caller seeks in-pipeline. `target_content_us` is absolute content time.
    fn maybe_reload_seek(&self, sender: &ComponentSender<Self>, target_content_us: i64) -> bool {
        if !self.is_transcode {
            return false;
        }
        let target = target_content_us.clamp(0, self.duration_us.max(0));
        let _ = sender.output(VideoPlayerOutput::SeekReload {
            position_secs: target as f64 / 1_000_000.0,
        });
        true
    }

    fn handle_seek_relative(&mut self, sender: &ComponentSender<Self>, secs: i64) {
        let Some(media) = self.media.as_ref() else {
            return;
        };
        if !media.is_prepared() {
            return;
        }
        let delta = secs.saturating_mul(1_000_000);
        // Transcode: reload at the new absolute content offset (KTD2).
        if self.maybe_reload_seek(sender, self.display_position_us().saturating_add(delta)) {
            sender.input(VideoPlayerMsg::PointerActive);
            return;
        }
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
        // Transcode: the slider spans absolute content time, so the target is
        // already content time — reload at it (KTD2).
        if self.maybe_reload_seek(sender, target) {
            sender.input(VideoPlayerMsg::PointerActive);
            return;
        }
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

    fn handle_user_seek(&mut self, sender: &ComponentSender<Self>, us: i64) {
        let Some(media) = self.media.as_ref() else {
            return;
        };
        if !media.is_prepared() {
            return;
        }
        // The slider spans absolute content time, so `us` is content time.
        // Transcode: reload at the new offset rather than seeking in-pipeline.
        if self.maybe_reload_seek(sender, us) {
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

fn wire_popover_handlers(widgets: &VideoPlayerWidgets, sender: &ComponentSender<VideoPlayer>) {
    for menu in [
        &widgets.audio_menu,
        &widgets.subtitle_menu,
        &widgets.quality_menu,
    ] {
        let Some(popover) = menu.popover() else {
            continue;
        };
        let sender_show = sender.clone();
        popover.connect_show(move |_| {
            sender_show.input(VideoPlayerMsg::PopoverVisibilityChanged(true));
        });
        let sender_hide = sender.clone();
        popover.connect_closed(move |_| {
            sender_hide.input(VideoPlayerMsg::PopoverVisibilityChanged(false));
        });
    }
}
