use crate::models::{MediaItem, MediaItemId, Movie};
use crate::services::commands::Command;
use crate::services::commands::media_commands::{
    GetItemDetailsCommand, MarkUnwatchedCommand, MarkWatchedCommand,
};
use crate::ui::shared::broker::{BROKER, BrokerMessage};
use crate::ui::shared::image_helpers::load_image_from_url;
use crate::ui::shared::person_card::create_person_card;
use adw::prelude::*;
use libadwaita as adw;
use relm4::RelmWidgetExt;
use relm4::gtk;
use relm4::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::error;

#[derive(Debug)]
pub struct MovieDetailsPage {
    movie: Option<Movie>,
    item_id: MediaItemId,
    db: Arc<crate::db::connection::DatabaseConnection>,
    loading: bool,
    genre_box: gtk::Box,
    cast_box: gtk::Box,
    crew_box: gtk::Box,
    poster_texture: Option<gtk::gdk::Texture>,
    backdrop_texture: Option<gtk::gdk::Texture>,
    person_textures: HashMap<String, gtk::gdk::Texture>,
    full_metadata_loaded: bool,
}

#[derive(Debug)]
pub enum MovieDetailsInput {
    PlayMovie,
    ToggleWatched,
    BrokerMsg(BrokerMessage),
}

#[derive(Debug)]
pub enum MovieDetailsOutput {
    PlayMedia(MediaItemId),
}

#[derive(Debug)]
pub enum MovieDetailsCommand {
    LoadDetails,
    LoadPosterImage {
        url: String,
    },
    LoadBackdropImage {
        url: String,
    },
    LoadPersonImage {
        person_id: String,
        url: String,
    },
    PosterImageLoaded {
        texture: gtk::gdk::Texture,
    },
    BackdropImageLoaded {
        texture: gtk::gdk::Texture,
    },
    PersonImageLoaded {
        person_id: String,
        texture: gtk::gdk::Texture,
    },
    LoadFullMetadata,
    FullMetadataLoaded,
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

