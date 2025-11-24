use gtk::prelude::*;
use libadwaita as adw;
use relm4::factory::FactoryVecDeque;
use relm4::gtk;
use relm4::prelude::*;
use std::time::Duration;
use tracing::{debug, trace};

// Module declarations first
mod data;
mod filters;
mod messages;
mod types;
mod ui_builders;

// Re-export public types (also makes them available in this module)
pub use messages::{LibraryPageInput, LibraryPageOutput};
pub use types::{
    ActiveFilter, ActiveFilterType, FilterState, FilterStatistics, SortBy, SortOrder, ViewMode,
    WatchStatus,
};

use crate::db::connection::DatabaseConnection;
use crate::db::entities::MediaItemModel;
use crate::models::LibraryId;
use crate::ui::factories::media_card::{MediaCard, MediaCardInit, MediaCardInput, MediaCardOutput};
use crate::ui::shared::broker::{BROKER, BrokerMessage};
use crate::workers::{ImageLoader, ImageLoaderOutput};
use std::collections::HashMap;

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
            .field("selected_view_mode", &self.selected_view_mode)
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
    // Current sort settings (updated when view mode changes)
    sort_by: SortBy,
    sort_order: SortOrder,
    // Sort preferences per view mode
    view_mode_sort_prefs: HashMap<ViewMode, types::ViewModeSortPrefs>,
    filter_text: String,
    search_visible: bool,
    // Genre filtering
    selected_genres: Vec<String>,
    available_genres: Vec<String>,
    genre_popover: Option<gtk::Popover>,
    // Year range filtering
    min_year: Option<i32>,
    max_year: Option<i32>,
    selected_min_year: Option<i32>,
    selected_max_year: Option<i32>,
    year_popover: Option<gtk::Popover>,
    // Rating filtering
    min_rating: Option<f32>,
    rating_popover: Option<gtk::Popover>,
    // Watch status filtering
    watch_status_filter: WatchStatus,
    watch_status_popover: Option<gtk::Popover>,
    // Media type filtering (for mixed libraries)
    library_type: Option<String>, // 'movies', 'shows', 'music', 'photos', 'mixed'
    selected_media_type: Option<String>, // Filter for mixed libraries
    // Viewport tracking
    visible_start_idx: usize,
    visible_end_idx: usize,
    // Scroll debouncing
    scroll_debounce_handle: Option<gtk::glib::SourceId>,
    // Image loading state
    images_requested: std::collections::HashSet<String>, // Track which images have been requested
    pending_image_cancels: Vec<String>,
    // Handler IDs for cleanup
    scroll_handler_id: Option<gtk::glib::SignalHandlerId>,
    // View mode selection
    selected_view_mode: ViewMode,
    // View switcher widgets for header bar
    view_stack: adw::ViewStack,
    view_switcher_bar: adw::ViewSwitcherBar,
    // Active filters container
    active_filters_box: Option<gtk::Box>,
    // Unified filters popover
    filters_popover: Option<gtk::Popover>,
    filters_button: Option<gtk::Button>,
    // Flag to defer factory clearing until items are ready to render
    needs_factory_clear: bool,
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

                // Toolbar with sort controls and stats
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 12,
                    set_margin_all: 12,

                    // Left side - controls
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 12,

                        gtk::Label {
                            set_text: "Sort by:",
                        },

                        gtk::DropDown {
                            set_model: Some(&gtk::StringList::new(&[
                                "Title",
                                "Year",
                                "Date Added",
                                "Rating",
                                "Last Watched",
                                "Duration",
                            ])),
                            #[watch]
                            set_selected: match model.sort_by {
                                SortBy::Title => 0,
                                SortBy::Year => 1,
                                SortBy::DateAdded => 2,
                                SortBy::Rating => 3,
                                SortBy::LastWatched => 4,
                                SortBy::Duration => 5,
                            },
                            connect_selected_notify[sender] => move |dropdown| {
                                let sort_by = match dropdown.selected() {
                                    0 => SortBy::Title,
                                    1 => SortBy::Year,
                                    2 => SortBy::DateAdded,
                                    3 => SortBy::Rating,
                                    4 => SortBy::LastWatched,
                                    5 => SortBy::Duration,
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

                        // Add filters button
                        #[name = "filters_button"]
                        gtk::Button {
                            set_icon_name: "funnel-symbolic",
                            set_tooltip_text: Some("Filters"),
                            add_css_class: "flat",
                            connect_clicked[sender] => move |_| {
                                sender.input(LibraryPageInput::ToggleFiltersPopover);
                            }
                        },
                    },

                    // Spacer to push stats to the right
                    gtk::Box {
                        set_hexpand: true,
                    },

                    // Right side - media stats
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 24,
                        #[watch]
                        set_visible: !model.is_loading && (model.has_active_filters() || !model.total_items.is_empty()),

                        // Result count
                        gtk::Label {
                            #[watch]
                            set_markup: &{
                                let stats = model.get_filter_statistics();
                                if stats.total_count == 0 {
                                    if model.has_active_filters() {
                                        format!("<b>No results found</b>")
                                    } else {
                                        String::new()
                                    }
                                } else {
                                    format!("<b>{} item{}</b>",
                                        stats.total_count,
                                        if stats.total_count == 1 { "" } else { "s" }
                                    )
                                }
                            },
                        },

                        // Average rating (if available)
                        gtk::Label {
                            #[watch]
                            set_markup: &{
                                let stats = model.get_filter_statistics();
                                if let Some(avg_rating) = stats.avg_rating {
                                    format!("Avg Rating: <b>{:.1} ★</b>", avg_rating)
                                } else {
                                    String::new()
                                }
                            },
                            #[watch]
                            set_visible: model.get_filter_statistics().avg_rating.is_some() && !model.total_items.is_empty(),
                            add_css_class: "dim-label",
                        },

                        // Year range (if available)
                        gtk::Label {
                            #[watch]
                            set_markup: &{
                                let stats = model.get_filter_statistics();
                                match (stats.min_year, stats.max_year) {
                                    (Some(min), Some(max)) if min == max => format!("Year: <b>{}</b>", min),
                                    (Some(min), Some(max)) => format!("Years: <b>{} - {}</b>", min, max),
                                    _ => String::new(),
                                }
                            },
                            #[watch]
                            set_visible: {
                                let stats = model.get_filter_statistics();
                                stats.min_year.is_some() && !model.total_items.is_empty()
                            },
                            add_css_class: "dim-label",
                        },
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

                // Main content area with media grid
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 0,
                    set_vexpand: true,
                    set_hexpand: true,

                    // Scrolled window with media content
                    #[name = "scrolled_window"]
                    gtk::ScrolledWindow {
                    set_vexpand: true,
                    set_hexpand: true,
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

                        // Filter summary section - active filters only
                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 12,
                            set_margin_start: 16,
                            set_margin_end: 16,
                            set_margin_top: 12,
                            #[watch]
                            set_visible: !model.get_active_filters_list().is_empty(),

                            // Active filters list with remove buttons
                            #[name = "active_filters_box"]
                            gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 6,
                                set_halign: gtk::Align::Start,
                            },

                            // No results state with suggestions
                            gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_spacing: 12,
                                set_margin_all: 24,
                                #[watch]
                                set_visible: !model.is_loading && model.has_active_filters() && model.total_items.is_empty(),

                                gtk::Label {
                                    set_markup: "<big><b>No results found</b></big>",
                                    set_halign: gtk::Align::Center,
                                },

                                gtk::Label {
                                    set_text: "Try adjusting your filters:",
                                    set_halign: gtk::Align::Center,
                                    add_css_class: "dim-label",
                                },

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_spacing: 6,
                                    set_halign: gtk::Align::Center,
                                    set_margin_top: 12,

                                    // We'll dynamically populate suggestions
                                    // For now, show a clear all filters button
                                    gtk::Button {
                                        set_label: "Clear All Filters",
                                        add_css_class: "suggested-action",
                                        connect_clicked[sender] => move |_| {
                                            sender.input(LibraryPageInput::ClearAllFilters);
                                        }
                                    },
                                },
                            },
                        },

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
                MediaCardOutput::GoToShow(id) => LibraryPageInput::MediaItemSelected(id),
                MediaCardOutput::MarkWatched(id) => LibraryPageInput::MarkWatched(id),
                MediaCardOutput::MarkUnwatched(id) => LibraryPageInput::MarkUnwatched(id),
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

        // Initialize view mode sort preferences with defaults
        let view_mode_sort_prefs = FilterState::default().view_mode_sort_prefs;

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
            view_mode_sort_prefs,
            filter_text: String::new(),
            search_visible: false,
            // Genre filtering
            selected_genres: Vec::new(),
            available_genres: Vec::new(),
            genre_popover: None,
            // Year range filtering
            min_year: None,
            max_year: None,
            selected_min_year: None,
            selected_max_year: None,
            year_popover: None,
            // Rating filtering
            min_rating: None,
            rating_popover: None,
            // Watch status filtering
            watch_status_filter: WatchStatus::All,
            watch_status_popover: None,
            // Media type filtering (for mixed libraries)
            library_type: None,
            selected_media_type: None,
            // Viewport tracking
            visible_start_idx: 0,
            visible_end_idx: 0,
            // Scroll debouncing
            scroll_debounce_handle: None,
            // Image loading state
            images_requested: std::collections::HashSet::new(),
            pending_image_cancels: Vec::new(),
            // Handler IDs for cleanup
            scroll_handler_id: None,
            // View mode selection
            selected_view_mode: ViewMode::All,
            // View switcher widgets for header bar
            view_stack: adw::ViewStack::new(),
            view_switcher_bar: adw::ViewSwitcherBar::new(),
            // Active filters container
            active_filters_box: None,
            // Unified filters popover
            filters_popover: None,
            filters_button: None,
            // Flag to defer factory clearing until items are ready to render
            needs_factory_clear: false,
        };

        let mut model = model;

        let widgets = view_output!();

        // Store reference to active filters box
        model.active_filters_box = Some(widgets.active_filters_box.clone());

        // Store reference to filters button and create unified filters popover
        model.filters_button = Some(widgets.filters_button.clone());
        let filters_popover = gtk::Popover::new();
        filters_popover.set_parent(&widgets.filters_button);
        model.filters_popover = Some(filters_popover);

        // Subscribe to MessageBroker for config updates
        {
            let broker_sender = sender.input_sender().clone();
            relm4::spawn(async move {
                let (tx, rx) = relm4::channel::<BrokerMessage>();
                BROKER.subscribe("LibraryPage".to_string(), tx).await;

                while let Some(msg) = rx.recv().await {
                    if broker_sender
                        .send(LibraryPageInput::BrokerMsg(msg))
                        .is_err()
                    {
                        // Component is shutting down, break the loop
                        break;
                    }
                }
            });
        }

        // Create and set the genre filter popover
        let genre_popover = gtk::Popover::new();
        genre_popover.set_child(Some(&gtk::Box::new(gtk::Orientation::Vertical, 0)));
        model.genre_popover = Some(genre_popover);

        // Create and set the year range filter popover
        let year_popover = gtk::Popover::new();
        year_popover.set_child(Some(&gtk::Box::new(gtk::Orientation::Vertical, 0)));
        model.year_popover = Some(year_popover);

        // Create and set the rating filter popover
        let rating_popover = gtk::Popover::new();
        rating_popover.set_child(Some(&gtk::Box::new(gtk::Orientation::Vertical, 0)));
        model.rating_popover = Some(rating_popover);

        // Create and set the watch status filter popover
        let watch_status_popover = gtk::Popover::new();
        watch_status_popover.set_child(Some(&gtk::Box::new(gtk::Orientation::Vertical, 0)));
        model.watch_status_popover = Some(watch_status_popover);

        // Connect scroll handler and store the ID
        let sender_for_scroll = sender.clone();
        let adjustment = widgets.scrolled_window.vadjustment();
        let scroll_handler_id = adjustment.connect_value_changed(move |_| {
            sender_for_scroll.input(LibraryPageInput::ViewportScrolled);
        });
        model.scroll_handler_id = Some(scroll_handler_id);

        // Setup view switcher for header bar
        // Add pages to the view stack - these are just placeholders, the actual content
        // is shown in the main scrolled window based on the selected view mode
        model.view_stack.add_titled(
            &gtk::Box::new(gtk::Orientation::Vertical, 0),
            Some("all"),
            "All",
        );
        model
            .view_stack
            .page(&model.view_stack.child_by_name("all").unwrap())
            .set_icon_name(Some("view-grid-symbolic"));

        model.view_stack.add_titled(
            &gtk::Box::new(gtk::Orientation::Vertical, 0),
            Some("unwatched"),
            "Unwatched",
        );
        model
            .view_stack
            .page(&model.view_stack.child_by_name("unwatched").unwrap())
            .set_icon_name(Some("non-starred-symbolic"));

        model.view_stack.add_titled(
            &gtk::Box::new(gtk::Orientation::Vertical, 0),
            Some("recent"),
            "Recently Added",
        );
        model
            .view_stack
            .page(&model.view_stack.child_by_name("recent").unwrap())
            .set_icon_name(Some("document-open-recent-symbolic"));

        // Connect view switcher bar to view stack
        model.view_switcher_bar.set_stack(Some(&model.view_stack));
        model.view_switcher_bar.set_reveal(true);

        // Connect view stack page changes to view mode updates
        let sender_for_view = sender.clone();
        model
            .view_stack
            .connect_visible_child_name_notify(move |view_stack| {
                if let Some(child_name) = view_stack.visible_child_name() {
                    let view_mode = match child_name.as_str() {
                        "all" => ViewMode::All,
                        "unwatched" => ViewMode::Unwatched,
                        "recent" => ViewMode::RecentlyAdded,
                        _ => ViewMode::All,
                    };
                    sender_for_view.input(LibraryPageInput::SetViewMode(view_mode));
                }
            });

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

                // Save current filter state before switching libraries
                self.save_filter_state().await;

                self.library_id = Some(library_id.clone());
                self.loaded_count = 0;
                self.total_items.clear();
                self.has_loaded_all = false;
                self.needs_factory_clear = true;
                // Cancel pending images BEFORE clearing image_requests
                // Otherwise cancel_pending_images() has no requests to cancel
                self.cancel_pending_images();
                self.image_requests.clear();
                self.images_requested.clear();
                self.visible_start_idx = 0;
                self.visible_end_idx = 0;

                // Send view switcher bar to main window header
                // The view switcher bar provides navigation tabs in the header
                sender
                    .output(LibraryPageOutput::SetHeaderTitleWidget(
                        self.view_switcher_bar.clone().upcast(),
                    ))
                    .expect("Failed to send header widget");

                // Load saved filter state for the new library
                let library_id_for_config = library_id.clone();
                let sender_for_config = sender.clone();
                relm4::spawn_local(async move {
                    use crate::services::config_service::config_service;

                    // Try to load full filter state first
                    if let Some(saved_state_json) = config_service()
                        .get_library_filter_state(library_id_for_config.as_ref())
                        .await
                    {
                        if let Ok(state) = serde_json::from_str::<FilterState>(&saved_state_json) {
                            debug!(
                                "Restoring saved filter state for library {}",
                                library_id_for_config
                            );
                            // Apply the entire filter state
                            sender_for_config.input(LibraryPageInput::RestoreFilterState(state));
                            return;
                        }
                    }

                    // Fallback to loading just the view mode (backward compatibility)
                    if let Some(saved_mode) = config_service()
                        .get_library_filter_tab(library_id_for_config.as_ref())
                        .await
                    {
                        // Parse saved mode string back to ViewMode enum
                        let view_mode = match saved_mode.as_str() {
                            "All" => ViewMode::All,
                            "Unwatched" => ViewMode::Unwatched,
                            "RecentlyAdded" => ViewMode::RecentlyAdded,
                            _ => ViewMode::All, // Default to All for legacy Genres/Years
                        };
                        sender_for_config.input(LibraryPageInput::SetViewMode(view_mode));
                    }
                });

                self.load_all_items(sender.clone());
            }

            LibraryPageInput::RestoreFilterState(state) => {
                debug!("Restoring filter state: {:?}", state);
                self.apply_filter_state(&state);

                // Trigger a refresh with the restored state
                self.loaded_count = 0;
                self.needs_factory_clear = true;
                self.image_requests.clear();
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
                }

                // Initialize the rating popover
                if let Some(ref popover) = self.rating_popover {
                    self.update_rating_popover(popover, sender.clone());
                }

                // Initialize the watch status popover
                if let Some(ref popover) = self.watch_status_popover {
                    self.update_watch_status_popover(popover, sender.clone());
                }

                // Fetch playback progress for all items if watch status filter is active
                // or if viewing Unwatched tab (which also needs to determine watched status)
                let playback_progress_map = if self.watch_status_filter != WatchStatus::All
                    || self.selected_view_mode == ViewMode::Unwatched
                {
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

                // Apply text, genre, year range, rating, watch status, and tab filtering
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

                        // Determine watched status based on media type
                        // Used for both user-added watch status filter and Unwatched view mode
                        let is_watched = if item.media_type == "show" {
                            // For TV shows, check metadata for watched_episode_count
                            let watched_count =
                                item.metadata
                                    .as_ref()
                                    .and_then(|m| m.get("watched_episode_count"))
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0) as u32;
                            let total_count = item
                                .metadata
                                .as_ref()
                                .and_then(|m| m.get("total_episode_count"))
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0) as u32;

                            total_count > 0 && watched_count == total_count
                        } else {
                            // For movies and episodes, use playback_progress table
                            playback_progress_map
                                .get(&item.id)
                                .map_or(false, |progress| progress.watched)
                        };

                        // Watch status filter (user-added filter)
                        let watch_status_match = match self.watch_status_filter {
                            WatchStatus::All => true,
                            WatchStatus::Watched => is_watched,
                            WatchStatus::Unwatched => !is_watched,
                        };

                        // Filter by view mode (built-in immutable filters)
                        let tab_match = match self.selected_view_mode {
                            ViewMode::All => true,
                            ViewMode::Unwatched => {
                                // Show only unwatched items in Unwatched view
                                !is_watched
                            }
                            ViewMode::RecentlyAdded => {
                                // Show items added in the last 30 days
                                if let Some(added_at) = item.added_at {
                                    let now = chrono::Utc::now();
                                    let thirty_days_ago =
                                        (now - chrono::Duration::days(30)).naive_utc();
                                    added_at >= thirty_days_ago
                                } else {
                                    false
                                }
                            }
                        };

                        text_match
                            && genre_match
                            && year_match
                            && rating_match
                            && watch_status_match
                            && tab_match
                    })
                    .collect();

                // Store filtered items
                self.total_items = filtered_items;
                self.is_loading = false;

                // Clear image requests when loading new items
                self.images_requested.clear();

                // Update active filters display
                self.update_active_filters_display(sender.clone());

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

                    // Clear factory before rendering first batch if needed
                    // This defers the clear until filtered items are ready, minimizing flashing
                    if self.needs_factory_clear && start_idx == 0 {
                        self.media_factory.guard().clear();
                        self.needs_factory_clear = false;
                    }

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
                    .expect("Failed to send output");
            }

            LibraryPageInput::MarkWatched(media_id) => {
                debug!("Marking item as watched: {}", media_id);
                let db = self.db.clone();
                let media_id_clone = media_id.clone();

                sender.oneshot_command(async move {
                    use crate::services::commands::Command;
                    use crate::services::commands::media_commands::MarkWatchedCommand;

                    let cmd = MarkWatchedCommand {
                        db,
                        media_id: media_id_clone,
                    };

                    if let Err(e) = cmd.execute().await {
                        tracing::error!("Failed to mark item as watched: {}", e);
                    }
                });
            }

            LibraryPageInput::MarkUnwatched(media_id) => {
                debug!("Marking item as unwatched: {}", media_id);
                let db = self.db.clone();
                let media_id_clone = media_id.clone();

                sender.oneshot_command(async move {
                    use crate::services::commands::Command;
                    use crate::services::commands::media_commands::MarkUnwatchedCommand;

                    let cmd = MarkUnwatchedCommand {
                        db,
                        media_id: media_id_clone,
                    };

                    if let Err(e) = cmd.execute().await {
                        tracing::error!("Failed to mark item as unwatched: {}", e);
                    }
                });
            }

            LibraryPageInput::SetSortBy(sort_by) => {
                // Ignore sort changes in Recently Added view mode (immutable filter)
                if self.selected_view_mode == ViewMode::RecentlyAdded {
                    debug!(
                        "Ignoring sort change in Recently Added view - sort is always DateAdded"
                    );
                    return;
                }

                if self.sort_by != sort_by {
                    self.sort_by = sort_by;
                    self.save_filter_state().await;
                    self.refresh(sender.clone());
                }
            }

            LibraryPageInput::ToggleSortOrder => {
                // Ignore sort order changes in Recently Added view mode (immutable filter)
                if self.selected_view_mode == ViewMode::RecentlyAdded {
                    debug!(
                        "Ignoring sort order change in Recently Added view - sort is always DateAdded/Descending"
                    );
                    return;
                }

                self.sort_order = match self.sort_order {
                    SortOrder::Ascending => SortOrder::Descending,
                    SortOrder::Descending => SortOrder::Ascending,
                };
                self.save_filter_state().await;
                self.refresh(sender.clone());
            }

            LibraryPageInput::SetFilter(text) => {
                self.filter_text = text;
                self.save_filter_state().await;
                self.loaded_count = 0;
                self.needs_factory_clear = true;
                self.image_requests.clear();
                self.load_all_items(sender.clone());
            }

            LibraryPageInput::ShowSearch => {
                self.search_visible = true;
                // Focus the search entry when shown
                if let Some(root_widget) = root.first_child() {
                    if let Some(search_box) = root_widget
                        .last_child()
                        .and_then(|w| w.downcast::<gtk::Box>().ok())
                    {
                        if let Some(search_entry) = search_box
                            .first_child()
                            .and_then(|w| w.downcast::<gtk::SearchEntry>().ok())
                        {
                            search_entry.grab_focus();
                        }
                    }
                }
            }

            LibraryPageInput::HideSearch => {
                self.search_visible = false;
                // Clear filter text when hiding search
                if !self.filter_text.is_empty() {
                    self.filter_text.clear();
                    self.save_filter_state().await;
                    self.loaded_count = 0;
                    self.needs_factory_clear = true;
                    self.image_requests.clear();
                    self.load_all_items(sender.clone());
                }
            }

            LibraryPageInput::ToggleGenreFilter(genre) => {
                if self.selected_genres.contains(&genre) {
                    self.selected_genres.retain(|g| g != &genre);
                } else {
                    self.selected_genres.push(genre);
                }
                self.save_filter_state().await;
                self.loaded_count = 0;
                self.needs_factory_clear = true;
                self.image_requests.clear();
                self.load_all_items(sender.clone());
            }

            LibraryPageInput::ClearGenreFilters => {
                self.selected_genres.clear();
                self.save_filter_state().await;
                self.loaded_count = 0;
                self.needs_factory_clear = true;
                self.image_requests.clear();
                self.load_all_items(sender.clone());
            }

            LibraryPageInput::SetYearRange { min, max } => {
                self.selected_min_year = min;
                self.selected_max_year = max;
                self.save_filter_state().await;
                self.loaded_count = 0;
                self.needs_factory_clear = true;
                self.image_requests.clear();
                self.load_all_items(sender.clone());
            }

            LibraryPageInput::ClearYearRange => {
                self.selected_min_year = None;
                self.selected_max_year = None;
                self.save_filter_state().await;
                self.loaded_count = 0;
                self.needs_factory_clear = true;
                self.image_requests.clear();
                self.load_all_items(sender.clone());
            }

            LibraryPageInput::SetRatingFilter(rating) => {
                self.min_rating = rating;
                self.save_filter_state().await;
                self.loaded_count = 0;
                self.needs_factory_clear = true;
                self.image_requests.clear();
                self.load_all_items(sender.clone());
            }

            LibraryPageInput::ClearRatingFilter => {
                self.min_rating = None;
                self.save_filter_state().await;
                self.loaded_count = 0;
                self.needs_factory_clear = true;
                self.image_requests.clear();
                self.load_all_items(sender.clone());
            }

            LibraryPageInput::SetWatchStatusFilter(status) => {
                self.watch_status_filter = status;
                self.save_filter_state().await;
                self.loaded_count = 0;
                self.needs_factory_clear = true;
                self.image_requests.clear();
                self.load_all_items(sender.clone());
            }

            LibraryPageInput::ClearWatchStatusFilter => {
                self.watch_status_filter = WatchStatus::All;
                self.save_filter_state().await;
                self.loaded_count = 0;
                self.needs_factory_clear = true;
                self.image_requests.clear();
                self.load_all_items(sender.clone());
            }

            LibraryPageInput::SetMediaTypeFilter(media_type) => {
                self.selected_media_type = media_type;
                self.save_filter_state().await;
                self.loaded_count = 0;
                self.needs_factory_clear = true;
                self.image_requests.clear();
                self.load_all_items(sender.clone());
            }

            LibraryPageInput::Refresh => {
                self.refresh(sender.clone());
            }

            LibraryPageInput::ToggleFiltersPopover => {
                if let Some(ref popover) = self.filters_popover {
                    // Update the popover content before showing
                    self.update_unified_filters_popover(sender.clone());

                    if popover.is_visible() {
                        popover.popdown();
                    } else {
                        popover.popup();
                    }
                }
            }

            LibraryPageInput::ClearAllFilters => {
                self.filter_text.clear();
                self.selected_genres.clear();
                self.selected_min_year = None;
                self.selected_max_year = None;
                self.min_rating = None;
                self.watch_status_filter = WatchStatus::All;
                self.selected_view_mode = ViewMode::All;
                self.save_filter_state().await;
                self.loaded_count = 0;
                self.needs_factory_clear = true;
                self.image_requests.clear();
                self.load_all_items(sender.clone());
            }

            LibraryPageInput::RemoveFilter(filter_type) => {
                match filter_type {
                    ActiveFilterType::Text => {
                        self.filter_text.clear();
                    }
                    ActiveFilterType::Genre(genre) => {
                        self.selected_genres.retain(|g| g != &genre);
                    }
                    ActiveFilterType::YearRange => {
                        self.selected_min_year = None;
                        self.selected_max_year = None;
                    }
                    ActiveFilterType::Rating => {
                        self.min_rating = None;
                    }
                    ActiveFilterType::WatchStatus => {
                        self.watch_status_filter = WatchStatus::All;
                    }
                }
                self.save_filter_state().await;
                self.loaded_count = 0;
                self.needs_factory_clear = true;
                self.image_requests.clear();
                self.load_all_items(sender.clone());
            }

            LibraryPageInput::SetViewMode(mode) => {
                if self.selected_view_mode != mode {
                    self.selected_view_mode = mode;

                    // Recently Added is an immutable filter - always use DateAdded/Descending
                    if mode == ViewMode::RecentlyAdded {
                        self.sort_by = SortBy::DateAdded;
                        self.sort_order = SortOrder::Descending;
                    } else {
                        // Restore sort preferences for other view modes
                        if let Some(prefs) = self.view_mode_sort_prefs.get(&mode) {
                            self.sort_by = prefs.sort_by;
                            self.sort_order = prefs.sort_order;
                        }
                    }

                    self.save_filter_state().await;

                    // Update the view stack selection to match
                    match mode {
                        ViewMode::All => self.view_stack.set_visible_child_name("all"),
                        ViewMode::Unwatched => self.view_stack.set_visible_child_name("unwatched"),
                        ViewMode::RecentlyAdded => self.view_stack.set_visible_child_name("recent"),
                    }

                    // Refresh the view with the new mode
                    self.loaded_count = 0;
                    self.needs_factory_clear = true;
                    self.image_requests.clear();
                    self.load_all_items(sender.clone());
                }
            }

            LibraryPageInput::ImageLoaded { id, texture } => {
                trace!("Image loaded for item: {}", id);
                if let Some(&index) = self.image_requests.get(&id) {
                    self.media_factory
                        .send(index, MediaCardInput::ImageLoaded(texture));
                }
            }

            LibraryPageInput::ImageLoadFailed { id } => {
                trace!("Image load failed for item: {}", id);
                if let Some(&index) = self.image_requests.get(&id) {
                    self.media_factory
                        .send(index, MediaCardInput::ImageLoadFailed);
                }
            }

            LibraryPageInput::ViewportScrolled => {
                // Cancel any pending debounce timer
                if let Some(handle) = self.scroll_debounce_handle.take() {
                    handle.remove();
                }

                // Set a new debounce timer
                let sender_clone = sender.clone();
                let handle = gtk::glib::timeout_add_local(Duration::from_millis(150), move || {
                    sender_clone.input(LibraryPageInput::ProcessDebouncedScroll);
                    gtk::glib::ControlFlow::Break
                });
                self.scroll_debounce_handle = Some(handle);
            }

            LibraryPageInput::ProcessDebouncedScroll => {
                self.scroll_debounce_handle = None;
                self.update_visible_range(root);
                sender.input(LibraryPageInput::LoadVisibleImages);
            }

            LibraryPageInput::LoadVisibleImages => {
                self.load_images_for_visible_range();
            }

            LibraryPageInput::BrokerMsg(msg) => {
                match msg {
                    BrokerMessage::Config(crate::ui::shared::broker::ConfigMessage::Updated {
                        ..
                    }) => {
                        // Reload filter state when config changes
                        if let Some(ref library_id) = self.library_id {
                            let library_id_clone = library_id.clone();
                            let sender_clone = sender.clone();
                            relm4::spawn_local(async move {
                                use crate::services::config_service::config_service;

                                if let Some(saved_state_json) = config_service()
                                    .get_library_filter_state(library_id_clone.as_ref())
                                    .await
                                {
                                    if let Ok(state) =
                                        serde_json::from_str::<FilterState>(&saved_state_json)
                                    {
                                        sender_clone
                                            .input(LibraryPageInput::RestoreFilterState(state));
                                    }
                                }
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn shutdown(&mut self, widgets: &mut Self::Widgets, _output: relm4::Sender<Self::Output>) {
        // Disconnect scroll handler to prevent signals firing after component is destroyed
        if let Some(handler_id) = self.scroll_handler_id.take() {
            let adjustment = widgets.scrolled_window.vadjustment();
            adjustment.disconnect(handler_id);
            debug!("Disconnected scroll handler on library page shutdown");
        }

        // Clean up any active debounce timer
        if let Some(handle) = self.scroll_debounce_handle.take() {
            handle.remove();
            debug!("Removed scroll debounce timer on library page shutdown");
        }

        // Unsubscribe from MessageBroker
        relm4::spawn(async move {
            BROKER.unsubscribe("LibraryPage").await;
        });
    }
}
