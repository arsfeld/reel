use gtk::prelude::*;
use libadwaita as adw;
use relm4::factory::FactoryVecDeque;
use relm4::gtk;
use relm4::prelude::*;
use tracing::{debug, error};

use crate::db::connection::DatabaseConnection;
use crate::db::entities::{MediaItemModel, media_items};
use crate::db::repository::{Repository, media_repository::MediaRepositoryImpl};
use crate::models::MediaItemId;
use crate::ui::factories::media_card::{MediaCard, MediaCardInit, MediaCardInput, MediaCardOutput};
use crate::workers::{ImageLoader, ImageLoaderInput, ImageLoaderOutput, ImageRequest, ImageSize};
use relm4::factory::DynamicIndex;

pub struct SearchPage {
    db: DatabaseConnection,
    media_factory: FactoryVecDeque<MediaCard>,
    image_loader: relm4::WorkerController<ImageLoader>,
    image_requests: std::collections::HashMap<String, Vec<DynamicIndex>>,
    query: String,
    results: Vec<MediaItemModel>,
    is_loading: bool,
}

impl std::fmt::Debug for SearchPage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SearchPage")
            .field("query", &self.query)
            .field("results_count", &self.results.len())
            .field("is_loading", &self.is_loading)
            .finish()
    }
}

#[derive(Debug)]
pub enum SearchPageInput {
    /// Set search query and results
    SetResults {
        query: String,
        results: Vec<MediaItemId>,
    },
    /// Results loaded from database
    ResultsLoaded(Vec<MediaItemModel>),
    /// Parent shows loaded for episodes
    ParentShowsLoaded {
        items: Vec<MediaItemModel>,
        parent_shows: std::collections::HashMap<String, MediaItemModel>,
    },
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
pub enum SearchPageOutput {
    /// Navigate to media item
    NavigateToMediaItem(MediaItemId),
}

#[relm4::component(pub async)]
impl AsyncComponent for SearchPage {
    type Init = DatabaseConnection;
    type Input = SearchPageInput;
    type Output = SearchPageOutput;
    type CommandOutput = ();

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 0,

            gtk::ScrolledWindow {
                set_vexpand: true,
                set_hscrollbar_policy: gtk::PolicyType::Never,

                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 0,

                    // Header with search query
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 8,
                        set_margin_top: 24,
                        set_margin_start: 16,
                        set_margin_end: 16,
                        #[watch]
                        set_visible: !model.query.is_empty() && !model.results.is_empty(),

                        gtk::Label {
                            set_halign: gtk::Align::Start,
                            #[watch]
                            set_text: &format!("Search results for: \"{}\"", model.query),
                            add_css_class: "title-2",
                        },

                        gtk::Label {
                            set_halign: gtk::Align::Start,
                            #[watch]
                            set_text: &format!("{} results", model.results.len()),
                            add_css_class: "dim-label",
                        },
                    },

                    // Results grid
                    #[local_ref]
                    media_factory -> gtk::FlowBox {
                        #[watch]
                        set_visible: !model.is_loading && !model.results.is_empty(),
                        set_column_spacing: 12,
                        set_row_spacing: 16,
                        set_homogeneous: true,
                        set_min_children_per_line: 4,
                        set_max_children_per_line: 12,
                        set_selection_mode: gtk::SelectionMode::None,
                        set_margin_top: 24,
                        set_margin_bottom: 16,
                        set_margin_start: 16,
                        set_margin_end: 16,
                        set_valign: gtk::Align::Start,
                    },

