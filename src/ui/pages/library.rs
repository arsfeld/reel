use gtk::prelude::*;
use libadwaita as adw;
use relm4::factory::FactoryVecDeque;
use relm4::gtk;
use relm4::prelude::*;
use std::time::{Duration, Instant};
use tracing::{debug, error, trace};

use crate::db::connection::DatabaseConnection;
use crate::db::entities::MediaItemModel;
use crate::models::{LibraryId, MediaItemId};
use crate::ui::factories::media_card::{MediaCard, MediaCardInit, MediaCardInput, MediaCardOutput};
use crate::ui::shared::broker::{BROKER, BrokerMessage};
use crate::workers::{ImageLoader, ImageLoaderInput, ImageLoaderOutput, ImageRequest, ImageSize};

impl std::fmt::Debug for LibraryPage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryPage")
            .field("library_id", &self.library_id)
            .field("is_loading", &self.is_loading)
            .field("loaded_count", &self.loaded_count)
            .field("batch_size", &self.batch_size)
            .field("has_loaded_all", &self.has_loaded_all)
            .field("sort_by", &self.sort_by)
            .field("filter_text", &self.filter_text)
            .field("search_visible", &self.search_visible)
            .finish()
    }
}

pub struct LibraryPage {
    db: DatabaseConnection,
    library_id: Option<LibraryId>,
    media_factory: FactoryVecDeque<MediaCard>,
    image_loader: relm4::WorkerController<ImageLoader>,
    image_requests: std::collections::HashMap<String, usize>,
    is_loading: bool,
    loaded_count: usize,
    batch_size: usize,
    total_items: Vec<MediaItemModel>,
    has_loaded_all: bool,
    sort_by: SortBy,
    sort_order: SortOrder,
    filter_text: String,
    search_visible: bool,
    // Genre filtering
    selected_genres: Vec<String>,
    available_genres: Vec<String>,
    genre_popover: Option<gtk::Popover>,
    genre_menu_button: Option<gtk::MenuButton>,
    genre_label_text: String,
    // Year range filtering
    min_year: Option<i32>,
    max_year: Option<i32>,
    selected_min_year: Option<i32>,
    selected_max_year: Option<i32>,
    year_popover: Option<gtk::Popover>,
    year_menu_button: Option<gtk::MenuButton>,
    // Rating filtering
    min_rating: Option<f32>,
    rating_popover: Option<gtk::Popover>,
    rating_menu_button: Option<gtk::MenuButton>,
    // Watch status filtering
    watch_status_filter: WatchStatus,
    watch_status_popover: Option<gtk::Popover>,
    watch_status_menu_button: Option<gtk::MenuButton>,
    // Media type filtering (for mixed libraries)
    library_type: Option<String>, // 'movies', 'shows', 'music', 'photos', 'mixed'
    selected_media_type: Option<String>, // Filter for mixed libraries
    media_type_buttons: Vec<gtk::ToggleButton>,
    // Viewport tracking
    visible_start_idx: usize,
    visible_end_idx: usize,
    // Scroll debouncing
    last_scroll_time: Option<Instant>,
    scroll_debounce_handle: Option<gtk::glib::SourceId>,
    // Image loading state
    images_requested: std::collections::HashSet<String>, // Track which images have been requested
    pending_image_cancels: Vec<String>,
    // Handler IDs for cleanup
    scroll_handler_id: Option<gtk::glib::SignalHandlerId>,
    // Filter panel visibility
    filter_panel_visible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortBy {
    Title,
    Year,
    DateAdded,
    Rating,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortOrder {
    Ascending,
    Descending,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WatchStatus {
    All,
    Watched,
    Unwatched,
}

#[derive(Debug)]
pub enum LibraryPageInput {
    /// Set the library to display
    SetLibrary(LibraryId),
    /// Load more items into view
    LoadMoreBatch,
    /// All media items loaded from database
    AllItemsLoaded {
        items: Vec<MediaItemModel>,
        library_type: Option<String>,
    },
    /// Render next batch of items
    RenderBatch,
    /// Media item selected
    MediaItemSelected(MediaItemId),
    /// Change sort order
    SetSortBy(SortBy),
    /// Toggle sort order (ascending/descending)
    ToggleSortOrder,
    /// Filter by text
    SetFilter(String),
    /// Toggle genre filter
    ToggleGenreFilter(String),
    /// Clear all genre filters
    ClearGenreFilters,
    /// Set year range filter
    SetYearRange { min: Option<i32>, max: Option<i32> },
    /// Clear year range filter
    ClearYearRange,
    /// Set rating filter (minimum rating threshold)
    SetRatingFilter(Option<f32>),
    /// Clear rating filter
    ClearRatingFilter,
    /// Set watch status filter
    SetWatchStatusFilter(WatchStatus),
    /// Clear watch status filter
    ClearWatchStatusFilter,
    /// Set media type filter (for mixed libraries)
    SetMediaTypeFilter(Option<String>),
    /// Clear all items and reload
    Refresh,
    /// Show search bar
    ShowSearch,
    /// Hide search bar
    HideSearch,
    /// Toggle filter panel visibility
    ToggleFilterPanel,
    /// Clear all filters
    ClearAllFilters,
    /// Image loaded from worker
    ImageLoaded {
        id: String,
        texture: gtk::gdk::Texture,
    },
    /// Image load failed
    ImageLoadFailed { id: String },
    /// Viewport scrolled, update visible range
    ViewportScrolled,
    /// Process debounced scroll event
    ProcessDebouncedScroll,
    /// Load images for visible items
    LoadVisibleImages,
    /// Message broker messages
    BrokerMsg(BrokerMessage),
}

#[derive(Debug)]
pub enum LibraryPageOutput {
    /// Navigate to media item
    NavigateToMediaItem(MediaItemId),
}

#[relm4::component(pub async)]
impl AsyncComponent for LibraryPage {
    type Init = DatabaseConnection;
    type Input = LibraryPageInput;
    type Output = LibraryPageOutput;
    type CommandOutput = ();

    view! {
        gtk::Overlay {
            set_can_focus: true,
            grab_focus: (),

            // Add keyboard event controller to capture typing
            add_controller = gtk::EventControllerKey {
                connect_key_pressed[sender] => move |_, key, _, _| {
                    // Show search on slash or Control+F
                    if key == gtk::gdk::Key::slash || key == gtk::gdk::Key::f {
                        sender.input(LibraryPageInput::ShowSearch);
                        gtk::glib::Propagation::Stop
                    }
                    // Hide search on Escape
                    else if key == gtk::gdk::Key::Escape {
                        sender.input(LibraryPageInput::HideSearch);
                        gtk::glib::Propagation::Stop
                    } else {
                        gtk::glib::Propagation::Proceed
                    }
                }
            },

            // Main content box
            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 0,

                // Toolbar with sort controls
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 12,
                    set_margin_all: 12,
                    set_halign: gtk::Align::Start,

                    // Filter panel toggle button with badge
                    gtk::Button {
                        set_icon_name: "funnel-symbolic",
                        set_tooltip_text: Some("Show filters"),
                        add_css_class: "flat",
                        #[watch]
                        set_label: &{
                            let count = model.get_active_filter_count();
                            if count > 0 {
                                format!(" {}", count)
                            } else {
                                String::new()
                            }
                        },
                        connect_clicked[sender] => move |_| {
                            sender.input(LibraryPageInput::ToggleFilterPanel);
                        }
                    },

                    gtk::Separator {
                        set_orientation: gtk::Orientation::Vertical,
                    },

                    gtk::Label {
                        set_text: "Sort by:",
                    },

                    gtk::DropDown {
                        set_model: Some(&gtk::StringList::new(&[
                            "Title",
                            "Year",
                            "Date Added",
                            "Rating",
                        ])),
                        #[watch]
                        set_selected: match model.sort_by {
                            SortBy::Title => 0,
                            SortBy::Year => 1,
                            SortBy::DateAdded => 2,
                            SortBy::Rating => 3,
                        },
                        connect_selected_notify[sender] => move |dropdown| {
                            let sort_by = match dropdown.selected() {
                                0 => SortBy::Title,
                                1 => SortBy::Year,
                                2 => SortBy::DateAdded,
                                3 => SortBy::Rating,
                                _ => SortBy::Title,
                            };
                            sender.input(LibraryPageInput::SetSortBy(sort_by));
                        }
                    },

                    // Sort order toggle button
                    gtk::Button {
                        #[watch]
                        set_icon_name: if model.sort_order == SortOrder::Ascending {
                            "view-sort-ascending-symbolic"
                        } else {
                            "view-sort-descending-symbolic"
                        },
                        set_tooltip_text: Some("Toggle sort order"),
                        add_css_class: "flat",
                        connect_clicked[sender] => move |_| {
                            sender.input(LibraryPageInput::ToggleSortOrder);
                        }
                    },

                    // Add search button
                    gtk::Button {
                        set_icon_name: "system-search-symbolic",
                        set_tooltip_text: Some("Search (/)"),
                        add_css_class: "flat",
                        connect_clicked[sender] => move |_| {
                            sender.input(LibraryPageInput::ShowSearch);
                        }
                    },
                },

                // Media type filter buttons (only visible for mixed libraries)
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 6,
                    set_margin_start: 12,
                    set_margin_end: 12,
                    set_margin_bottom: 6,
                    #[watch]
                    set_visible: model.library_type.as_ref().is_some_and(|t| t == "mixed"),
                    add_css_class: "linked",

                    gtk::ToggleButton {
                        set_label: "All",
                        #[watch]
                        set_active: model.selected_media_type.is_none(),
                        connect_toggled[sender] => move |btn| {
                            if btn.is_active() {
                                sender.input(LibraryPageInput::SetMediaTypeFilter(None));
                            }
                        }
                    },

                    gtk::ToggleButton {
                        set_label: "Movies",
                        #[watch]
                        set_active: model.selected_media_type.as_ref().is_some_and(|t| t == "movie"),
                        connect_toggled[sender] => move |btn| {
                            if btn.is_active() {
                                sender.input(LibraryPageInput::SetMediaTypeFilter(Some("movie".to_string())));
                            }
                        }
                    },

                    gtk::ToggleButton {
                        set_label: "Shows",
                        #[watch]
                        set_active: model.selected_media_type.as_ref().is_some_and(|t| t == "show"),
                        connect_toggled[sender] => move |btn| {
                            if btn.is_active() {
                                sender.input(LibraryPageInput::SetMediaTypeFilter(Some("show".to_string())));
                            }
                        }
                    },

                    gtk::ToggleButton {
                        set_label: "Music",
                        #[watch]
                        set_active: model.selected_media_type.as_ref().is_some_and(|t| t == "album"),
                        connect_toggled[sender] => move |btn| {
                            if btn.is_active() {
                                sender.input(LibraryPageInput::SetMediaTypeFilter(Some("album".to_string())));
                            }
                        }
                    },

                    gtk::ToggleButton {
                        set_label: "Photos",
                        #[watch]
                        set_active: model.selected_media_type.as_ref().is_some_and(|t| t == "photo"),
                        connect_toggled[sender] => move |btn| {
                            if btn.is_active() {
                                sender.input(LibraryPageInput::SetMediaTypeFilter(Some("photo".to_string())));
                            }
                        }
                    },
                },

                // Main content area with filter panel and media grid
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 0,
                    set_vexpand: true,

                    // Filter panel sidebar
                    gtk::Revealer {
                        set_transition_type: gtk::RevealerTransitionType::SlideRight,
                        set_transition_duration: 200,
                        #[watch]
                        set_reveal_child: model.filter_panel_visible,

                        gtk::ScrolledWindow {
                            set_hscrollbar_policy: gtk::PolicyType::Never,
                            set_vscrollbar_policy: gtk::PolicyType::Automatic,
                            set_width_request: 280,

                            gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_spacing: 12,
                                set_margin_all: 16,

                                // Header with title and clear all button
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 12,

                                    gtk::Label {
                                        set_label: "Filters",
                                        set_halign: gtk::Align::Start,
                                        set_hexpand: true,
                                        add_css_class: "title-3",
                                    },

                                    gtk::Button {
                                        set_label: "Clear All",
                                        add_css_class: "flat",
                                        #[watch]
                                        set_sensitive: model.get_active_filter_count() > 0,
                                        connect_clicked[sender] => move |_| {
                                            sender.input(LibraryPageInput::ClearAllFilters);
                                        }
                                    },
                                },

                                gtk::Separator {
                                    set_orientation: gtk::Orientation::Horizontal,
                                },

                                // Genre filter section
                                #[name = "genre_section"]
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_spacing: 8,
                                    #[watch]
                                    set_visible: !model.available_genres.is_empty(),

                                    gtk::Label {
                                        set_label: "Genre",
                                        set_halign: gtk::Align::Start,
                                        add_css_class: "heading",
                                    },

                                    #[name = "genre_menu_button"]
                                    gtk::MenuButton {
                                        set_label: &model.get_genre_label(),
                                        set_always_show_arrow: true,
                                    },
                                },

                                // Year range filter section
                                #[name = "year_section"]
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_spacing: 8,
                                    #[watch]
                                    set_visible: model.min_year.is_some() && model.max_year.is_some(),

                                    gtk::Label {
                                        set_label: "Year",
                                        set_halign: gtk::Align::Start,
                                        add_css_class: "heading",
                                    },

                                    #[name = "year_menu_button"]
                                    gtk::MenuButton {
                                        set_label: &model.get_year_label(),
                                        set_always_show_arrow: true,
                                    },
                                },

                                // Rating filter section
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_spacing: 8,

                                    gtk::Label {
                                        set_label: "Rating",
                                        set_halign: gtk::Align::Start,
                                        add_css_class: "heading",
                                    },

                                    #[name = "rating_menu_button"]
                                    gtk::MenuButton {
                                        set_label: &model.get_rating_label(),
                                        set_always_show_arrow: true,
                                    },
                                },

                                // Watch status filter section
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_spacing: 8,

                                    gtk::Label {
                                        set_label: "Watch Status",
                                        set_halign: gtk::Align::Start,
                                        add_css_class: "heading",
                                    },

                                    #[name = "watch_status_menu_button"]
                                    gtk::MenuButton {
                                        set_label: &model.get_watch_status_label(),
                                        set_always_show_arrow: true,
                                    },
                                },
                            }
                        }
                    },

                    gtk::Separator {
                        set_orientation: gtk::Orientation::Vertical,
                        #[watch]
                        set_visible: model.filter_panel_visible,
                    },

                    // Scrolled window with media content
                    #[name = "scrolled_window"]
                    gtk::ScrolledWindow {
                    set_vexpand: true,
                    set_hscrollbar_policy: gtk::PolicyType::Never,
                    set_vscrollbar_policy: gtk::PolicyType::Automatic,

                    // Connect to edge-reached signal for infinite scrolling
                    connect_edge_reached[sender] => move |_, pos| {
                        if pos == gtk::PositionType::Bottom {
                            sender.input(LibraryPageInput::LoadMoreBatch);
                        }
                    },

                    // Track scroll position changes for viewport-based loading
                    #[wrap(Some)]
                    set_vadjustment = &gtk::Adjustment {},  // Handler will be connected after init

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 0,

                        #[local_ref]
                        media_box -> gtk::FlowBox {
                            set_column_spacing: 12,  // Tighter grid spacing
                            set_row_spacing: 16,      // Reduced vertical spacing
                            set_homogeneous: true,
                            set_min_children_per_line: 4,   // More items per row with smaller sizes
                            set_max_children_per_line: 12,  // Allow more on wide screens
                            set_selection_mode: gtk::SelectionMode::None,
                            set_margin_top: 24,
                            set_margin_bottom: 16,
                            set_margin_start: 16,
                            set_margin_end: 16,
                            set_valign: gtk::Align::Start,
                        },

                        // Loading indicator at bottom
                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_halign: gtk::Align::Center,
                            set_margin_all: 12,
                            #[watch]
                            set_visible: model.is_loading && model.loaded_count > 0,

                            gtk::Spinner {
                                set_spinning: true,
                            },

                            gtk::Label {
                                set_text: "Loading more...",
                                set_margin_start: 12,
                                add_css_class: "dim-label",
                            },
                        },

                        // Empty state - modern design with large icon
                        adw::StatusPage {
                            #[watch]
                            set_visible: !model.is_loading && model.media_factory.is_empty(),
                            set_icon_name: Some("folder-videos-symbolic"),
                            set_title: "No Media Found",
                            set_description: Some("This library is empty or still syncing"),
                            add_css_class: "compact",
                        }
                    },
                }
                }
            },

            // Floating search bar overlay
            add_overlay = &gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_halign: gtk::Align::Center,
                set_valign: gtk::Align::Start,
                set_margin_top: 12,
                #[watch]
                set_visible: model.search_visible,
                add_css_class: "osd",
                add_css_class: "toolbar",
                set_css_classes: &["osd", "toolbar", "floating-search"],

                #[name = "search_entry"]
                gtk::SearchEntry {
                    set_placeholder_text: Some("Type to search..."),
                    set_width_request: 350,
                    #[watch]
                    set_text: &model.filter_text,
                    connect_search_changed[sender] => move |entry| {
                        sender.input(LibraryPageInput::SetFilter(entry.text().to_string()));
                    },
                    connect_stop_search[sender] => move |_| {
                        sender.input(LibraryPageInput::HideSearch);
                    }
                },
            }
        }
    }

    async fn init(
        db: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let media_box = gtk::FlowBox::default();

        let media_factory = FactoryVecDeque::<MediaCard>::builder()
            .launch(media_box.clone())
            .forward(sender.input_sender(), |output| match output {
                MediaCardOutput::Clicked(id) => LibraryPageInput::MediaItemSelected(id),
                MediaCardOutput::Play(id) => LibraryPageInput::MediaItemSelected(id),
            });

        // Create the image loader worker
        let image_loader =
            ImageLoader::builder()
                .detach_worker(())
                .forward(sender.input_sender(), |output| match output {
                    ImageLoaderOutput::ImageLoaded { id, texture, .. } => {
                        LibraryPageInput::ImageLoaded { id, texture }
                    }
                    ImageLoaderOutput::LoadFailed { id, .. } => {
                        LibraryPageInput::ImageLoadFailed { id }
                    }
                    ImageLoaderOutput::CacheCleared => {
                        // Ignore cache cleared events for now
                        LibraryPageInput::Refresh
                    }
                });

        let model = Self {
            db,
            library_id: None,
            media_factory,
            image_loader,
            image_requests: std::collections::HashMap::new(),
            is_loading: false,
            loaded_count: 0,
            batch_size: 50, // Number of items to render at once
            total_items: Vec::new(),
            has_loaded_all: false,
            sort_by: SortBy::Title,
            sort_order: SortOrder::Ascending,
            filter_text: String::new(),
            search_visible: false,
            // Genre filtering
            selected_genres: Vec::new(),
            available_genres: Vec::new(),
            genre_popover: None,
            genre_menu_button: None,
            genre_label_text: String::new(),
            // Year range filtering
            min_year: None,
            max_year: None,
            selected_min_year: None,
            selected_max_year: None,
            year_popover: None,
            year_menu_button: None,
            // Rating filtering
            min_rating: None,
            rating_popover: None,
            rating_menu_button: None,
            // Watch status filtering
            watch_status_filter: WatchStatus::All,
            watch_status_popover: None,
            watch_status_menu_button: None,
            // Media type filtering (for mixed libraries)
            library_type: None,
            selected_media_type: None,
            media_type_buttons: Vec::new(),
            // Viewport tracking
            visible_start_idx: 0,
            visible_end_idx: 0,
            // Scroll debouncing
            last_scroll_time: None,
            scroll_debounce_handle: None,
            // Image loading state
            images_requested: std::collections::HashSet::new(),
            pending_image_cancels: Vec::new(),
            // Handler IDs for cleanup
            scroll_handler_id: None,
            // Filter panel visibility
            filter_panel_visible: false,
        };

        let mut model = model;

        let widgets = view_output!();

        // Subscribe to MessageBroker for config updates
        {
            let broker_sender = sender.input_sender().clone();
            relm4::spawn(async move {
                let (tx, rx) = relm4::channel::<BrokerMessage>();
                BROKER.subscribe("LibraryPage".to_string(), tx).await;

                while let Some(msg) = rx.recv().await {
                    broker_sender
                        .send(LibraryPageInput::BrokerMsg(msg))
                        .unwrap();
                }
            });
        }

        // Create and set the genre filter popover
        let genre_popover = gtk::Popover::new();
        genre_popover.set_child(Some(&gtk::Box::new(gtk::Orientation::Vertical, 0)));
        widgets.genre_menu_button.set_popover(Some(&genre_popover));
        widgets
            .genre_menu_button
            .set_label(&model.get_genre_label());
        model.genre_popover = Some(genre_popover);
        model.genre_menu_button = Some(widgets.genre_menu_button.clone());

        // Create and set the year range filter popover
        let year_popover = gtk::Popover::new();
        year_popover.set_child(Some(&gtk::Box::new(gtk::Orientation::Vertical, 0)));
        widgets.year_menu_button.set_popover(Some(&year_popover));
        widgets.year_menu_button.set_label(&model.get_year_label());
        model.year_popover = Some(year_popover);
        model.year_menu_button = Some(widgets.year_menu_button.clone());

        // Create and set the rating filter popover
        let rating_popover = gtk::Popover::new();
        rating_popover.set_child(Some(&gtk::Box::new(gtk::Orientation::Vertical, 0)));
        widgets
            .rating_menu_button
            .set_popover(Some(&rating_popover));
        widgets
            .rating_menu_button
            .set_label(&model.get_rating_label());
        model.rating_popover = Some(rating_popover);
        model.rating_menu_button = Some(widgets.rating_menu_button.clone());

        // Create and set the watch status filter popover
        let watch_status_popover = gtk::Popover::new();
        watch_status_popover.set_child(Some(&gtk::Box::new(gtk::Orientation::Vertical, 0)));
        widgets
            .watch_status_menu_button
            .set_popover(Some(&watch_status_popover));
        widgets
            .watch_status_menu_button
            .set_label(&model.get_watch_status_label());
        model.watch_status_popover = Some(watch_status_popover);
        model.watch_status_menu_button = Some(widgets.watch_status_menu_button.clone());

        // Connect scroll handler and store the ID
        let sender_for_scroll = sender.clone();
        let adjustment = widgets.scrolled_window.vadjustment();
        let scroll_handler_id = adjustment.connect_value_changed(move |_| {
            sender_for_scroll.input(LibraryPageInput::ViewportScrolled);
        });
        model.scroll_handler_id = Some(scroll_handler_id);

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        msg: Self::Input,
        sender: AsyncComponentSender<Self>,
        root: &Self::Root,
    ) {
        match msg {
            LibraryPageInput::SetLibrary(library_id) => {
                debug!("Setting library: {}", library_id);
                self.library_id = Some(library_id.clone());
                self.loaded_count = 0;
                self.total_items.clear();
                self.has_loaded_all = false;
                self.media_factory.guard().clear();
                self.image_requests.clear();
                self.images_requested.clear();
                self.visible_start_idx = 0;
                self.visible_end_idx = 0;
                self.cancel_pending_images();
                self.load_all_items(sender.clone());
            }

            LibraryPageInput::LoadMoreBatch => {
                if !self.is_loading && !self.has_loaded_all && !self.total_items.is_empty() {
                    debug!("Loading more items into view");
                    sender.input(LibraryPageInput::RenderBatch);
                }
            }

            LibraryPageInput::AllItemsLoaded {
                items,
                library_type,
            } => {
                debug!("Loaded all {} items from database", items.len());

                // Store library type
                self.library_type = library_type;

                // Extract all unique genres from items
                let mut genres_set = std::collections::HashSet::new();
                for item in &items {
                    for genre in item.get_genres() {
                        genres_set.insert(genre);
                    }
                }
                let mut available_genres: Vec<String> = genres_set.into_iter().collect();
                available_genres.sort();
                self.available_genres = available_genres;
                debug!("Found {} unique genres", self.available_genres.len());

                // Update the genre popover with available genres
                if !self.available_genres.is_empty()
                    && let Some(ref popover) = self.genre_popover
                {
                    self.update_genre_popover(popover, sender.clone());
                }

                // Calculate min and max years from items
                let years: Vec<i32> = items.iter().filter_map(|item| item.year).collect();

                if !years.is_empty() {
                    self.min_year = years.iter().min().copied();
                    self.max_year = years.iter().max().copied();
                    debug!("Year range: {:?} - {:?}", self.min_year, self.max_year);

                    // Update the year popover with available range
                    if let Some(ref popover) = self.year_popover {
                        self.update_year_popover(popover, sender.clone());
                    }

                    // Update the year menu button label
                    if let Some(ref button) = self.year_menu_button {
                        button.set_label(&self.get_year_label());
                    }
                }

                // Initialize the rating popover
                if let Some(ref popover) = self.rating_popover {
                    self.update_rating_popover(popover, sender.clone());
                }

                // Update the rating menu button label
                if let Some(ref button) = self.rating_menu_button {
                    button.set_label(&self.get_rating_label());
                }

                // Initialize the watch status popover
                if let Some(ref popover) = self.watch_status_popover {
                    self.update_watch_status_popover(popover, sender.clone());
                }

                // Fetch playback progress for all items if watch status filter is active
                let playback_progress_map = if self.watch_status_filter != WatchStatus::All {
                    let media_ids: Vec<String> = items.iter().map(|item| item.id.clone()).collect();
                    match crate::services::core::MediaService::get_playback_progress_batch(
                        &self.db, &media_ids,
                    )
                    .await
                    {
                        Ok(map) => map,
                        Err(e) => {
                            debug!("Failed to fetch playback progress for filtering: {}", e);
                            std::collections::HashMap::new()
                        }
                    }
                } else {
                    std::collections::HashMap::new()
                };

                // Apply text, genre, year range, rating, and watch status filtering
                let filtered_items: Vec<MediaItemModel> = items
                    .into_iter()
                    .filter(|item| {
                        // Text filter
                        let text_match = self.filter_text.is_empty()
                            || item
                                .title
                                .to_lowercase()
                                .contains(&self.filter_text.to_lowercase());

                        // Genre filter
                        let genre_match = self.selected_genres.is_empty() || {
                            let item_genres = item.get_genres();
                            self.selected_genres
                                .iter()
                                .any(|selected| item_genres.contains(selected))
                        };

                        // Year range filter
                        let year_match = if let Some(year) = item.year {
                            let min_match = self.selected_min_year.map_or(true, |min| year >= min);
                            let max_match = self.selected_max_year.map_or(true, |max| year <= max);
                            min_match && max_match
                        } else {
                            // Include items without a year if no year filter is set
                            self.selected_min_year.is_none() && self.selected_max_year.is_none()
                        };

                        // Rating filter
                        let rating_match = if let Some(min_rating) = self.min_rating {
                            // Check if item has a rating and it meets the minimum threshold
                            item.rating.map_or(false, |r| r >= min_rating)
                        } else {
                            // No rating filter set, include all items
                            true
                        };

                        // Watch status filter
                        let watch_status_match = match self.watch_status_filter {
                            WatchStatus::All => true,
                            WatchStatus::Watched | WatchStatus::Unwatched => {
                                // Determine watched status based on media type
                                let is_watched = if item.media_type == "show" {
                                    // For TV shows, check metadata for watched_episode_count
                                    let watched_count = item
                                        .metadata
                                        .as_ref()
                                        .and_then(|m| m.get("watched_episode_count"))
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(0)
                                        as u32;
                                    let total_count = item
                                        .metadata
                                        .as_ref()
                                        .and_then(|m| m.get("total_episode_count"))
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(0)
                                        as u32;

                                    total_count > 0 && watched_count == total_count
                                } else {
                                    // For movies and episodes, use playback_progress table
                                    playback_progress_map
                                        .get(&item.id)
                                        .map_or(false, |progress| progress.watched)
                                };

                                // Apply filter based on selected status
                                match self.watch_status_filter {
                                    WatchStatus::Watched => is_watched,
                                    WatchStatus::Unwatched => !is_watched,
                                    WatchStatus::All => true,
                                }
                            }
                        };

                        text_match
                            && genre_match
                            && year_match
                            && rating_match
                            && watch_status_match
                    })
                    .collect();

                // Store filtered items
                self.total_items = filtered_items;
                self.is_loading = false;

                // Clear image requests when loading new items
                self.images_requested.clear();

                // Start rendering the first batch immediately
                if !self.total_items.is_empty() {
                    // Render initial batch
                    sender.input(LibraryPageInput::RenderBatch);
                }
            }

            LibraryPageInput::RenderBatch => {
                // This message triggers rendering of the next batch of items
                let start_idx = self.loaded_count;
                let end_idx = (start_idx + self.batch_size).min(self.total_items.len());

                if start_idx < self.total_items.len() {
                    debug!(
                        "Rendering items {} to {} (no images yet)",
                        start_idx, end_idx
                    );

                    // Collect media IDs for this batch
                    let batch_media_ids: Vec<String> = self.total_items[start_idx..end_idx]
                        .iter()
                        .map(|item| item.id.clone())
                        .collect();

                    // Batch fetch playback progress for this batch
                    let playback_progress_map = if !batch_media_ids.is_empty() {
                        match crate::services::core::MediaService::get_playback_progress_batch(
                            &self.db,
                            &batch_media_ids,
                        )
                        .await
                        {
                            Ok(map) => map,
                            Err(e) => {
                                debug!("Failed to fetch playback progress: {}", e);
                                std::collections::HashMap::new()
                            }
                        }
                    } else {
                        std::collections::HashMap::new()
                    };

                    {
                        let mut factory_guard = self.media_factory.guard();
                        for idx in start_idx..end_idx {
                            let item = &self.total_items[idx];

                            // Calculate watched status differently for shows vs movies/episodes
                            let (watched, progress_percent) = if item.media_type == "show" {
                                // For TV shows, check metadata for watched_episode_count
                                let watched_count =
                                    item.metadata
                                        .as_ref()
                                        .and_then(|m| m.get("watched_episode_count"))
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(0) as u32;
                                let total_count =
                                    item.metadata
                                        .as_ref()
                                        .and_then(|m| m.get("total_episode_count"))
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(0) as u32;

                                if total_count > 0 {
                                    let watched = watched_count > 0 && watched_count == total_count;
                                    let progress =
                                        if watched_count < total_count && watched_count > 0 {
                                            watched_count as f64 / total_count as f64
                                        } else {
                                            0.0
                                        };
                                    (watched, progress)
                                } else {
                                    (false, 0.0)
                                }
                            } else {
                                // For movies and episodes, use playback_progress table
                                if let Some(progress) = playback_progress_map.get(&item.id) {
                                    (progress.watched, progress.get_progress_percentage() as f64)
                                } else {
                                    (false, 0.0)
                                }
                            };

                            let index = factory_guard.push_back(MediaCardInit {
                                item: item.clone(),
                                show_progress: progress_percent > 0.0,
                                watched,
                                progress_percent,
                                show_media_type_icon: self
                                    .library_type
                                    .as_ref()
                                    .is_some_and(|t| t == "mixed"),
                            });

                            // Store the mapping but don't request images yet
                            if item.poster_url.is_some() {
                                let id = item.id.clone();
                                self.image_requests
                                    .insert(id.clone(), index.current_index());
                            }
                        }
                    } // Guard dropped here

                    self.loaded_count = end_idx;
                    self.has_loaded_all = end_idx >= self.total_items.len();

                    // Update visible range after rendering new items
                    self.update_visible_range(root);

                    // Load images for visible items
                    sender.input(LibraryPageInput::LoadVisibleImages);
                }

                self.is_loading = false;
            }

            LibraryPageInput::MediaItemSelected(item_id) => {
                debug!("Media item selected: {}", item_id);
                sender
                    .output(LibraryPageOutput::NavigateToMediaItem(item_id))
                    .unwrap();
            }

            LibraryPageInput::SetSortBy(sort_by) => {
                debug!("Setting sort by: {:?}", sort_by);
                self.sort_by = sort_by;
                // Set default sort order based on field
                self.sort_order = match sort_by {
                    SortBy::Title => SortOrder::Ascending,
                    SortBy::Year | SortBy::DateAdded | SortBy::Rating => SortOrder::Descending,
                };
                self.refresh(sender.clone());
            }

            LibraryPageInput::ToggleSortOrder => {
                debug!("Toggling sort order");
                self.sort_order = match self.sort_order {
                    SortOrder::Ascending => SortOrder::Descending,
                    SortOrder::Descending => SortOrder::Ascending,
                };
                self.refresh(sender.clone());
            }

            LibraryPageInput::SetFilter(filter) => {
                debug!("Setting filter: {}", filter);
                self.filter_text = filter;
                // Re-filter and re-render from the beginning
                self.loaded_count = 0;
                self.media_factory.guard().clear();
                self.image_requests.clear();

                // If we have items loaded, just re-filter and render
                if !self.total_items.is_empty() {
                    sender.input(LibraryPageInput::AllItemsLoaded {
                        items: self.total_items.clone(),
                        library_type: self.library_type.clone(),
                    });
                } else {
                    self.load_all_items(sender.clone());
                }
            }

            LibraryPageInput::ToggleGenreFilter(genre) => {
                debug!("Toggling genre filter: {}", genre);
                if let Some(pos) = self.selected_genres.iter().position(|g| g == &genre) {
                    self.selected_genres.remove(pos);
                } else {
                    self.selected_genres.push(genre);
                }

                // Update the menu button label
                if let Some(ref button) = self.genre_menu_button {
                    button.set_label(&self.get_genre_label());
                }

                // Update the popover UI
                if let Some(ref popover) = self.genre_popover {
                    self.update_genre_popover(popover, sender.clone());
                }

                // Apply filter and re-render
                self.loaded_count = 0;
                self.media_factory.guard().clear();
                self.image_requests.clear();
                self.load_all_items(sender.clone());
            }

            LibraryPageInput::ClearGenreFilters => {
                debug!("Clearing all genre filters");
                self.selected_genres.clear();

                // Update the menu button label
                if let Some(ref button) = self.genre_menu_button {
                    button.set_label(&self.get_genre_label());
                }

                // Update the popover UI
                if let Some(ref popover) = self.genre_popover {
                    self.update_genre_popover(popover, sender.clone());
                }

                // Apply filter and re-render
                self.loaded_count = 0;
                self.media_factory.guard().clear();
                self.image_requests.clear();
                self.load_all_items(sender.clone());
            }

            LibraryPageInput::SetYearRange { min, max } => {
                debug!("Setting year range filter: {:?} - {:?}", min, max);
                self.selected_min_year = min;
                self.selected_max_year = max;

                // Update the menu button label
                if let Some(ref button) = self.year_menu_button {
                    button.set_label(&self.get_year_label());
                }

                // Update the popover UI
                if let Some(ref popover) = self.year_popover {
                    self.update_year_popover(popover, sender.clone());
                }

                // Apply filter and re-render
                self.loaded_count = 0;
                self.media_factory.guard().clear();
                self.image_requests.clear();
                self.load_all_items(sender.clone());
            }

            LibraryPageInput::ClearYearRange => {
                debug!("Clearing year range filter");
                self.selected_min_year = None;
                self.selected_max_year = None;

                // Update the menu button label
                if let Some(ref button) = self.year_menu_button {
                    button.set_label(&self.get_year_label());
                }

                // Update the popover UI
                if let Some(ref popover) = self.year_popover {
                    self.update_year_popover(popover, sender.clone());
                }

                // Apply filter and re-render
                self.loaded_count = 0;
                self.media_factory.guard().clear();
                self.image_requests.clear();
                self.load_all_items(sender.clone());
            }

            LibraryPageInput::SetRatingFilter(min_rating) => {
                debug!("Setting rating filter: {:?}", min_rating);
                self.min_rating = min_rating;

                // Update the menu button label
                if let Some(ref button) = self.rating_menu_button {
                    button.set_label(&self.get_rating_label());
                }

                // Update the popover UI
                if let Some(ref popover) = self.rating_popover {
                    self.update_rating_popover(popover, sender.clone());
                }

                // Apply filter and re-render
                self.loaded_count = 0;
                self.media_factory.guard().clear();
                self.image_requests.clear();
                self.load_all_items(sender.clone());
            }

            LibraryPageInput::ClearRatingFilter => {
                debug!("Clearing rating filter");
                self.min_rating = None;

                // Update the menu button label
                if let Some(ref button) = self.rating_menu_button {
                    button.set_label(&self.get_rating_label());
                }

                // Update the popover UI
                if let Some(ref popover) = self.rating_popover {
                    self.update_rating_popover(popover, sender.clone());
                }

                // Apply filter and re-render
                self.loaded_count = 0;
                self.media_factory.guard().clear();
                self.image_requests.clear();
                self.load_all_items(sender.clone());
            }

            LibraryPageInput::SetWatchStatusFilter(status) => {
                debug!("Setting watch status filter: {:?}", status);
                self.watch_status_filter = status;

                // Update the menu button label
                if let Some(ref button) = self.watch_status_menu_button {
                    button.set_label(&self.get_watch_status_label());
                }

                // Update the popover UI
                if let Some(ref popover) = self.watch_status_popover {
                    self.update_watch_status_popover(popover, sender.clone());
                }

                // Apply filter and re-render
                self.loaded_count = 0;
                self.media_factory.guard().clear();
                self.image_requests.clear();
                self.load_all_items(sender.clone());
            }

            LibraryPageInput::ClearWatchStatusFilter => {
                debug!("Clearing watch status filter");
                self.watch_status_filter = WatchStatus::All;

                // Update the menu button label
                if let Some(ref button) = self.watch_status_menu_button {
                    button.set_label(&self.get_watch_status_label());
                }

                // Update the popover UI
                if let Some(ref popover) = self.watch_status_popover {
                    self.update_watch_status_popover(popover, sender.clone());
                }

                // Apply filter and re-render
                self.loaded_count = 0;
                self.media_factory.guard().clear();
                self.image_requests.clear();
                self.load_all_items(sender.clone());
            }

            LibraryPageInput::SetMediaTypeFilter(media_type) => {
                debug!("Setting media type filter: {:?}", media_type);
                self.selected_media_type = media_type;

                // Clear and reload with new filter
                self.loaded_count = 0;
                self.media_factory.guard().clear();
                self.image_requests.clear();
                self.load_all_items(sender.clone());
            }

            LibraryPageInput::Refresh => {
                debug!("Refreshing library");
                self.refresh(sender.clone());
            }

            LibraryPageInput::ShowSearch => {
                self.search_visible = true;
            }

            LibraryPageInput::HideSearch => {
                self.search_visible = false;
                if !self.filter_text.is_empty() {
                    self.filter_text.clear();
                    self.refresh(sender.clone());
                }
            }

            LibraryPageInput::ToggleFilterPanel => {
                self.filter_panel_visible = !self.filter_panel_visible;
            }

            LibraryPageInput::ClearAllFilters => {
                debug!("Clearing all filters");

                // Clear genre filters
                if !self.selected_genres.is_empty() {
                    self.selected_genres.clear();
                    if let Some(ref button) = self.genre_menu_button {
                        button.set_label(&self.get_genre_label());
                    }
                    if let Some(ref popover) = self.genre_popover {
                        self.update_genre_popover(popover, sender.clone());
                    }
                }

                // Clear year range filter
                if self.selected_min_year.is_some() || self.selected_max_year.is_some() {
                    self.selected_min_year = None;
                    self.selected_max_year = None;
                    if let Some(ref button) = self.year_menu_button {
                        button.set_label(&self.get_year_label());
                    }
                    if let Some(ref popover) = self.year_popover {
                        self.update_year_popover(popover, sender.clone());
                    }
                }

                // Clear rating filter
                if self.min_rating.is_some() {
                    self.min_rating = None;
                    if let Some(ref button) = self.rating_menu_button {
                        button.set_label(&self.get_rating_label());
                    }
                    if let Some(ref popover) = self.rating_popover {
                        self.update_rating_popover(popover, sender.clone());
                    }
                }

                // Clear watch status filter
                if self.watch_status_filter != WatchStatus::All {
                    self.watch_status_filter = WatchStatus::All;
                    if let Some(ref button) = self.watch_status_menu_button {
                        button.set_label(&self.get_watch_status_label());
                    }
                    if let Some(ref popover) = self.watch_status_popover {
                        self.update_watch_status_popover(popover, sender.clone());
                    }
                }

                // Clear text filter
                if !self.filter_text.is_empty() {
                    self.filter_text.clear();
                }

                // Reload items with cleared filters
                self.loaded_count = 0;
                self.media_factory.guard().clear();
                self.image_requests.clear();
                self.load_all_items(sender.clone());
            }

            LibraryPageInput::ImageLoaded { id, texture } => {
                // Find the card index for this image ID and update it
                if let Some(&index) = self.image_requests.get(&id) {
                    // Send message to the specific card in the factory
                    self.media_factory
                        .send(index, MediaCardInput::ImageLoaded(texture));
                }
            }

            LibraryPageInput::ImageLoadFailed { id } => {
                debug!("Failed to load image for item: {}", id);
                // Find the card index for this image ID and notify it of the failure
                if let Some(&index) = self.image_requests.get(&id) {
                    // Send failure message to the specific card in the factory
                    self.media_factory
                        .send(index, MediaCardInput::ImageLoadFailed);
                }
                // Remove from tracking
                self.image_requests.remove(&id);
            }

            LibraryPageInput::ViewportScrolled => {
                // Debounce scroll events
                self.last_scroll_time = Some(Instant::now());

                // Cancel previous debounce if exists
                if let Some(handle) = self.scroll_debounce_handle.take() {
                    handle.remove();
                }

                // Set up new debounce timer (150ms delay)
                let sender_clone = sender.clone();
                self.scroll_debounce_handle = Some(gtk::glib::timeout_add_local(
                    Duration::from_millis(150),
                    move || {
                        sender_clone.input(LibraryPageInput::ProcessDebouncedScroll);
                        gtk::glib::ControlFlow::Break
                    },
                ));
            }

            LibraryPageInput::ProcessDebouncedScroll => {
                self.scroll_debounce_handle = None;
                let _old_start = self.visible_start_idx;
                self.update_visible_range(root);

                // No need to clear tracking on viewport change anymore

                sender.input(LibraryPageInput::LoadVisibleImages);
            }

            LibraryPageInput::LoadVisibleImages => {
                self.load_images_for_visible_range();
            }
            LibraryPageInput::BrokerMsg(msg) => {
                match msg {
                    BrokerMessage::Config(_) => {
                        // Library page might reload if display preferences changed
                        debug!("Library page received config update");
                        // Could potentially reload with new display preferences
                        // sender.input(LibraryPageInput::Refresh);
                    }
                    _ => {
                        // Ignore other broker messages
                    }
                }
            }
        }
    }

    fn shutdown(&mut self, _widgets: &mut Self::Widgets, _output: relm4::Sender<Self::Output>) {
        // Unsubscribe from MessageBroker
        relm4::spawn(async move {
            BROKER.unsubscribe("LibraryPage").await;
        });
    }
}

