use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use relm4::prelude::*;

use crate::player::backend::{self, PlayState, TrackInfo, TrackType};

#[derive(Debug)]
#[allow(dead_code)]
pub enum ControlsInput {
    Position { position: f64, duration: f64 },
    PlayStateChanged(PlayState),
    Volume { volume: f64, muted: bool },
    Speed(f64),
    Tracks(Vec<TrackInfo>),
    ChapterCount(i64),
}

#[derive(Debug)]
pub enum ControlsOutput {
    TogglePause,
    SeekTo(f64),
    SetVolume(f64),
    ToggleMute,
    ToggleFullscreen,
    SetSpeed(f64),
    SelectAudioTrack(i64),
    SelectSubtitleTrack(i64),
    DisableSubtitles,
    LoadSubtitleFile,
    PrevChapter,
    NextChapter,
}

/// Shared widget references for updating outside the view macro.
struct ControlWidgets {
    progress_scale: gtk4::Scale,
    position_label: gtk4::Label,
    duration_label: gtk4::Label,
    play_pause_btn: gtk4::Button,
    volume_btn: gtk4::Button,
    volume_scale: gtk4::Scale,
    speed_btn: gtk4::MenuButton,
    track_btn: gtk4::MenuButton,
    chapter_prev_btn: gtk4::Button,
    chapter_next_btn: gtk4::Button,
}

pub struct PlayerControls {
    widgets: Rc<RefCell<Option<ControlWidgets>>>,
    output_sender: Option<relm4::Sender<ControlsOutput>>,
}

#[relm4::component(pub)]
impl SimpleComponent for PlayerControls {
    type Init = ();
    type Input = ControlsInput;
    type Output = ControlsOutput;

