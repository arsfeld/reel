use crate::models::{Episode, MediaItem, MediaItemId, PlaylistContext, Show, ShowId};
use crate::services::commands::Command;
use crate::services::commands::media_commands::{
    GetEpisodesCommand, GetItemDetailsCommand, MarkSeasonUnwatchedCommand,
    MarkSeasonWatchedCommand, MarkShowUnwatchedCommand, MarkShowWatchedCommand,
    MarkUnwatchedCommand, MarkWatchedCommand,
};
use crate::services::core::PlaylistService;
use crate::ui::shared::broker::{BROKER, BrokerMessage};
use crate::ui::shared::image_helpers::load_image_from_url;
use crate::ui::shared::person_card::create_person_card;
use crate::workers::image_loader::{
    ImageLoader, ImageLoaderInput, ImageLoaderOutput, ImageRequest, ImageSize,
};
use adw::prelude::*;
use libadwaita as adw;
use relm4::RelmWidgetExt;
use relm4::WorkerController;
use relm4::gtk;
use relm4::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::error;

pub struct ShowDetailsPage {
    show: Option<Show>,
    episodes: Vec<Episode>,
    current_season: u32,
    season_numbers: Vec<u32>, // Maps dropdown index to actual season number
    item_id: MediaItemId,
    db: Arc<crate::db::connection::DatabaseConnection>,
    loading: bool,
    episode_grid: gtk::FlowBox,
    season_dropdown: gtk::DropDown,
    cast_box: gtk::Box,
    poster_texture: Option<gtk::gdk::Texture>,
    backdrop_texture: Option<gtk::gdk::Texture>,
    image_loader: WorkerController<ImageLoader>,
    episode_pictures: HashMap<usize, gtk::Picture>,
    episode_popovers: HashMap<usize, gtk::PopoverMenu>,
    person_textures: HashMap<String, gtk::gdk::Texture>,
    full_metadata_loaded: bool,
    // Sync status tracking
    sync_status: crate::ui::shared::sync_status::SyncStatus,
    failed_syncs: Vec<(String, String)>, // (media_item_id, error)
    sync_indicator: gtk::Box,
}

impl std::fmt::Debug for ShowDetailsPage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShowDetailsPage")
            .field("show", &self.show)
            .field("episodes", &self.episodes)
            .field("current_season", &self.current_season)
            .field("item_id", &self.item_id)
            .field("loading", &self.loading)
            .field("episode_pictures_count", &self.episode_pictures.len())
            .finish()
    }
}

#[derive(Debug)]
pub enum ShowDetailsInput {
    LoadShow(MediaItemId),
    SelectSeason(u32), // Season dropdown index (not season number)
    PlayEpisode(MediaItemId),
    ToggleEpisodeWatched(usize),
    ToggleShowWatched,
    ToggleSeasonWatched,
    LoadEpisodes,
    ImageLoaded {
        id: String,
        texture: gtk::gdk::Texture,
    },
    ImageLoadFailed {
        id: String,
    },
    BrokerMsg(crate::ui::shared::broker::BrokerMessage),
}

#[derive(Debug)]
pub enum ShowDetailsOutput {
    PlayMedia(MediaItemId),
    PlayMediaWithContext {
        media_id: MediaItemId,
        context: PlaylistContext,
    },
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
    LoadPersonImage {
        person_id: String,
        url: String,
    },
    PersonImageLoaded {
        person_id: String,
        texture: gtk::gdk::Texture,
    },
    PlayWithContext {
        episode_id: MediaItemId,
        context: PlaylistContext,
    },
    PlayWithoutContext(MediaItemId),
    LoadFullMetadata,
    FullMetadataLoaded,
}

