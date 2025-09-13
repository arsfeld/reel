use crate::db::entities::MediaItemModel;
use crate::models::MediaItemId;
use gtk::prelude::*;
use relm4::factory::FactoryComponent;
use relm4::prelude::*;

#[derive(Debug, Clone)]
pub struct MediaCardInit {
    pub item: MediaItemModel,
    pub show_progress: bool,
}

#[tracker::track]
#[derive(Debug)]
pub struct MediaCard {
    item: MediaItemModel,
    #[do_not_track]
    item_id: MediaItemId,
    show_progress: bool,
    hover: bool,
    selected: bool,
    progress_percent: f64,
    image_loaded: bool,
}

#[derive(Debug, Clone)]
pub enum MediaCardInput {
    SetHover(bool),
    SetSelected(bool),
    UpdateProgress(f64),
    ImageLoaded,
    Play,
}

#[derive(Debug, Clone)]
pub enum MediaCardOutput {
    Clicked(MediaItemId),
    Play(MediaItemId),
}

#[relm4::factory(pub)]
impl FactoryComponent for MediaCard {
    type Init = MediaCardInit;
    type Input = MediaCardInput;
    type Output = MediaCardOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::FlowBox;

    view! {
        root = gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 0,
            add_css_class: "card",
            set_width_request: 200,
            set_height_request: 340,

            gtk::Overlay {
                #[name(poster)]
                gtk::Picture {
                    set_content_fit: gtk::ContentFit::Cover,
                    set_height_request: 300,
                    add_css_class: "card-poster",
                },

                add_overlay = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_valign: gtk::Align::End,
                    set_visible: self.show_progress && self.progress_percent > 0.0,

                    gtk::ProgressBar {
                        set_fraction: self.progress_percent,
                        add_css_class: "osd",
                        set_margin_all: 6,
                    }
                },

                add_overlay = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_halign: gtk::Align::Center,
                    set_valign: gtk::Align::Center,
                    #[track(self.changed(MediaCard::hover()))]
                    set_visible: self.hover,

                    gtk::Button {
                        add_css_class: "circular",
                        add_css_class: "suggested-action",
                        add_css_class: "osd",
                        set_icon_name: "media-playback-start-symbolic",
                        set_width_request: 48,
                        set_height_request: 48,

                        connect_clicked[sender, item_id = self.item_id.clone()] => move |_| {
                            sender.output(MediaCardOutput::Play(item_id.clone())).unwrap();
                        }
                    }
                }
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 3,
                set_margin_all: 12,

                gtk::Label {
                    set_label: &self.item.title,
                    set_ellipsize: gtk::pango::EllipsizeMode::End,
                    set_max_width_chars: 20,
                    set_xalign: 0.0,
                    add_css_class: "heading",
                },

                gtk::Label {
                    set_label: &self.format_subtitle(),
                    set_ellipsize: gtk::pango::EllipsizeMode::End,
                    set_max_width_chars: 20,
                    set_xalign: 0.0,
                    add_css_class: "dim-label",
                    add_css_class: "caption",
                    #[track(self.changed(MediaCard::item()))]
                    set_visible: !self.format_subtitle().is_empty(),
                }
            },

            add_controller = gtk::EventControllerMotion {
                connect_enter[sender] => move |_, _, _| {
                    sender.input(MediaCardInput::SetHover(true));
                },
                connect_leave[sender] => move |_| {
                    sender.input(MediaCardInput::SetHover(false));
                }
            },

            add_controller = gtk::GestureClick {
                connect_released[sender, item_id = self.item_id.clone()] => move |_, _, _, _| {
                    sender.output(MediaCardOutput::Clicked(item_id.clone())).unwrap();
                }
            }
        }
    }

    fn init_model(init: Self::Init, _index: &DynamicIndex, sender: FactorySender<Self>) -> Self {
        let item_id = MediaItemId::new(init.item.id.clone());

        // Start loading the image
        if let Some(poster_url) = &init.item.poster_url {
            let sender = sender.clone();
            let url = poster_url.clone();
            relm4::spawn_local(async move {
                // TODO: Integrate with ImageWorker when available
                // For now, just signal that image is loaded
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                sender.input(MediaCardInput::ImageLoaded);
            });
        }

        Self {
            item: init.item,
            item_id,
            show_progress: init.show_progress,
            hover: false,
            selected: false,
            progress_percent: 0.0,
            image_loaded: false,
            tracker: 0,
        }
    }

    fn update(&mut self, msg: Self::Input, sender: FactorySender<Self>) {
        self.reset();

        match msg {
            MediaCardInput::SetHover(hover) => {
                self.set_hover(hover);
            }
            MediaCardInput::SetSelected(selected) => {
                self.set_selected(selected);
            }
            MediaCardInput::UpdateProgress(progress) => {
                self.set_progress_percent(progress);
            }
            MediaCardInput::ImageLoaded => {
                self.set_image_loaded(true);
            }
            MediaCardInput::Play => {
                sender
                    .output(MediaCardOutput::Play(self.item_id.clone()))
                    .unwrap();
            }
        }
    }
}

impl MediaCard {
    fn format_subtitle(&self) -> String {
        match self.item.media_type.as_str() {
            "movie" => {
                if let Some(year) = self.item.year {
                    format!("{} • Movie", year)
                } else {
                    "Movie".to_string()
                }
            }
            "show" => {
                format!("TV Show")
            }
            "episode" => {
                if let (Some(season), Some(episode)) =
                    (self.item.season_number, self.item.episode_number)
                {
                    format!("S{:02}E{:02}", season, episode)
                } else {
                    "Episode".to_string()
                }
            }
            _ => String::new(),
        }
    }
}