impl Drop for LibraryPage {
    fn drop(&mut self) {
        // Cancel any pending debounce timer
        if let Some(handle) = self.scroll_debounce_handle.take() {
            handle.remove();
        }

        // Note: We can't disconnect the scroll handler here because we don't have access to widgets
        // However, the handler ID is stored and GTK should clean it up when the adjustment is destroyed

        // Cancel all pending image loads
        self.cancel_pending_images();
    }
}

impl LibraryPage {
    fn get_active_filter_count(&self) -> usize {
        let mut count = 0;

        // Count genre filters
        if !self.selected_genres.is_empty() {
            count += 1;
        }

        // Count year range filter
        if self.selected_min_year.is_some() || self.selected_max_year.is_some() {
            count += 1;
        }

        // Count rating filter
        if self.min_rating.is_some() {
            count += 1;
        }

        // Count watch status filter
        if self.watch_status_filter != WatchStatus::All {
            count += 1;
        }

        // Don't count text filter as it's shown in the search bar

        count
    }

    fn get_genre_label(&self) -> String {
        if self.selected_genres.is_empty() {
            "All Genres".to_string()
        } else if self.selected_genres.len() == 1 {
            self.selected_genres[0].clone()
        } else {
            format!("{} genres", self.selected_genres.len())
        }
    }