                // Hero Section with balanced height
                gtk::Overlay {
                    set_height_request: 480,  // Balanced to accommodate overview text
                    add_css_class: "hero-section",

                    // Backdrop image with Ken Burns animation
                    gtk::Picture {
                        set_content_fit: gtk::ContentFit::Cover,
                        add_css_class: "hero-backdrop",
                        #[watch]
                        set_paintable: model.backdrop_texture.as_ref(),
                        #[watch]
                        set_visible: !model.loading,
                    },

                    // Enhanced gradient overlay with glass morphism
                    add_overlay = &gtk::Box {
                        add_css_class: "hero-gradient-modern",
                        set_valign: gtk::Align::End,
                        #[watch]
                        set_visible: !model.loading,

                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_margin_all: 24,
                            set_margin_bottom: 16,  // Reduce bottom margin
                            set_spacing: 24,

                            // Poster with original size
                            gtk::Picture {
                                set_width_request: 300,
                                set_height_request: 450,
                                add_css_class: "card",
                                add_css_class: "poster-styled",
                                add_css_class: "fade-in-scale",
                                #[watch]
                                set_paintable: model.poster_texture.as_ref(),
                            },

                            // Movie info with overview integrated
                            gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_valign: gtk::Align::End,  // Align to bottom to reduce gap
                                set_spacing: 12,
                                set_hexpand: true,

                                // Title with hero typography - moved to top
                                gtk::Label {
                                    set_halign: gtk::Align::Start,
                                    add_css_class: "title-hero",
                                    add_css_class: "fade-in-up",
                                    set_wrap: true,
                                    set_margin_bottom: 8,  // Small spacing before overview
                                    #[watch]
                                    set_label: &model.movie.as_ref().map(|m| m.title.clone()).unwrap_or_default(),
                                },

                                // Overview text below the title
                                gtk::Label {
                                    set_halign: gtk::Align::Start,
                                    set_wrap: true,
                                    set_wrap_mode: gtk::pango::WrapMode::WordChar,
                                    set_max_width_chars: 60,
                                    set_ellipsize: gtk::pango::EllipsizeMode::End,
                                    set_lines: 3,  // Limit to 3 lines to reduce vertical space
                                    add_css_class: "overview-hero",
                                    #[watch]
                                    set_label: &model.movie.as_ref()
                                        .and_then(|m| m.overview.clone())
                                        .unwrap_or_default(),
                                    #[watch]
                                    set_visible: model.movie.as_ref().and_then(|m| m.overview.as_ref()).is_some(),
                                },

                                // Metadata row with modern styling
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 12,
                                    set_margin_bottom: 16,  // Add space before action buttons
                                    add_css_class: "stagger-animation",

                                    // Year pill
                                    gtk::Box {
                                        add_css_class: "metadata-pill-modern",
                                        add_css_class: "interactive-element",
                                        #[watch]
                                        set_visible: model.movie.as_ref().and_then(|m| m.year).is_some(),

                                        gtk::Label {
                                            set_margin_start: 12,
                                            set_margin_end: 12,
                                            set_margin_top: 6,
                                            set_margin_bottom: 6,
                                            #[watch]
                                            set_label: &model.movie.as_ref()
                                                .and_then(|m| m.year.map(|y| y.to_string()))
                                                .unwrap_or_default(),
                                        },
                                    },

                                    // Rating pill with star gradient
                                    gtk::Box {
                                        add_css_class: "metadata-pill-modern",
                                        add_css_class: "rating-pill",
                                        add_css_class: "interactive-element",
                                        set_spacing: 6,
                                        #[watch]
                                        set_visible: model.movie.as_ref().and_then(|m| m.rating).is_some(),

                                        gtk::Image {
                                            set_icon_name: Some("starred-symbolic"),
                                            set_margin_start: 12,
                                        },

                                        gtk::Label {
                                            set_margin_end: 12,
                                            set_margin_top: 6,
                                            set_margin_bottom: 6,
                                            #[watch]
                                            set_label: &model.movie.as_ref()
                                                .and_then(|m| m.rating.map(|r| format!("{:.1}", r)))
                                                .unwrap_or_default(),
                                        }
                                    },

                                    // Duration pill
                                    gtk::Box {
                                        add_css_class: "metadata-pill-modern",
                                        add_css_class: "interactive-element",
                                        #[watch]
                                        set_visible: model.movie.is_some(),

                                        gtk::Label {
                                            set_margin_start: 12,
                                            set_margin_end: 12,
                                            set_margin_top: 6,
                                            set_margin_bottom: 6,
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
                                },

                                // Action buttons - separated from metadata
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 12,
                                    set_margin_top: 8,  // Extra spacing from metadata

                                    gtk::Button {
                                        add_css_class: "action-button-primary",
                                        add_css_class: "play-button-prominent",
                                        add_css_class: "ripple",
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
                                        add_css_class: "action-button-secondary",
                                        add_css_class: "interactive-element",
                                        #[watch]
                                        set_tooltip_text: Some(if model.movie.as_ref()
                                            .map(|m| m.watched)
                                            .unwrap_or(false) {
                                            "Mark as unwatched"
                                        } else {
                                            "Mark as watched"
                                        }),

                                        gtk::Box {
                                            set_width_request: 20,
                                            set_height_request: 20,
                                            set_halign: gtk::Align::Center,
                                            set_valign: gtk::Align::Center,

                                            gtk::Image {
                                                #[watch]
                                                set_icon_name: Some(if model.movie.as_ref()
                                                    .map(|m| m.watched)
                                                    .unwrap_or(false) {
                                                    "object-select-symbolic"
                                                } else {
                                                    "media-record-symbolic"  // Circle icon for unwatched
                                                }),
                                                set_pixel_size: 18,
                                            },
                                        },

                                        connect_clicked => MovieDetailsInput::ToggleWatched,
                                    },
                                },
                            },
                        },
                    },
                },

                // Content section with animations
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_margin_all: 24,
                    set_margin_top: 12,  // Reduce top margin to bring content up
                    set_spacing: 20,
                    add_css_class: "fade-in-up",
                    #[watch]
                    set_visible: !model.loading,

                    // Genres
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 6,
                        #[watch]
                        set_visible: model.movie.as_ref().map(|m| !m.genres.is_empty()).unwrap_or(false),

                        append: &model.genre_box,
                    },

                    // Removed redundant overview section since it's now in the hero

                    // Cast
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

                    // Crew
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 12,
                        #[watch]
                        set_visible: model.movie.as_ref().map(|m| !m.crew.is_empty()).unwrap_or(false),

                        gtk::Label {
                            set_label: "Crew",
                            set_halign: gtk::Align::Start,
                            add_css_class: "title-4",
                        },

                        gtk::ScrolledWindow {
                            set_vscrollbar_policy: gtk::PolicyType::Never,
                            set_min_content_height: 120,

                            set_child: Some(&model.crew_box),
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
            .spacing(8)
            .css_classes(["stagger-animation"])
            .build();
        let cast_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(16)
            .css_classes(["stagger-animation"])
            .build();
        let crew_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(16)
            .css_classes(["stagger-animation"])
            .build();

        let model = Self {
            movie: None,
            item_id: init.0.clone(),
            db: init.1,
            loading: true,
            genre_box: genre_box.clone(),
            cast_box: cast_box.clone(),
            crew_box: crew_box.clone(),
            poster_texture: None,
            backdrop_texture: None,
            person_textures: HashMap::new(),
            full_metadata_loaded: false,
        };

        let widgets = view_output!();

        // Subscribe to MessageBroker for playback progress updates
        {
            let broker_sender = sender.input_sender().clone();
            relm4::spawn(async move {
                let (tx, mut rx) = relm4::channel::<BrokerMessage>();
                BROKER.subscribe("MovieDetailsPage".to_string(), tx).await;

                while let Some(msg) = rx.recv().await {
                    broker_sender
                        .send(MovieDetailsInput::BrokerMsg(msg))
                        .unwrap();
                }
            });
        }

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
            MovieDetailsInput::PlayMovie => {
                // For now, movies play without PlayQueue context
                // TODO: Consider creating PlayQueue for movies to enable features like
                // continue watching from different devices
                sender
                    .output(MovieDetailsOutput::PlayMedia(self.item_id.clone()))
                    .unwrap();
            }
            MovieDetailsInput::ToggleWatched => {
                if let Some(movie) = &self.movie {
                    let db = (*self.db).clone();
                    let media_id = self.item_id.clone();
                    let watched = movie.watched;

                    relm4::spawn(async move {
                        let result = if watched {
                            // Mark as unwatched
                            let cmd = MarkUnwatchedCommand { db, media_id };
                            Command::execute(&cmd).await
                        } else {
                            // Mark as watched
                            let cmd = MarkWatchedCommand { db, media_id };
                            Command::execute(&cmd).await
                        };

                        if let Err(e) = result {
                            error!("Failed to toggle watch status: {}", e);
                        }
                    });
                }
            }
            MovieDetailsInput::BrokerMsg(msg) => match msg {
                BrokerMessage::Data(data_msg) => match data_msg {
                    crate::ui::shared::broker::DataMessage::PlaybackProgressUpdated {
                        media_id,
                        watched,
                    } => {
                        // Check if the updated media is this movie
                        if self.item_id.to_string() == media_id {
                            tracing::debug!(
                                "Movie progress updated for {}: watched={}, reloading details",
                                media_id,
                                watched
                            );
                            // Reload movie details to update watch status
                            sender.oneshot_command(async { MovieDetailsCommand::LoadDetails });
                        }
                    }
                    _ => {}
                },
                _ => {}
            },
        }
    }

    async fn update_cmd(
        &mut self,
        msg: Self::CommandOutput,
        sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
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

                            // Check if we need to load full cast/crew (if cast count <= 3, likely only preview)
                            // Only attempt once to avoid infinite loop if movie really has ≤3 cast members
                            if movie.cast.len() <= 3 && !self.full_metadata_loaded {
                                tracing::debug!(
                                    "Movie has {} cast members, loading full metadata",
                                    movie.cast.len()
                                );
                                sender.oneshot_command(async {
                                    MovieDetailsCommand::LoadFullMetadata
                                });
                            }

                            // Load poster and backdrop images
                            if let Some(poster_url) = movie.poster_url.clone() {
                                sender.oneshot_command(async move {
                                    MovieDetailsCommand::LoadPosterImage { url: poster_url }
                                });
                            }

                            if let Some(backdrop_url) = movie.backdrop_url.clone() {
                                sender.oneshot_command(async move {
                                    MovieDetailsCommand::LoadBackdropImage { url: backdrop_url }
                                });
                            }

                            // Update genre pills
                            while let Some(child) = self.genre_box.first_child() {
                                self.genre_box.remove(&child);
                            }

                            for genre in &movie.genres {
                                let pill = gtk::Box::builder()
                                    .css_classes(["metadata-pill-modern", "interactive-element"])
                                    .build();
                                let label = gtk::Label::builder()
                                    .label(genre)
                                    .margin_start(12)
                                    .margin_end(12)
                                    .margin_top(6)
                                    .margin_bottom(6)
                                    .build();
                                pill.append(&label);
                                self.genre_box.append(&pill);
                            }

                            // Load person images for cast and crew
                            for person in movie.cast.iter().take(10) {
                                if let Some(image_url) = &person.image_url {
                                    let person_id = person.id.clone();
                                    let url = image_url.clone();
                                    sender.oneshot_command(async move {
                                        MovieDetailsCommand::LoadPersonImage { person_id, url }
                                    });
                                }
                            }

                            // Filter crew to only directors and writers, take first 10
                            let crew_filtered: Vec<_> = movie
                                .crew
                                .iter()
                                .filter(|p| {
                                    p.role
                                        .as_ref()
                                        .map(|r| {
                                            r.eq_ignore_ascii_case("Director")
                                                || r.eq_ignore_ascii_case("Writer")
                                        })
                                        .unwrap_or(false)
                                })
                                .take(10)
                                .collect();

                            for person in &crew_filtered {
                                if let Some(image_url) = &person.image_url {
                                    let person_id = person.id.clone();
                                    let url = image_url.clone();
                                    sender.oneshot_command(async move {
                                        MovieDetailsCommand::LoadPersonImage { person_id, url }
                                    });
                                }
                            }

                            // Update cast cards
                            while let Some(child) = self.cast_box.first_child() {
                                self.cast_box.remove(&child);
                            }

                            for person in movie.cast.iter().take(10) {
                                let texture = self.person_textures.get(&person.id).cloned();
                                let card = create_person_card(person, texture.as_ref());
                                self.cast_box.append(&card);
                            }

                            // Update crew cards
                            while let Some(child) = self.crew_box.first_child() {
                                self.crew_box.remove(&child);
                            }

                            for person in crew_filtered {
                                let texture = self.person_textures.get(&person.id).cloned();
                                let card = create_person_card(person, texture.as_ref());
                                self.crew_box.append(&card);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to load movie details: {}", e);
                        self.loading = false;
                    }
                }
            }
            MovieDetailsCommand::LoadPosterImage { url } => {
                // Spawn async task to download and create texture
                let sender_clone = sender.clone();
                relm4::spawn(async move {
                    match load_image_from_url(&url, 300, 450).await {
                        Ok(texture) => {
                            sender_clone.oneshot_command(async move {
                                MovieDetailsCommand::PosterImageLoaded { texture }
                            });
                        }
                        Err(e) => {
                            tracing::error!("Failed to load poster image: {}", e);
                        }
                    }
                });
            }
            MovieDetailsCommand::LoadBackdropImage { url } => {
                // Spawn async task to download and create texture
                let sender_clone = sender.clone();
                relm4::spawn(async move {
                    match load_image_from_url(&url, -1, 550).await {
                        Ok(texture) => {
                            sender_clone.oneshot_command(async move {
                                MovieDetailsCommand::BackdropImageLoaded { texture }
                            });
                        }
                        Err(e) => {
                            tracing::error!("Failed to load backdrop image: {}", e);
                        }
                    }
                });
            }
            MovieDetailsCommand::LoadPersonImage { person_id, url } => {
                // Spawn async task to download and create texture
                let sender_clone = sender.clone();
                relm4::spawn(async move {
                    match load_image_from_url(&url, 120, 120).await {
                        Ok(texture) => {
                            sender_clone.oneshot_command(async move {
                                MovieDetailsCommand::PersonImageLoaded { person_id, texture }
                            });
                        }
                        Err(e) => {
                            tracing::error!("Failed to load person image for {}: {}", person_id, e);
                        }
                    }
                });
            }
            MovieDetailsCommand::PosterImageLoaded { texture } => {
                self.poster_texture = Some(texture);
            }
            MovieDetailsCommand::BackdropImageLoaded { texture } => {
                self.backdrop_texture = Some(texture);
            }
            MovieDetailsCommand::PersonImageLoaded { person_id, texture } => {
                // Store the loaded texture
                self.person_textures.insert(person_id.clone(), texture);

                // Recreate cast and crew cards with the new texture
                if let Some(movie) = &self.movie {
                    // Update cast cards
                    while let Some(child) = self.cast_box.first_child() {
                        self.cast_box.remove(&child);
                    }

                    for person in movie.cast.iter().take(10) {
                        let texture = self.person_textures.get(&person.id).cloned();
                        let card = create_person_card(person, texture.as_ref());
                        self.cast_box.append(&card);
                    }

                    // Update crew cards (filter for directors and writers)
                    while let Some(child) = self.crew_box.first_child() {
                        self.crew_box.remove(&child);
                    }

                    let crew_filtered: Vec<_> = movie
                        .crew
                        .iter()
                        .filter(|p| {
                            p.role
                                .as_ref()
                                .map(|r| {
                                    r.eq_ignore_ascii_case("Director")
                                        || r.eq_ignore_ascii_case("Writer")
                                })
                                .unwrap_or(false)
                        })
                        .take(10)
                        .collect();

                    for person in crew_filtered {
                        let texture = self.person_textures.get(&person.id).cloned();
                        let card = create_person_card(person, texture.as_ref());
                        self.crew_box.append(&card);
                    }
                }
            }
            MovieDetailsCommand::LoadFullMetadata => {
                use crate::services::commands::media_commands::LoadFullMovieMetadataCommand;

                tracing::info!("Loading full metadata for movie {}", self.item_id);
                let cmd = LoadFullMovieMetadataCommand {
                    db: (*self.db).clone(),
                    movie_id: self.item_id.clone(),
                };

                match Command::execute(&cmd).await {
                    Ok(_) => {
                        tracing::info!("Full metadata loaded, refreshing movie details");
                        sender.oneshot_command(async { MovieDetailsCommand::FullMetadataLoaded });
                    }
                    Err(e) => {
                        tracing::error!("Failed to load full metadata: {}", e);
                    }
                }
            }
            MovieDetailsCommand::FullMetadataLoaded => {
                // Mark that we've loaded full metadata to prevent infinite loop
                self.full_metadata_loaded = true;

                // Directly reload cast/crew from database without full page refresh
                use crate::db::repository::{PeopleRepository, PeopleRepositoryImpl};

                let db = (*self.db).clone();
                let item_id = self.item_id.clone();

                match (async move {
                    let people_repo = PeopleRepositoryImpl::new(db);
                    people_repo.find_by_media_item(item_id.as_ref()).await
                })
                .await
                {
                    Ok(people_with_relations) => {
                        // Separate cast and crew based on person_type
                        let mut cast = Vec::new();
                        let mut crew = Vec::new();

                        for (person, media_person) in people_with_relations {
                            let person_obj = crate::models::Person {
                                id: person.id.clone(),
                                name: person.name.clone(),
                                role: media_person.role.clone(),
                                image_url: person.image_url.clone(),
                            };

                            match media_person.person_type.as_str() {
                                "actor" | "cast" => cast.push(person_obj),
                                "director" | "writer" | "producer" | "crew" => {
                                    crew.push(person_obj)
                                }
                                _ => {}
                            }
                        }

                        // Update the movie's cast/crew in place
                        if let Some(movie) = &mut self.movie {
                            movie.cast = cast.clone();
                            movie.crew = crew.clone();
                        }

                        // Clear and rebuild cast box
                        while let Some(child) = self.cast_box.first_child() {
                            self.cast_box.remove(&child);
                        }

                        for person in cast.iter().take(10) {
                            let texture = self.person_textures.get(&person.id).cloned();
                            let card = create_person_card(person, texture.as_ref());
                            self.cast_box.append(&card);

                            // Load image if not already loaded
                            if texture.is_none() {
                                if let Some(image_url) = &person.image_url {
                                    let person_id = person.id.clone();
                                    let url = image_url.clone();
                                    sender.oneshot_command(async move {
                                        MovieDetailsCommand::LoadPersonImage { person_id, url }
                                    });
                                }
                            }
                        }

                        // Clear and rebuild crew box (filter for directors and writers)
                        while let Some(child) = self.crew_box.first_child() {
                            self.crew_box.remove(&child);
                        }

                        let crew_filtered: Vec<_> = crew
                            .iter()
                            .filter(|p| {
                                p.role
                                    .as_ref()
                                    .map(|r| {
                                        r.eq_ignore_ascii_case("Director")
                                            || r.eq_ignore_ascii_case("Writer")
                                    })
                                    .unwrap_or(false)
                            })
                            .take(10)
                            .collect();

                        for person in crew_filtered {
                            let texture = self.person_textures.get(&person.id).cloned();
                            let card = create_person_card(person, texture.as_ref());
                            self.crew_box.append(&card);

                            // Load image if not already loaded
                            if texture.is_none() {
                                if let Some(image_url) = &person.image_url {
                                    let person_id = person.id.clone();
                                    let url = image_url.clone();
                                    sender.oneshot_command(async move {
                                        MovieDetailsCommand::LoadPersonImage { person_id, url }
                                    });
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to reload cast/crew after metadata load: {}", e);
                    }
                }
            }
        }
    }

    fn shutdown(&mut self, _widgets: &mut Self::Widgets, _output: relm4::Sender<Self::Output>) {
        // Unsubscribe from MessageBroker
        relm4::spawn(async move {
            BROKER.unsubscribe("MovieDetailsPage").await;
        });
    }
}
