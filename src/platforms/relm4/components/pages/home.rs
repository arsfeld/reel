use gtk::prelude::*;
use relm4::Worker;
use relm4::factory::FactoryVecDeque;
use relm4::gtk;
use relm4::prelude::*;
use std::collections::HashMap;
use tracing::{debug, error, info, trace};

use crate::db::connection::DatabaseConnection;
use crate::db::entities::MediaItemModel;
use crate::models::{HomeSection, HomeSectionType, MediaItem, MediaItemId, SourceId};
use crate::platforms::relm4::components::factories::media_card::{
    MediaCard, MediaCardInit, MediaCardInput, MediaCardOutput,
};
use crate::platforms::relm4::components::workers::{
    ImageLoader, ImageLoaderInput, ImageLoaderOutput, ImageRequest, ImageSize,
};
use crate::services::core::BackendService;
use std::time::Duration;
use tokio::time::timeout;

#[derive(Debug, Clone)]
pub enum SectionLoadState {
    Loading,
    Loaded(Vec<HomeSection>),
    Failed(String), // Error message
}

pub struct HomePage {
    db: DatabaseConnection,
    sections: Vec<HomeSection>,
    section_factories: HashMap<String, FactoryVecDeque<MediaCard>>,
    sections_container: gtk::Box,
    image_loader: relm4::WorkerController<ImageLoader>,
    image_requests: HashMap<String, (String, usize)>, // item_id -> (section_id, card_index)
    is_loading: bool,
    source_states: HashMap<SourceId, SectionLoadState>, // Track per-source loading states
    loading_containers: HashMap<SourceId, gtk::Box>,    // UI containers for loading/error states
}

impl std::fmt::Debug for HomePage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HomePage")
            .field("sections", &self.sections.len())
            .field("is_loading", &self.is_loading)
            .field("image_requests", &self.image_requests.len())
            .finish()
    }
}

#[derive(Debug)]
pub enum HomePageInput {
    /// Load home page data
    LoadData,
    /// Home sections loaded from backends
    HomeSectionsLoaded(Vec<HomeSection>),
    /// Source-specific sections loaded
    SourceSectionsLoaded {
        source_id: SourceId,
        sections: Result<Vec<HomeSection>, String>,
    },
    /// Retry loading a specific source
    RetrySource(SourceId),
    /// Media item selected
    MediaItemSelected(MediaItemId),
    /// Image loaded from worker
    ImageLoaded {
        id: String,
        texture: gtk::gdk::Texture,
    },
    /// Image load failed
    ImageLoadFailed { id: String },
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

            // Scrollable content
            gtk::ScrolledWindow {
                set_vexpand: true,
                set_hscrollbar_policy: gtk::PolicyType::Never,

                #[local_ref]
                sections_container -> gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_margin_all: 24,
                    set_spacing: 48,

                    // Loading indicator
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 12,
                        set_valign: gtk::Align::Center,
                        set_vexpand: true,
                        #[watch]
                        set_visible: model.is_loading,

                        gtk::Spinner {
                            set_spinning: true,
                        },

                        gtk::Label {
                            set_text: "Loading home sections...",
                        },
                    },

                    // Empty state
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 12,
                        set_valign: gtk::Align::Center,
                        set_vexpand: true,
                        #[watch]
                        set_visible: !model.is_loading && model.sections.is_empty(),

                        gtk::Image {
                            set_icon_name: Some("user-home-symbolic"),
                            set_pixel_size: 64,
                            add_css_class: "dim-label",
                        },

                        gtk::Label {
                            set_text: "No content available",
                            add_css_class: "title-2",
                            add_css_class: "dim-label",
                        },

