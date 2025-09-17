use gtk::prelude::*;
use relm4::factory::FactoryVecDeque;
use relm4::gtk;
use relm4::prelude::*;
use std::collections::HashMap;
use tracing::{debug, error, info};

use crate::db::connection::DatabaseConnection;
use crate::db::entities::MediaItemModel;
use crate::models::{HomeSection, HomeSectionType, MediaItem, MediaItemId};
use crate::platforms::relm4::components::factories::media_card::{
    MediaCard, MediaCardInit, MediaCardOutput,
};
use crate::services::core::BackendService;

#[derive(Debug)]
pub struct HomePage {
    db: DatabaseConnection,
    sections: Vec<HomeSection>,
    section_factories: HashMap<String, FactoryVecDeque<MediaCard>>,
    sections_container: gtk::Box,
    is_loading: bool,
}

#[derive(Debug)]
pub enum HomePageInput {
    /// Load home page data
    LoadData,
    /// Home sections loaded from backends
    HomeSectionsLoaded(Vec<HomeSection>),
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

                #[local_ref]
                sections_container -> gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_margin_all: 24,
                    set_margin_top: 0,
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

        let model = Self {
            db,
            sections: Vec::new(),
            section_factories: HashMap::new(),
            sections_container: sections_container.clone(),
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
                debug!("Loading home page data from all backends");
                self.is_loading = true;

                // Clear existing sections
                self.clear_sections();

                // Clone database for async operations
                let db = self.db.clone();
                let sender_clone = sender.clone();

                // Load home sections from all backends
                relm4::spawn(async move {
                    info!("Fetching home sections from BackendService");
                    match BackendService::get_all_home_sections(&db).await {
                        Ok(sections) => {
                            info!("Successfully loaded {} home sections", sections.len());
                            for section in &sections {
                                info!(
                                    "  Section '{}' (type: {:?}) with {} items",
                                    section.title,
                                    section.section_type,
                                    section.items.len()
                                );
                            }
                            sender_clone.input(HomePageInput::HomeSectionsLoaded(sections));
                        }
                        Err(e) => {
                            error!("Failed to load home sections: {}", e);
                            // Fallback to empty sections
                            sender_clone.input(HomePageInput::HomeSectionsLoaded(Vec::new()));
                        }
                    }
                });
            }

            HomePageInput::HomeSectionsLoaded(sections) => {
                info!("Processing {} home sections for display", sections.len());

                // Clear existing sections first
                self.clear_sections();

                // Store sections
                self.sections = sections;

                // Create UI for each section
                for section in &self.sections {
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

                    // Section title
                    let title_label = gtk::Label::builder()
                        .label(&section.title)
                        .halign(gtk::Align::Start)
                        .css_classes(["title-2"])
                        .build();
                    section_box.append(&title_label);

                    // Scrollable content area
                    let scrolled_window = gtk::ScrolledWindow::builder()
                        .hscrollbar_policy(gtk::PolicyType::Automatic)
                        .vscrollbar_policy(gtk::PolicyType::Never)
                        .overlay_scrolling(true)
                        .build();

                    // Flow box for media cards
                    let flow_box = gtk::FlowBox::builder()
                        .orientation(gtk::Orientation::Horizontal)
                        .column_spacing(12)
                        .min_children_per_line(1)
                        .max_children_per_line(10)
                        .selection_mode(gtk::SelectionMode::None)
                        .build();

                    // Create factory for this section
                    let sender_input = sender.input_sender();
                    let mut factory = FactoryVecDeque::<MediaCard>::builder()
                        .launch(flow_box.clone())
                        .forward(sender_input, |output| match output {
                            MediaCardOutput::Clicked(id) => HomePageInput::MediaItemSelected(id),
                            MediaCardOutput::Play(id) => HomePageInput::MediaItemSelected(id),
                        });

                    // Add items to factory
                    {
                        let mut guard = factory.guard();
                        for item in &section.items {
                            // Convert MediaItem to MediaItemModel
                            let model = self.media_item_to_model(item);

                            // Determine if we should show progress
                            let show_progress =
                                matches!(section.section_type, HomeSectionType::ContinueWatching);

                            guard.push_back(MediaCardInit {
                                item: model,
                                show_progress,
                                watched: false,
                                progress_percent: 0.0,
                            });
                        }
                    }

                    // Store factory
                    self.section_factories.insert(section.id.clone(), factory);

                    scrolled_window.set_child(Some(&flow_box));
                    section_box.append(&scrolled_window);

                    // Add section to container
                    self.sections_container.append(&section_box);
                }

                self.is_loading = false;
                info!("Home page sections loaded and displayed");
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

impl HomePage {
    /// Clear all existing sections from the UI and factories
    fn clear_sections(&mut self) {
        debug!("Clearing all existing sections");

        // Clear section factories
        self.section_factories.clear();

        // Remove all children from sections container
        while let Some(child) = self.sections_container.first_child() {
            self.sections_container.remove(&child);
        }

        // Clear sections data
        self.sections.clear();
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
                poster_url: episode.thumbnail_url.clone(),
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