#[allow(unused_assignments)]
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
                    },

                    // Enhanced gradient overlay with glass morphism
                    add_overlay = &gtk::Box {
                        add_css_class: "hero-gradient-modern",
                        set_valign: gtk::Align::End,

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

                            // Show info with overview integrated
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
                                    set_label: &model.show.as_ref().map(|s| s.title.clone()).unwrap_or_default(),
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
                                    set_label: &model.show.as_ref()
                                        .and_then(|s| s.overview.clone())
                                        .unwrap_or_default(),
                                    #[watch]
                                    set_visible: model.show.as_ref().and_then(|s| s.overview.as_ref()).is_some(),
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

                                // Action buttons for show watch status
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 12,
                                    set_margin_top: 16,

                                    gtk::Button {
                                        add_css_class: "pill",
                                        #[watch]
                                        set_tooltip_text: Some(if model.show.as_ref()
                                            .map(|s| s.watched_episode_count == s.total_episode_count && s.total_episode_count > 0)
                                            .unwrap_or(false) {
                                            "Mark all episodes as unwatched"
                                        } else {
                                            "Mark all episodes as watched"
                                        }),

                                        adw::ButtonContent {
                                            #[watch]
                                            set_icon_name: if model.show.as_ref()
                                                .map(|s| s.watched_episode_count == s.total_episode_count && s.total_episode_count > 0)
                                                .unwrap_or(false) {
                                                "view-list-symbolic"
                                            } else {
                                                "media-playlist-consecutive-symbolic"
                                            },
                                            #[watch]
                                            set_label: if model.show.as_ref()
                                                .map(|s| s.watched_episode_count == s.total_episode_count && s.total_episode_count > 0)
                                                .unwrap_or(false) {
                                                "Mark Show as Unwatched"
                                            } else {
                                                "Mark Show as Watched"
                                            },
                                        },

                                        connect_clicked => ShowDetailsInput::ToggleShowWatched,
                                    },

                                    gtk::Button {
                                        add_css_class: "pill",
                                        #[watch]
                                        set_tooltip_text: Some(if model.episodes.iter().filter(|ep| !ep.watched).count() == 0 && !model.episodes.is_empty() {
                                            "Mark this season as unwatched"
                                        } else {
                                            "Mark this season as watched"
                                        }),

                                        adw::ButtonContent {
                                            #[watch]
                                            set_icon_name: if model.episodes.iter().filter(|ep| !ep.watched).count() == 0 && !model.episodes.is_empty() {
                                                "folder-open-symbolic"
                                            } else {
                                                "folder-symbolic"
                                            },
                                            #[watch]
                                            set_label: if model.episodes.iter().filter(|ep| !ep.watched).count() == 0 && !model.episodes.is_empty() {
                                                "Mark Season as Unwatched"
                                            } else {
                                                "Mark Season as Watched"
                                            },
                                        },

                                        connect_clicked => ShowDetailsInput::ToggleSeasonWatched,
                                    },

                                    // Sync status indicator
                                    append: &model.sync_indicator,
                                },

                                // Season selector
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 12,
                                    set_margin_top: 12,

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

                // Content section with episodes prioritized
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_margin_all: 24,
                    set_margin_top: 12,  // Reduce top margin to bring content up
                    set_spacing: 20,

                    // Episodes - moved to top priority
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 12,

                        gtk::Label {
                            set_label: "Episodes",
                            set_halign: gtk::Align::Start,
                            add_css_class: "title-3",
                        },

                        gtk::ScrolledWindow {
                            set_hscrollbar_policy: gtk::PolicyType::Automatic,
                            set_vscrollbar_policy: gtk::PolicyType::Never,
                            set_height_request: 240,  // Slightly increased height for better visibility
                            set_overlay_scrolling: true,

                            set_child: Some(&model.episode_grid),
                        },
                    },

                    // Cast
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 12,
                        #[watch]
                        set_visible: model.show.as_ref().map(|s| !s.cast.is_empty()).unwrap_or(false),

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

                    // Removed redundant overview section since it's now in the hero
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
            .homogeneous(false)
            .selection_mode(gtk::SelectionMode::None)
            .min_children_per_line(1) // Will be updated dynamically based on episode count
            .max_children_per_line(100) // Will be updated dynamically based on episode count
            .valign(gtk::Align::Start)
            .build();

        let season_dropdown = gtk::DropDown::builder()
            .enable_search(false)
            .css_classes(["season-dropdown-styled"])
            .build();

        {
            let sender = sender.clone();
            season_dropdown.connect_selected_notify(move |dropdown| {
                let selected = dropdown.selected();
                // Send the dropdown index directly
                sender.input(ShowDetailsInput::SelectSeason(selected));
            });
        }

        // Create the image loader worker
        let image_loader =
            ImageLoader::builder()
                .detach_worker(())
                .forward(sender.input_sender(), |output| match output {
                    ImageLoaderOutput::ImageLoaded { id, texture, .. } => {
                        ShowDetailsInput::ImageLoaded { id, texture }
                    }
                    ImageLoaderOutput::LoadFailed { id, .. } => {
                        ShowDetailsInput::ImageLoadFailed { id }
                    }
                    ImageLoaderOutput::CacheCleared => {
                        // Not used in this context
                        ShowDetailsInput::LoadEpisodes
                    }
                });

        let cast_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(16)
            .css_classes(["stagger-animation"])
            .build();

        // Create sync status indicator
        let sync_indicator = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        sync_indicator.set_visible(false);

        let model = Self {
            show: None,
            episodes: Vec::new(),
            current_season: 1,
            season_numbers: Vec::new(),
            item_id: init.0.clone(),
            db: init.1,
            loading: true,
            episode_grid,
            season_dropdown,
            cast_box: cast_box.clone(),
            poster_texture: None,
            backdrop_texture: None,
            image_loader,
            episode_pictures: HashMap::new(),
            episode_popovers: HashMap::new(),
            person_textures: HashMap::new(),
            full_metadata_loaded: false,
            sync_status: crate::ui::shared::sync_status::SyncStatus::Idle,
            failed_syncs: Vec::new(),
            sync_indicator,
        };

        let widgets = view_output!();

        // Subscribe to MessageBroker for playback progress updates
        {
            let broker_sender = sender.input_sender().clone();
            relm4::spawn(async move {
                let (tx, rx) = relm4::channel::<BrokerMessage>();
                BROKER.subscribe("ShowDetailsPage".to_string(), tx).await;

                while let Some(msg) = rx.recv().await {
                    broker_sender
                        .send(ShowDetailsInput::BrokerMsg(msg))
                        .unwrap();
                }
            });
        }

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
                self.season_numbers.clear();
                self.loading = true;
                self.poster_texture = None;
                self.backdrop_texture = None;
                sender.oneshot_command(async { ShowDetailsCommand::LoadDetails });
            }
            ShowDetailsInput::SelectSeason(season_index) => {
                // Look up the actual season number from the stored mapping
                if let Some(&season_num) = self.season_numbers.get(season_index as usize) {
                    self.current_season = season_num;
                    if let Some(show) = &self.show {
                        let show_id = show.id.clone();
                        sender.oneshot_command(async move {
                            ShowDetailsCommand::LoadEpisodes(show_id, season_num)
                        });
                    }
                } else {
                    tracing::warn!(
                        "Invalid season index {} (only {} seasons available)",
                        season_index,
                        self.season_numbers.len()
                    );
                }
            }
            ShowDetailsInput::PlayEpisode(episode_id) => {
                // Build playlist context for the episode
                let db_clone = self.db.clone();
                let episode_id_clone = episode_id.clone();

                sender.oneshot_command(async move {
                    // Try to build playlist context for TV show navigation
                    // PlayQueueService::create_from_media already handles Plex PlayQueue creation
                    // and falls back to regular context if not available
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
                if let Some(episode) = self.episodes.get(index) {
                    let db = (*self.db).clone();
                    let media_id = MediaItemId::new(&episode.id);
                    let watched = episode.watched;

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
                            error!("Failed to toggle episode watch status: {}", e);
                        }
                    });
                }
            }
            ShowDetailsInput::ToggleShowWatched => {
                if let Some(show) = &self.show {
                    let db = (*self.db).clone();
                    let show_id = ShowId::new(show.id.clone());
                    let all_watched = show.watched_episode_count == show.total_episode_count
                        && show.total_episode_count > 0;

                    tracing::info!(
                        "ToggleShowWatched clicked: show_id={}, all_watched={}, watched_count={}, total_count={}",
                        show.id,
                        all_watched,
                        show.watched_episode_count,
                        show.total_episode_count
                    );

                    relm4::spawn(async move {
                        let result = if all_watched {
                            // Mark show as unwatched
                            tracing::info!("Executing MarkShowUnwatchedCommand");
                            let cmd = MarkShowUnwatchedCommand { db, show_id };
                            Command::execute(&cmd).await
                        } else {
                            // Mark show as watched
                            tracing::info!("Executing MarkShowWatchedCommand");
                            let cmd = MarkShowWatchedCommand { db, show_id };
                            Command::execute(&cmd).await
                        };

                        if let Err(e) = result {
                            error!("Failed to toggle show watch status: {}", e);
                        } else {
                            tracing::info!("Successfully toggled show watch status");
                        }
                    });
                }
            }
            ShowDetailsInput::ToggleSeasonWatched => {
                if let Some(show) = &self.show {
                    let db = (*self.db).clone();
                    let show_id = ShowId::new(show.id.clone());
                    let season_number = self.current_season;

                    // Determine if current season is fully watched by checking episodes
                    let season_watched = self.episodes.iter().filter(|ep| !ep.watched).count() == 0
                        && !self.episodes.is_empty();

                    tracing::info!(
                        "ToggleSeasonWatched clicked: show_id={}, season={}, season_watched={}, episode_count={}",
                        show.id,
                        season_number,
                        season_watched,
                        self.episodes.len()
                    );

                    relm4::spawn(async move {
                        let result = if season_watched {
                            // Mark season as unwatched
                            tracing::info!(
                                "Executing MarkSeasonUnwatchedCommand for season {}",
                                season_number
                            );
                            let cmd = MarkSeasonUnwatchedCommand {
                                db,
                                show_id,
                                season_number,
                            };
                            Command::execute(&cmd).await
                        } else {
                            // Mark season as watched
                            tracing::info!(
                                "Executing MarkSeasonWatchedCommand for season {}",
                                season_number
                            );
                            let cmd = MarkSeasonWatchedCommand {
                                db,
                                show_id,
                                season_number,
                            };
                            Command::execute(&cmd).await
                        };

                        if let Err(e) = result {
                            error!("Failed to toggle season watch status: {}", e);
                        } else {
                            tracing::info!("Successfully toggled season watch status");
                        }
                    });
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
            ShowDetailsInput::ImageLoaded { id, texture } => {
                // Find the picture widget for this episode
                if let Ok(index) = id.parse::<usize>()
                    && let Some(picture) = self.episode_pictures.get(&index)
                {
                    picture.set_paintable(Some(&texture));
                    picture.remove_css_class("loading");
                }
            }
            ShowDetailsInput::ImageLoadFailed { id } => {
                // Remove loading indicator for failed image
                if let Ok(index) = id.parse::<usize>()
                    && let Some(picture) = self.episode_pictures.get(&index)
                {
                    picture.remove_css_class("loading");
                }
            }
            ShowDetailsInput::BrokerMsg(msg) => match msg {
                BrokerMessage::Data(data_msg) => match data_msg {
                    crate::ui::shared::broker::DataMessage::PlaybackProgressUpdated {
                        media_id,
                        watched,
                    } => {
                        // Check if the updated media is one of the episodes in this show
                        if self.episodes.iter().any(|ep| ep.id.to_string() == media_id) {
                            tracing::debug!(
                                "Episode progress updated for {}: watched={}, reloading episodes",
                                media_id,
                                watched
                            );
                            // Reload episodes to update watch status
                            sender.input(ShowDetailsInput::LoadEpisodes);
                        }
                    }
                    crate::ui::shared::broker::DataMessage::MediaUpdated { media_id } => {
                        // Check if the updated media is this show
                        if self
                            .show
                            .as_ref()
                            .map(|s| s.id == media_id)
                            .unwrap_or(false)
                        {
                            tracing::debug!(
                                "Show/Season updated for {}, reloading show details and episodes",
                                media_id
                            );
                            // Reload show details to update watched counts and episode list
                            sender.oneshot_command(async { ShowDetailsCommand::LoadDetails });
                        }
                    }
                    _ => {}
                },
                BrokerMessage::PlaybackSync(sync_msg) => {
                    use crate::ui::shared::broker::PlaybackSyncMessage;
                    use crate::ui::shared::sync_status::SyncStatus;

                    match sync_msg {
                        PlaybackSyncMessage::SyncStarted { pending_count } => {
                            self.sync_status = SyncStatus::Syncing {
                                count: pending_count,
                            };
                            self.update_sync_indicator();
                        }
                        PlaybackSyncMessage::SyncProgress {
                            synced: _,
                            failed: _,
                            remaining,
                        } => {
                            self.sync_status = SyncStatus::Syncing { count: remaining };
                            self.update_sync_indicator();
                        }
                        PlaybackSyncMessage::SyncCompleted { synced, failed } => {
                            if failed > 0 {
                                // Keep showing failures
                            } else {
                                self.sync_status = SyncStatus::Synced { count: synced };
                                self.update_sync_indicator();
                            }
                        }
                        PlaybackSyncMessage::ItemSyncFailed {
                            media_item_id,
                            error,
                            ..
                        } => {
                            // Track failed items
                            self.failed_syncs
                                .push((media_item_id.clone(), error.clone()));
                            self.sync_status = SyncStatus::Failed {
                                error: error.clone(),
                            };
                            self.update_sync_indicator();
                        }
                        _ => {}
                    }
                }
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
                            // Store the season numbers mapping for dropdown index -> season number
                            self.season_numbers =
                                show.seasons.iter().map(|s| s.season_number).collect();
                            tracing::debug!("Season numbers mapping: {:?}", self.season_numbers);

                            self.show = Some(show.clone());
                            self.loading = false;

                            tracing::info!(
                                "Show loaded: watched_count={}, total_count={}",
                                show.watched_episode_count,
                                show.total_episode_count
                            );

                            // Check if we need to load full cast (if cast count <= 3, likely only preview)
                            // Only attempt once to avoid infinite loop if show really has ≤3 cast members
                            if show.cast.len() <= 3 && !self.full_metadata_loaded {
                                tracing::debug!(
                                    "Show has {} cast members, loading full metadata",
                                    show.cast.len()
                                );
                                sender.oneshot_command(async {
                                    ShowDetailsCommand::LoadFullMetadata
                                });
                            }

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

                            // Update cast cards
                            while let Some(child) = self.cast_box.first_child() {
                                self.cast_box.remove(&child);
                            }

                            for person in show.cast.iter().take(10) {
                                let texture = self.person_textures.get(&person.id).cloned();
                                let card = create_person_card(person, texture.as_ref());
                                self.cast_box.append(&card);

                                // Load image if not already loaded
                                if texture.is_none() {
                                    if let Some(image_url) = &person.image_url {
                                        let person_id = person.id.clone();
                                        let url = image_url.clone();
                                        sender.oneshot_command(async move {
                                            ShowDetailsCommand::LoadPersonImage { person_id, url }
                                        });
                                    }
                                }
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

                            // Note: Episodes will be loaded by the season dropdown's
                            // connect_selected_notify handler, no need to trigger explicitly
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
                    show_id: crate::models::ShowId::new(show_id.clone()),
                    season_number: Some(season_num),
                };

                match Command::execute(&cmd).await {
                    Ok(episodes) => {
                        tracing::debug!(
                            "Loaded {} episodes from database for show {} season {}",
                            episodes.len(),
                            show_id,
                            season_num
                        );

                        // Debug log each episode
                        for (index, episode) in episodes.iter().enumerate() {
                            tracing::debug!(
                                "Episode {}: ID={}, Title='{}', Season={}, Episode={}, Watched={}",
                                index,
                                episode.id,
                                episode.title,
                                episode.season_number,
                                episode.episode_number,
                                episode.watched
                            );
                        }

                        self.episodes = episodes;
                        tracing::debug!(
                            "About to update episode grid with {} episodes",
                            self.episodes.len()
                        );
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
            ShowDetailsCommand::LoadPersonImage { person_id, url } => {
                // Spawn async task to download and create texture
                let sender_clone = sender.clone();
                relm4::spawn(async move {
                    match load_image_from_url(&url, 150, 150).await {
                        Ok(texture) => {
                            sender_clone.oneshot_command(async move {
                                ShowDetailsCommand::PersonImageLoaded { person_id, texture }
                            });
                        }
                        Err(e) => {
                            tracing::error!("Failed to load person image for {}: {}", person_id, e);
                        }
                    }
                });
            }
            ShowDetailsCommand::PersonImageLoaded { person_id, texture } => {
                // Store the loaded texture
                self.person_textures.insert(person_id.clone(), texture);

                // Recreate cast cards with the new texture
                if let Some(show) = &self.show {
                    // Update cast cards
                    while let Some(child) = self.cast_box.first_child() {
                        self.cast_box.remove(&child);
                    }

                    for person in show.cast.iter().take(10) {
                        let texture = self.person_textures.get(&person.id).cloned();
                        let card = create_person_card(person, texture.as_ref());
                        self.cast_box.append(&card);
                    }
                }
            }
            ShowDetailsCommand::LoadFullMetadata => {
                use crate::services::commands::media_commands::LoadFullShowMetadataCommand;

                tracing::info!("Loading full metadata for show {}", self.item_id);
                let cmd = LoadFullShowMetadataCommand {
                    db: (*self.db).clone(),
                    show_id: crate::models::ShowId::new(self.item_id.to_string()),
                };

                match Command::execute(&cmd).await {
                    Ok(_) => {
                        tracing::info!("Full metadata loaded, refreshing show details");
                        sender.oneshot_command(async { ShowDetailsCommand::FullMetadataLoaded });
                    }
                    Err(e) => {
                        tracing::error!("Failed to load full metadata: {}", e);
                    }
                }
            }
            ShowDetailsCommand::FullMetadataLoaded => {
                // Mark that we've loaded full metadata to prevent infinite loop
                self.full_metadata_loaded = true;

                // Directly reload cast from database without full page refresh
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
                        // Separate cast based on person_type
                        let mut cast = Vec::new();

                        for (person, media_person) in people_with_relations {
                            let person_obj = crate::models::Person {
                                id: person.id.clone(),
                                name: person.name.clone(),
                                role: media_person.role.clone(),
                                image_url: person.image_url.clone(),
                            };

                            match media_person.person_type.as_str() {
                                "actor" | "cast" => cast.push(person_obj),
                                _ => {}
                            }
                        }

                        // Update the show's cast in place
                        if let Some(show) = &mut self.show {
                            show.cast = cast.clone();
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
                                        ShowDetailsCommand::LoadPersonImage { person_id, url }
                                    });
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to reload cast after metadata load: {}", e);
                    }
                }
            }
        }
    }

    fn shutdown(&mut self, _widgets: &mut Self::Widgets, _output: relm4::Sender<Self::Output>) {
        // Unsubscribe from MessageBroker
        relm4::spawn(async move {
            BROKER.unsubscribe("ShowDetailsPage").await;
        });
    }
}

