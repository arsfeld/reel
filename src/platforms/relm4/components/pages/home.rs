use gtk::prelude::*;
use libadwaita as adw;
use relm4::factory::FactoryVecDeque;
use relm4::gtk;
use relm4::prelude::*;
use tracing::{debug, error};

use crate::db::connection::DatabaseConnection;
use crate::db::entities::MediaItemModel;
use crate::models::{HomeSection, MediaItem, MediaItemId};
use crate::platforms::relm4::components::factories::media_card::{
    MediaCard, MediaCardInit, MediaCardOutput,
};
use crate::services::core::BackendService;

#[derive(Debug)]
pub struct HomePage {
    db: DatabaseConnection,
    continue_watching_factory: FactoryVecDeque<MediaCard>,
    recently_added_factory: FactoryVecDeque<MediaCard>,
    is_loading: bool,
}

#[derive(Debug)]
pub enum HomePageInput {
    /// Load home page data
    LoadData,
    /// Home sections loaded from backends
    HomeSectionsLoaded(Vec<HomeSection>),
    /// Continue watching section loaded (fallback)
    ContinueWatchingLoaded(Vec<MediaItemModel>),
    /// Recently added section loaded (fallback)
    RecentlyAddedLoaded(Vec<MediaItemModel>),
    /// Media item selected
    MediaItemSelected(MediaItemId),
}

#[derive(Debug)]
pub enum HomePageOutput {
    /// Navigate to media item
    NavigateToMediaItem(MediaItemId),
}

#[relm4::component(pub async)]
impl AsyncComponent for HomePage {
    type Init = DatabaseConnection;
    type Input = HomePageInput;
    type Output = HomePageOutput;
    type CommandOutput = ();

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 24,
            add_css_class: "background",

            // Header
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_margin_all: 24,
                set_margin_bottom: 0,