    fn get_year_label(&self) -> String {
        match (self.selected_min_year, self.selected_max_year) {
            (Some(min), Some(max)) if min == max => format!("{}", min),
            (Some(min), Some(max)) => format!("{} - {}", min, max),
            (Some(min), None) => format!("{} +", min),
            (None, Some(max)) => format!("- {}", max),
            (None, None) => "All Years".to_string(),
        }
    }

    fn get_rating_label(&self) -> String {
        match self.min_rating {
            Some(rating) => format!("{:.1}+ ★", rating),
            None => "All Ratings".to_string(),
        }
    }

    fn get_watch_status_label(&self) -> String {
        match self.watch_status_filter {
            WatchStatus::All => "All Items".to_string(),
            WatchStatus::Watched => "Watched".to_string(),
            WatchStatus::Unwatched => "Unwatched".to_string(),
        }
    }

    fn update_genre_popover(&self, popover: &gtk::Popover, sender: AsyncComponentSender<Self>) {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.set_margin_start(12);
        content.set_margin_end(12);
        content.set_width_request(250);

        // Header with clear button
        let header_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let header_label = gtk::Label::new(Some("Filter by Genre"));
        header_label.set_halign(gtk::Align::Start);
        header_label.set_hexpand(true);
        header_label.add_css_class("heading");
        header_box.append(&header_label);

        if !self.selected_genres.is_empty() {
            let clear_button = gtk::Button::with_label("Clear");
            clear_button.add_css_class("flat");
            let sender_clone = sender.clone();
            clear_button.connect_clicked(move |_| {
                sender_clone.input(LibraryPageInput::ClearGenreFilters);
            });
            header_box.append(&clear_button);
        }
        content.append(&header_box);

        // Separator
        let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
        content.append(&separator);

        // Scrolled window for genre list
        let scrolled_window = gtk::ScrolledWindow::new();
        scrolled_window.set_max_content_height(400);
        scrolled_window.set_propagate_natural_height(true);
        scrolled_window.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);

