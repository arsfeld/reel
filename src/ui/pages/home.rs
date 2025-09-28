use gtk::prelude::*;
use relm4::factory::FactoryVecDeque;
use relm4::gtk;
use relm4::prelude::*;
use std::collections::HashMap;
use tracing::{debug, error, info, trace};

use crate::db::connection::DatabaseConnection;
use crate::db::entities::MediaItemModel;
use crate::db::repository::{
    Repository,
    home_section_repository::{HomeSectionRepository, HomeSectionRepositoryImpl},
};
use crate::models::{HomeSectionType, HomeSectionWithModels, MediaItemId, SourceId};
use crate::ui::factories::media_card::{MediaCard, MediaCardInit, MediaCardInput, MediaCardOutput};
use crate::ui::shared::broker::{BROKER, BrokerMessage};
use crate::workers::{ImageLoader, ImageLoaderInput, ImageLoaderOutput, ImageRequest, ImageSize};

#[derive(Debug, Clone)]
pub enum SectionLoadState {
    Loading,
    Loaded(Vec<HomeSectionWithModels>),
    Failed(String), // Error message
}

pub struct HomePage {
    db: DatabaseConnection,
    sections: Vec<HomeSectionWithModels>,
    section_factories: HashMap<String, FactoryVecDeque<MediaCard>>,
    sections_container: gtk::Box,
    image_loader: relm4::WorkerController<ImageLoader>,
    image_requests: HashMap<String, (String, usize)>, // item_id -> (section_id, card_index)
    is_loading: bool,
    source_states: HashMap<SourceId, SectionLoadState>, // Track per-source loading states
    loading_containers: HashMap<SourceId, gtk::Box>,    // UI containers for loading/error states
    section_ui_containers: HashMap<String, gtk::Box>, // Track actual section UI containers by section_id
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
    HomeSectionsLoaded(Vec<HomeSectionWithModels>),
    /// Source-specific sections loaded
    SourceSectionsLoaded {
        source_id: SourceId,
        sections: Result<Vec<HomeSectionWithModels>, String>,
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
    /// Message broker messages
    BrokerMsg(BrokerMessage),
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
            section_ui_containers: HashMap::new(),
        };

        let widgets = view_output!();

        // Subscribe to MessageBroker for config updates
        {
            let broker_sender = sender.input_sender().clone();
            relm4::spawn(async move {
                let (tx, rx) = relm4::channel::<BrokerMessage>();
                BROKER.subscribe("HomePage".to_string(), tx).await;

                while let Some(msg) = rx.recv().await {
                    broker_sender.send(HomePageInput::BrokerMsg(msg)).unwrap();
                }
            });
        }

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

                    // Load cached sections from HomeSectionRepository for instant display
                    let section_repo = HomeSectionRepositoryImpl::new(db.clone());