                gtk::Label {
                    set_text: "Home",
                    set_halign: gtk::Align::Start,
                    set_hexpand: true,
                    add_css_class: "title-1",
                },
            },

            // Scrollable content
            gtk::ScrolledWindow {
                set_vexpand: true,
                set_hscrollbar_policy: gtk::PolicyType::Never,

                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_margin_all: 24,
                    set_margin_top: 0,
                    set_spacing: 48,

                    // Continue Watching section
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 12,
                        #[watch]
                        set_visible: !model.continue_watching_factory.is_empty(),

                        gtk::Label {
                            set_text: "Continue Watching",
                            set_halign: gtk::Align::Start,
                            add_css_class: "title-2",
                        },

                        gtk::ScrolledWindow {
                            set_hscrollbar_policy: gtk::PolicyType::Automatic,
                            set_vscrollbar_policy: gtk::PolicyType::Never,
                            set_overlay_scrolling: true,

                            #[local_ref]
                            continue_watching_box -> gtk::FlowBox {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_column_spacing: 12,
                                set_min_children_per_line: 1,
                                set_max_children_per_line: 10,
                                set_selection_mode: gtk::SelectionMode::None,
                            },
                        },
                    },

                    // Recently Added section
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 12,
                        #[watch]
                        set_visible: !model.recently_added_factory.is_empty() || model.is_loading,

                        gtk::Label {
                            set_text: "Recently Added",
                            set_halign: gtk::Align::Start,
                            add_css_class: "title-2",
                        },

                        gtk::ScrolledWindow {
                            set_hscrollbar_policy: gtk::PolicyType::Automatic,
                            set_vscrollbar_policy: gtk::PolicyType::Never,
                            set_overlay_scrolling: true,
                            #[watch]
                            set_visible: !model.is_loading,

                            #[local_ref]
                            recently_added_box -> gtk::FlowBox {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_column_spacing: 12,
                                set_min_children_per_line: 1,
                                set_max_children_per_line: 10,
                                set_selection_mode: gtk::SelectionMode::None,
                            },
                        },

                        // Loading indicator
                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: 12,
                            #[watch]
                            set_visible: model.is_loading,

                            // Loading placeholders
                            gtk::Box {
                                set_width_request: 200,
                                set_height_request: 300,
                                add_css_class: "card",

                                gtk::Spinner {
                                    set_spinning: true,
                                    set_size_request: (32, 32),
                                },
                            },

                            gtk::Box {
                                set_width_request: 200,
                                set_height_request: 300,
                                add_css_class: "card",

                                gtk::Spinner {
                                    set_spinning: true,
                                    set_size_request: (32, 32),
                                },
                            },

                            gtk::Box {
                                set_width_request: 200,
                                set_height_request: 300,
                                add_css_class: "card",

                                gtk::Spinner {
                                    set_spinning: true,
                                    set_size_request: (32, 32),
                                },
                            },
                        }
                    },

                    // Empty state - modern design with large icon
                    adw::StatusPage {
                        #[watch]
                        set_visible: !model.is_loading && model.continue_watching_factory.is_empty() && model.recently_added_factory.is_empty(),
                        set_icon_name: Some("folder-videos-symbolic"),
                        set_title: "Welcome to Reel",
                        set_description: Some("Add a source from the sidebar to start watching"),
                        add_css_class: "compact",
                    }
                },
            },
        }
    }

    async fn init(
        db: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let continue_watching_box = gtk::FlowBox::default();
        let recently_added_box = gtk::FlowBox::default();

        let mut continue_watching_factory = FactoryVecDeque::<MediaCard>::builder()
            .launch(continue_watching_box.clone())
            .forward(sender.input_sender(), |output| match output {
                MediaCardOutput::Clicked(id) => HomePageInput::MediaItemSelected(id),
                MediaCardOutput::Play(id) => HomePageInput::MediaItemSelected(id),
            });

        let mut recently_added_factory = FactoryVecDeque::<MediaCard>::builder()
            .launch(recently_added_box.clone())
            .forward(sender.input_sender(), |output| match output {
                MediaCardOutput::Clicked(id) => HomePageInput::MediaItemSelected(id),
                MediaCardOutput::Play(id) => HomePageInput::MediaItemSelected(id),
            });

        let model = Self {
            db,
            continue_watching_factory,
            recently_added_factory,
            is_loading: true,
        };

        let widgets = view_output!();

        // Load initial data
        sender.input(HomePageInput::LoadData);

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        msg: Self::Input,
        sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            HomePageInput::LoadData => {
                debug!("Loading home page data");
                self.is_loading = true;

                // Clone database for async operations
                let db = self.db.clone();
                let db_clone = self.db.clone();
                let sender_clone = sender.clone();

                // Load recently added items
                relm4::spawn(async move {
                    // For now, get the models directly from the repository
                    // TODO: Update command to return MediaItemModel or convert MediaItem
                    use crate::db::repository::{MediaRepository, MediaRepositoryImpl};
                    let repo = MediaRepositoryImpl::new(db.clone());

                    match repo.find_recently_added(20).await {
                        Ok(items) => {
                            debug!("Loaded {} recently added items", items.len());
                            sender_clone.input(HomePageInput::RecentlyAddedLoaded(items));
                        }
                        Err(e) => {
                            error!("Failed to load recently added items: {}", e);
                            sender_clone.input(HomePageInput::RecentlyAddedLoaded(Vec::new()));
                        }
                    }
                });

                // Load continue watching items
                let sender_clone = sender.clone();
                relm4::spawn(async move {
                    use crate::db::repository::{
                        MediaRepository, MediaRepositoryImpl, PlaybackRepository,
                        PlaybackRepositoryImpl, Repository,
                    };

                    let playback_repo = PlaybackRepositoryImpl::new(db_clone.clone());
                    let media_repo = MediaRepositoryImpl::new(db_clone.clone());

                    // Get items with progress
                    match playback_repo.find_in_progress(None).await {
                        Ok(progress_items) => {
                            // Fetch the full media items
                            let mut items = Vec::new();
                            for progress in progress_items.into_iter().take(20) {
                                if let Ok(Some(model)) =
                                    media_repo.find_by_id(&progress.media_id).await
                                {
                                    items.push(model);
                                }
                            }
                            debug!("Loaded {} continue watching items", items.len());
                            sender_clone.input(HomePageInput::ContinueWatchingLoaded(items));
                        }
                        Err(e) => {
                            error!("Failed to load continue watching items: {}", e);
                            sender_clone.input(HomePageInput::ContinueWatchingLoaded(Vec::new()));
                        }
                    }
                });
            }

            HomePageInput::ContinueWatchingLoaded(items) => {
                debug!("Continue watching loaded: {} items", items.len());

                // Clear existing items and add new ones
                self.continue_watching_factory.guard().clear();
                for item in items {
                    self.continue_watching_factory
                        .guard()
                        .push_back(MediaCardInit {
                            item,
                            show_progress: true, // Continue watching shows progress
                            watched: false,
                            progress_percent: 0.0,
                        });
                }

                // Check if we're done loading
                if !self.recently_added_factory.is_empty() || !self.is_loading {
                    self.is_loading = false;
                }
            }

            HomePageInput::RecentlyAddedLoaded(items) => {
                debug!("Recently added loaded: {} items", items.len());

                // Clear existing items and add new ones
                self.recently_added_factory.guard().clear();
                for item in items {
                    self.recently_added_factory
                        .guard()
                        .push_back(MediaCardInit {
                            item,
                            show_progress: false, // Recently added doesn't show progress
                            watched: false,
                            progress_percent: 0.0,
                        });
                }

                self.is_loading = false;
            }

            HomePageInput::MediaItemSelected(item_id) => {
                debug!("Media item selected: {}", item_id);
                sender
                    .output(HomePageOutput::NavigateToMediaItem(item_id))
                    .unwrap();
            }
        }
    }
}
