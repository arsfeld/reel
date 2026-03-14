use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use relm4::prelude::*;

use crate::player::backend::{self, PlayState};

#[derive(Debug)]
pub enum ControlsInput {
    Position { position: f64, duration: f64 },
    PlayStateChanged(PlayState),
    Volume { volume: f64, muted: bool },
}

#[derive(Debug)]
pub enum ControlsOutput {
    TogglePause,
    SeekTo(f64),
    SetVolume(f64),
    ToggleMute,
    ToggleFullscreen,
}

/// Shared widget references for updating outside the view macro.
struct ControlWidgets {
    progress_scale: gtk4::Scale,
    position_label: gtk4::Label,
    duration_label: gtk4::Label,
    play_pause_btn: gtk4::Button,
    volume_btn: gtk4::Button,
    volume_scale: gtk4::Scale,
}

pub struct PlayerControls {
    widgets: Rc<RefCell<Option<ControlWidgets>>>,
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

        let control_widgets = ControlWidgets {
            progress_scale: widgets.progress_scale.clone(),
            position_label: widgets.position_label.clone(),
            duration_label: widgets.duration_label.clone(),
            play_pause_btn: widgets.play_pause_btn.clone(),
            volume_btn: widgets.volume_btn.clone(),
            volume_scale: widgets.volume_scale.clone(),
        };

        let model = Self {
            widgets: Rc::new(RefCell::new(Some(control_widgets))),
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
        }
    }
}