                        gtk::Label {
                            set_text: "Add a media source to see content here",
                            add_css_class: "dim-label",
                        },
                    },
                },
            },
        }
    }

    async fn init(
        db: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let sections_container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(48)
            .build();

        // Create the image loader worker
        let image_loader =
            ImageLoader::builder()
                .detach_worker(())
                .forward(sender.input_sender(), |output| match output {
                    ImageLoaderOutput::ImageLoaded { id, texture, .. } => {
                        HomePageInput::ImageLoaded { id, texture }
                    }
                    ImageLoaderOutput::LoadFailed { id, .. } => {
                        HomePageInput::ImageLoadFailed { id }
                    }
                    ImageLoaderOutput::CacheCleared => HomePageInput::LoadData,
                });

        let model = Self {
            db,
            sections: Vec::new(),
            section_factories: HashMap::new(),
            sections_container: sections_container.clone(),
            image_loader,
            image_requests: HashMap::new(),
            is_loading: false, // Start with no loading state for offline-first
            source_states: HashMap::new(),
            loading_containers: HashMap::new(),
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
                debug!("Loading home page data - offline-first approach");

                // Clear existing sections
                self.clear_sections();

                // Clone database for async operations
                let db = self.db.clone();
                let sender_clone = sender.clone();

                // First, load cached data immediately (offline-first)
                relm4::spawn(async move {
                    info!("Loading cached home sections from database");

                    // Load cached sections synchronously for instant display
                    let cached_sections = BackendService::get_cached_home_sections(&db).await;

                    // Display cached sections immediately
                    for (source_id, sections) in cached_sections {
                        if !sections.is_empty() {
                            info!(
                                "Displaying {} cached sections for source {}",
                                sections.len(),
                                source_id
                            );
                            sender_clone.input(HomePageInput::SourceSectionsLoaded {
                                source_id: source_id.clone(),
                                sections: Ok(sections),
                            });
                        }
                    }

                    // Then, trigger background API updates (non-blocking)
                    info!("Starting background API refresh");
                    let source_results = BackendService::get_home_sections_per_source(&db).await;

                    // Send individual source results (will update/replace cached data)
                    for (source_id, result) in source_results {
                        let sections_result = match result {
                            Ok(sections) => {
                                info!(
                                    "API: Source {} loaded {} sections",
                                    source_id,
                                    sections.len()
                                );
                                Ok(sections)
                            }
                            Err(e) => {
                                error!(
                                    "API: Source {} failed: {} - keeping cached data",
                                    source_id, e
                                );
                                // Don't send error if we already have cached data
                                continue;
                            }
                        };

                        sender_clone.input(HomePageInput::SourceSectionsLoaded {
                            source_id,
                            sections: sections_result,
                        });
                    }
                });
            }

            HomePageInput::SourceSectionsLoaded {
                source_id,
                sections,
            } => {
                match sections {
                    Ok(sections) => {
                        info!("Source {} loaded {} sections", source_id, sections.len());

                        // Check if we already have sections from this source (from cache)
                        let had_cached_sections = self
                            .source_states
                            .get(&source_id)
                            .map(|state| matches!(state, SectionLoadState::Loaded(_)))
                            .unwrap_or(false);

                        // Update source state to loaded
                        self.source_states.insert(
                            source_id.clone(),
                            SectionLoadState::Loaded(sections.clone()),
                        );

                        // Remove loading container if it exists
                        if let Some(container) = self.loading_containers.remove(&source_id) {
                            self.sections_container.remove(&container);
                        }

                        // If we had cached sections, clear them before displaying fresh ones
                        if had_cached_sections {
                            self.clear_source_sections(&source_id);
                        }

                        // Process and display sections for this source
                        self.display_source_sections(&source_id, sections, &sender);
                    }
                    Err(error) => {
                        error!("Source {} failed with error: {}", source_id, error);

                        // Update source state to failed
                        self.source_states
                            .insert(source_id.clone(), SectionLoadState::Failed(error.clone()));

                        // Remove loading container if it exists
                        if let Some(container) = self.loading_containers.remove(&source_id) {
                            self.sections_container.remove(&container);
                        }

                        // Create error UI for this source
                        self.display_source_error(&source_id, &error, &sender);
                    }
                }

                // Check if all sources have finished loading
                self.update_overall_loading_state();
            }

            HomePageInput::RetrySource(source_id) => {
                info!("Retrying source {}", source_id);

                // Update state to loading
                self.source_states
                    .insert(source_id.clone(), SectionLoadState::Loading);

                // Remove error container if it exists
                if let Some(container) = self.loading_containers.remove(&source_id) {
                    self.sections_container.remove(&container);
                }

                // Show loading UI
                self.display_source_loading(&source_id);

                // Clone for async operation
                let db = self.db.clone();
                let source_id_clone = source_id.clone();
                let sender_clone = sender.clone();

                // Retry loading for this specific source
                relm4::spawn(async move {
                    // Get the source entity
                    use crate::db::repository::{
                        Repository,
                        source_repository::{SourceRepository, SourceRepositoryImpl},
                    };
                    let source_repo = SourceRepositoryImpl::new(db.clone());

                    if let Ok(Some(source_entity)) =
                        source_repo.find_by_id(source_id_clone.as_str()).await
                    {
                        // Try to load sections with timeout
                        // Note: We'll just reuse the get_home_sections_per_source method for the specific source
                        let all_results = BackendService::get_home_sections_per_source(&db).await;

                        // Find the result for this specific source
                        let sections_result = all_results
                            .into_iter()
                            .find(|(id, _)| id == &source_id_clone)
                            .map(|(_, result)| match result {
                                Ok(sections) => Ok(sections),
                                Err(e) => Err(e.to_string()),
                            })
                            .unwrap_or_else(|| Err("Failed to retry source".to_string()));

                        sender_clone.input(HomePageInput::SourceSectionsLoaded {
                            source_id: source_id_clone,
                            sections: sections_result,
                        });
                    } else {
                        sender_clone.input(HomePageInput::SourceSectionsLoaded {
                            source_id: source_id_clone,
                            sections: Err("Source not found".to_string()),
                        });
                    }
                });
            }

            HomePageInput::HomeSectionsLoaded(sections) => {
                // Legacy handler for backward compatibility
                info!(
                    "Processing {} home sections for display (legacy)",
                    sections.len()
                );
                self.sections = sections;
                self.is_loading = false;

                // This is now handled in display_source_sections method
                // Legacy code path should not be reached in normal operation
            }

            HomePageInput::MediaItemSelected(item_id) => {
                debug!("Media item selected: {}", item_id);
                sender
                    .output(HomePageOutput::NavigateToMediaItem(item_id))
                    .unwrap();
            }

            HomePageInput::ImageLoaded { id, texture } => {
                trace!("Image loaded for item: {}", id);
                // Find the section and card index for this image
                if let Some((section_id, card_idx)) = self.image_requests.get(&id) {
                    if let Some(factory) = self.section_factories.get(section_id) {
                        // Send the texture to the specific card
                        factory.send(*card_idx, MediaCardInput::ImageLoaded(texture));
                    }
                }
            }

            HomePageInput::ImageLoadFailed { id } => {
                debug!("Failed to load image for item: {}", id);
                // Find the section and card index for this image
                if let Some((section_id, card_idx)) = self.image_requests.get(&id) {
                    if let Some(factory) = self.section_factories.get(section_id) {
                        // Notify the card that the image failed to load
                        factory.send(*card_idx, MediaCardInput::ImageLoadFailed);
                    }
                }
                // Remove from tracking
                self.image_requests.remove(&id);
            }
        }
    }
}

