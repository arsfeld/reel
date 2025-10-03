use crate::models::QualityOption;
use crate::player::AdaptiveMode;
use gtk::prelude::*;
use relm4::gtk;
use relm4::prelude::*;

#[derive(Debug)]
pub struct QualitySelector {
    qualities: Vec<QualityOption>,
    selected_index: usize,
    adaptive_mode: AdaptiveMode,
    adaptive_active: bool,
    current_bandwidth_mbps: u64,
}

#[derive(Debug)]
pub enum QualitySelectorMsg {
    UpdateQualities(Vec<QualityOption>),
    SelectQuality(usize),
    ToggleAdaptiveMode(bool),
    UpdateAdaptiveStatus { active: bool, bandwidth_mbps: u64 },
}

#[derive(Debug)]
pub enum QualitySelectorOutput {
    QualityChanged(QualityOption),
    AdaptiveModeChanged(AdaptiveMode),
}

#[relm4::component(pub)]
impl SimpleComponent for QualitySelector {
    type Init = Vec<QualityOption>;
    type Input = QualitySelectorMsg;
    type Output = QualitySelectorOutput;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Horizontal,
            set_spacing: 8,
            set_margin_start: 8,
            set_margin_end: 8,

            // Adaptive mode toggle
            gtk::ToggleButton {
                set_label: "Auto",
                set_tooltip_text: Some("Enable adaptive quality based on network conditions"),
                #[watch]
                set_active: model.adaptive_mode == AdaptiveMode::Auto,

                connect_toggled[sender] => move |btn| {
                    sender.input(QualitySelectorMsg::ToggleAdaptiveMode(btn.is_active()));
                },
            },

            gtk::Label {
                set_label: "Quality:",
                set_halign: gtk::Align::Start,
            },

            gtk::DropDown {
                #[wrap(Some)]
                set_model = &gtk::StringList::new(
                    &model.qualities
                        .iter()
                        .map(|q| q.name.as_str())
                        .collect::<Vec<_>>()
                ),
                set_selected: model.selected_index as u32,
                #[watch]
                set_sensitive: model.adaptive_mode == AdaptiveMode::Manual,

                connect_selected_notify[sender] => move |dropdown| {
                    let selected = dropdown.selected() as usize;
                    sender.input(QualitySelectorMsg::SelectQuality(selected));
                },
            },

            gtk::Label {
                #[watch]
                set_label: &model.current_quality_info(),
                add_css_class: "caption",
                add_css_class: "dim-label",
            },

            // Adaptive quality indicator
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 4,
                #[watch]
                set_visible: model.adaptive_active,

                gtk::Image {
                    set_icon_name: Some("network-wireless-signal-good-symbolic"),
                    add_css_class: "accent",
                },

                gtk::Label {
                    #[watch]
                    set_label: &format!("{} Mbps", model.current_bandwidth_mbps),
                    add_css_class: "caption",
                    add_css_class: "dim-label",
                },
            },
        }
    }

    fn init(
        qualities: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = QualitySelector {
            qualities,
            selected_index: 0,
            adaptive_mode: AdaptiveMode::Auto,
            adaptive_active: false,
            current_bandwidth_mbps: 0,
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            QualitySelectorMsg::UpdateQualities(qualities) => {
                self.qualities = qualities;
                self.selected_index = 0;
            }
            QualitySelectorMsg::SelectQuality(index) => {
                if index < self.qualities.len() && index != self.selected_index {
                    self.selected_index = index;
                    let quality = self.qualities[index].clone();
                    let _ = sender.output(QualitySelectorOutput::QualityChanged(quality));
                }
            }
            QualitySelectorMsg::ToggleAdaptiveMode(is_auto) => {
                let new_mode = if is_auto {
                    AdaptiveMode::Auto
                } else {
                    AdaptiveMode::Manual
                };
                self.adaptive_mode = new_mode.clone();
                let _ = sender.output(QualitySelectorOutput::AdaptiveModeChanged(new_mode));
            }
            QualitySelectorMsg::UpdateAdaptiveStatus {
                active,
                bandwidth_mbps,
            } => {
                self.adaptive_active = active;
                self.current_bandwidth_mbps = bandwidth_mbps;
            }
        }
    }
}

impl QualitySelector {
    fn current_quality_info(&self) -> String {
        if let Some(quality) = self.qualities.get(self.selected_index) {
            let bitrate_mbps = quality.bitrate / 1_000_000;
            format!(
                "{}x{} @ {} Mbps",
                quality.resolution.width, quality.resolution.height, bitrate_mbps
            )
        } else {
            String::new()
        }
    }
}