    view! {
        gtk4::Box {
            set_orientation: gtk4::Orientation::Vertical,
            set_valign: gtk4::Align::End,
            add_css_class: "player-controls",

            // Progress bar row
            #[name = "progress_scale"]
            gtk4::Scale {
                set_hexpand: true,
                set_range: (0.0, 1.0),
                set_draw_value: false,
                add_css_class: "progress-bar",
                set_margin_start: 8,
                set_margin_end: 8,
                connect_change_value[sender] => move |_scale, _scroll_type, value| {
                    let _ = sender.output(ControlsOutput::SeekTo(value));
                    gtk4::glib::Propagation::Stop
                },
            },

            // Controls row
            gtk4::Box {
                set_orientation: gtk4::Orientation::Horizontal,
                set_spacing: 4,
                set_margin_start: 8,
                set_margin_end: 8,
                set_margin_bottom: 8,
                set_margin_top: 4,

                // Play/pause button
                #[name = "play_pause_btn"]
                gtk4::Button {
                    set_icon_name: "media-playback-start-symbolic",
                    add_css_class: "flat",
                    add_css_class: "circular",
                    set_tooltip_text: Some("Play/Pause (Space)"),
                    connect_clicked[sender] => move |_| {
                        let _ = sender.output(ControlsOutput::TogglePause);
                    },
                },

                // Chapter previous
                #[name = "chapter_prev_btn"]
                gtk4::Button {
                    set_icon_name: "media-skip-backward-symbolic",
                    add_css_class: "flat",
                    add_css_class: "circular",
                    set_tooltip_text: Some("Previous Chapter"),
                    set_visible: false,
                    connect_clicked[sender] => move |_| {
                        let _ = sender.output(ControlsOutput::PrevChapter);
                    },
                },

                // Chapter next
                #[name = "chapter_next_btn"]
                gtk4::Button {
                    set_icon_name: "media-skip-forward-symbolic",
                    add_css_class: "flat",
                    add_css_class: "circular",
                    set_tooltip_text: Some("Next Chapter"),
                    set_visible: false,
                    connect_clicked[sender] => move |_| {
                        let _ = sender.output(ControlsOutput::NextChapter);
                    },
                },

                // Position label
                #[name = "position_label"]
                gtk4::Label {
                    set_label: "0:00",
                    add_css_class: "monospace",
                    add_css_class: "dim-label",
                    set_width_chars: 7,
                    set_xalign: 1.0,
                },

                gtk4::Label {
                    set_label: " / ",
                    add_css_class: "dim-label",
                },

                #[name = "duration_label"]
                gtk4::Label {
                    set_label: "0:00",
                    add_css_class: "monospace",
                    add_css_class: "dim-label",
                    set_width_chars: 7,
                    set_xalign: 0.0,
                },

                // Spacer
                gtk4::Box { set_hexpand: true },

                // Speed button with popover
                #[name = "speed_btn"]
                gtk4::MenuButton {
                    set_label: "1x",
                    add_css_class: "flat",
                    set_tooltip_text: Some("Playback Speed (] / [)"),
                },

                // Track selector button with popover
                #[name = "track_btn"]
                gtk4::MenuButton {
                    set_icon_name: "preferences-desktop-locale-symbolic",
                    add_css_class: "flat",
                    add_css_class: "circular",
                    set_tooltip_text: Some("Audio & Subtitles"),
                },

                // Volume button
                #[name = "volume_btn"]
                gtk4::Button {
                    set_icon_name: "audio-volume-high-symbolic",
                    add_css_class: "flat",
                    add_css_class: "circular",
                    set_tooltip_text: Some("Mute (M)"),
                    connect_clicked[sender] => move |_| {
                        let _ = sender.output(ControlsOutput::ToggleMute);
                    },
                },

                // Volume slider
                #[name = "volume_scale"]
                gtk4::Scale {
                    set_range: (0.0, 150.0),
                    set_value: 100.0,
                    set_draw_value: false,
                    set_width_request: 100,
                    add_css_class: "volume-slider",
                    connect_change_value[sender] => move |_scale, _scroll_type, value| {
                        let _ = sender.output(ControlsOutput::SetVolume(value));
                        gtk4::glib::Propagation::Stop
                    },
                },

                // Fullscreen button
                gtk4::Button {
                    set_icon_name: "view-fullscreen-symbolic",
                    add_css_class: "flat",
                    add_css_class: "circular",
                    set_tooltip_text: Some("Fullscreen (F11)"),
                    connect_clicked[sender] => move |_| {
                        let _ = sender.output(ControlsOutput::ToggleFullscreen);
                    },
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        _root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let _ = &sender; // used by view_output! macro
        let widgets = view_output!();

        // Build speed popover
        let speed_popover = build_speed_popover(sender.output_sender());
        widgets.speed_btn.set_popover(Some(&speed_popover));

        let control_widgets = ControlWidgets {
            progress_scale: widgets.progress_scale.clone(),
            position_label: widgets.position_label.clone(),
            duration_label: widgets.duration_label.clone(),
            play_pause_btn: widgets.play_pause_btn.clone(),
            volume_btn: widgets.volume_btn.clone(),
            volume_scale: widgets.volume_scale.clone(),
            speed_btn: widgets.speed_btn.clone(),
            track_btn: widgets.track_btn.clone(),
            chapter_prev_btn: widgets.chapter_prev_btn.clone(),
            chapter_next_btn: widgets.chapter_next_btn.clone(),
        };

        let model = Self {
            widgets: Rc::new(RefCell::new(Some(control_widgets))),
            output_sender: Some(sender.output_sender().clone()),
        };

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        let w = self.widgets.borrow();
        let Some(w) = w.as_ref() else { return };

        match msg {
            ControlsInput::Position { position, duration } => {
                if duration > 0.0 {
                    w.progress_scale
                        .set_value(backend::progress_fraction(position, duration));
                }
                w.position_label
                    .set_label(&backend::format_position(position));
                w.duration_label
                    .set_label(&backend::format_position(duration));
            }
            ControlsInput::PlayStateChanged(state) => {
                let icon = if state == PlayState::Playing {
                    "media-playback-pause-symbolic"
                } else {
                    "media-playback-start-symbolic"
                };
                w.play_pause_btn.set_icon_name(icon);
            }
            ControlsInput::Volume { volume, muted } => {
                let vol_icon = if muted || volume < 1.0 {
                    "audio-volume-muted-symbolic"
                } else if volume < 34.0 {
                    "audio-volume-low-symbolic"
                } else if volume < 67.0 {
                    "audio-volume-medium-symbolic"
                } else {
                    "audio-volume-high-symbolic"
                };
                w.volume_btn.set_icon_name(vol_icon);
                w.volume_scale.set_value(volume);
            }
            ControlsInput::Speed(speed) => {
                w.speed_btn.set_label(&backend::format_speed_label(speed));
            }
            ControlsInput::Tracks(tracks) => {
                if let Some(ref output_sender) = self.output_sender {
                    let popover = build_track_popover(&tracks, output_sender);
                    w.track_btn.set_popover(Some(&popover));
                }
            }
            ControlsInput::ChapterCount(count) => {
                let has_chapters = count > 0;
                w.chapter_prev_btn.set_visible(has_chapters);
                w.chapter_next_btn.set_visible(has_chapters);
            }
        }
    }
}

/// Build the speed preset popover.
fn build_speed_popover(sender: &relm4::Sender<ControlsOutput>) -> gtk4::Popover {
    let popover = gtk4::Popover::new();
    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    vbox.set_margin_top(4);
    vbox.set_margin_bottom(4);
    vbox.set_margin_start(4);
    vbox.set_margin_end(4);

    let presets = [0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 3.0, 4.0];

    for speed in presets {
        let btn = gtk4::Button::with_label(&backend::format_speed_label(speed));
        btn.add_css_class("flat");
        let sender = sender.clone();
        let popover_ref = popover.downgrade();
        btn.connect_clicked(move |_| {
            let _ = sender.send(ControlsOutput::SetSpeed(speed));
            if let Some(p) = popover_ref.upgrade() {
                p.popdown();
            }
        });
        vbox.append(&btn);
    }

    popover.set_child(Some(&vbox));
    popover
}

/// Build the track selector popover from current track list.
fn build_track_popover(
    tracks: &[TrackInfo],
    sender: &relm4::Sender<ControlsOutput>,
) -> gtk4::Popover {
    let popover = gtk4::Popover::new();
    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    vbox.set_margin_top(8);
    vbox.set_margin_bottom(8);
    vbox.set_margin_start(8);
    vbox.set_margin_end(8);

    let audio_tracks: Vec<_> = tracks
        .iter()
        .filter(|t| t.track_type == TrackType::Audio)
        .collect();
    let sub_tracks: Vec<_> = tracks
        .iter()
        .filter(|t| t.track_type == TrackType::Subtitle)
        .collect();

    // Audio section
    if !audio_tracks.is_empty() {
        let header = gtk4::Label::new(Some("Audio"));
        header.add_css_class("heading");
        header.set_xalign(0.0);
        vbox.append(&header);

        for track in &audio_tracks {
            let label = backend::format_track_label(track);
            let btn = gtk4::CheckButton::with_label(&label);
            btn.set_active(track.selected);
            let sender = sender.clone();
            let track_id = track.id;
            let popover_ref = popover.downgrade();
            btn.connect_toggled(move |cb| {
                if cb.is_active() {
                    let _ = sender.send(ControlsOutput::SelectAudioTrack(track_id));
                    if let Some(p) = popover_ref.upgrade() {
                        p.popdown();
                    }
                }
            });
            vbox.append(&btn);
        }
    }

    // Separator
    if !audio_tracks.is_empty() && !sub_tracks.is_empty() {
        vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
    }

    // Subtitle section
    if !sub_tracks.is_empty() || !audio_tracks.is_empty() {
        let header = gtk4::Label::new(Some("Subtitles"));
        header.add_css_class("heading");
        header.set_xalign(0.0);
        vbox.append(&header);

        // "None" option
        let none_btn = gtk4::CheckButton::with_label("None");
        let any_sub_selected = sub_tracks.iter().any(|t| t.selected);
        none_btn.set_active(!any_sub_selected);
        let sender_none = sender.clone();
        let popover_ref = popover.downgrade();
        none_btn.connect_toggled(move |cb| {
            if cb.is_active() {
                let _ = sender_none.send(ControlsOutput::DisableSubtitles);
                if let Some(p) = popover_ref.upgrade() {
                    p.popdown();
                }
            }
        });
        vbox.append(&none_btn);

        for track in &sub_tracks {
            let label = backend::format_track_label(track);
            let btn = gtk4::CheckButton::with_label(&label);
            btn.set_active(track.selected);
            let sender = sender.clone();
            let track_id = track.id;
            let popover_ref = popover.downgrade();
            btn.connect_toggled(move |cb| {
                if cb.is_active() {
                    let _ = sender.send(ControlsOutput::SelectSubtitleTrack(track_id));
                    if let Some(p) = popover_ref.upgrade() {
                        p.popdown();
                    }
                }
            });
            vbox.append(&btn);
        }
    }

    // Load subtitle file button
    vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
    let load_btn = gtk4::Button::with_label("Load Subtitle File\u{2026}");
    load_btn.add_css_class("flat");
    let sender_load = sender.clone();
    let popover_ref = popover.downgrade();
    load_btn.connect_clicked(move |_| {
        let _ = sender_load.send(ControlsOutput::LoadSubtitleFile);
        if let Some(p) = popover_ref.upgrade() {
            p.popdown();
        }
    });
    vbox.append(&load_btn);

    popover.set_child(Some(&vbox));
    popover
}
