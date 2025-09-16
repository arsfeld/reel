use gtk::prelude::*;
use libadwaita as adw;
use relm4::factory::FactoryVecDeque;
use relm4::gtk;
use relm4::prelude::*;
use tracing::{debug, error};

use crate::db::connection::DatabaseConnection;
use crate::db::entities::MediaItemModel;
use crate::models::{LibraryId, MediaItemId};
use crate::platforms::relm4::components::factories::media_card::{
    MediaCard, MediaCardInit, MediaCardInput, MediaCardOutput,
};
use crate::platforms::relm4::components::workers::{
    ImageLoader, ImageLoaderInput, ImageLoaderOutput, ImageRequest, ImageSize,
};
use relm4::Worker;

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
    filter_text: String,
    search_visible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortBy {
    Title,
    Year,
    DateAdded,
    Rating,
}

#[derive(Debug)]
pub enum LibraryPageInput {
    /// Set the library to display
    SetLibrary(LibraryId),
    /// Load more items into view
    LoadMoreBatch,
    /// All media items loaded from database
    AllItemsLoaded { items: Vec<MediaItemModel> },
    /// Render next batch of items
    RenderBatch,
    /// Media item selected
    MediaItemSelected(MediaItemId),
    /// Change sort order
    SetSortBy(SortBy),
    /// Filter by text
    SetFilter(String),
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

            // Main content
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
            filter_text: String::new(),
            search_visible: false,
        };

        let widgets = view_output!();

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        msg: Self::Input,
        sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
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
                self.load_all_items(sender.clone());
            }

            LibraryPageInput::LoadMoreBatch => {
                if !self.is_loading && !self.has_loaded_all && !self.total_items.is_empty() {
                    debug!("Loading more items into view");
                    sender.input(LibraryPageInput::RenderBatch);
                }
            }

            LibraryPageInput::AllItemsLoaded { items } => {
                debug!("Loaded all {} items from database", items.len());

                // Apply filtering if needed
                let filtered_items: Vec<MediaItemModel> = if self.filter_text.is_empty() {
                    items
                } else {
                    items
                        .into_iter()
                        .filter(|item| {
                            item.title
                                .to_lowercase()
                                .contains(&self.filter_text.to_lowercase())
                        })
                        .collect()
                };

                // Store filtered items
                self.total_items = filtered_items;
                self.is_loading = false;

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
                    debug!("Rendering items {} to {}", start_idx, end_idx);

                    let mut factory_guard = self.media_factory.guard();
                    for idx in start_idx..end_idx {
                        let item = &self.total_items[idx];
                        let index = factory_guard.push_back(MediaCardInit {
                            item: item.clone(),
                            show_progress: false,
                            watched: false,
                            progress_percent: 0.0,
                        });

                        // Request image loading with priority based on position
                        if let Some(poster_url) = &item.poster_url {
                            let id = item.id.clone();
                            self.image_requests
                                .insert(id.clone(), index.current_index());

                            // Higher priority for items closer to the top
                            let priority = if idx < 20 {
                                10
                            } else if idx < 50 {
                                5
                            } else {
                                1
                            };

                            self.image_loader
                                .emit(ImageLoaderInput::LoadImage(ImageRequest {
                                    id,
                                    url: poster_url.clone(),
                                    size: ImageSize::Thumbnail,
                                    priority,
                                }));
                        }
                    }

                    self.loaded_count = end_idx;
                    self.has_loaded_all = end_idx >= self.total_items.len();
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
                    });
                } else {
                    self.load_all_items(sender.clone());
                }
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
        }
    }
}

impl LibraryPage {
    fn load_all_items(&mut self, sender: AsyncComponentSender<Self>) {
        if let Some(library_id) = &self.library_id {
            self.is_loading = true;

            let db = self.db.clone();
            let library_id = library_id.clone();
            let sort_by = self.sort_by;

            relm4::spawn(async move {
                use crate::db::repository::{
                    LibraryRepository, LibraryRepositoryImpl, MediaRepository, MediaRepositoryImpl,
                    Repository,
                };
                let library_repo = LibraryRepositoryImpl::new(db.clone());
                let media_repo = MediaRepositoryImpl::new(db.clone());

                // First, get the library to determine its type
                let library_result = library_repo.find_by_id(&library_id.to_string()).await;

                let media_result = match library_result {
                    Ok(Some(library)) => {
                        // Determine the appropriate media type filter based on library type
                        let media_type = match library.library_type.to_lowercase().as_str() {
                            "movies" => Some("movie"),
                            "shows" => Some("show"),
                            "music" => Some("album"), // For music libraries, show albums, not individual tracks
                            _ => None,                // For mixed or unknown types, get all items
                        };

                        // Get ALL items for this library without pagination
                        if let Some(media_type) = media_type {
                            media_repo
                                .find_by_library_and_type(&library_id.to_string(), media_type)
                                .await
                        } else {
                            // For mixed or unknown types, get all items
                            media_repo.find_by_library(&library_id.to_string()).await
                        }
                    }
                    _ => {
                        // If we can't get library info, get all items
                        media_repo.find_by_library(&library_id.to_string()).await
                    }
                };

                match media_result {
                    Ok(mut items) => {
                        // Sort items based on sort criteria
                        match sort_by {
                            SortBy::Title => {
                                items.sort_by(|a, b| a.sort_title.cmp(&b.sort_title));
                            }
                            SortBy::Year => {
                                items.sort_by(|a, b| b.year.cmp(&a.year));
                            }
                            SortBy::DateAdded => {
                                items.sort_by(|a, b| b.added_at.cmp(&a.added_at));
                            }
                            SortBy::Rating => {
                                items.sort_by(|a, b| {
                                    b.rating
                                        .partial_cmp(&a.rating)
                                        .unwrap_or(std::cmp::Ordering::Equal)
                                });
                            }
                        }

                        sender.input(LibraryPageInput::AllItemsLoaded { items });
                    }
                    Err(e) => {
                        error!("Failed to load library items: {}", e);
                        sender.input(LibraryPageInput::AllItemsLoaded { items: Vec::new() });
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
        self.load_all_items(sender);
    }
}