        let genre_box = gtk::Box::new(gtk::Orientation::Vertical, 2);

        // Add checkboxes for each genre
        for genre in &self.available_genres {
            let is_selected = self.selected_genres.contains(genre);
            let check_button = gtk::CheckButton::with_label(genre);
            check_button.set_active(is_selected);
            check_button.add_css_class("flat");

            let genre_clone = genre.clone();
            let sender_clone = sender.clone();
            check_button.connect_toggled(move |_| {
                sender_clone.input(LibraryPageInput::ToggleGenreFilter(genre_clone.clone()));
            });

            genre_box.append(&check_button);
        }

        scrolled_window.set_child(Some(&genre_box));
        content.append(&scrolled_window);

        popover.set_child(Some(&content));
    }

    fn update_year_popover(&self, popover: &gtk::Popover, sender: AsyncComponentSender<Self>) {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.set_margin_start(12);
        content.set_margin_end(12);
        content.set_width_request(250);

        // Header with clear button
        let header_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let header_label = gtk::Label::new(Some("Filter by Year"));
        header_label.set_halign(gtk::Align::Start);
        header_label.set_hexpand(true);
        header_label.add_css_class("heading");
        header_box.append(&header_label);

        if self.selected_min_year.is_some() || self.selected_max_year.is_some() {
            let clear_button = gtk::Button::with_label("Clear");
            clear_button.add_css_class("flat");
            let sender_clone = sender.clone();
            clear_button.connect_clicked(move |_| {
                sender_clone.input(LibraryPageInput::ClearYearRange);
            });
            header_box.append(&clear_button);
        }
        content.append(&header_box);

        // Separator
        let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
        content.append(&separator);

        // Year range info
        if let (Some(min), Some(max)) = (self.min_year, self.max_year) {
            let info_label = gtk::Label::new(Some(&format!("Available: {} - {}", min, max)));
            info_label.set_halign(gtk::Align::Start);
            info_label.add_css_class("dim-label");
            content.append(&info_label);

            // Min year input
            let min_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            min_box.set_margin_top(6);
            let min_label = gtk::Label::new(Some("From:"));
            min_label.set_width_request(50);
            min_label.set_halign(gtk::Align::Start);
            min_box.append(&min_label);

            let min_spinbutton = gtk::SpinButton::with_range(min as f64, max as f64, 1.0);
            min_spinbutton.set_value(self.selected_min_year.unwrap_or(min) as f64);
            min_spinbutton.set_hexpand(true);
            min_box.append(&min_spinbutton);
            content.append(&min_box);

            // Max year input
            let max_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            max_box.set_margin_top(6);
            let max_label = gtk::Label::new(Some("To:"));
            max_label.set_width_request(50);
            max_label.set_halign(gtk::Align::Start);
            max_box.append(&max_label);

            let max_spinbutton = gtk::SpinButton::with_range(min as f64, max as f64, 1.0);
            max_spinbutton.set_value(self.selected_max_year.unwrap_or(max) as f64);
            max_spinbutton.set_hexpand(true);
            max_box.append(&max_spinbutton);
            content.append(&max_box);

            // Apply button
            let apply_button = gtk::Button::with_label("Apply");
            apply_button.set_margin_top(12);
            apply_button.add_css_class("suggested-action");

            let sender_clone = sender.clone();
            apply_button.connect_clicked(move |_| {
                let min_year = min_spinbutton.value() as i32;
                let max_year = max_spinbutton.value() as i32;
                sender_clone.input(LibraryPageInput::SetYearRange {
                    min: Some(min_year),
                    max: Some(max_year),
                });
            });
            content.append(&apply_button);
        }

        popover.set_child(Some(&content));
    }

    fn update_rating_popover(&self, popover: &gtk::Popover, sender: AsyncComponentSender<Self>) {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.set_margin_start(12);
        content.set_margin_end(12);
        content.set_width_request(250);

        // Header with clear button
        let header_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let header_label = gtk::Label::new(Some("Filter by Rating"));
        header_label.set_halign(gtk::Align::Start);
        header_label.set_hexpand(true);
        header_label.add_css_class("heading");
        header_box.append(&header_label);

        if self.min_rating.is_some() {
            let clear_button = gtk::Button::with_label("Clear");
            clear_button.add_css_class("flat");
            let sender_clone = sender.clone();
            clear_button.connect_clicked(move |_| {
                sender_clone.input(LibraryPageInput::ClearRatingFilter);
            });
            header_box.append(&clear_button);
        }
        content.append(&header_box);

        // Separator
        let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
        content.append(&separator);

        // Info label
        let info_label = gtk::Label::new(Some("Minimum rating (0-10):"));
        info_label.set_halign(gtk::Align::Start);
        info_label.add_css_class("dim-label");
        content.append(&info_label);

        // Rating scale
        let rating_scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 10.0, 0.5);
        rating_scale.set_value(self.min_rating.unwrap_or(0.0) as f64);
        rating_scale.set_draw_value(true);
        rating_scale.set_value_pos(gtk::PositionType::Right);
        rating_scale.set_digits(1);
        rating_scale.set_hexpand(true);

        // Add marks for better UX
        for i in 0..=10 {
            rating_scale.add_mark(i as f64, gtk::PositionType::Bottom, None);
        }

        content.append(&rating_scale);

        // Apply button
        let apply_button = gtk::Button::with_label("Apply");
        apply_button.set_margin_top(12);
        apply_button.add_css_class("suggested-action");

        let sender_clone = sender.clone();
        apply_button.connect_clicked(move |_| {
            let rating = rating_scale.value() as f32;
            sender_clone.input(LibraryPageInput::SetRatingFilter(if rating > 0.0 {
                Some(rating)
            } else {
                None
            }));
        });
        content.append(&apply_button);

        popover.set_child(Some(&content));
    }

    fn update_watch_status_popover(
        &self,
        popover: &gtk::Popover,
        sender: AsyncComponentSender<Self>,
    ) {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.set_margin_start(12);
        content.set_margin_end(12);
        content.set_width_request(250);

        // Header with clear button
        let header_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let header_label = gtk::Label::new(Some("Filter by Watch Status"));
        header_label.set_halign(gtk::Align::Start);
        header_label.set_hexpand(true);
        header_label.add_css_class("heading");
        header_box.append(&header_label);

        if self.watch_status_filter != WatchStatus::All {
            let clear_button = gtk::Button::with_label("Clear");
            clear_button.add_css_class("flat");
            let sender_clone = sender.clone();
            clear_button.connect_clicked(move |_| {
                sender_clone.input(LibraryPageInput::ClearWatchStatusFilter);
            });
            header_box.append(&clear_button);
        }
        content.append(&header_box);

        // Separator
        let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
        content.append(&separator);

        // Radio buttons for watch status
        let radio_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
        radio_box.set_margin_top(6);

        // All items option
        let all_radio = gtk::CheckButton::with_label("All Items");
        all_radio.set_active(self.watch_status_filter == WatchStatus::All);
        let sender_clone = sender.clone();
        all_radio.connect_toggled(move |btn| {
            if btn.is_active() {
                sender_clone.input(LibraryPageInput::SetWatchStatusFilter(WatchStatus::All));
            }
        });
        radio_box.append(&all_radio);

        // Watched option
        let watched_radio = gtk::CheckButton::with_label("Watched");
        watched_radio.set_group(Some(&all_radio));
        watched_radio.set_active(self.watch_status_filter == WatchStatus::Watched);
        let sender_clone = sender.clone();
        watched_radio.connect_toggled(move |btn| {
            if btn.is_active() {
                sender_clone.input(LibraryPageInput::SetWatchStatusFilter(WatchStatus::Watched));
            }
        });
        radio_box.append(&watched_radio);

        // Unwatched option
        let unwatched_radio = gtk::CheckButton::with_label("Unwatched");
        unwatched_radio.set_group(Some(&all_radio));
        unwatched_radio.set_active(self.watch_status_filter == WatchStatus::Unwatched);
        let sender_clone = sender;
        unwatched_radio.connect_toggled(move |btn| {
            if btn.is_active() {
                sender_clone.input(LibraryPageInput::SetWatchStatusFilter(
                    WatchStatus::Unwatched,
                ));
            }
        });
        radio_box.append(&unwatched_radio);

        content.append(&radio_box);

        popover.set_child(Some(&content));
    }

    fn update_visible_range(&mut self, root: &gtk::Overlay) {
        // Get the Box from the overlay, then the scrolled window
        let box_widget = root
            .first_child()
            .and_then(|w| w.downcast::<gtk::Box>().ok());

        let scrolled = box_widget
            .and_then(|b| b.last_child())
            .and_then(|w| w.downcast::<gtk::ScrolledWindow>().ok());

        if let Some(scrolled) = scrolled {
            let adjustment = scrolled.vadjustment();
            let scroll_pos = adjustment.value();
            let page_size = adjustment.page_size();

            // Get the flow box to determine actual item dimensions
            let flow_box = scrolled
                .child()
                .and_then(|w| w.first_child())
                .and_then(|w| w.downcast::<gtk::FlowBox>().ok());

            let items_per_row = if let Some(flow_box) = flow_box {
                // Use actual columns from flowbox
                flow_box.min_children_per_line() as usize
            } else {
                4 // Default fallback
            };

            // More accurate row height accounting for reduced spacing
            let row_height = 270.0; // Card height (180) + spacing (16)

            let visible_start_row = (scroll_pos / row_height).floor() as usize;
            let visible_end_row = ((scroll_pos + page_size) / row_height).ceil() as usize + 1; // Add 1 for partial visibility

            self.visible_start_idx = visible_start_row * items_per_row;
            self.visible_end_idx = ((visible_end_row + 1) * items_per_row).min(self.loaded_count);

            trace!(
                "Viewport updated: scroll_pos={:.0}, page_size={:.0}, items {} to {} visible",
                scroll_pos, page_size, self.visible_start_idx, self.visible_end_idx
            );
        }
    }

    fn load_images_for_visible_range(&mut self) {
        // Calculate which items need images with lookahead
        let lookahead_items = 30; // Load 30 items ahead and behind for smoother scrolling
        let load_start = self.visible_start_idx.saturating_sub(lookahead_items);
        let load_end = (self.visible_end_idx + lookahead_items).min(self.loaded_count);

        debug!(
            "Loading images for items {} to {} (visible: {} to {})",
            load_start, load_end, self.visible_start_idx, self.visible_end_idx
        );

        // Cancel images outside visible range
        let mut to_cancel = Vec::new();
        for idx in 0..self.loaded_count {
            if (idx < load_start || idx >= load_end) && idx < self.total_items.len() {
                let item_id = &self.total_items[idx].id;
                if self.image_requests.contains_key(item_id) {
                    to_cancel.push(item_id.clone());
                }
            }
        }

        // Cancel out-of-range images
        for id in to_cancel {
            trace!("Cancelling image load for out-of-range item: {}", id);
            let _ = self
                .image_loader
                .sender()
                .send(ImageLoaderInput::CancelLoad { id: id.clone() });
            self.pending_image_cancels.push(id);
        }

        // Load images for items in range
        let mut images_queued = 0;
        for idx in load_start..load_end {
            if idx < self.total_items.len() {
                let item = &self.total_items[idx];
                if let Some(poster_url) = &item.poster_url {
                    let id = item.id.clone();

                    // Skip if already requested or recently cancelled
                    if self.images_requested.contains(&id)
                        || self.pending_image_cancels.contains(&id)
                    {
                        continue;
                    }

                    // Calculate priority based on distance from current viewport
                    let priority = if idx >= self.visible_start_idx && idx < self.visible_end_idx {
                        0 // Highest priority for visible items
                    } else {
                        // Priority increases with distance from viewport
                        let distance = if idx < self.visible_start_idx {
                            self.visible_start_idx - idx
                        } else {
                            idx - self.visible_end_idx
                        };
                        (distance / 10).min(10) as u8
                    };

                    trace!(
                        "Queueing image for item {} (id: {}) with priority {}",
                        idx, id, priority
                    );

                    let _ = self.image_loader.sender().send(ImageLoaderInput::LoadImage(
                        ImageRequest {
                            id: id.clone(),
                            url: poster_url.clone(),
                            size: ImageSize::Thumbnail,
                            priority,
                        },
                    ));

                    // Mark this image as requested
                    self.images_requested.insert(id);
                    images_queued += 1;
                }
            }
        }

        if images_queued > 0 {
            debug!("Queued {} new image loads", images_queued);
        }

        // Clear pending cancels after a delay
        self.pending_image_cancels.clear();
    }

    fn cancel_pending_images(&mut self) {
        // Cancel all pending image loads
        for (id, _) in self.image_requests.iter() {
            let _ = self
                .image_loader
                .sender()
                .send(ImageLoaderInput::CancelLoad { id: id.clone() });
        }
    }

    fn load_all_items(&mut self, sender: AsyncComponentSender<Self>) {
        if let Some(library_id) = &self.library_id {
            self.is_loading = true;

            let db = self.db.clone();
            let library_id = library_id.clone();
            let sort_by = self.sort_by;
            let sort_order = self.sort_order;
            let selected_media_type = self.selected_media_type.clone();

            relm4::spawn(async move {
                use crate::db::repository::{
                    LibraryRepositoryImpl, MediaRepository, MediaRepositoryImpl, Repository,
                };
                let library_repo = LibraryRepositoryImpl::new(db.clone());
                let media_repo = MediaRepositoryImpl::new(db.clone());

                // First, get the library to determine its type
                let library_result = library_repo.find_by_id(library_id.as_ref()).await;

                let (library_type, media_result) = match library_result {
                    Ok(Some(library)) => {
                        let lib_type = library.library_type.to_lowercase();

                        // For mixed libraries, check if we have a media type filter
                        let media_result = if lib_type == "mixed" {
                            // Use the selected media type filter if set
                            if let Some(media_type) = selected_media_type {
                                media_repo
                                    .find_by_library_and_type(library_id.as_ref(), &media_type)
                                    .await
                            } else {
                                // Get all items if no filter is set
                                media_repo.find_by_library(library_id.as_ref()).await
                            }
                        } else {
                            // Determine the appropriate media type filter based on library type
                            let media_type = match lib_type.as_str() {
                                "movies" => Some("movie"),
                                "shows" => Some("show"),
                                "music" => Some("album"), // For music libraries, show albums, not individual tracks
                                _ => None,                // For unknown types, get all items
                            };

                            // Get ALL items for this library without pagination
                            if let Some(media_type) = media_type {
                                media_repo
                                    .find_by_library_and_type(library_id.as_ref(), media_type)
                                    .await
                            } else {
                                // For unknown types, get all items
                                media_repo.find_by_library(library_id.as_ref()).await
                            }
                        };

                        (Some(lib_type), media_result)
                    }
                    _ => {
                        // If we can't get library info, get all items
                        (None, media_repo.find_by_library(library_id.as_ref()).await)
                    }
                };

                match media_result {
                    Ok(mut items) => {
                        // Sort items based on sort criteria and order
                        match (sort_by, sort_order) {
                            (SortBy::Title, SortOrder::Ascending) => {
                                items.sort_by(|a, b| a.sort_title.cmp(&b.sort_title));
                            }
                            (SortBy::Title, SortOrder::Descending) => {
                                items.sort_by(|a, b| b.sort_title.cmp(&a.sort_title));
                            }
                            (SortBy::Year, SortOrder::Ascending) => {
                                items.sort_by(|a, b| a.year.cmp(&b.year));
                            }
                            (SortBy::Year, SortOrder::Descending) => {
                                items.sort_by(|a, b| b.year.cmp(&a.year));
                            }
                            (SortBy::DateAdded, SortOrder::Ascending) => {
                                items.sort_by(|a, b| a.added_at.cmp(&b.added_at));
                            }
                            (SortBy::DateAdded, SortOrder::Descending) => {
                                items.sort_by(|a, b| b.added_at.cmp(&a.added_at));
                            }
                            (SortBy::Rating, SortOrder::Ascending) => {
                                items.sort_by(|a, b| {
                                    a.rating
                                        .partial_cmp(&b.rating)
                                        .unwrap_or(std::cmp::Ordering::Equal)
                                });
                            }
                            (SortBy::Rating, SortOrder::Descending) => {
                                items.sort_by(|a, b| {
                                    b.rating
                                        .partial_cmp(&a.rating)
                                        .unwrap_or(std::cmp::Ordering::Equal)
                                });
                            }
                        }

                        sender.input(LibraryPageInput::AllItemsLoaded {
                            items,
                            library_type,
                        });
                    }
                    Err(e) => {
                        error!("Failed to load library items: {}", e);
                        sender.input(LibraryPageInput::AllItemsLoaded {
                            items: Vec::new(),
                            library_type,
                        });
                    }
                }
            });
        }
    }

    fn refresh(&mut self, sender: AsyncComponentSender<Self>) {
        self.loaded_count = 0;
        self.total_items.clear();
        self.has_loaded_all = false;
        self.media_factory.guard().clear();
        self.image_requests.clear();
        self.images_requested.clear();
        self.visible_start_idx = 0;
        self.visible_end_idx = 0;
        // Keep genre filters during refresh to maintain user selection
        self.cancel_pending_images();
        self.load_all_items(sender);
    }
}
