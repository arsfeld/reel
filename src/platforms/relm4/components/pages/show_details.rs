use crate::models::{Episode, MediaItem, MediaItemId, PlaylistContext, Season, Show};
use crate::services::commands::Command;
use crate::services::commands::media_commands::{GetEpisodesCommand, GetItemDetailsCommand};
use crate::services::core::PlaylistService;
use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;
use relm4::RelmWidgetExt;
use relm4::gtk;
use relm4::prelude::*;
use std::sync::Arc;
use tracing::error;

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
    poster_texture: Option<gtk::gdk::Texture>,
    backdrop_texture: Option<gtk::gdk::Texture>,
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
    PlayMediaWithContext {
        media_id: MediaItemId,
        context: PlaylistContext,
    },
    NavigateBack,
}

#[derive(Debug)]
pub enum ShowDetailsCommand {
    LoadDetails,
    LoadEpisodes(String, u32),
    LoadPosterImage {
        url: String,
    },
    LoadBackdropImage {
        url: String,
    },
    PosterImageLoaded {
        texture: gtk::gdk::Texture,
    },
    BackdropImageLoaded {
        texture: gtk::gdk::Texture,
    },
    PlayWithContext {
        episode_id: MediaItemId,
        context: PlaylistContext,
    },
    PlayWithoutContext(MediaItemId),
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

