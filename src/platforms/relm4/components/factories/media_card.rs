use crate::db::entities::MediaItemModel;
use crate::models::MediaItemId;
use gtk::prelude::*;
use relm4::factory::FactoryComponent;
use relm4::prelude::*;

#[derive(Debug, Clone)]
pub struct MediaCardInit {
    pub item: MediaItemModel,
    pub show_progress: bool,
    pub watched: bool,
    pub progress_percent: f64,
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
    watched: bool,
    #[do_not_track]
    texture: Option<gtk::gdk::Texture>,
}

#[derive(Debug, Clone)]
pub enum MediaCardInput {
    SetHover(bool),
    SetSelected(bool),
    UpdateProgress(f64),
    ImageLoaded(gtk::gdk::Texture),
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
        // Root button matching GTK MediaCard button style
        root = gtk::Button {
            add_css_class: "flat",
            add_css_class: "media-card",
            add_css_class: "poster-card",
            set_width_request: 130,
            set_height_request: 195,

            // Main overlay container
            gtk::Overlay {
                add_css_class: "poster-overlay",

                // Poster image
                #[name(poster)]
                gtk::Picture {
                    set_content_fit: gtk::ContentFit::Cover,
                    set_width_request: 130,
                    set_height_request: 195,
                    add_css_class: "rounded-poster",
                },

                // Loading spinner overlay
                add_overlay = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_halign: gtk::Align::Center,
                    set_valign: gtk::Align::Center,
                    #[track(self.changed(MediaCard::image_loaded()))]
                    set_visible: !self.image_loaded,

                    gtk::Spinner {
                        set_spinning: true,
                        set_width_request: 24,
                        set_height_request: 24,
                    }
                },

                // Info gradient at bottom
                add_overlay = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_valign: gtk::Align::End,
                    add_css_class: "poster-info-gradient",

                    // Inner box with text
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 2,
                        set_margin_bottom: 8,
                        set_margin_start: 8,
                        set_margin_end: 8,
                        set_margin_top: 8,
                        add_css_class: "media-card-info",

                        gtk::Label {
                            set_label: &self.item.title,
                            set_xalign: 0.0,
                            set_single_line_mode: true,
                            set_ellipsize: gtk::pango::EllipsizeMode::End,
                            add_css_class: "title-4",
                        },

                        gtk::Label {
                            set_label: &self.format_subtitle(),
                            set_xalign: 0.0,
                            set_single_line_mode: true,
                            set_ellipsize: gtk::pango::EllipsizeMode::End,
                            add_css_class: "subtitle",
                            #[track(self.changed(MediaCard::item()))]
                            set_visible: !self.format_subtitle().is_empty(),
                        }
                    }
                },

                // Unwatched indicator (top-right glowing dot)
                add_overlay = &gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_halign: gtk::Align::End,
                    set_valign: gtk::Align::Start,
                    set_margin_top: 8,
                    set_margin_end: 8,
                    #[track(self.changed(MediaCard::item()))]
                    set_visible: !self.is_watched(),
                    add_css_class: "unwatched-indicator",

                    gtk::Box {
                        set_width_request: 14,
                        set_height_request: 14,
                        add_css_class: "unwatched-glow-dot",
                    }
                },

                // Progress bar overlay (bottom)
                add_overlay = &gtk::ProgressBar {
                    set_valign: gtk::Align::End,
                    #[track(self.changed(MediaCard::progress_percent()))]
                    set_visible: self.is_partially_watched(),
                    #[track(self.changed(MediaCard::progress_percent()))]
                    set_fraction: self.progress_percent,
                    add_css_class: "media-progress",
                }
            },

            connect_clicked[sender, item_id = self.item_id.clone()] => move |_| {
                sender.output(MediaCardOutput::Clicked(item_id.clone())).unwrap();
            }
        }
    }

    fn init_model(init: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        let item_id = MediaItemId::new(init.item.id.clone());

        Self {
            item: init.item,
            item_id,
            show_progress: init.show_progress,
            hover: false,
            selected: false,
            progress_percent: init.progress_percent,
            image_loaded: false,
            watched: init.watched,
            texture: None,
            tracker: 0,
        }
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        msg: Self::Input,
        sender: FactorySender<Self>,
    ) {
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
            MediaCardInput::ImageLoaded(texture) => {
                // Set the texture on the picture widget
                widgets.poster.set_paintable(Some(&texture));
                self.texture = Some(texture);
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
                    format!("{}", year)
                } else {
                    String::new()
                }
            }
            "show" => {
                // For shows, we'll extract episode count from metadata if available
                // Otherwise default to "TV Series"
                if let Some(metadata) = &self.item.metadata {
                    // Try to extract episode count from metadata JSON
                    if let Some(episode_count) =
                        metadata.get("total_episode_count").and_then(|v| v.as_u64())
                    {
                        if episode_count == 1 {
                            "1 episode".to_string()
                        } else {
                            format!("{} episodes", episode_count)
                        }
                    } else if let Some(season_count) =
                        metadata.get("season_count").and_then(|v| v.as_u64())
                    {
                        if season_count == 1 {
                            "1 season".to_string()
                        } else {
                            format!("{} seasons", season_count)
                        }
                    } else {
                        "TV Series".to_string()
                    }
                } else {
                    "TV Series".to_string()
                }
            }
            "episode" => {
                // For episodes, format like GTK version
                if let (Some(season), Some(episode)) =
                    (self.item.season_number, self.item.episode_number)
                {
                    format!("S{}E{}", season, episode)
                } else {
                    "Episode".to_string()
                }
            }
            _ => String::new(),
        }
    }

    fn is_watched(&self) -> bool {
        // This will be determined by playback progress from the database
        // For now, use the watched field we track
        self.watched
    }

    fn is_partially_watched(&self) -> bool {
        // Check if progress is between 0 and 90%
        self.progress_percent > 0.0 && self.progress_percent < 0.9
    }
}
