use super::{Property, PropertySubscriber, ViewModel};
use crate::db::entities::libraries::Model as Library;
use crate::events::{DatabaseEvent, EventBus, EventFilter, EventType};
use crate::models::MediaItem;
use crate::services::{AppInitializationState, DataService, SourceReadiness};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info};

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
    Suggested,
    TopRated,
    Library(String),
    Genre(String),
    Trending,
    RecentlyPlayed,
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
    current_source_id: Property<Option<String>>, // Filter by source
    backends_ready: Property<bool>,              // Track if backends are initialized and ready
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
            current_source_id: Property::new(None, "current_source_id"),
            backends_ready: Property::new(false, "backends_ready"),
            event_bus: None,
        }
    }

    /// Create a friendly display name for a library title
    fn create_friendly_library_name(library_title: &str, source_id: &str) -> String {
        // Remove common prefixes that include source IDs
        if library_title.starts_with(&format!("{}_", source_id)) {
            // Remove "source_jellyfin_xxxxx_" or similar prefixes
            if let Some(clean_title) = library_title.strip_prefix(&format!("{}_", source_id)) {
                return clean_title.to_string();
            }
        }

        // Handle cases where the library title contains the source ID pattern
        if library_title.contains("source_jellyfin_") || library_title.contains("source_plex_") {
            // Try to extract just the library name after the last underscore
            if let Some(last_underscore) = library_title.rfind('_') {
                let potential_name = &library_title[last_underscore + 1..];
                // Only use this if it looks like a real library name (not just numbers/hex)
                if !potential_name.is_empty()
                    && !potential_name.chars().all(|c| c.is_ascii_hexdigit())
                {
                    return potential_name.to_string();
                }
            }
        }

        // For Jellyfin libraries that might have been prefixed, clean them up
        if library_title.starts_with("jellyfin_") {
            if let Some(clean_title) = library_title.strip_prefix("jellyfin_") {
                return clean_title.to_string();
            }
        }

        // Return the original title if no cleanup patterns match
        library_title.to_string()
    }

    pub async fn load_home_content(&self) -> Result<()> {
        // First load from cache immediately (offline-first)
        let _ = self.load_home_content_from_cache().await;

        // Then trigger a sync in the background if needed
        let _ = self.load_home_content_with_sync().await;

        Ok(())
    }

    pub async fn load_home_content_from_cache(&self) -> Result<()> {
        use tracing::{debug, info, warn};

        self.is_loading.set(true).await;
        self.error.set(None).await;

        let mut sections = Vec::new();
        let current_source_id = self.current_source_id.get().await;

        info!(
            "Loading home content for source filter: {:?}",
            current_source_id
        );

        // First, try to get home sections from backends
        let backend_sections = if let Some(ref source_id) = current_source_id {
            // Get sections for specific source
            debug!("Getting home sections for source: {}", source_id);
            self.data_service
                .get_home_sections_for_source(source_id)
                .await
                .unwrap_or_default()
        } else {
            // Get sections from all sources
            debug!("Getting home sections from all sources");
            self.data_service
                .get_all_home_sections()
                .await
                .unwrap_or_default()
        };

        debug!("Found {} backend sections", backend_sections.len());

        // Convert backend home sections to our format
        for backend_section in backend_sections {
            sections.push(MediaSection {
                title: backend_section.title,
                items: backend_section.items,
                library_id: None, // Backend sections don't map to specific libraries
                section_type: match backend_section.section_type {
                    crate::models::HomeSectionType::ContinueWatching => {
                        SectionType::ContinueWatching
                    }
                    crate::models::HomeSectionType::RecentlyAdded => SectionType::RecentlyAdded,
                    crate::models::HomeSectionType::Suggested => SectionType::Suggested,
                    crate::models::HomeSectionType::TopRated => SectionType::TopRated,
                    crate::models::HomeSectionType::Trending => SectionType::Trending,
                    crate::models::HomeSectionType::RecentlyPlayed => SectionType::RecentlyPlayed,
                    crate::models::HomeSectionType::Custom(name) => SectionType::Genre(name),
                },
            });
        }

        // If we have backend sections, use them and skip the fallback
        if !sections.is_empty() {
            info!("Using {} backend sections", sections.len());
            self.sections.set(sections).await;
            self.is_loading.set(false).await;
            return Ok(());
        }

        // Fallback: Create sections from database if no backend sections available
        warn!("No backend sections found, falling back to database sections");

        match self.load_continue_watching().await {
            Ok(items) if !items.is_empty() => {
                info!("Loaded {} continue watching items", items.len());
                sections.push(MediaSection {
                    title: "Continue Watching".to_string(),
                    items: items.clone(),
                    library_id: None,
                    section_type: SectionType::ContinueWatching,
                });

                self.continue_watching.set(items).await;
            }
            Ok(items) => {
                info!("Continue watching returned {} items (empty)", items.len());
            }
            Err(e) => error!("Failed to load continue watching: {}", e),
        }

        match self.load_recently_added().await {
            Ok(items) if !items.is_empty() => {
                info!("Loaded {} recently added items", items.len());
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
            Ok(items) => {
                info!("Recently added returned {} items (empty)", items.len());
            }
            Err(e) => error!("Failed to load recently added: {}", e),
        }

        // Load libraries - filter by source if specified
        let libraries_result = if let Some(ref source_id) = current_source_id {
            info!("Loading libraries for source: {}", source_id);
            self.data_service.get_libraries(source_id).await
        } else {
            info!("Loading libraries from all sources");
            self.data_service.get_all_libraries().await
        };

        match libraries_result {
            Ok(libraries) => {
                info!("Found {} libraries", libraries.len());
                self.libraries.set(libraries.clone()).await;

                let limit = self
                    .section_limits
                    .get()
                    .await
                    .get("library")
                    .copied()
                    .unwrap_or(10);

                for library in libraries.iter().take(3) {
                    debug!("Processing library: {} ({})", library.title, library.id);
                    if let Ok(items) = self.data_service.get_media_items(&library.id).await {
                        let items_count = items.len();
                        let limited_items: Vec<MediaItem> = items.into_iter().take(limit).collect();
                        debug!(
                            "Library {} has {} items (limited to {})",
                            library.title,
                            items_count,
                            limited_items.len()
                        );

                        if !limited_items.is_empty() {
                            let friendly_title = if current_source_id.is_some() {
                                // When filtering by source, don't show source prefix
                                library.title.clone()
                            } else {
                                // When showing all sources, show friendly names
                                Self::create_friendly_library_name(
                                    &library.title,
                                    &library.source_id,
                                )
                            };

                            debug!("Adding library section: {}", friendly_title);
                            sections.push(MediaSection {
                                title: friendly_title,
                                items: limited_items,
                                library_id: Some(library.id.clone()),
                                section_type: SectionType::Library(library.id.clone()),
                            });
                        } else {
                            debug!("Skipping empty library: {}", library.title);
                        }
                    } else {
                        warn!("Failed to get media items for library: {}", library.title);
                    }
                }
            }
            Err(e) => error!("Failed to load libraries: {}", e),
        }

        info!("Final sections count: {}", sections.len());
        if sections.is_empty() {
            warn!(
                "No sections available for source filter: {:?}",
                current_source_id
            );
        }

        self.sections.set(sections).await;
        self.is_loading.set(false).await;

        Ok(())
    }

    async fn load_home_content_with_sync(&self) -> Result<()> {
        // This method can trigger sync operations and refresh data
        // For now, we just refresh from cache - the sync manager will
        // handle background syncing and trigger events when new data arrives
        self.load_home_content_from_cache().await
    }

    async fn load_continue_watching(&self) -> Result<Vec<MediaItem>> {
        // Get media items that are in progress, filtered by current source if specified
        let current_source_id = self.current_source_id.get().await;
        let items = if let Some(source_id) = current_source_id {
            self.data_service
                .get_continue_watching_for_source(&source_id)
                .await?
        } else {
            self.data_service.get_continue_watching().await?
        };

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

        // Filter by current source if specified
        let current_source_id = self.current_source_id.get().await;
        if let Some(source_id) = current_source_id {
            // Get recently added from specific source
            self.data_service
                .get_recently_added_for_source(&source_id, Some(limit))
                .await
        } else {
            // Get recently added from all sources
            self.data_service.get_recently_added(Some(limit)).await
        }
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

                // Get the library info to update the title as well
                if let Ok(Some(library)) = self.data_service.get_library(&library_id).await {
                    let friendly_title =
                        Self::create_friendly_library_name(&library.title, &library.source_id);

                    // Use update instead of get/set to avoid race conditions
                    self.sections.update(|sections| {
                        if let Some(section) = sections.iter_mut().find(
                            |s| matches!(&s.section_type, SectionType::Library(id) if id == &library_id),
                        ) {
                            section.items = limited_items;
                            section.title = friendly_title; // Update title with friendly name
                        }
                    }).await;
                } else {
                    // Fallback if library lookup fails
                    self.sections.update(|sections| {
                        if let Some(section) = sections.iter_mut().find(
                            |s| matches!(&s.section_type, SectionType::Library(id) if id == &library_id),
                        ) {
                            section.items = limited_items;
                        }
                    }).await;
                }
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

        let _ = self.load_home_content_from_cache().await;
    }

    pub async fn set_featured_item(&self, item: MediaItem) {
        self.featured_item.set(Some(item)).await;
    }

    async fn handle_event(&self, event: DatabaseEvent) {
        match event.event_type {
            EventType::MediaCreated | EventType::MediaUpdated | EventType::MediaDeleted => {
                let _ = self.load_home_content_from_cache().await;
            }
            EventType::PlaybackPositionUpdated | EventType::PlaybackCompleted => {
                let _ = self.refresh_section(SectionType::ContinueWatching).await;
            }
            EventType::LibraryCreated
            | EventType::LibraryDeleted
            | EventType::LibraryItemCountChanged => {
                let _ = self.load_home_content_from_cache().await;
            }
            EventType::HomeSectionsUpdated => {
                // Reload content when home sections are updated from sync
                let _ = self.load_home_content_from_cache().await;
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

    pub fn current_source_id(&self) -> &Property<Option<String>> {
        &self.current_source_id
    }

    pub fn backends_ready(&self) -> &Property<bool> {
        &self.backends_ready
    }

    pub fn error(&self) -> &Property<Option<String>> {
        &self.error
    }

    pub async fn set_source_filter(&self, source_id: Option<String>) -> Result<()> {
        use tracing::info;

        let old_source = self.current_source_id.get().await;
        info!(
            "Changing source filter from {:?} to {:?}",
            old_source, source_id
        );

        // Clear existing sections immediately for instant feedback
        self.sections.set(Vec::new()).await;
        self.is_loading.set(true).await;

        self.current_source_id.set(source_id).await;

        // Reload content with new filter (from cache for immediate response)
        info!("Reloading home content with new source filter");
        self.load_home_content_from_cache().await
    }

    pub async fn refresh(&self) -> Result<()> {
        // Use the existing load_home_content method which properly loads all sections
        self.load_home_content().await
    }

    /// Bind this ViewModel to the application initialization state for progressive enhancement
    pub async fn bind_to_initialization_state(&self, init_state: &AppInitializationState) {
        // Update backends_ready based on any source being connected or playback ready
        let sources_connected = init_state.sources_connected.clone();
        let backends_ready = self.backends_ready.clone();

        tokio::spawn(async move {
            let mut subscriber = sources_connected.subscribe();
            while subscriber.wait_for_change().await {
                let sources = sources_connected.get().await;

                // Check if any source is ready (connected or playback ready)
                let any_ready = sources.values().any(|status| match status {
                    SourceReadiness::Connected { .. }
                    | SourceReadiness::PlaybackReady { .. }
                    | SourceReadiness::Syncing { .. } => true,
                    _ => false,
                });

                backends_ready.set(any_ready).await;

                if any_ready {
                    info!("At least one backend is ready - HomeViewModel can load content");
                }
            }
        });

        // When cached data is loaded, trigger initial content load
        let cached_data_loaded = init_state.cached_data_loaded.clone();
        let self_clone = self.clone();

        tokio::spawn(async move {
            let mut subscriber = cached_data_loaded.subscribe();
            while subscriber.wait_for_change().await {
                if cached_data_loaded.get().await {
                    info!("Cached data loaded - loading home content from cache");
                    let _ = self_clone.load_home_content_from_cache().await;
                }
            }
        });

        // When sync becomes ready, refresh content to get latest data
        let sync_ready = init_state.sync_ready.clone();
        let self_clone = self.clone();

        tokio::spawn(async move {
            let mut subscriber = sync_ready.subscribe();
            while subscriber.wait_for_change().await {
                if sync_ready.get().await {
                    info!("Sync ready - refreshing home content with latest data");
                    // Small delay to let sync complete
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    let _ = self_clone.load_home_content().await;
                }
            }
        });
    }

    /// Handle partial initialization gracefully - load what's available
    pub async fn handle_partial_initialization(&self) -> Result<()> {
        info!("Handling partial initialization - loading available content");

        // First, always try to load from cache
        self.is_loading.set(true).await;

        // Load cached content immediately (this is non-blocking)
        match self.load_home_content_from_cache().await {
            Ok(_) => {
                info!("Successfully loaded content from cache");

                // Check if we have any content
                let sections = self.sections.get().await;
                if sections.is_empty() {
                    info!("No cached content available - showing empty state");
                    self.error
                        .set(Some(
                            "No content available. Please check your media sources.".to_string(),
                        ))
                        .await;
                } else {
                    info!("Loaded {} sections from cache", sections.len());
                    self.error.set(None).await;
                }
            }
            Err(e) => {
                error!("Failed to load cached content: {}", e);
                self.error
                    .set(Some(format!("Failed to load content: {}", e)))
                    .await;
            }
        }

        self.is_loading.set(false).await;

        // Check if any backends are ready
        if !self.backends_ready.get().await {
            info!("No backends ready - will retry when backends become available");

            // Set up a one-time listener for when backends become ready
            let backends_ready = self.backends_ready.clone();
            let self_clone = self.clone();

            tokio::spawn(async move {
                let mut subscriber = backends_ready.subscribe();
                // Wait for backends to become ready
                while !backends_ready.get().await {
                    subscriber.wait_for_change().await;
                }

                info!("Backends now ready - loading content");
                let _ = self_clone.load_home_content().await;
            });
        }

        Ok(())
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
            EventType::HomeSectionsUpdated,
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
            "current_source_id" => Some(self.current_source_id.subscribe()),
            "backends_ready" => Some(self.backends_ready.subscribe()),
            "error" => Some(self.error.subscribe()),
            _ => None,
        }
    }

    async fn refresh(&self) {
        let _ = self.load_home_content_from_cache().await;
    }
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
            current_source_id: self.current_source_id.clone(),
            backends_ready: self.backends_ready.clone(),
            event_bus: self.event_bus.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::initialization::{ApiClientStatus, ConnectionStatus, SourceInfo};
    use std::time::Duration;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_partial_initialization_with_no_backends() {
        // Create a mock DataService (would need proper mocking in production)
        let data_service = Arc::new(DataService::new(None, None));
        let home_vm = HomeViewModel::new(data_service);

        // Create initialization state with no backends ready
        let init_state = AppInitializationState::new();
        init_state.ui_ready.set(true).await;
        init_state.cached_data_loaded.set(true).await;

        // Bind to initialization state
        home_vm.bind_to_initialization_state(&init_state).await;

        // Handle partial initialization
        let result = home_vm.handle_partial_initialization().await;
        assert!(result.is_ok());

        // Should be loading initially
        assert!(!home_vm.backends_ready.get().await);

        // Simulate a backend becoming ready
        let mut sources = HashMap::new();
        sources.insert(
            "test_source".to_string(),
            SourceReadiness::PlaybackReady {
                server_name: "Test Server".to_string(),
                credentials_valid: true,
                last_successful_connection: None,
            },
        );
        init_state.sources_connected.set(sources).await;

        // Wait a bit for the reactive update
        sleep(Duration::from_millis(100)).await;

        // Now backends should be ready
        assert!(home_vm.backends_ready.get().await);
    }

    #[tokio::test]
    async fn test_graceful_fallback_to_cache() {
        let data_service = Arc::new(DataService::new(None, None));
        let home_vm = HomeViewModel::new(data_service);

        // Create initialization state with cached data available
        let init_state = AppInitializationState::new();
        init_state.ui_ready.set(true).await;
        init_state.cached_data_loaded.set(true).await;

        // Bind and handle partial initialization
        home_vm.bind_to_initialization_state(&init_state).await;
        let _ = home_vm.handle_partial_initialization().await;

        // Should have attempted to load from cache
        // In a real test, we'd verify the cache was queried
        assert!(!home_vm.is_loading.get().await);

        // If no content, should show appropriate error message
        let sections = home_vm.sections.get().await;
        if sections.is_empty() {
            let error = home_vm.error.get().await;
            assert!(error.is_some());
            assert!(error.unwrap().contains("No content available"));
        }
    }
}