impl HomePage {
    /// Display loading state for a source
    fn display_source_loading(&mut self, source_id: &SourceId) {
        let loading_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .margin_start(24)
            .margin_end(24)
            .margin_top(12)
            .margin_bottom(12)
            .build();

        let spinner = gtk::Spinner::builder().spinning(true).build();

        let label = gtk::Label::builder()
            .label(&format!("Loading content from {}...", source_id))
            .build();

        loading_box.append(&spinner);
        loading_box.append(&label);

        self.loading_containers
            .insert(source_id.clone(), loading_box.clone());
        self.sections_container.append(&loading_box);
    }

    /// Display error state for a source
    fn display_source_error(
        &mut self,
        source_id: &SourceId,
        error: &str,
        sender: &AsyncComponentSender<Self>,
    ) {
        let error_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .margin_start(24)
            .margin_end(24)
            .margin_top(12)
            .margin_bottom(12)
            .build();

        // Error icon
        let icon = gtk::Image::builder()
            .icon_name("dialog-error-symbolic")
            .build();
        icon.add_css_class("error");

        // Error message
        let label = gtk::Label::builder()
            .label(&format!("Failed to load content: {}", error))
            .hexpand(true)
            .xalign(0.0)
            .build();
        label.add_css_class("dim-label");

        // Retry button
        let retry_button = gtk::Button::builder().label("Retry").build();

        let source_id_clone = source_id.clone();
        let sender_clone = sender.clone();
        retry_button.connect_clicked(move |_| {
            sender_clone.input(HomePageInput::RetrySource(source_id_clone.clone()));
        });

        error_box.append(&icon);
        error_box.append(&label);
        error_box.append(&retry_button);

        self.loading_containers
            .insert(source_id.clone(), error_box.clone());
        self.sections_container.append(&error_box);
    }