impl ShowDetailsPage {
    fn update_episode_grid(&mut self, sender: &AsyncComponentSender<Self>) {
        tracing::debug!(
            "update_episode_grid called with {} episodes",
            self.episodes.len()
        );

        // Clear existing children and unparent popovers to prevent GTK warnings
        let mut child_count = 0;
        while let Some(child) = self.episode_grid.first_child() {
            self.episode_grid.remove(&child);
            child_count += 1;
        }
        tracing::debug!("Cleared {} existing children from grid", child_count);

        // Unparent all popovers before clearing
        for (_, popover) in self.episode_popovers.drain() {
            popover.unparent();
        }

        self.episode_pictures.clear();

        // Dynamically adjust FlowBox to show all episodes in a single row
        // This matches the behavior of the homepage sections
        let episode_count = self.episodes.len() as u32;
        if episode_count > 0 {
            self.episode_grid.set_min_children_per_line(episode_count);
            self.episode_grid.set_max_children_per_line(episode_count);
            tracing::debug!(
                "Set episode grid to display {} episodes in single row",
                episode_count
            );
        }

        // Add episode cards
        for (index, episode) in self.episodes.iter().enumerate() {
            let (card, picture, popover) = create_episode_card(episode, index, sender.clone());
            self.episode_grid.append(&card);
            tracing::debug!(
                "Added episode card {} to grid: '{}' (S{}E{})",
                index,
                episode.title,
                episode.season_number,
                episode.episode_number
            );

            // Store picture and popover references for later updates and cleanup
            self.episode_pictures.insert(index, picture.clone());
            self.episode_popovers.insert(index, popover);

            // Send image load request to the worker
            if let Some(thumbnail_url) = &episode.thumbnail_url {
                let _ =
                    self.image_loader
                        .sender()
                        .send(ImageLoaderInput::LoadImage(ImageRequest {
                            id: index.to_string(),
                            url: thumbnail_url.clone(),
                            size: ImageSize::Custom(240, 135),
                            priority: index as u8, // Earlier episodes have higher priority
                        }));
            }
        }

        // Scroll to the first unwatched episode
        if let Some(first_unwatched_index) = self.episodes.iter().position(|e| !e.watched)
            && let Some(child) = self
                .episode_grid
                .child_at_index(first_unwatched_index as i32)
        {
            // Focus on the first unwatched episode for better visibility
            child.grab_focus();
        }

        // Final check: ensure the grid has children and is visible
        let final_child_count = {
            let mut count = 0;
            let mut child = self.episode_grid.first_child();
            while let Some(c) = child {
                count += 1;
                child = c.next_sibling();
            }
            count
        };
        tracing::debug!(
            "Episode grid update complete: {} children in grid, is_visible={}, is_realized={}",
            final_child_count,
            self.episode_grid.is_visible(),
            self.episode_grid.is_realized()
        );
    }

