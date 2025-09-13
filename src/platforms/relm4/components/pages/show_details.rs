use crate::models::{Episode, MediaItem, MediaItemId, Season, Show};
use crate::services::commands::Command;
use crate::services::commands::media_commands::{GetEpisodesCommand, GetItemDetailsCommand};
use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;
use relm4::RelmWidgetExt;
use relm4::gtk;
use relm4::prelude::*;
use std::sync::Arc;

#[derive(Debug)]
pub struct ShowDetailsPage {
    show: Option<Show>,
    episodes: Vec<Episode>,
    current_season: u32,
    item_id: MediaItemId,
    db: Arc<crate::db::connection::DatabaseConnection>,
    loading: bool,
    episode_grid: gtk::FlowBox,
    season_dropdown: gtk::DropDown,
}

#[derive(Debug)]
pub enum ShowDetailsInput {
    LoadShow(MediaItemId),
    SelectSeason(u32),
    PlayEpisode(MediaItemId),
    ToggleEpisodeWatched(usize),
    LoadEpisodes,
}

#[derive(Debug)]
pub enum ShowDetailsOutput {
    PlayMedia(MediaItemId),
    NavigateBack,
}

#[derive(Debug)]
pub enum ShowDetailsCommand {
    LoadDetails,
    LoadEpisodes(String, u32),
}

#[relm4::component(pub, async)]
impl AsyncComponent for ShowDetailsPage {
    type Init = (MediaItemId, Arc<crate::db::connection::DatabaseConnection>);
    type Input = ShowDetailsInput;
    type Output = ShowDetailsOutput;
    type CommandOutput = ShowDetailsCommand;