                    // Get all sources to load sections for
                    use crate::db::repository::source_repository::{
                        SourceRepository, SourceRepositoryImpl,
                    };
                    let source_repo = SourceRepositoryImpl::new(db.clone());
                    if let Ok(sources) = source_repo.find_all().await {
                        for source in sources {
                            let source_id = SourceId::new(source.id.clone());

                            // Load cached sections for this source
                            if let Ok(persisted_sections) =
                                section_repo.find_by_source_with_items(&source.id).await
                            {
                                if !persisted_sections.is_empty() {
                                    // Convert to HomeSectionWithModels
                                    let mut sections = Vec::new();
                                    for (section_model, items) in persisted_sections {
                                        if !items.is_empty() {
                                            let section_type =
                                                match section_model.section_type.as_str() {
                                                    "continue_watching" => {
                                                        HomeSectionType::ContinueWatching
                                                    }
                                                    "on_deck" => HomeSectionType::OnDeck,
                                                    "suggested" => HomeSectionType::Suggested,
                                                    "top_rated" => HomeSectionType::TopRated,
                                                    "trending" => HomeSectionType::Trending,
                                                    "recently_played" => {
                                                        HomeSectionType::RecentlyPlayed
                                                    }
                                                    "recent_playlists" => {
                                                        HomeSectionType::RecentPlaylists
                                                    }
                                                    s if s.starts_with("recently_added_") => {
                                                        let media_type = s
                                                            .strip_prefix("recently_added_")
                                                            .unwrap_or("unknown");
                                                        HomeSectionType::RecentlyAdded(
                                                            media_type.to_string(),
                                                        )
                                                    }
                                                    custom => {
                                                        HomeSectionType::Custom(custom.to_string())
                                                    }
                                                };

                                            sections.push(HomeSectionWithModels {
                                                id: section_model.hub_identifier.clone(),
                                                title: section_model.title.clone(),
                                                section_type,
                                                items,
                                            });
                                        }
                                    }

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
                            }
                        }
                    }

                    // Note: Background API refresh should be triggered by sync worker, not here
                    // The home page should only read from cache, never fetch from API directly
                    // This ensures data is properly persisted to the database
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
                        self.display_source_sections(&source_id, sections, &sender)
                            .await;
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
                        Repository, source_repository::SourceRepositoryImpl,
                    };
                    let source_repo = SourceRepositoryImpl::new(db.clone());

                    // Note: Retry should trigger sync worker to refresh this source
                    // UI should only display what's in the cache
                    info!(
                        "Source {} retry requested - should trigger sync worker",
                        source_id_clone
                    );

