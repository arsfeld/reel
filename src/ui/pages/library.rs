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
    /// Set media type filter (for mixed libraries)
    SetMediaTypeFilter(Option<String>),
    /// Clear all items and reload
    Refresh,
    /// Show search bar
    ShowSearch,
    /// Hide search bar
    HideSearch,
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

                    // Genre filter button with dropdown
                    #[name = "genre_menu_button"]
                    gtk::MenuButton {
                        set_icon_name: "view-filter-symbolic",
                        set_always_show_arrow: true,
                        set_tooltip_text: Some("Filter by genre"),
                        #[watch]
                        set_visible: !model.available_genres.is_empty(),
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
                    set_visible: model.library_type.as_ref().map_or(false, |t| t == "mixed"),
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
                        set_active: model.selected_media_type.as_ref().map_or(false, |t| t == "movie"),
                        connect_toggled[sender] => move |btn| {
                            if btn.is_active() {
                                sender.input(LibraryPageInput::SetMediaTypeFilter(Some("movie".to_string())));
                            }
                        }
                    },

                    gtk::ToggleButton {
                        set_label: "Shows",
                        #[watch]
                        set_active: model.selected_media_type.as_ref().map_or(false, |t| t == "show"),
                        connect_toggled[sender] => move |btn| {
                            if btn.is_active() {
                                sender.input(LibraryPageInput::SetMediaTypeFilter(Some("show".to_string())));
                            }
                        }
                    },

                    gtk::ToggleButton {
                        set_label: "Music",
                        #[watch]
                        set_active: model.selected_media_type.as_ref().map_or(false, |t| t == "album"),
                        connect_toggled[sender] => move |btn| {
                            if btn.is_active() {
                                sender.input(LibraryPageInput::SetMediaTypeFilter(Some("album".to_string())));
                            }
                        }
                    },

                    gtk::ToggleButton {
                        set_label: "Photos",
                        #[watch]
                        set_active: model.selected_media_type.as_ref().map_or(false, |t| t == "photo"),
                        connect_toggled[sender] => move |btn| {
                            if btn.is_active() {
                                sender.input(LibraryPageInput::SetMediaTypeFilter(Some("photo".to_string())));
                            }
                        }
                    },
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
        };

        let mut model = model;

        let widgets = view_output!();

        // Create and set the genre filter popover
        let genre_popover = gtk::Popover::new();
        genre_popover.set_child(Some(&gtk::Box::new(gtk::Orientation::Vertical, 0)));
        widgets.genre_menu_button.set_popover(Some(&genre_popover));
        widgets
            .genre_menu_button
            .set_label(&model.get_genre_label());
        model.genre_popover = Some(genre_popover);
        model.genre_menu_button = Some(widgets.genre_menu_button.clone());

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
                if !self.available_genres.is_empty() {
                    if let Some(ref popover) = self.genre_popover {
                        self.update_genre_popover(popover, sender.clone());
                    }
                }

                // Apply text and genre filtering
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

                        text_match && genre_match
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

                            // Use the pre-fetched playback progress data
                            let (watched, progress_percent) =
                                if let Some(progress) = playback_progress_map.get(&item.id) {
                                    (progress.watched, progress.get_progress_percentage() as f64)
                                } else {
                                    (false, 0.0) // No progress record means unwatched
                                };

                            let index = factory_guard.push_back(MediaCardInit {
                                item: item.clone(),
                                show_progress: false,
                                watched,
                                progress_percent,
                                show_media_type_icon: self
                                    .library_type
                                    .as_ref()
                                    .map_or(false, |t| t == "mixed"),
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
        }
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
    fn get_genre_label(&self) -> String {
        if self.selected_genres.is_empty() {
            "All Genres".to_string()
        } else if self.selected_genres.len() == 1 {
            self.selected_genres[0].clone()
        } else {
            format!("{} genres", self.selected_genres.len())
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
            if idx < load_start || idx >= load_end {
                if idx < self.total_items.len() {
                    let item_id = &self.total_items[idx].id;
                    if self.image_requests.contains_key(item_id) {
                        to_cancel.push(item_id.clone());
                    }
                }
            }
        }

        // Cancel out-of-range images
        for id in to_cancel {
            trace!("Cancelling image load for out-of-range item: {}", id);
            self.image_loader
                .emit(ImageLoaderInput::CancelLoad { id: id.clone() });
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

                    self.image_loader
                        .emit(ImageLoaderInput::LoadImage(ImageRequest {
                            id: id.clone(),
                            url: poster_url.clone(),
                            size: ImageSize::Thumbnail,
                            priority,
                        }));

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
            self.image_loader
                .emit(ImageLoaderInput::CancelLoad { id: id.clone() });
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
                let library_result = library_repo.find_by_id(&library_id.to_string()).await;

                let (library_type, media_result) = match library_result {
                    Ok(Some(library)) => {
                        let lib_type = library.library_type.to_lowercase();

                        // For mixed libraries, check if we have a media type filter
                        let media_result = if lib_type == "mixed" {
                            // Use the selected media type filter if set
                            if let Some(media_type) = selected_media_type {
                                media_repo
                                    .find_by_library_and_type(&library_id.to_string(), &media_type)
                                    .await
                            } else {
                                // Get all items if no filter is set
                                media_repo.find_by_library(&library_id.to_string()).await
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
                                    .find_by_library_and_type(&library_id.to_string(), media_type)
                                    .await
                            } else {
                                // For unknown types, get all items
                                media_repo.find_by_library(&library_id.to_string()).await
                            }
                        };

                        (Some(lib_type), media_result)
                    }
                    _ => {
                        // If we can't get library info, get all items
                        (
                            None,
                            media_repo.find_by_library(&library_id.to_string()).await,
                        )
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