                // Hero Section with full-bleed backdrop and Ken Burns effect
                gtk::Overlay {
                    set_height_request: 600,  // Even taller for cinematic feel
                    add_css_class: "hero-section",

                    // Backdrop image with Ken Burns animation
                    gtk::Picture {
                        set_content_fit: gtk::ContentFit::Cover,
                        add_css_class: "hero-backdrop",
                        #[watch]
                        set_paintable: model.backdrop_texture.as_ref(),
                    },

                    // Enhanced gradient overlay with glass morphism
                    add_overlay = &gtk::Box {
                        add_css_class: "hero-gradient-modern",
                        set_valign: gtk::Align::End,

                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_margin_all: 32,
                            set_spacing: 32,

                            // Premium poster with depth effect
                            gtk::Picture {
                                set_width_request: 300,
                                set_height_request: 450,
                                add_css_class: "card",
                                add_css_class: "poster-premium",
                                add_css_class: "fade-in-scale",
                                #[watch]
                                set_paintable: model.poster_texture.as_ref(),
                            },

                            // Show info
                            gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_valign: gtk::Align::End,
                                set_spacing: 12,
                                set_hexpand: true,

                                // Title with hero typography
                                gtk::Label {
                                    set_halign: gtk::Align::Start,
                                    add_css_class: "title-hero",
                                    add_css_class: "fade-in-up",
                                    set_wrap: true,
                                    #[watch]
                                    set_label: &model.show.as_ref().map(|s| s.title.clone()).unwrap_or_default(),
                                },

                                // Metadata row with modern styling
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 12,
                                    add_css_class: "stagger-animation",

                                    // Year pill
                                    gtk::Box {
                                        add_css_class: "metadata-pill-modern",
                                        add_css_class: "interactive-element",
                                        #[watch]
                                        set_visible: model.show.as_ref().and_then(|s| s.year).is_some(),

                                        gtk::Label {
                                            set_margin_start: 12,
                                            set_margin_end: 12,
                                            set_margin_top: 6,
                                            set_margin_bottom: 6,
                                            #[watch]
                                            set_label: &model.show.as_ref()
                                                .and_then(|s| s.year.map(|y| y.to_string()))
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
                                        set_visible: model.show.as_ref().and_then(|s| s.rating).is_some(),

                                        gtk::Image {
                                            set_icon_name: Some("starred-symbolic"),
                                            set_margin_start: 12,
                                        },

                                        gtk::Label {
                                            set_margin_end: 12,
                                            set_margin_top: 6,
                                            set_margin_bottom: 6,
                                            #[watch]
                                            set_label: &model.show.as_ref()
                                                .and_then(|s| s.rating.map(|r| format!("{:.1}", r)))
                                                .unwrap_or_default(),
                                        }
                                    },

                                    // Episode count pill
                                    gtk::Box {
                                        add_css_class: "metadata-pill-modern",
                                        add_css_class: "interactive-element",
                                        gtk::Label {
                                            set_margin_start: 12,
                                            set_margin_end: 12,
                                            set_margin_top: 6,
                                            set_margin_bottom: 6,
                                            #[watch]
                                            set_label: &model.show.as_ref()
                                                .map(|s| format!("{} episodes", s.total_episode_count))
                                                .unwrap_or_default(),
                                        },
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

                // Content section with animations
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_margin_all: 32,
                    set_spacing: 32,
                    add_css_class: "fade-in-up",

                    // Overview with glass card
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 16,
                        add_css_class: "glass-card",
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
                            add_css_class: "overview-text",
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
                            set_hscrollbar_policy: gtk::PolicyType::Automatic,
                            set_vscrollbar_policy: gtk::PolicyType::Never,
                            set_height_request: 220,  // Fixed height for episode cards
                            set_overlay_scrolling: true,

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
            .homogeneous(false) // Changed to false to allow proper sizing
            .selection_mode(gtk::SelectionMode::None)
            .min_children_per_line(100) // Force single row by setting high min
            .max_children_per_line(100) // Match max to min
            .valign(gtk::Align::Start)
            .build();

        let season_dropdown = gtk::DropDown::builder().enable_search(false).build();

        {
            let sender = sender.clone();
            season_dropdown.connect_selected_notify(move |dropdown| {
                let selected = dropdown.selected();
                // Use checked_add to prevent overflow, default to season 1 if overflow occurs
                let season_num = (selected as u32).checked_add(1).unwrap_or(1);
                sender.input(ShowDetailsInput::SelectSeason(season_num));
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
            poster_texture: None,
            backdrop_texture: None,
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
                self.poster_texture = None;
                self.backdrop_texture = None;
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
                // Build playlist context for the episode
                let db_clone = self.db.clone();
                let episode_id_clone = episode_id.clone();

                sender.oneshot_command(async move {
                    // Try to build playlist context for TV show navigation
                    match PlaylistService::build_show_context(&db_clone, &episode_id_clone).await {
                        Ok(context) => ShowDetailsCommand::PlayWithContext {
                            episode_id: episode_id_clone,
                            context,
                        },
                        Err(e) => {
                            tracing::warn!(
                                "Failed to build playlist context: {}, playing without context",
                                e
                            );
                            ShowDetailsCommand::PlayWithoutContext(episode_id_clone)
                        }
                    }
                });
            }
            ShowDetailsInput::ToggleEpisodeWatched(index) => {
                if let Some(episode) = self.episodes.get_mut(index) {
                    episode.watched = !episode.watched;

                    // Update database with watched status
                    let db = (*self.db).clone();
                    let media_id = episode.id.clone();
                    let watched = episode.watched;

                    relm4::spawn(async move {
                        use crate::db::repository::{PlaybackRepository, PlaybackRepositoryImpl};

                        let repo = PlaybackRepositoryImpl::new(db);
                        if watched {
                            if let Err(e) = repo.mark_watched(&media_id.to_string(), None).await {
                                error!("Failed to mark episode as watched: {}", e);
                            }
                        } else {
                            if let Err(e) = repo.mark_unwatched(&media_id.to_string(), None).await {
                                error!("Failed to mark episode as unwatched: {}", e);
                            }
                        }
                    });

                    self.update_episode_grid(&sender);
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
                            // Debug log the show data
                            tracing::debug!("Loaded show: {:?}", show.title);
                            tracing::debug!("Show has {} seasons", show.seasons.len());
                            tracing::debug!("Total episode count: {}", show.total_episode_count);
                            for (idx, season) in show.seasons.iter().enumerate() {
                                tracing::debug!(
                                    "Season {}: number={}, episodes={}",
                                    idx,
                                    season.season_number,
                                    season.episode_count
                                );
                            }
                            self.show = Some(show.clone());
                            self.loading = false;

                            // Load poster and backdrop images
                            if let Some(poster_url) = show.poster_url.clone() {
                                sender.oneshot_command(async move {
                                    ShowDetailsCommand::LoadPosterImage { url: poster_url }
                                });
                            }

                            if let Some(backdrop_url) = show.backdrop_url.clone() {
                                sender.oneshot_command(async move {
                                    ShowDetailsCommand::LoadBackdropImage { url: backdrop_url }
                                });
                            }

                            // Update season dropdown
                            let seasons: Vec<String> = if show.seasons.is_empty() {
                                // If no seasons data (shouldn't happen after proper sync), show placeholder
                                tracing::warn!(
                                    "Show {} has no seasons data in database",
                                    show.title
                                );
                                vec!["Season 1".to_string()]
                            } else {
                                show.seasons
                                    .iter()
                                    .map(|s| {
                                        if s.season_number == 0 {
                                            "Specials".to_string()
                                        } else {
                                            format!("Season {}", s.season_number)
                                        }
                                    })
                                    .collect()
                            };

                            let model = gtk::StringList::new(
                                &seasons.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                            );
                            self.season_dropdown.set_model(Some(&model));

                            // Find the season with the first unwatched episode
                            let db_clone = (*self.db).clone();
                            let season_to_select =
                                find_season_with_next_unwatched(&show, db_clone).await;

                            // Select the appropriate season (default to first if none found)
                            let season_index = show
                                .seasons
                                .iter()
                                .position(|s| s.season_number == season_to_select)
                                .unwrap_or(0);

                            self.season_dropdown.set_selected(season_index as u32);
                            self.current_season = season_to_select;

                            // Load episodes for the selected season
                            // Always try to load episodes, even if seasons is empty
                            sender.input(ShowDetailsInput::LoadEpisodes);
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
                        self.update_episode_grid(&sender);
                    }
                    Err(e) => {
                        tracing::error!("Failed to load episodes: {}", e);
                    }
                }
            }
            ShowDetailsCommand::PlayWithContext {
                episode_id,
                context,
            } => {
                // Send output with context to main window
                sender
                    .output(ShowDetailsOutput::PlayMediaWithContext {
                        media_id: episode_id,
                        context,
                    })
                    .unwrap();
            }
            ShowDetailsCommand::PlayWithoutContext(episode_id) => {
                // Fall back to playing without context
                sender
                    .output(ShowDetailsOutput::PlayMedia(episode_id))
                    .unwrap();
            }
            ShowDetailsCommand::LoadPosterImage { url } => {
                // Spawn async task to download and create texture
                let sender_clone = sender.clone();
                relm4::spawn(async move {
                    match load_image_from_url(&url, 300, 450).await {
                        Ok(texture) => {
                            sender_clone.oneshot_command(async move {
                                ShowDetailsCommand::PosterImageLoaded { texture }
                            });
                        }
                        Err(e) => {
                            tracing::error!("Failed to load poster image: {}", e);
                        }
                    }
                });
            }
            ShowDetailsCommand::LoadBackdropImage { url } => {
                // Spawn async task to download and create texture
                let sender_clone = sender.clone();
                relm4::spawn(async move {
                    match load_image_from_url(&url, -1, 550).await {
                        Ok(texture) => {
                            sender_clone.oneshot_command(async move {
                                ShowDetailsCommand::BackdropImageLoaded { texture }
                            });
                        }
                        Err(e) => {
                            tracing::error!("Failed to load backdrop image: {}", e);
                        }
                    }
                });
            }
            ShowDetailsCommand::PosterImageLoaded { texture } => {
                self.poster_texture = Some(texture);
            }
            ShowDetailsCommand::BackdropImageLoaded { texture } => {
                self.backdrop_texture = Some(texture);
            }
        }
    }
}

async fn load_image_from_url(
    url: &str,
    _width: i32,
    _height: i32,
) -> Result<gtk::gdk::Texture, String> {
    // Download the image
    let response = reqwest::get(url)
        .await
        .map_err(|e| format!("Failed to download image: {}", e))?;

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read bytes: {}", e))?;

    // Create texture from bytes
    let glib_bytes = gtk::glib::Bytes::from(&bytes[..]);
    let texture = gtk::gdk::Texture::from_bytes(&glib_bytes)
        .map_err(|e| format!("Failed to create texture: {}", e))?;

    // If width and height are specified (not -1), we could resize here
    // For now, just return the texture as is
    Ok(texture)
}

impl ShowDetailsPage {
    fn update_episode_grid(&self, sender: &AsyncComponentSender<Self>) {
        // Clear existing children
        while let Some(child) = self.episode_grid.first_child() {
            self.episode_grid.remove(&child);
        }

        // Add episode cards
        for (index, episode) in self.episodes.iter().enumerate() {
            let card = create_episode_card(episode, index, sender.clone());
            self.episode_grid.append(&card);
        }

        // Scroll to the first unwatched episode
        if let Some(first_unwatched_index) = self.episodes.iter().position(|e| !e.watched) {
            if let Some(child) = self
                .episode_grid
                .child_at_index(first_unwatched_index as i32)
            {
                // Focus on the first unwatched episode for better visibility
                child.grab_focus();
            }
        }
    }
}

async fn find_season_with_next_unwatched(
    show: &Show,
    db: crate::db::connection::DatabaseConnection,
) -> u32 {
    use crate::db::repository::{
        MediaRepository, MediaRepositoryImpl, PlaybackRepository, PlaybackRepositoryImpl,
    };

    // Get all episodes for the show
    let media_repo = MediaRepositoryImpl::new(db.clone());
    let playback_repo = PlaybackRepositoryImpl::new(db);

    // Try to find the first season with an unwatched episode
    for season in &show.seasons {
        match media_repo
            .find_episodes_by_season(&show.id, season.season_number as i32)
            .await
        {
            Ok(episode_models) => {
                // Check if this season has any unwatched episodes
                for episode_model in &episode_models {
                    // Check playback progress to determine if watched
                    let is_watched = match playback_repo.find_by_media_id(&episode_model.id).await {
                        Ok(Some(progress)) => progress.watched,
                        _ => false,
                    };

                    if !is_watched {
                        return season.season_number;
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to get episodes for season {}: {}",
                    season.season_number,
                    e
                );
            }
        }
    }

    // If no unwatched episodes found or all watched, return the first season
    show.seasons.first().map(|s| s.season_number).unwrap_or(1)
}

fn create_episode_card(
    episode: &Episode,
    index: usize,
    sender: AsyncComponentSender<ShowDetailsPage>,
) -> gtk::Box {
    let card = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .width_request(220)
        .css_classes(["episode-card-modern", "interactive-element"])
        .build();

    // Make the card clickable
    let click_controller = gtk::GestureClick::new();
    let episode_id = MediaItemId::new(&episode.id);
    click_controller.connect_released(move |_, _, _, _| {
        sender.input(ShowDetailsInput::PlayEpisode(episode_id.clone()));
    });
    card.add_controller(click_controller);

    // Add hover effects
    card.set_cursor_from_name(Some("pointer"));

    // Episode thumbnail with number overlay
    let overlay = gtk::Overlay::builder()
        .css_classes(["episode-thumbnail"])
        .build();

    let picture = gtk::Picture::builder()
        .width_request(220)
        .height_request(124)
        .content_fit(gtk::ContentFit::Cover)
        .build();

    // Load thumbnail image if available
    if let Some(thumbnail_url) = &episode.thumbnail_url {
        let url = thumbnail_url.clone();
        let picture_clone = picture.clone();
        gtk::glib::spawn_future_local(async move {
            if let Ok(texture) = load_image_from_url(&url, 220, 124).await {
                picture_clone.set_paintable(Some(&texture));
            }
        });
    }

    overlay.set_child(Some(&picture));

    // Episode number badge with modern styling
    let badge = gtk::Label::builder()
        .label(&format!("E{}", episode.episode_number))
        .css_classes(["osd", "pill"])
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Start)
        .margin_top(8)
        .margin_start(8)
        .build();
    overlay.add_overlay(&badge);

    // Modern progress bar if partially watched
    if let Some(position) = episode.playback_position {
        if position.as_secs() > 0 && !episode.watched {
            let progress_container = gtk::Box::builder()
                .css_classes(["episode-progress-bar"])
                .valign(gtk::Align::End)
                .build();

            let progress = gtk::Box::builder()
                .css_classes(["progress"])
                .width_request(
                    (220.0 * position.as_secs_f64() / episode.duration.as_secs_f64()) as i32,
                )
                .build();

            progress_container.append(&progress);
            overlay.add_overlay(&progress_container);
        }
    }

    // Watched indicator with modern styling
    if episode.watched {
        let check = gtk::Image::builder()
            .icon_name("object-select-symbolic")
            .css_classes(["osd", "fade-in-scale"])
            .halign(gtk::Align::End)
            .valign(gtk::Align::Start)
            .margin_top(8)
            .margin_end(8)
            .pixel_size(20)
            .build();
        overlay.add_overlay(&check);
    }

    // Episode info with glass effect
    let info_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();

    let title = gtk::Label::builder()
        .label(&episode.title)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .xalign(0.0)
        .css_classes(["subtitle-hero", "caption-heading"])
        .build();

    let duration = episode.duration.as_secs() / 60;
    let details = gtk::Label::builder()
        .label(&format!("{}m", duration))
        .xalign(0.0)
        .css_classes(["dim-label", "caption"])
        .opacity(0.8)
        .build();

    info_box.append(&title);
    info_box.append(&details);

    card.append(&overlay);
    card.append(&info_box);

    card
}