    /// Display sections from a successfully loaded source
    fn display_source_sections(
        &mut self,
        source_id: &SourceId,
        sections: Vec<HomeSection>,
        sender: &AsyncComponentSender<Self>,
    ) {
        // Add sections to our list
        self.sections.extend(sections.clone());

        // Create UI for each section
        for section in &sections {
            if section.items.is_empty() {
                debug!("Skipping empty section: {}", section.title);
                continue;
            }

            debug!(
                "Creating UI for section '{}' with {} items",
                section.title,
                section.items.len()
            );

            // Create section container
            let section_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(12)
                .build();

            // Section header with title and scroll indicators
            let header_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(12)
                .build();

            // Section title
            let title_label = gtk::Label::builder()
                .label(&section.title)
                .halign(gtk::Align::Start)
                .hexpand(true)
                .build();
            title_label.add_css_class("title-2");
            header_box.append(&title_label);

            // Scroll navigation buttons
            let scroll_left_button = gtk::Button::builder()
                .icon_name("go-previous-symbolic")
                .sensitive(false) // Initially disabled
                .tooltip_text("Scroll left")
                .build();
            scroll_left_button.add_css_class("flat");
            scroll_left_button.add_css_class("circular");

            let scroll_right_button = gtk::Button::builder()
                .icon_name("go-next-symbolic")
                .tooltip_text("Scroll right")
                .build();
            scroll_right_button.add_css_class("flat");
            scroll_right_button.add_css_class("circular");

            header_box.append(&scroll_left_button);
            header_box.append(&scroll_right_button);
            section_box.append(&header_box);

            // Scrollable content area with horizontal scrolling
            let scrolled_window = gtk::ScrolledWindow::builder()
                .hscrollbar_policy(gtk::PolicyType::Automatic)
                .vscrollbar_policy(gtk::PolicyType::Never)
                .overlay_scrolling(true)
                .height_request(290) // Fixed height for media cards + margin
                .build();

            // Use FlowBox but constrain it to single row
            let cards_box = gtk::FlowBox::builder()
                .orientation(gtk::Orientation::Horizontal)
                .column_spacing(12)
                .min_children_per_line(100) // Force single row by setting high min
                .max_children_per_line(100) // Match max to min
                .selection_mode(gtk::SelectionMode::None)
                .valign(gtk::Align::Start)
                .homogeneous(false)
                .build();

            // Create factory for this section
            let sender_input = sender.input_sender();
            let mut factory = FactoryVecDeque::<MediaCard>::builder()
                .launch(cards_box.clone())
                .forward(sender_input, |output| match output {
                    MediaCardOutput::Clicked(id) => HomePageInput::MediaItemSelected(id),
                    MediaCardOutput::Play(id) => HomePageInput::MediaItemSelected(id),
                });

            // Add items to factory and queue image loads
            {
                let mut guard = factory.guard();
                for (idx, item) in section.items.iter().enumerate() {
                    // Convert MediaItem to MediaItemModel
                    let model = self.media_item_to_model(item);
                    let item_id = model.id.clone();

                    // Determine if we should show progress
                    let show_progress =
                        matches!(section.section_type, HomeSectionType::ContinueWatching);

                    guard.push_back(MediaCardInit {
                        item: model.clone(),
                        show_progress,
                        watched: false,
                        progress_percent: 0.0,
                    });

                    // Queue image load if poster URL exists
                    if let Some(poster_url) = &model.poster_url {
                        if !poster_url.is_empty() {
                            // Track this request
                            self.image_requests
                                .insert(item_id.clone(), (section.id.clone(), idx));

                            // Queue the image load with priority based on position
                            let priority = (idx / 10).min(10) as u8;
                            trace!(
                                "Queueing image for item {} with priority {}",
                                item_id, priority
                            );

                            self.image_loader
                                .emit(ImageLoaderInput::LoadImage(ImageRequest {
                                    id: item_id,
                                    url: poster_url.clone(),
                                    size: ImageSize::Thumbnail,
                                    priority,
                                }));
                        }
                    }
                }
            }

            // Store factory
            self.section_factories.insert(section.id.clone(), factory);

            scrolled_window.set_child(Some(&cards_box));

            // Connect scroll button handlers
            let h_adjustment = scrolled_window.hadjustment();

            // Update button sensitivity based on scroll position
            let left_btn = scroll_left_button.clone();
            let right_btn = scroll_right_button.clone();
            h_adjustment.connect_value_changed(move |adj| {
                let value = adj.value();
                let lower = adj.lower();
                let upper = adj.upper();
                let page_size = adj.page_size();

                // Enable/disable buttons based on position
                left_btn.set_sensitive(value > lower);
                right_btn.set_sensitive(value < upper - page_size);
            });

            // Scroll left button handler
            let h_adj = h_adjustment.clone();
            scroll_left_button.connect_clicked(move |_| {
                let current = h_adj.value();
                let step = h_adj.page_size() * 0.8; // Scroll 80% of visible area
                let new_value = (current - step).max(h_adj.lower());
                h_adj.set_value(new_value);
            });

            // Scroll right button handler
            let h_adj = h_adjustment.clone();
            scroll_right_button.connect_clicked(move |_| {
                let current = h_adj.value();
                let step = h_adj.page_size() * 0.8; // Scroll 80% of visible area
                let max_value = h_adj.upper() - h_adj.page_size();
                let new_value = (current + step).min(max_value);
                h_adj.set_value(new_value);
            });

            // Add keyboard navigation support
            let h_adj = h_adjustment.clone();
            let key_controller = gtk::EventControllerKey::new();
            key_controller.connect_key_pressed(move |_, key, _, _| {
                match key {
                    gtk::gdk::Key::Left => {
                        let current = h_adj.value();
                        let step = 192.0; // Width of one card + spacing
                        let new_value = (current - step).max(h_adj.lower());
                        h_adj.set_value(new_value);
                        gtk::glib::Propagation::Stop
                    }
                    gtk::gdk::Key::Right => {
                        let current = h_adj.value();
                        let step = 192.0; // Width of one card + spacing
                        let max_value = h_adj.upper() - h_adj.page_size();
                        let new_value = (current + step).min(max_value);
                        h_adj.set_value(new_value);
                        gtk::glib::Propagation::Stop
                    }
                    _ => gtk::glib::Propagation::Proceed,
                }
            });
            scrolled_window.add_controller(key_controller);

            // Enable smooth scrolling and kinetic scrolling for touch/trackpad
            scrolled_window.set_kinetic_scrolling(true);

            // Trigger initial button state update
            h_adjustment.emit_by_name::<()>("value-changed", &[]);
            section_box.append(&scrolled_window);

            // Add section to container
            self.sections_container.append(&section_box);
        }

        info!(
            "Displayed {} sections for source {}",
            sections.len(),
            source_id
        );
    }