                    // Loading indicator
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
                            set_text: "Searching...",
                            set_margin_start: 12,
                            add_css_class: "dim-label",
                        },
                    },

                    // Empty state - no query
                    adw::StatusPage {
                        #[watch]
                        set_visible: !model.is_loading && model.query.is_empty(),
                        set_icon_name: Some("system-search-symbolic"),
                        set_title: "Search Your Library",
                        set_description: Some("Use the search bar above to find movies, shows, and episodes"),
                        add_css_class: "compact",
                    },

                    // Empty state - no results
                    adw::StatusPage {
                        #[watch]
                        set_visible: !model.is_loading && !model.query.is_empty() && model.results.is_empty(),
                        set_icon_name: Some("edit-find-symbolic"),
                        #[watch]
                        set_title: &format!("No results for \"{}\"", model.query),
                        set_description: Some("Try a different search query"),
                        add_css_class: "compact",
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
        // Initialize image loader
        let image_loader =
            ImageLoader::builder()
                .detach_worker(())
                .forward(sender.input_sender(), |output| match output {
                    ImageLoaderOutput::ImageLoaded { id, texture, .. } => {
                        SearchPageInput::ImageLoaded { id, texture }
                    }
                    ImageLoaderOutput::LoadFailed { id, .. } => {
                        SearchPageInput::ImageLoadFailed { id }
                    }
                    _ => SearchPageInput::ImageLoadFailed { id: String::new() },
                });

        // Initialize media factory
        let media_factory = FactoryVecDeque::builder()
            .launch(gtk::FlowBox::default())
            .forward(sender.input_sender(), |output| match output {
                MediaCardOutput::Clicked(id) => SearchPageInput::MediaItemSelected(id),
                MediaCardOutput::Play(id) => SearchPageInput::MediaItemSelected(id),
            });

        let model = SearchPage {
            db,
            media_factory,
            image_loader,
            image_requests: std::collections::HashMap::new(),
            query: String::new(),
            results: Vec::new(),
            is_loading: false,
        };

        let media_factory = model.media_factory.widget().clone();
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
            SearchPageInput::SetResults { query, results } => {
                debug!(
                    "Setting search results for query: {}, {} results",
                    query,
                    results.len()
                );
                self.query = query;
                self.is_loading = true;

                // Load media items from database
                let db = self.db.clone();
                let ids = results.clone();
                let input_sender = sender.input_sender().clone();
                sender.oneshot_command(async move {
                    let repo = MediaRepositoryImpl::new(db);
                    let mut items = Vec::new();

                    for id in ids {
                        match repo.find_by_id(&id.to_string()).await {
                            Ok(Some(item)) => items.push(item),
                            Ok(None) => debug!("Media item not found: {}", id),
                            Err(e) => error!("Error loading media item {}: {}", id, e),
                        }
                    }

                    input_sender
                        .send(SearchPageInput::ResultsLoaded(items))
                        .ok();
                });
            }

            SearchPageInput::ResultsLoaded(items) => {
                debug!("Loaded {} media items from database", items.len());

                // Collect parent show IDs for episodes
                let episode_parent_ids: Vec<String> = items
                    .iter()
                    .filter(|item| item.media_type == "episode")
                    .filter_map(|item| item.parent_id.clone())
                    .collect();

                // Batch fetch parent shows for episodes
                let db = self.db.clone();
                let input_sender = sender.input_sender().clone();
                sender.oneshot_command(async move {
                    use crate::db::repository::Repository;
                    use crate::db::repository::media_repository::MediaRepositoryImpl;

                    let media_repo = MediaRepositoryImpl::new(db);
                    let mut parent_shows_map = std::collections::HashMap::new();

                    if !episode_parent_ids.is_empty() {
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

                    // Send message with parent shows data
                    input_sender
                        .send(SearchPageInput::ParentShowsLoaded {
                            items,
                            parent_shows: parent_shows_map,
                        })
                        .ok();
                });
            }

            SearchPageInput::ParentShowsLoaded {
                items,
                parent_shows,
            } => {
                debug!("Rendering {} items with parent show data", items.len());
                self.results = items.clone();
                self.is_loading = false;

                // Clear existing media cards
                self.media_factory.guard().clear();

                // Clear image requests
                self.image_requests.clear();

                // Add new media cards
                for item in &items {
                    // For episodes, use parent show poster
                    let mut display_item = item.clone();
                    if item.media_type == "episode" {
                        if let Some(parent_id) = &item.parent_id {
                            if let Some(parent_show) = parent_shows.get(parent_id) {
                                // Use the show's poster instead of episode thumbnail
                                display_item.poster_url = parent_show.poster_url.clone();
                            } else {
                                error!(
                                    "Parent show not found for episode {} with parent_id {:?}",
                                    item.id, parent_id
                                );
                                // Skip this episode if we can't find its parent show
                                continue;
                            }
                        } else {
                            error!("Episode {} has no parent_id set!", item.id);
                            // Skip episodes without parent shows
                            continue;
                        }
                    }

                    let poster_url_to_load = display_item.poster_url.clone();

                    let card_init = MediaCardInit {
                        item: display_item,
                        show_progress: true,
                        watched: false,
                        progress_percent: 0.0,
                        show_media_type_icon: true,
                    };

                    let index = self.media_factory.guard().push_back(card_init);

                    // Request poster image
                    if let Some(poster_url) = poster_url_to_load {
                        // Track multiple cards that use the same poster URL
                        self.image_requests
                            .entry(poster_url.clone())
                            .or_insert_with(Vec::new)
                            .push(index);

                        // Only request the image once per unique URL
                        if self
                            .image_requests
                            .get(&poster_url)
                            .map(|v| v.len())
                            .unwrap_or(0)
                            == 1
                        {
                            self.image_loader
                                .emit(ImageLoaderInput::LoadImage(ImageRequest {
                                    id: poster_url.clone(),
                                    url: poster_url.clone(),
                                    size: ImageSize::Thumbnail,
                                    priority: 1,
                                }));
                        }
                    }
                }
            }

            SearchPageInput::MediaItemSelected(id) => {
                debug!("Media item selected: {}", id);
                sender
                    .output(SearchPageOutput::NavigateToMediaItem(id))
                    .ok();
            }

            SearchPageInput::ImageLoaded { id, texture } => {
                // Send the texture to all cards that share this poster URL
                if let Some(indices) = self.image_requests.get(&id) {
                    for index in indices {
                        self.media_factory.send(
                            index.current_index(),
                            MediaCardInput::ImageLoaded(texture.clone()),
                        );
                    }
                }
            }

            SearchPageInput::ImageLoadFailed { id } => {
                debug!("Image load failed for: {}", id);
                // Card will show placeholder
            }
        }
    }
}
