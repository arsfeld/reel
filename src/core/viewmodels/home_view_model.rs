use super::{Property, PropertySubscriber, ViewModel};
use crate::db::entities::libraries::Model as Library;
use crate::events::{DatabaseEvent, EventBus, EventFilter, EventType};
use crate::models::MediaItem;
use crate::services::DataService;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::error;

#[derive(Debug, Clone)]
pub struct MediaSection {
    pub title: String,
    pub items: Vec<MediaItem>,
    pub library_id: Option<String>,
    pub section_type: SectionType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SectionType {
    RecentlyAdded,
    ContinueWatching,
    Recommended,
    Library(String),
    Genre(String),
    Trending,
}

pub struct HomeViewModel {
    data_service: Arc<DataService>,
    sections: Property<Vec<MediaSection>>,
    featured_item: Property<Option<MediaItem>>,
    continue_watching: Property<Vec<MediaItem>>,
    recently_added: Property<Vec<MediaItem>>,
    libraries: Property<Vec<Library>>,
    is_loading: Property<bool>,
    error: Property<Option<String>>,
    section_limits: Property<HashMap<String, usize>>,
    event_bus: Option<Arc<EventBus>>,
}

impl HomeViewModel {
    pub fn new(data_service: Arc<DataService>) -> Self {
        let mut default_limits = HashMap::new();
        default_limits.insert("recently_added".to_string(), 20);
        default_limits.insert("continue_watching".to_string(), 10);
        default_limits.insert("recommended".to_string(), 15);
        default_limits.insert("library".to_string(), 10);

        Self {
            data_service,
            sections: Property::new(Vec::new(), "sections"),
            featured_item: Property::new(None, "featured_item"),
            continue_watching: Property::new(Vec::new(), "continue_watching"),
            recently_added: Property::new(Vec::new(), "recently_added"),
            libraries: Property::new(Vec::new(), "libraries"),
            is_loading: Property::new(false, "is_loading"),
            error: Property::new(None, "error"),
            section_limits: Property::new(default_limits, "section_limits"),
            event_bus: None,
        }
    }

    pub async fn load_home_content(&self) -> Result<()> {
        self.is_loading.set(true).await;
        self.error.set(None).await;

        let mut sections = Vec::new();

        match self.load_continue_watching().await {
            Ok(items) if !items.is_empty() => {
                sections.push(MediaSection {
                    title: "Continue Watching".to_string(),
                    items: items.clone(),
                    library_id: None,
                    section_type: SectionType::ContinueWatching,
                });

                self.continue_watching.set(items).await;
            }
            Err(e) => error!("Failed to load continue watching: {}", e),
            _ => {}
        }

        match self.load_recently_added().await {
            Ok(items) if !items.is_empty() => {
                if self.featured_item.get().await.is_none() && !items.is_empty() {
                    self.featured_item.set(Some(items[0].clone())).await;
                }

                sections.push(MediaSection {
                    title: "Recently Added".to_string(),
                    items: items.clone(),
                    library_id: None,
                    section_type: SectionType::RecentlyAdded,
                });

                self.recently_added.set(items).await;
            }
            Err(e) => error!("Failed to load recently added: {}", e),
            _ => {}
        }

        match self.data_service.get_all_libraries().await {
            Ok(libraries) => {
                self.libraries.set(libraries.clone()).await;

                let limit = self
                    .section_limits
                    .get()
                    .await
                    .get("library")
                    .copied()
                    .unwrap_or(10);

                for library in libraries.iter().take(3) {
                    if let Ok(items) = self.data_service.get_media_items(&library.id).await {
                        let limited_items: Vec<MediaItem> = items.into_iter().take(limit).collect();

                        if !limited_items.is_empty() {
                            sections.push(MediaSection {
                                title: library.title.clone(),
                                items: limited_items,
                                library_id: Some(library.id.clone()),
                                section_type: SectionType::Library(library.id.clone()),
                            });
                        }
                    }
                }
            }
            Err(e) => error!("Failed to load libraries: {}", e),
        }

        self.sections.set(sections).await;
        self.is_loading.set(false).await;

        Ok(())
    }

    async fn load_continue_watching(&self) -> Result<Vec<MediaItem>> {
        // Get media items that are in progress
        let items = self.data_service.get_continue_watching().await?;

        // Filter items that have playback progress
        let mut result = Vec::new();
        for item in items {
            if let Ok(Some((position_ms, duration_ms))) =
                self.data_service.get_playback_progress(item.id()).await
            {
                let completion = if duration_ms > 0 {
                    (position_ms as f64 / duration_ms as f64) * 100.0
                } else {
                    0.0
                };

                if completion > 5.0 && completion < 90.0 {
                    result.push(item);
                }
            }
        }

        Ok(result)
    }

    async fn load_recently_added(&self) -> Result<Vec<MediaItem>> {
        let limit = self
            .section_limits
            .get()
            .await
            .get("recently_added")
            .copied()
            .unwrap_or(20);

        self.data_service.get_recently_added(Some(limit)).await
    }