    fn update_sync_indicator(&mut self) {
        use crate::ui::shared::sync_status::create_sync_status_indicator;

        // Clear existing children
        while let Some(child) = self.sync_indicator.first_child() {
            self.sync_indicator.remove(&child);
        }

        // Create new indicator with current status
        let new_indicator = create_sync_status_indicator(&self.sync_status, true);

        // Copy children from the new indicator to our stored widget
        while let Some(child) = new_indicator.first_child() {
            new_indicator.remove(&child);
            self.sync_indicator.append(&child);
        }

        // Update visibility based on status
        self.sync_indicator.set_visible(!matches!(
            self.sync_status,
            crate::ui::shared::sync_status::SyncStatus::Idle
        ));
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
) -> (gtk::Box, gtk::Picture, gtk::PopoverMenu) {
    let card = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .width_request(240)
        .css_classes(["episode-card-minimal"])
        .build();

    // Make the card clickable
    let click_controller = gtk::GestureClick::new();
    let episode_id = MediaItemId::new(&episode.id);
    let sender_clone = sender.clone();
    click_controller.connect_released(move |_, _, _, _| {
        sender_clone.input(ShowDetailsInput::PlayEpisode(episode_id.clone()));
    });
    card.add_controller(click_controller);

    // Create context menu
    let menu = gtk::gio::Menu::new();

    // Add "Play Episode" action
    menu.append(Some("Play Episode"), Some("episode.play"));

    // Add watch status toggle
    if episode.watched {
        menu.append(Some("Mark as Unwatched"), Some("episode.mark_unwatched"));
    } else {
        menu.append(Some("Mark as Watched"), Some("episode.mark_watched"));
    }

    // Create popover menu
    let popover = gtk::PopoverMenu::from_model(Some(&menu));
    popover.set_parent(&card);
    popover.set_has_arrow(false);

    // Create action group for menu actions
    let action_group = gtk::gio::SimpleActionGroup::new();

    // Play action
    let play_action = gtk::gio::SimpleAction::new("play", None);
    let sender_clone = sender.clone();
    let episode_id_clone = MediaItemId::new(&episode.id);
    play_action.connect_activate(move |_, _| {
        sender_clone.input(ShowDetailsInput::PlayEpisode(episode_id_clone.clone()));
    });
    action_group.add_action(&play_action);

    // Mark Watched action
    let mark_watched_action = gtk::gio::SimpleAction::new("mark_watched", None);
    let sender_clone = sender.clone();
    mark_watched_action.connect_activate(move |_, _| {
        sender_clone.input(ShowDetailsInput::ToggleEpisodeWatched(index));
    });
    action_group.add_action(&mark_watched_action);

    // Mark Unwatched action
    let mark_unwatched_action = gtk::gio::SimpleAction::new("mark_unwatched", None);
    let sender_clone = sender.clone();
    mark_unwatched_action.connect_activate(move |_, _| {
        sender_clone.input(ShowDetailsInput::ToggleEpisodeWatched(index));
    });
    action_group.add_action(&mark_unwatched_action);

    // Insert action group into the card
    card.insert_action_group("episode", Some(&action_group));

    // Add right-click gesture
    let right_click = gtk::GestureClick::new();
    right_click.set_button(3); // Right mouse button
    let popover_clone = popover.clone();
    right_click.connect_released(move |gesture, _, x, y| {
        // Get the widget that was clicked
        if let Some(_widget) = gesture.widget() {
            // Calculate the position relative to the widget
            let rect = gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1);
            popover_clone.set_pointing_to(Some(&rect));
            popover_clone.popup();
        }
    });
    card.add_controller(right_click);