    /// Update the overall loading state based on all source states
    fn update_overall_loading_state(&mut self) {
        // Check if any source is still loading
        let any_loading = self
            .source_states
            .values()
            .any(|state| matches!(state, SectionLoadState::Loading));

        self.is_loading = any_loading;

        if !any_loading {
            info!(
                "All sources finished loading. Total sections: {}",
                self.sections.len()
            );
        }
    }

    /// Clear all existing sections from the UI and factories
    fn clear_sections(&mut self) {
        debug!("Clearing all existing sections");

        // Cancel all pending image loads
        for (id, _) in self.image_requests.iter() {
            self.image_loader
                .emit(ImageLoaderInput::CancelLoad { id: id.clone() });
        }
        self.image_requests.clear();

        // Clear section factories
        self.section_factories.clear();

        // Remove all children from sections container
        while let Some(child) = self.sections_container.first_child() {
            self.sections_container.remove(&child);
        }

        // Clear sections data
        self.sections.clear();
    }

    /// Clear sections for a specific source
    fn clear_source_sections(&mut self, source_id: &SourceId) {
        debug!("Clearing sections for source {}", source_id);

        // Find and remove section factories for this source
        let mut factories_to_remove = Vec::new();
        for (section_id, _) in &self.section_factories {
            if section_id.starts_with(&format!("{}::", source_id)) {
                factories_to_remove.push(section_id.clone());
            }
        }

        for factory_id in factories_to_remove {
            self.section_factories.remove(&factory_id);
        }

        // Remove sections from data
        self.sections
            .retain(|section| !section.id.starts_with(&format!("{}::", source_id)));

        // Note: We can't easily remove specific UI elements from sections_container
        // without tracking them individually, so we rely on the full clear/rebuild approach
    }