    view! {
        #[root]
        gtk::ScrolledWindow {
            set_hscrollbar_policy: gtk::PolicyType::Never,
            #[watch]
            set_visible: !model.loading,

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,

                // Hero Section with backdrop
                gtk::Overlay {
                    set_height_request: 400,

                    // Backdrop image
                    gtk::Picture {
                        set_content_fit: gtk::ContentFit::Cover,
                        #[watch]
                        set_paintable: model.show.as_ref()
                            .and_then(|s| s.backdrop_url.as_ref())
                            .and_then(|url| {
                                gtk::gdk_pixbuf::Pixbuf::from_file_at_size(url, -1, 400)
                                    .ok()
                                    .map(|pb| gtk::gdk::Texture::for_pixbuf(&pb))
                            })
                            .as_ref(),
                    },

                    // Gradient overlay
                    add_overlay = &gtk::Box {
                        add_css_class: "osd",
                        set_valign: gtk::Align::End,

                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_margin_all: 24,
                            set_spacing: 24,

                            // Poster
                            gtk::Picture {
                                set_width_request: 200,
                                set_height_request: 300,
                                add_css_class: "card",
                                #[watch]
                                set_paintable: model.show.as_ref()
                                    .and_then(|s| s.poster_url.as_ref())
                                    .and_then(|url| {
                                        gtk::gdk_pixbuf::Pixbuf::from_file_at_size(url, 200, 300)
                                            .ok()
                                            .map(|pb| gtk::gdk::Texture::for_pixbuf(&pb))
                                    })
                                    .as_ref(),
                            },

                            // Show info
                            gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_valign: gtk::Align::End,
                                set_spacing: 12,
                                set_hexpand: true,

                                // Title
                                gtk::Label {
                                    set_halign: gtk::Align::Start,
                                    add_css_class: "title-1",
                                    set_wrap: true,
                                    #[watch]
                                    set_label: &model.show.as_ref().map(|s| s.title.clone()).unwrap_or_default(),
                                },

                                // Metadata row
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 12,

                                    // Year
                                    gtk::Label {
                                        add_css_class: "dim-label",
                                        #[watch]
                                        set_visible: model.show.as_ref().and_then(|s| s.year).is_some(),
                                        #[watch]
                                        set_label: &model.show.as_ref()
                                            .and_then(|s| s.year.map(|y| y.to_string()))
                                            .unwrap_or_default(),
                                    },

                                    // Rating
                                    gtk::Box {
                                        set_spacing: 6,
                                        #[watch]
                                        set_visible: model.show.as_ref().and_then(|s| s.rating).is_some(),

                                        gtk::Image {
                                            set_icon_name: Some("starred-symbolic"),
                                        },

                                        gtk::Label {
                                            #[watch]
                                            set_label: &model.show.as_ref()
                                                .and_then(|s| s.rating.map(|r| format!("{:.1}", r)))
                                                .unwrap_or_default(),
                                        }
                                    },

                                    // Episode count
                                    gtk::Label {
                                        add_css_class: "dim-label",
                                        #[watch]
                                        set_label: &model.show.as_ref()
                                            .map(|s| format!("{} episodes", s.total_episode_count))
                                            .unwrap_or_default(),
                                    },
                                },

                                // Progress
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_spacing: 6,
                                    #[watch]
                                    set_visible: model.show.as_ref()
                                        .map(|s| s.watched_episode_count > 0)
                                        .unwrap_or(false),

                                    gtk::Label {
                                        set_halign: gtk::Align::Start,
                                        add_css_class: "dim-label",
                                        #[watch]
                                        set_label: &model.show.as_ref()
                                            .map(|s| format!("{}/{} watched",
                                                s.watched_episode_count,
                                                s.total_episode_count))
                                            .unwrap_or_default(),
                                    },

                                    gtk::ProgressBar {
                                        set_show_text: false,
                                        #[watch]
                                        set_fraction: model.show.as_ref()
                                            .map(|s| {
                                                if s.total_episode_count > 0 {
                                                    s.watched_episode_count as f64 / s.total_episode_count as f64
                                                } else {
                                                    0.0
                                                }
                                            })
                                            .unwrap_or(0.0),
                                    },
                                },

                                // Season selector
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 12,

                                    gtk::Label {
                                        set_label: "Season:",
                                        add_css_class: "body",
                                    },

                                    append: &model.season_dropdown,
                                },
                            },
                        },
                    },
                },

                // Content section
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_margin_all: 24,
                    set_spacing: 24,

                    // Overview
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 12,
                        #[watch]
                        set_visible: model.show.as_ref().and_then(|s| s.overview.as_ref()).is_some(),

                        gtk::Label {
                            set_label: "Overview",
                            set_halign: gtk::Align::Start,
                            add_css_class: "title-4",
                        },

                        gtk::Label {
                            set_halign: gtk::Align::Start,
                            set_wrap: true,
                            set_selectable: true,
                            #[watch]
                            set_label: &model.show.as_ref()
                                .and_then(|s| s.overview.clone())
                                .unwrap_or_default(),
                        },
                    },

                    // Episodes
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 12,

                        gtk::Label {
                            set_label: "Episodes",
                            set_halign: gtk::Align::Start,
                            add_css_class: "title-4",
                        },

                        gtk::ScrolledWindow {
                            set_vscrollbar_policy: gtk::PolicyType::Automatic,
                            set_min_content_height: 300,

                            set_child: Some(&model.episode_grid),
                        },
                    },
                },
            },
        }
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let episode_grid = gtk::FlowBox::builder()
            .orientation(gtk::Orientation::Horizontal)
            .column_spacing(12)
            .row_spacing(12)
            .homogeneous(true)
            .selection_mode(gtk::SelectionMode::None)
            .build();

        let season_dropdown = gtk::DropDown::builder().enable_search(false).build();

        {
            let sender = sender.clone();
            season_dropdown.connect_selected_notify(move |dropdown| {
                let selected = dropdown.selected();
                sender.input(ShowDetailsInput::SelectSeason(selected as u32 + 1));
            });
        }

        let model = Self {
            show: None,
            episodes: Vec::new(),
            current_season: 1,
            item_id: init.0.clone(),
            db: init.1,
            loading: true,
            episode_grid,
            season_dropdown,
        };

        let widgets = view_output!();

        sender.oneshot_command(async { ShowDetailsCommand::LoadDetails });

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        msg: Self::Input,
        sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            ShowDetailsInput::LoadShow(item_id) => {
                self.item_id = item_id;
                self.show = None;
                self.episodes.clear();
                self.loading = true;
                sender.oneshot_command(async { ShowDetailsCommand::LoadDetails });
            }
            ShowDetailsInput::SelectSeason(season_num) => {
                self.current_season = season_num;
                if let Some(show) = &self.show {
                    let show_id = show.id.clone();
                    sender.oneshot_command(async move {
                        ShowDetailsCommand::LoadEpisodes(show_id, season_num)
                    });
                }
            }
            ShowDetailsInput::PlayEpisode(episode_id) => {
                sender
                    .output(ShowDetailsOutput::PlayMedia(episode_id))
                    .unwrap();
            }
            ShowDetailsInput::ToggleEpisodeWatched(index) => {
                if let Some(episode) = self.episodes.get_mut(index) {
                    episode.watched = !episode.watched;
                    // TODO: Update database
                    self.update_episode_grid();
                }
            }
            ShowDetailsInput::LoadEpisodes => {
                if let Some(show) = &self.show {
                    let show_id = show.id.clone();
                    let season = self.current_season;
                    sender.oneshot_command(async move {
                        ShowDetailsCommand::LoadEpisodes(show_id, season)
                    });
                }
            }
        }
    }

    async fn update_cmd(
        &mut self,
        msg: Self::CommandOutput,
        sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            ShowDetailsCommand::LoadDetails => {
                let cmd = GetItemDetailsCommand {
                    db: (*self.db).clone(),
                    item_id: self.item_id.clone(),
                };

                match Command::execute(&cmd).await {
                    Ok(item) => {
                        if let MediaItem::Show(show) = item {
                            self.show = Some(show.clone());
                            self.loading = false;

                            // Update season dropdown
                            let seasons: Vec<String> = show
                                .seasons
                                .iter()
                                .map(|s| format!("Season {}", s.season_number))
                                .collect();

                            let model = gtk::StringList::new(
                                &seasons.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                            );
                            self.season_dropdown.set_model(Some(&model));
                            self.season_dropdown.set_selected(0);

                            // Load episodes for first season
                            if !show.seasons.is_empty() {
                                sender.input(ShowDetailsInput::LoadEpisodes);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to load show details: {}", e);
                        self.loading = false;
                    }
                }
            }
            ShowDetailsCommand::LoadEpisodes(show_id, season_num) => {
                let cmd = GetEpisodesCommand {
                    db: (*self.db).clone(),
                    show_id: crate::models::ShowId::new(show_id),
                    season_number: Some(season_num),
                };

                match Command::execute(&cmd).await {
                    Ok(episodes) => {
                        self.episodes = episodes;
                        self.update_episode_grid();
                    }
                    Err(e) => {
                        tracing::error!("Failed to load episodes: {}", e);
                    }
                }
            }
        }
    }
}