    pub async fn refresh_section(&self, section_type: SectionType) -> Result<()> {
        match section_type {
            SectionType::ContinueWatching => {
                let items = self.load_continue_watching().await?;
                self.continue_watching.set(items.clone()).await;

                // Use update instead of get/set to avoid race conditions
                self.sections
                    .update(|sections| {
                        if let Some(section) = sections
                            .iter_mut()
                            .find(|s| s.section_type == SectionType::ContinueWatching)
                        {
                            section.items = items.clone();
                        } else if !items.is_empty() {
                            // Add the section if it doesn't exist
                            sections.insert(
                                0,
                                MediaSection {
                                    title: "Continue Watching".to_string(),
                                    items,
                                    library_id: None,
                                    section_type: SectionType::ContinueWatching,
                                },
                            );
                        }
                    })
                    .await;
            }
            SectionType::RecentlyAdded => {
                let items = self.load_recently_added().await?;
                self.recently_added.set(items.clone()).await;

                // Use update instead of get/set to avoid race conditions
                self.sections
                    .update(|sections| {
                        if let Some(section) = sections
                            .iter_mut()
                            .find(|s| s.section_type == SectionType::RecentlyAdded)
                        {
                            section.items = items.clone();
                        } else if !items.is_empty() {
                            // Add the section if it doesn't exist
                            sections.push(MediaSection {
                                title: "Recently Added".to_string(),
                                items,
                                library_id: None,
                                section_type: SectionType::RecentlyAdded,
                            });
                        }
                    })
                    .await;
            }
            SectionType::Library(ref library_id) => {
                let items = self.data_service.get_media_items(library_id).await?;
                let limit = self
                    .section_limits
                    .get()
                    .await
                    .get("library")
                    .copied()
                    .unwrap_or(10);

                let library_id = library_id.clone();
                let limited_items: Vec<MediaItem> = items.into_iter().take(limit).collect();

                // Use update instead of get/set to avoid race conditions
                self.sections.update(|sections| {
                    if let Some(section) = sections.iter_mut().find(
                        |s| matches!(&s.section_type, SectionType::Library(id) if id == &library_id),
                    ) {
                        section.items = limited_items;
                    }
                }).await;
            }
            _ => {}
        }

        Ok(())
    }

    pub async fn set_section_limit(&self, section_key: String, limit: usize) {
        self.section_limits
            .update(|limits| {
                limits.insert(section_key, limit);
            })
            .await;

        let _ = self.load_home_content().await;
    }

    pub async fn set_featured_item(&self, item: MediaItem) {
        self.featured_item.set(Some(item)).await;
    }

    async fn handle_event(&self, event: DatabaseEvent) {
        match event.event_type {
            EventType::MediaCreated | EventType::MediaUpdated | EventType::MediaDeleted => {
                let _ = self.load_home_content().await;
            }
            EventType::PlaybackPositionUpdated | EventType::PlaybackCompleted => {
                let _ = self.refresh_section(SectionType::ContinueWatching).await;
            }
            EventType::LibraryCreated
            | EventType::LibraryDeleted
            | EventType::LibraryItemCountChanged => {
                let _ = self.load_home_content().await;
            }
            _ => {}
        }
    }

    pub fn sections(&self) -> &Property<Vec<MediaSection>> {
        &self.sections
    }

    pub fn featured_item(&self) -> &Property<Option<MediaItem>> {
        &self.featured_item
    }

    pub fn continue_watching(&self) -> &Property<Vec<MediaItem>> {
        &self.continue_watching
    }

    pub fn recently_added(&self) -> &Property<Vec<MediaItem>> {
        &self.recently_added
    }

    pub fn libraries(&self) -> &Property<Vec<Library>> {
        &self.libraries
    }

    pub fn is_loading(&self) -> &Property<bool> {
        &self.is_loading
    }

    pub async fn refresh(&self) -> Result<()> {
        // Use the existing load_home_content method which properly loads all sections
        self.load_home_content().await
    }
}

#[async_trait::async_trait]
impl ViewModel for HomeViewModel {
    async fn initialize(&self, event_bus: Arc<EventBus>) {
        let filter = EventFilter::new().with_types(vec![
            EventType::MediaCreated,
            EventType::MediaUpdated,
            EventType::MediaDeleted,
            EventType::PlaybackPositionUpdated,
            EventType::PlaybackCompleted,
            EventType::LibraryCreated,
            EventType::LibraryDeleted,
            EventType::LibraryItemCountChanged,
        ]);

        let mut subscriber = event_bus.subscribe_filtered(filter);
        let self_clone = self.clone();

        tokio::spawn(async move {
            while let Ok(event) = subscriber.recv().await {
                self_clone.handle_event(event).await;
            }
        });

        let _ = self.load_home_content().await;
    }

    fn subscribe_to_property(&self, property_name: &str) -> Option<PropertySubscriber> {
        match property_name {
            "sections" => Some(self.sections.subscribe()),
            "featured_item" => Some(self.featured_item.subscribe()),
            "continue_watching" => Some(self.continue_watching.subscribe()),
            "recently_added" => Some(self.recently_added.subscribe()),
            "libraries" => Some(self.libraries.subscribe()),
            "is_loading" => Some(self.is_loading.subscribe()),
            _ => None,
        }
    }

    async fn refresh(&self) {
        let _ = self.load_home_content().await;
    }

    fn dispose(&self) {}
}

impl Clone for HomeViewModel {
    fn clone(&self) -> Self {
        Self {
            data_service: self.data_service.clone(),
            sections: self.sections.clone(),
            featured_item: self.featured_item.clone(),
            continue_watching: self.continue_watching.clone(),
            recently_added: self.recently_added.clone(),
            libraries: self.libraries.clone(),
            is_loading: self.is_loading.clone(),
            error: self.error.clone(),
            section_limits: self.section_limits.clone(),
            event_bus: self.event_bus.clone(),
        }
    }
}