    /// Convert a MediaItem to MediaItemModel for the factory
    fn media_item_to_model(&self, item: &MediaItem) -> MediaItemModel {
        use chrono::{NaiveDateTime, Utc};
        use sea_orm::prelude::{DateTime, Json};

        match item {
            MediaItem::Movie(movie) => MediaItemModel {
                id: movie.id.clone(),
                source_id: movie.backend_id.clone(),
                library_id: String::new(), // Not needed for display
                title: movie.title.clone(),
                sort_title: None,
                media_type: "movie".to_string(),
                year: movie.year.map(|y| y as i32),
                rating: movie.rating,
                overview: movie.overview.clone(),
                genres: if movie.genres.is_empty() {
                    None
                } else {
                    Some(Json::from(movie.genres.clone()))
                },
                duration_ms: Some(movie.duration.as_millis() as i64),
                poster_url: movie.poster_url.clone(),
                backdrop_url: movie.backdrop_url.clone(),
                added_at: movie.added_at.map(|dt| {
                    NaiveDateTime::from_timestamp_opt(dt.timestamp(), 0)
                        .unwrap_or_else(|| NaiveDateTime::default())
                }),
                updated_at: movie
                    .updated_at
                    .map(|dt| {
                        NaiveDateTime::from_timestamp_opt(dt.timestamp(), 0)
                            .unwrap_or_else(|| NaiveDateTime::default())
                    })
                    .unwrap_or_else(|| Utc::now().naive_utc()),
                metadata: None,
                parent_id: None,
                season_number: None,
                episode_number: None,
            },
            MediaItem::Show(show) => MediaItemModel {
                id: show.id.clone(),
                source_id: show.backend_id.clone(),
                library_id: String::new(),
                title: show.title.clone(),
                sort_title: None,
                media_type: "show".to_string(),
                year: show.year.map(|y| y as i32),
                rating: show.rating,
                overview: show.overview.clone(),
                genres: if show.genres.is_empty() {
                    None
                } else {
                    Some(Json::from(show.genres.clone()))
                },
                duration_ms: None,
                poster_url: show.poster_url.clone(),
                backdrop_url: show.backdrop_url.clone(),
                added_at: show.added_at.map(|dt| {
                    NaiveDateTime::from_timestamp_opt(dt.timestamp(), 0)
                        .unwrap_or_else(|| NaiveDateTime::default())
                }),
                updated_at: show
                    .updated_at
                    .map(|dt| {
                        NaiveDateTime::from_timestamp_opt(dt.timestamp(), 0)
                            .unwrap_or_else(|| NaiveDateTime::default())
                    })
                    .unwrap_or_else(|| Utc::now().naive_utc()),
                metadata: None,
                parent_id: None,
                season_number: None,
                episode_number: None,
            },
            MediaItem::Episode(episode) => MediaItemModel {
                id: episode.id.clone(),
                source_id: episode.backend_id.clone(),
                library_id: String::new(),
                title: episode.title.clone(),
                sort_title: None,
                media_type: "episode".to_string(),
                year: None,
                rating: None, // Episodes don't have ratings in the current model
                overview: episode.overview.clone(),
                genres: None,
                duration_ms: Some(episode.duration.as_millis() as i64),
                // Use show poster for episodes in Continue Watching, fallback to episode thumbnail
                poster_url: episode
                    .show_poster_url
                    .clone()
                    .or(episode.thumbnail_url.clone()),
                backdrop_url: None,
                added_at: None, // Episodes don't have added_at in the current model
                updated_at: Utc::now().naive_utc(),
                metadata: None,
                parent_id: episode.show_id.clone(),
                season_number: Some(episode.season_number as i32),
                episode_number: Some(episode.episode_number as i32),
            },
            _ => {
                // For other media types, create a basic model
                MediaItemModel {
                    id: String::new(),
                    source_id: String::new(),
                    library_id: String::new(),
                    title: "Unknown".to_string(),
                    sort_title: None,
                    media_type: "unknown".to_string(),
                    year: None,
                    rating: None,
                    overview: None,
                    genres: None,
                    duration_ms: None,
                    poster_url: None,
                    backdrop_url: None,
                    added_at: None,
                    updated_at: Utc::now().naive_utc(),
                    metadata: None,
                    parent_id: None,
                    season_number: None,
                    episode_number: None,
                }
            }
        }
    }
}
