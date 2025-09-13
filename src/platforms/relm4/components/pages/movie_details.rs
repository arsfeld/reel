use crate::models::{MediaItem, MediaItemId, Movie, Person};
use crate::services::commands::Command;
use crate::services::commands::media_commands::GetItemDetailsCommand;
use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;
use relm4::RelmWidgetExt;
use relm4::gtk;
use relm4::prelude::*;
use std::sync::Arc;

#[derive(Debug)]
pub struct MovieDetailsPage {
    movie: Option<Movie>,
    item_id: MediaItemId,
    db: Arc<crate::db::connection::DatabaseConnection>,
    loading: bool,
    genre_box: gtk::Box,
    cast_box: gtk::Box,
}

#[derive(Debug)]
pub enum MovieDetailsInput {
    LoadMovie(MediaItemId),
    PlayMovie,
    ToggleWatched,
}

#[derive(Debug)]
pub enum MovieDetailsOutput {
    PlayMedia(MediaItemId),
    NavigateBack,
}

#[derive(Debug)]
pub enum MovieDetailsCommand {
    LoadDetails,
}

#[relm4::component(pub, async)]
impl AsyncComponent for MovieDetailsPage {
    type Init = (MediaItemId, Arc<crate::db::connection::DatabaseConnection>);
    type Input = MovieDetailsInput;
    type Output = MovieDetailsOutput;
    type CommandOutput = MovieDetailsCommand;

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
                        set_paintable: model.movie.as_ref()
                            .and_then(|m| m.backdrop_url.as_ref())
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
                                set_paintable: model.movie.as_ref()
                                    .and_then(|m| m.poster_url.as_ref())
                                    .and_then(|url| {
                                        gtk::gdk_pixbuf::Pixbuf::from_file_at_size(url, 200, 300)
                                            .ok()
                                            .map(|pb| gtk::gdk::Texture::for_pixbuf(&pb))
                                    })
                                    .as_ref(),
                            },

                            // Movie info
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
                                    set_label: &model.movie.as_ref().map(|m| m.title.clone()).unwrap_or_default(),
                                },

                                // Metadata row
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 12,

                                    // Year
                                    gtk::Label {
                                        add_css_class: "dim-label",
                                        #[watch]
                                        set_visible: model.movie.as_ref().and_then(|m| m.year).is_some(),
                                        #[watch]
                                        set_label: &model.movie.as_ref()
                                            .and_then(|m| m.year.map(|y| y.to_string()))
                                            .unwrap_or_default(),
                                    },

                                    // Rating
                                    gtk::Box {
                                        set_spacing: 6,
                                        #[watch]
                                        set_visible: model.movie.as_ref().and_then(|m| m.rating).is_some(),

                                        gtk::Image {
                                            set_icon_name: Some("starred-symbolic"),
                                        },

                                        gtk::Label {
                                            #[watch]
                                            set_label: &model.movie.as_ref()
                                                .and_then(|m| m.rating.map(|r| format!("{:.1}", r)))
                                                .unwrap_or_default(),
                                        }
                                    },

                                    // Duration
                                    gtk::Label {
                                        add_css_class: "dim-label",
                                        #[watch]
                                        set_label: &model.movie.as_ref()
                                            .map(|m| {
                                                let total_minutes = m.duration.as_secs() / 60;
                                                let hours = total_minutes / 60;
                                                let minutes = total_minutes % 60;
                                                if hours > 0 {
                                                    format!("{}h {}m", hours, minutes)
                                                } else {
                                                    format!("{}m", minutes)
                                                }
                                            })
                                            .unwrap_or_default(),
                                    },
                                },

                                // Action buttons
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 12,

                                    gtk::Button {
                                        add_css_class: "suggested-action",
                                        add_css_class: "pill",
                                        set_can_focus: true,

                                        adw::ButtonContent {
                                            set_icon_name: "media-playback-start-symbolic",
                                            #[watch]
                                            set_label: if model.movie.as_ref()
                                                .and_then(|m| m.playback_position)
                                                .map(|p| p.as_secs() > 0)
                                                .unwrap_or(false) {
                                                "Resume"
                                            } else {
                                                "Play"
                                            },
                                        },

                                        connect_clicked => MovieDetailsInput::PlayMovie,
                                    },

                                    gtk::Button {
                                        add_css_class: "flat",

                                        gtk::Image {
                                            #[watch]
                                            set_icon_name: Some(if model.movie.as_ref()
                                                .map(|m| m.watched)
                                                .unwrap_or(false) {
                                                "object-select-symbolic"
                                            } else {
                                                "circle-outline-thick-symbolic"
                                            }),
                                        },

                                        connect_clicked => MovieDetailsInput::ToggleWatched,
                                    },
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

                    // Genres
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 6,
                        #[watch]
                        set_visible: model.movie.as_ref().map(|m| !m.genres.is_empty()).unwrap_or(false),

                        append: &model.genre_box,
                    },

                    // Overview
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 12,
                        #[watch]
                        set_visible: model.movie.as_ref().and_then(|m| m.overview.as_ref()).is_some(),

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
                            set_label: &model.movie.as_ref()
                                .and_then(|m| m.overview.clone())
                                .unwrap_or_default(),
                        },
                    },

                    // Cast & Crew
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 12,
                        #[watch]
                        set_visible: model.movie.as_ref().map(|m| !m.cast.is_empty()).unwrap_or(false),

                        gtk::Label {
                            set_label: "Cast",
                            set_halign: gtk::Align::Start,
                            add_css_class: "title-4",
                        },

                        gtk::ScrolledWindow {
                            set_vscrollbar_policy: gtk::PolicyType::Never,
                            set_min_content_height: 120,

                            set_child: Some(&model.cast_box),
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
        let genre_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(6)
            .build();
        let cast_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .build();

        let model = Self {
            movie: None,
            item_id: init.0.clone(),
            db: init.1,
            loading: true,
            genre_box: genre_box.clone(),
            cast_box: cast_box.clone(),
        };

        let widgets = view_output!();

        sender.oneshot_command(async { MovieDetailsCommand::LoadDetails });

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        msg: Self::Input,
        sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            MovieDetailsInput::LoadMovie(item_id) => {
                self.item_id = item_id;
                self.movie = None;
                self.loading = true;
                sender.oneshot_command(async { MovieDetailsCommand::LoadDetails });
            }
            MovieDetailsInput::PlayMovie => {
                sender
                    .output(MovieDetailsOutput::PlayMedia(self.item_id.clone()))
                    .unwrap();
            }
            MovieDetailsInput::ToggleWatched => {
                if let Some(movie) = &mut self.movie {
                    movie.watched = !movie.watched;
                    // TODO: Update database
                }
            }
        }
    }

    async fn update_cmd(
        &mut self,
        msg: Self::CommandOutput,
        sender: AsyncComponentSender<Self>,
        root: &Self::Root,
    ) {
        match msg {
            MovieDetailsCommand::LoadDetails => {
                let cmd = GetItemDetailsCommand {
                    db: (*self.db).clone(),
                    item_id: self.item_id.clone(),
                };

                match Command::execute(&cmd).await {
                    Ok(item) => {
                        if let MediaItem::Movie(movie) = item {
                            self.movie = Some(movie.clone());
                            self.loading = false;

                            // Update genre pills
                            while let Some(child) = self.genre_box.first_child() {
                                self.genre_box.remove(&child);
                            }

                            for genre in &movie.genres {
                                let pill = gtk::Label::builder()
                                    .label(genre)
                                    .css_classes(["pill"])
                                    .build();
                                self.genre_box.append(&pill);
                            }

                            // Update cast cards
                            while let Some(child) = self.cast_box.first_child() {
                                self.cast_box.remove(&child);
                            }

                            for person in movie.cast.iter().take(10) {
                                let card = create_person_card(&person);
                                self.cast_box.append(&card);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to load movie details: {}", e);
                        self.loading = false;
                    }
                }
            }
        }
    }
}

fn create_person_card(person: &Person) -> gtk::Box {
    let card = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .width_request(100)
        .css_classes(["card"])
        .build();

    // Person image or placeholder
    let picture = gtk::Picture::builder()
        .width_request(100)
        .height_request(100)
        .content_fit(gtk::ContentFit::Cover)
        .build();

    if let Some(image_url) = &person.image_url {
        if let Ok(pixbuf) = gtk::gdk_pixbuf::Pixbuf::from_file_at_size(image_url, 100, 100) {
            let texture = gtk::gdk::Texture::for_pixbuf(&pixbuf);
            picture.set_paintable(Some(&texture));
        }
    }

    let name = gtk::Label::builder()
        .label(&person.name)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .css_classes(["caption"])
        .build();

    let role = gtk::Label::builder()
        .label(person.role.as_deref().unwrap_or(""))
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .css_classes(["dim-label", "caption"])
        .build();

    card.append(&picture);
    card.append(&name);
    if person.role.is_some() {
        card.append(&role);
    }

    card
}