    // Add hover effects
    card.set_cursor_from_name(Some("pointer"));

    // Episode thumbnail with number overlay
    let overlay = gtk::Overlay::builder()
        .css_classes(["episode-thumbnail-container"])
        .build();

    let picture = gtk::Picture::builder()
        .width_request(240)
        .height_request(135)
        .content_fit(gtk::ContentFit::Cover)
        .css_classes(["episode-thumbnail-image"])
        .build();

    // Set a placeholder background color while loading
    picture.add_css_class("loading");

    overlay.set_child(Some(&picture));

    // Episode number badge - subtle and minimal
    let badge = gtk::Label::builder()
        .label(format!("E{}", episode.episode_number))
        .css_classes(["episode-number-badge"])
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Start)
        .margin_top(8)
        .margin_start(8)
        .build();
    overlay.add_overlay(&badge);

    // Modern progress bar if partially watched
    if let Some(position) = episode.playback_position
        && position.as_secs() > 0
        && !episode.watched
    {
        let progress_container = gtk::Box::builder()
            .css_classes(["episode-progress-container"])
            .valign(gtk::Align::End)
            .build();

        let progress = gtk::Box::builder()
            .css_classes(["episode-progress-bar"])
            .width_request((240.0 * position.as_secs_f64() / episode.duration.as_secs_f64()) as i32)
            .build();

        progress_container.append(&progress);
        overlay.add_overlay(&progress_container);
    }

    // New episode indicator (unwatched) OR watched check
    if episode.watched {
        let check = gtk::Image::builder()
            .icon_name("object-select-symbolic")
            .css_classes(["episode-watched-check"])
            .halign(gtk::Align::End)
            .valign(gtk::Align::Start)
            .margin_top(8)
            .margin_end(8)
            .pixel_size(16)
            .build();
        overlay.add_overlay(&check);
    } else {
        // New episode indicator - white glow dot
        let new_indicator = gtk::Box::builder()
            .css_classes(["episode-new-indicator"])
            .halign(gtk::Align::End)
            .valign(gtk::Align::Start)
            .margin_top(8)
            .margin_end(8)
            .width_request(10)
            .height_request(10)
            .build();
        overlay.add_overlay(&new_indicator);
    }

    // Episode info - minimal and clean
    let info_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(4)
        .margin_end(4)
        .build();

    let title = gtk::Label::builder()
        .label(&episode.title)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .xalign(0.0)
        .css_classes(["episode-title", "body"])
        .build();

    let duration = episode.duration.as_secs() / 60;
    let details = gtk::Label::builder()
        .label(format!("{}m", duration))
        .xalign(0.0)
        .css_classes(["episode-duration", "dim-label", "caption"])
        .build();

    info_box.append(&title);
    info_box.append(&details);

    // Add episode description if available
    if let Some(overview) = &episode.overview {
        let description = gtk::Label::builder()
            .label(overview)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::Word)
            .lines(2)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .xalign(0.0)
            .css_classes(["episode-description", "dim-label", "caption"])
            .margin_top(4)
            .build();
        info_box.append(&description);
    }

    card.append(&overlay);
    card.append(&info_box);

    (card, picture, popover)
}