                    // For now, just show an error state since we don't have sync worker integration
                    sender_clone.input(HomePageInput::SourceSectionsLoaded {
                        source_id: source_id_clone,
                        sections: Err(
                            "Refresh not yet implemented - sync worker integration needed"
                                .to_string(),
                        ),
                    });
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
                if let Some((section_id, card_idx)) = self.image_requests.get(&id)
                    && let Some(factory) = self.section_factories.get(section_id)
                {
                    // Send the texture to the specific card
                    factory.send(*card_idx, MediaCardInput::ImageLoaded(texture));
                }
            }

            HomePageInput::ImageLoadFailed { id } => {
                debug!("Failed to load image for item: {}", id);
                // Find the section and card index for this image
                if let Some((section_id, card_idx)) = self.image_requests.get(&id)
                    && let Some(factory) = self.section_factories.get(section_id)
                {
                    // Notify the card that the image failed to load
                    factory.send(*card_idx, MediaCardInput::ImageLoadFailed);
                }
                // Remove from tracking
                self.image_requests.remove(&id);
            }
            HomePageInput::BrokerMsg(msg) => {
                match msg {
                    BrokerMessage::Config(_) => {
                        // Home page might reload sections if config changes affect display
                        // For now, we'll just log the config update
                        debug!("Home page received config update");
                        // Could potentially reload sections if display preferences changed
                        // sender.input(HomePageInput::LoadData);
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
            BROKER.unsubscribe("HomePage").await;
        });
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
            .label(format!("Loading content from {}...", source_id))
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
            .label(format!("Failed to load content: {}", error))
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
    async fn display_source_sections(
        &mut self,
        source_id: &SourceId,
        sections: Vec<HomeSectionWithModels>,
        sender: &AsyncComponentSender<Self>,
    ) {
        // Filter out empty sections before processing
        let non_empty_sections: Vec<HomeSectionWithModels> = sections
            .into_iter()
            .filter(|s| !s.items.is_empty())
            .collect();

        // Collect all media IDs from all sections
        let all_media_ids: Vec<String> = non_empty_sections
            .iter()
            .flat_map(|s| s.items.iter().map(|item| item.id.clone()))
            .collect();

        // Batch fetch playback progress for all items
        let playback_progress_map = if !all_media_ids.is_empty() {
            match crate::services::core::MediaService::get_playback_progress_batch(
                &self.db,
                &all_media_ids,
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

        // Collect parent show IDs for episodes
        let episodes_with_parents: Vec<(MediaItemModel, String)> = non_empty_sections
            .iter()
            .flat_map(|s| s.items.iter())
            .filter(|item| item.media_type == "episode")
            .filter_map(|item| {
                item.parent_id
                    .as_ref()
                    .map(|pid| (item.clone(), pid.clone()))
            })
            .collect();

        let episode_parent_ids: Vec<String> = episodes_with_parents
            .iter()
            .map(|(_, pid)| pid.clone())
            .collect();

        // Batch fetch parent shows for episodes
        let mut parent_shows_map = std::collections::HashMap::new();
        if !episode_parent_ids.is_empty() {
            use crate::db::repository::media_repository::{MediaRepository, MediaRepositoryImpl};
            let media_repo = MediaRepositoryImpl::new(self.db.clone());

            // Deduplicate parent IDs
            let unique_parent_ids: std::collections::HashSet<String> =
                episode_parent_ids.iter().cloned().collect();

            for parent_id in unique_parent_ids {
                match media_repo.find_by_id(&parent_id).await {
                    Ok(Some(parent_show)) => {
                        parent_shows_map.insert(parent_id, parent_show);
                    }
                    Ok(None) => {
                        error!("Parent show not found in database: {}", parent_id);
                    }
                    Err(e) => {
                        error!("Failed to fetch parent show {}: {}", parent_id, e);
                    }
                }
            }
        }

        // Add only non-empty sections to our list
        self.sections.extend(non_empty_sections.clone());

        // Create UI for each non-empty section
        for section in &non_empty_sections {
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

            // Use FlowBox configured for horizontal scrolling
            let cards_box = gtk::FlowBox::builder()
                .orientation(gtk::Orientation::Horizontal)
                .column_spacing(12)
                .min_children_per_line(section.items.len() as u32) // Set to actual number of items
                .max_children_per_line(section.items.len() as u32) // Force single row with all items
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
                for (idx, model) in section.items.iter().enumerate() {
                    // Items are already MediaItemModel
                    let item_id = model.id.clone();

                    // Determine if we should show progress
                    let show_progress =
                        matches!(section.section_type, HomeSectionType::ContinueWatching);

                    // Use the pre-fetched playback progress data
                    let (watched, progress_percent) =
                        if let Some(progress) = playback_progress_map.get(&model.id) {
                            (progress.watched, progress.get_progress_percentage() as f64)
                        } else {
                            (false, 0.0) // No progress record means unwatched
                        };

                    // For episodes, we want to show the show poster instead
                    let mut display_item = model.clone();
                    if model.media_type == "episode" {
                        // Check if we have the parent show data
                        if let Some(parent_id) = &model.parent_id {
                            if let Some(parent_show) = parent_shows_map.get(parent_id) {
                                // Use the show's poster instead of episode thumbnail
                                display_item.poster_url = parent_show.poster_url.clone();

                                // Store original episode title in metadata for subtitle display
                                let episode_info = if let (Some(season), Some(episode)) =
                                    (model.season_number, model.episode_number)
                                {
                                    format!("S{}E{} - {}", season, episode, model.title)
                                } else {
                                    model.title.clone()
                                };

                                // Update title to show title
                                display_item.title = parent_show.title.clone();

                                // Store episode info in metadata for subtitle display
                                let mut metadata_obj = if let Some(metadata) = &model.metadata {
                                    serde_json::from_value::<
                                        serde_json::Map<String, serde_json::Value>,
                                    >(metadata.clone())
                                    .unwrap_or_else(|_| serde_json::Map::new())
                                } else {
                                    serde_json::Map::new()
                                };

                                metadata_obj.insert(
                                    "episode_subtitle".to_string(),
                                    serde_json::Value::String(episode_info),
                                );
                                display_item.metadata =
                                    Some(serde_json::to_value(metadata_obj).unwrap());
                            } else {
                                error!(
                                    "Parent show not found for episode {} with parent_id {:?}",
                                    model.id, parent_id
                                );
                                // Skip this episode if we can't find its parent show
                                continue;
                            }
                        } else {
                            error!("Episode {} has no parent_id set!", model.id);
                            // Skip episodes without parent shows
                            continue;
                        }
                    }

                    // Clone the poster URL before moving display_item
                    let poster_url_to_load = display_item.poster_url.clone();

                    guard.push_back(MediaCardInit {
                        item: display_item,
                        show_progress,
                        watched,
                        progress_percent,
                        show_media_type_icon: false,
                    });

                    // Queue image load if poster URL exists (use the correct poster from display_item)
                    if let Some(poster_url) = poster_url_to_load
                        && !poster_url.is_empty()
                    {
                        // Create a unique key for tracking that includes both section and item ID
                        // This allows the same item to appear in multiple sections
                        let tracking_key = format!("{}::{}", section.id, item_id);

                        // Track this request with the unique key
                        self.image_requests
                            .insert(tracking_key.clone(), (section.id.clone(), idx));

                        // Queue the image load with priority based on position
                        let priority = (idx / 10).min(10) as u8;
                        trace!(
                            "Queueing image for item {} with priority {}",
                            item_id, priority
                        );

                        let _ = self.image_loader.sender().send(ImageLoaderInput::LoadImage(
                            ImageRequest {
                                id: tracking_key, // Use the unique tracking key as the ID
                                url: poster_url.clone(),
                                size: ImageSize::Thumbnail,
                                priority,
                            },
                        ));
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

                // Enable/disable buttons based on position with small threshold
                // Use 1.0 pixel threshold to avoid floating point comparison issues
                left_btn.set_sensitive(value > lower + 1.0);
                right_btn.set_sensitive(value + 1.0 < upper - page_size);
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

            // Store UI container reference for proper removal later
            self.section_ui_containers
                .insert(section.id.clone(), section_box.clone());

            // Add section to container
            self.sections_container.append(&section_box);
        }

        info!(
            "Displayed {} sections for source {} (filtered from {})",
            non_empty_sections.len(),
            source_id,
            self.sections.len()
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
        for (tracking_key, _) in self.image_requests.iter() {
            let _ = self
                .image_loader
                .sender()
                .send(ImageLoaderInput::CancelLoad {
                    id: tracking_key.clone(),
                });
        }
        self.image_requests.clear();

        // Clear section factories
        self.section_factories.clear();

        // Clear UI container references
        self.section_ui_containers.clear();

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

        // Cancel image loads for this source's sections
        let mut requests_to_remove = Vec::new();
        for (tracking_key, (section_id, _)) in self.image_requests.iter() {
            if section_id.starts_with(&format!("{}::", source_id)) {
                let _ = self
                    .image_loader
                    .sender()
                    .send(ImageLoaderInput::CancelLoad {
                        id: tracking_key.clone(),
                    });
                requests_to_remove.push(tracking_key.clone());
            }
        }
        for tracking_key in requests_to_remove {
            self.image_requests.remove(&tracking_key);
        }

        // Find and remove section UI containers and factories for this source
        let mut sections_to_remove = Vec::new();
        for section_id in self.section_factories.keys() {
            if section_id.starts_with(&format!("{}::", source_id)) {
                sections_to_remove.push(section_id.clone());
            }
        }

        // Remove UI containers and factories
        for section_id in sections_to_remove {
            // Remove factory
            self.section_factories.remove(&section_id);

            // Remove UI container
            if let Some(container) = self.section_ui_containers.remove(&section_id) {
                self.sections_container.remove(&container);
            }
        }

        // Remove sections from data
        self.sections
            .retain(|section| !section.id.starts_with(&format!("{}::", source_id)));
    }
}