impl ShowDetailsPage {
    fn update_episode_grid(&self) {
        // Clear existing children
        while let Some(child) = self.episode_grid.first_child() {
            self.episode_grid.remove(&child);
        }

        // Add episode cards
        for (index, episode) in self.episodes.iter().enumerate() {
            let card = create_episode_card(episode, index);
            self.episode_grid.append(&card);
        }
    }
}

fn create_episode_card(episode: &Episode, index: usize) -> gtk::Box {
    let card = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .width_request(200)
        .css_classes(["card"])
        .build();

    // Episode thumbnail with number overlay
    let overlay = gtk::Overlay::new();

    let picture = gtk::Picture::builder()
        .width_request(200)
        .height_request(112)
        .content_fit(gtk::ContentFit::Cover)
        .build();

    if let Some(thumbnail_url) = &episode.thumbnail_url {
        if let Ok(pixbuf) = gtk::gdk_pixbuf::Pixbuf::from_file_at_size(thumbnail_url, 200, 112) {
            let texture = gtk::gdk::Texture::for_pixbuf(&pixbuf);
            picture.set_paintable(Some(&texture));
        }
    }

    overlay.set_child(Some(&picture));

    // Episode number badge
    let badge = gtk::Label::builder()
        .label(&format!("E{}", episode.episode_number))
        .css_classes(["osd", "pill"])
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Start)
        .margin_top(6)
        .margin_start(6)
        .build();
    overlay.add_overlay(&badge);

    // Progress bar if partially watched
    if let Some(position) = episode.playback_position {
        if position.as_secs() > 0 && !episode.watched {
            let progress = gtk::ProgressBar::builder()
                .valign(gtk::Align::End)
                .css_classes(["osd"])
                .fraction(position.as_secs_f64() / episode.duration.as_secs_f64())
                .build();
            overlay.add_overlay(&progress);
        }
    }

    // Watched indicator
    if episode.watched {
        let check = gtk::Image::builder()
            .icon_name("object-select-symbolic")
            .css_classes(["osd"])
            .halign(gtk::Align::End)
            .valign(gtk::Align::Start)
            .margin_top(6)
            .margin_end(6)
            .build();
        overlay.add_overlay(&check);
    }

    // Episode info
    let info_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(3)
        .margin_top(6)
        .margin_bottom(6)
        .margin_start(6)
        .margin_end(6)
        .build();

    let title = gtk::Label::builder()
        .label(&episode.title)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .xalign(0.0)
        .css_classes(["caption-heading"])
        .build();

    let duration = episode.duration.as_secs() / 60;
    let details = gtk::Label::builder()
        .label(&format!("{}m", duration))
        .xalign(0.0)
        .css_classes(["dim-label", "caption"])
        .build();

    info_box.append(&title);
    info_box.append(&details);

    card.append(&overlay);
    card.append(&info_box);

    card
}
