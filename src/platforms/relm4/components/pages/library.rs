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
            .field("current_page", &self.current_page)
            .field("items_per_page", &self.items_per_page)
            .field("has_more", &self.has_more)
            .field("view_mode", &self.view_mode)
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
    current_page: usize,
    items_per_page: usize,
    has_more: bool,
    view_mode: ViewMode,
    sort_by: SortBy,
    filter_text: String,
    search_visible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewMode {
    Grid,
    List,
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
    /// Load the next page of items
    LoadMore,
    /// Media items loaded
    MediaItemsLoaded {
        items: Vec<MediaItemModel>,
        has_more: bool,
    },
    /// Media items loaded with progress
    MediaItemsLoadedWithProgress {
        items: Vec<(MediaItemModel, bool, f64)>,
        has_more: bool,
    },
    /// Media item selected
    MediaItemSelected(MediaItemId),
    /// Change view mode
    SetViewMode(ViewMode),
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
            gtk::ScrolledWindow {
                set_vexpand: true,
                set_hscrollbar_policy: gtk::PolicyType::Never,
                set_vscrollbar_policy: gtk::PolicyType::Automatic,

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
                        set_visible: model.is_loading,

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
            current_page: 0,
            items_per_page: 50,
            has_more: false,
            view_mode: ViewMode::Grid,
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
                self.current_page = 0;
                self.media_factory.guard().clear();
                self.load_page(sender.clone());
            }

            LibraryPageInput::LoadMore => {
                if !self.is_loading && self.has_more {
                    debug!("Loading more items");
                    self.current_page += 1;
                    self.load_page(sender.clone());
                }
            }

            LibraryPageInput::MediaItemsLoaded { items, has_more } => {
                debug!("Loaded {} items", items.len());

                let mut factory_guard = self.media_factory.guard();
                for item in items {
                    let index = factory_guard.push_back(MediaCardInit {
                        item: item.clone(),
                        show_progress: false,
                        watched: false,
                        progress_percent: 0.0,
                    });

                    // Request image loading if poster URL exists
                    if let Some(poster_url) = &item.poster_url {
                        let id = item.id.clone();
                        self.image_requests
                            .insert(id.clone(), index.current_index());

                        self.image_loader
                            .emit(ImageLoaderInput::LoadImage(ImageRequest {
                                id,
                                url: poster_url.clone(),
                                size: ImageSize::Card,
                                priority: 5,
                            }));
                    }
                }

                self.has_more = has_more;
                self.is_loading = false;
            }

            LibraryPageInput::MediaItemsLoadedWithProgress { items, has_more } => {
                debug!("Loaded {} items with progress", items.len());

                let mut factory_guard = self.media_factory.guard();
                for (item, watched, progress_percent) in items {
                    let index = factory_guard.push_back(MediaCardInit {
                        item: item.clone(),
                        show_progress: progress_percent > 0.0 && progress_percent < 0.9,
                        watched,
                        progress_percent,
                    });

                    // Request image loading if poster URL exists
                    if let Some(poster_url) = &item.poster_url {
                        let id = item.id.clone();
                        self.image_requests
                            .insert(id.clone(), index.current_index());

                        self.image_loader
                            .emit(ImageLoaderInput::LoadImage(ImageRequest {
                                id,
                                url: poster_url.clone(),
                                size: ImageSize::Card,
                                priority: 5,
                            }));
                    }
                }

                self.has_more = has_more;
                self.is_loading = false;
            }

            LibraryPageInput::MediaItemSelected(item_id) => {
                debug!("Media item selected: {}", item_id);
                sender
                    .output(LibraryPageOutput::NavigateToMediaItem(item_id))
                    .unwrap();
            }

            LibraryPageInput::SetViewMode(mode) => {
                debug!("Setting view mode: {:?}", mode);
                self.view_mode = mode;
                // TODO: Update FlowBox layout based on view mode
            }

            LibraryPageInput::SetSortBy(sort_by) => {
                debug!("Setting sort by: {:?}", sort_by);
                self.sort_by = sort_by;
                self.refresh(sender.clone());
            }

            LibraryPageInput::SetFilter(filter) => {
                debug!("Setting filter: {}", filter);
                self.filter_text = filter;
                self.refresh(sender.clone());
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
                // Remove from tracking but keep card functional without image
                self.image_requests.remove(&id);
            }
        }
    }
}

impl LibraryPage {
    fn load_page(&mut self, sender: AsyncComponentSender<Self>) {
        if let Some(library_id) = &self.library_id {
            self.is_loading = true;

            let db = self.db.clone();
            let library_id = library_id.clone();
            let page = self.current_page;
            let items_per_page = self.items_per_page;
            let sort_by = self.sort_by;
            let filter = self.filter_text.clone();

            relm4::spawn(async move {
                use crate::db::repository::{
                    MediaRepository, MediaRepositoryImpl, PlaybackRepository,
                    PlaybackRepositoryImpl,
                };
                let media_repo = MediaRepositoryImpl::new(db.clone());
                let playback_repo = PlaybackRepositoryImpl::new(db.clone());

                // Calculate offset
                let offset = page * items_per_page;

                // Get items for this library with pagination
                match media_repo
                    .find_by_library_paginated(
                        &library_id.to_string(),
                        offset as u64,
                        items_per_page as u64,
                    )
                    .await
                {
                    Ok(items) => {
                        let has_more = items.len() == items_per_page;

                        // Apply client-side filtering if needed
                        let mut filtered_items = if filter.is_empty() {
                            items
                        } else {
                            items
                                .into_iter()
                                .filter(|item| {
                                    item.title.to_lowercase().contains(&filter.to_lowercase())
                                })
                                .collect()
                        };

                        // Load playback progress for each item
                        let items_with_progress: Vec<(MediaItemModel, bool, f64)> = {
                            let mut result = Vec::new();
                            for item in filtered_items {
                                let progress = playback_repo
                                    .find_by_media_id(&item.id)
                                    .await
                                    .ok()
                                    .flatten();
                                let watched = progress.as_ref().map(|p| p.watched).unwrap_or(false);
                                let progress_percent = progress
                                    .as_ref()
                                    .map(|p| p.get_progress_percentage() as f64)
                                    .unwrap_or(0.0);
                                result.push((item, watched, progress_percent));
                            }
                            result
                        };

                        sender.input(LibraryPageInput::MediaItemsLoadedWithProgress {
                            items: items_with_progress,
                            has_more,
                        });
                    }
                    Err(e) => {
                        error!("Failed to load library items: {}", e);
                        sender.input(LibraryPageInput::MediaItemsLoadedWithProgress {
                            items: Vec::new(),
                            has_more: false,
                        });
                    }
                }
            });
        }
    }

    fn refresh(&mut self, sender: AsyncComponentSender<Self>) {
        self.current_page = 0;
        self.media_factory.guard().clear();
        self.load_page(sender);
    }
}
