use super::{Property, PropertySubscriber, ViewModel};
use crate::events::{EventBus, EventPayload, EventType};
use crate::services::data::DataService;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Debug)]
pub struct LibraryInfo {
    pub id: String,
    pub source_id: String,
    pub title: String,
    pub library_type: String,
    pub item_count: i32,
    pub icon: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SourceInfo {
    pub id: String,
    pub name: String,
    pub source_type: String,
    pub libraries: Vec<LibraryInfo>,
    pub is_online: bool,
}

pub struct SidebarViewModel {
    // Properties
    sources: Property<Vec<SourceInfo>>,
    is_loading: Property<bool>,
    is_connected: Property<bool>,
    status_text: Property<String>,
    status_icon: Property<String>,
    show_spinner: Property<bool>,

    // Services
    data_service: Arc<DataService>,
    event_bus: RwLock<Option<Arc<EventBus>>>,
}

impl std::fmt::Debug for SidebarViewModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SidebarViewModel")
            .field("sources", &"Property<Vec<SourceInfo>>")
            .field("is_loading", &"Property<bool>")
            .field("is_connected", &"Property<bool>")
            .field("status_text", &"Property<String>")
            .field("status_icon", &"Property<String>")
            .field("show_spinner", &"Property<bool>")
            .field("data_service", &"Arc<DataService>")
            .field("event_bus", &"Option<Arc<EventBus>>")
            .finish()
    }
}

impl SidebarViewModel {
    pub fn new(data_service: Arc<DataService>) -> Self {
        Self {
            sources: Property::new(Vec::new(), "sources"),
            is_loading: Property::new(false, "is_loading"),
            is_connected: Property::new(false, "is_connected"),
            status_text: Property::new("Not connected".to_string(), "status_text"),
            status_icon: Property::new("network-offline-symbolic".to_string(), "status_icon"),
            show_spinner: Property::new(false, "show_spinner"),
            data_service,
            event_bus: RwLock::new(None),
        }
    }

    /// Get the sources property for subscription
    pub fn sources(&self) -> &Property<Vec<SourceInfo>> {
        &self.sources
    }

    /// Get the loading state property
    pub fn is_loading(&self) -> &Property<bool> {
        &self.is_loading
    }

    /// Get the connection status property
    pub fn is_connected(&self) -> &Property<bool> {
        &self.is_connected
    }

    /// Get the status text property
    pub fn status_text(&self) -> &Property<String> {
        &self.status_text
    }

    /// Get the status icon property
    pub fn status_icon(&self) -> &Property<String> {
        &self.status_icon
    }

    /// Get the spinner visibility property
    pub fn show_spinner(&self) -> &Property<bool> {
        &self.show_spinner
    }

    /// Load all sources and their libraries from the database
    pub async fn load_sources(&self) {
        use futures::future::join_all;

        self.is_loading.set(true).await;
        self.show_spinner.set(true).await;
        self.status_text.set("Loading sources...".to_string()).await;
        self.status_icon
            .set("content-loading-symbolic".to_string())
            .await;

        // Get all sources
        let sources_result = self.data_service.get_all_sources().await;

        match sources_result {
            Ok(sources) => {
                // Load all libraries in parallel
                let library_futures = sources.iter().map(|source| {
                    let source_id = source.id.clone();
                    let data_service = self.data_service.clone();
                    async move {
                        match data_service.get_libraries(&source_id).await {
                            Ok(libs) => (source_id.clone(), libs),
                            Err(e) => {
                                tracing::error!(
                                    "Failed to get libraries for source {}: {}",
                                    source_id,
                                    e
                                );
                                (source_id, Vec::new())
                            }
                        }
                    }
                });

                let library_results = join_all(library_futures).await;

                // Create a map for quick lookup
                let mut libraries_map = std::collections::HashMap::new();
                for (source_id, libraries) in library_results {
                    libraries_map.insert(source_id, libraries);
                }

                // Build source infos
                let mut source_infos = Vec::new();
                for source in sources {
                    let libraries = libraries_map.remove(&source.id).unwrap_or_default();

                    // Convert to LibraryInfo
                    let library_infos: Vec<LibraryInfo> = libraries
                        .into_iter()
                        .map(|lib| LibraryInfo {
                            id: lib.id,
                            source_id: lib.source_id,
                            title: lib.title,
                            library_type: lib.library_type,
                            item_count: lib.item_count,
                            icon: lib.icon,
                        })
                        .collect();

                    tracing::info!(
                        "load_sources: processing source - id: '{}', name: '{}', type: '{}'",
                        source.id,
                        source.name,
                        source.source_type
                    );

                    let friendly_name = crate::models::source_utils::create_friendly_name(
                        &source.name,
                        &source.source_type,
                    );

                    source_infos.push(SourceInfo {
                        id: source.id.clone(),
                        name: friendly_name,
                        source_type: source.source_type.clone(),
                        libraries: library_infos,
                        is_online: source.is_online,
                    });
                }

                // Update properties
                let has_sources = !source_infos.is_empty();
                let online_count = source_infos.iter().filter(|s| s.is_online).count();
                let total_count = source_infos.len();

                self.sources.set(source_infos).await;
                self.is_connected.set(has_sources).await;

                if has_sources {
                    if online_count == total_count {
                        self.status_text
                            .set(format!(
                                "{} source{} connected",
                                total_count,
                                if total_count == 1 { "" } else { "s" }
                            ))
                            .await;
                        self.status_icon
                            .set("network-transmit-receive-symbolic".to_string())
                            .await;
                    } else if online_count > 0 {
                        self.status_text
                            .set(format!("{}/{} sources online", online_count, total_count))
                            .await;
                        self.status_icon
                            .set("network-wireless-symbolic".to_string())
                            .await;
                    } else {
                        self.status_text
                            .set("Sources offline (cached)".to_string())
                            .await;
                        self.status_icon
                            .set("folder-remote-symbolic".to_string())
                            .await;
                    }
                } else {
                    self.status_text
                        .set("No sources configured".to_string())
                        .await;
                    self.status_icon
                        .set("network-offline-symbolic".to_string())
                        .await;
                }
            }
            Err(e) => {
                tracing::error!("Failed to load sources: {}", e);
                self.sources.set(Vec::new()).await;
                self.is_connected.set(false).await;
                self.status_text
                    .set("Failed to load sources".to_string())
                    .await;
                self.status_icon
                    .set("dialog-error-symbolic".to_string())
                    .await;
            }
        }

        self.is_loading.set(false).await;
        self.show_spinner.set(false).await;
    }

    /// Static method to reload sources from event handler context
    async fn reload_sources(
        data_service: Arc<DataService>,
        sources_prop: Property<Vec<SourceInfo>>,
        is_connected_prop: Property<bool>,
        status_text_prop: Property<String>,
        status_icon_prop: Property<String>,
        is_loading_prop: Property<bool>,
    ) {
        let _ = is_loading_prop.set(true).await;

        // Get all sources
        let sources_result = data_service.get_all_sources().await;

        match sources_result {
            Ok(sources) => {
                let mut source_infos = Vec::new();

                for source in sources {
                    // Get libraries for this source
                    let libraries = match data_service.get_libraries(&source.id).await {
                        Ok(libs) => libs,
                        Err(e) => {
                            tracing::error!(
                                "Failed to get libraries for source {}: {}",
                                source.id,
                                e
                            );
                            Vec::new()
                        }
                    };

                    // Convert to LibraryInfo
                    let library_infos: Vec<LibraryInfo> = libraries
                        .into_iter()
                        .map(|lib| LibraryInfo {
                            id: lib.id,
                            source_id: lib.source_id,
                            title: lib.title,
                            library_type: lib.library_type,
                            item_count: lib.item_count,
                            icon: lib.icon,
                        })
                        .collect();

                    tracing::info!(
                        "reload_sources: processing source - id: '{}', name: '{}', type: '{}'",
                        source.id,
                        source.name,
                        source.source_type
                    );

                    let friendly_name = crate::models::source_utils::create_friendly_name(
                        &source.name,
                        &source.source_type,
                    );

                    source_infos.push(SourceInfo {
                        id: source.id.clone(),
                        name: friendly_name,
                        source_type: source.source_type.clone(),
                        libraries: library_infos,
                        is_online: source.is_online,
                    });
                }

                // Update properties
                let has_sources = !source_infos.is_empty();
                let online_count = source_infos.iter().filter(|s| s.is_online).count();
                let total_count = source_infos.len();

                let _ = sources_prop.set(source_infos).await;
                let _ = is_connected_prop.set(has_sources).await;

                if has_sources {
                    if online_count == total_count {
                        let _ = status_text_prop
                            .set(format!(
                                "{} source{} connected",
                                total_count,
                                if total_count == 1 { "" } else { "s" }
                            ))
                            .await;
                        let _ = status_icon_prop
                            .set("network-transmit-receive-symbolic".to_string())
                            .await;
                    } else if online_count > 0 {
                        let _ = status_text_prop
                            .set(format!("{}/{} sources online", online_count, total_count))
                            .await;
                        let _ = status_icon_prop
                            .set("network-wireless-symbolic".to_string())
                            .await;
                    } else {
                        let _ = status_text_prop
                            .set("Sources offline (cached)".to_string())
                            .await;
                        let _ = status_icon_prop
                            .set("folder-remote-symbolic".to_string())
                            .await;
                    }
                } else {
                    let _ = status_text_prop
                        .set("No sources configured".to_string())
                        .await;
                    let _ = status_icon_prop
                        .set("network-offline-symbolic".to_string())
                        .await;
                }
            }
            Err(e) => {
                tracing::error!("Failed to load sources: {}", e);
                let _ = sources_prop.set(Vec::new()).await;
                let _ = is_connected_prop.set(false).await;
                let _ = status_text_prop
                    .set("Failed to load sources".to_string())
                    .await;
                let _ = status_icon_prop
                    .set("dialog-error-symbolic".to_string())
                    .await;
            }
        }

        let _ = is_loading_prop.set(false).await;
    }
}

#[async_trait::async_trait]
impl ViewModel for SidebarViewModel {
    async fn initialize(&self, event_bus: Arc<EventBus>) {
        // Store event bus reference using RwLock
        *self.event_bus.write().await = Some(event_bus.clone());

        // Subscribe to events
        let mut receiver = event_bus.subscribe();

        // Load initial data
        self.load_sources().await;

        // Clone everything we need for the event handler
        let data_service = self.data_service.clone();
        let sources = self.sources.clone();
        let is_connected = self.is_connected.clone();
        let show_spinner = self.show_spinner.clone();
        let status_text = self.status_text.clone();
        let status_icon = self.status_icon.clone();
        let is_loading = self.is_loading.clone();

        tokio::spawn(async move {
            while let Ok(event) = receiver.recv().await {
                match event.event_type {
                    EventType::LibraryCreated => {
                        tracing::info!(
                            "SidebarViewModel received LibraryCreated event - reloading sources"
                        );
                        Self::reload_sources(
                            data_service.clone(),
                            sources.clone(),
                            is_connected.clone(),
                            status_text.clone(),
                            status_icon.clone(),
                            is_loading.clone(),
                        )
                        .await;
                    }
                    EventType::LibraryUpdated => {
                        tracing::info!(
                            "SidebarViewModel received LibraryUpdated event - reloading sources"
                        );
                        Self::reload_sources(
                            data_service.clone(),
                            sources.clone(),
                            is_connected.clone(),
                            status_text.clone(),
                            status_icon.clone(),
                            is_loading.clone(),
                        )
                        .await;
                    }
                    EventType::LibraryItemCountChanged => {
                        if let EventPayload::Library {
                            id: library_id,
                            source_id,
                            item_count,
                        } = &event.payload
                            && let Some(new_count) = item_count
                        {
                            tracing::info!(
                                "SidebarViewModel received LibraryItemCountChanged for library {} with count {}",
                                library_id,
                                new_count
                            );

                            // Update the specific library's item count in memory
                            let mut current_sources = sources.get().await;
                            let mut updated = false;

                            for source in &mut current_sources {
                                if source.id == *source_id {
                                    for library in &mut source.libraries {
                                        if library.id == *library_id {
                                            library.item_count = *new_count;
                                            updated = true;
                                            break;
                                        }
                                    }
                                    break;
                                }
                            }

                            if updated {
                                // Update the sources property to trigger UI refresh
                                let _ = sources.set(current_sources).await;
                            }
                        }
                    }
                    EventType::SourceAdded => {
                        tracing::info!(
                            "SidebarViewModel received SourceAdded event - reloading sources"
                        );
                        Self::reload_sources(
                            data_service.clone(),
                            sources.clone(),
                            is_connected.clone(),
                            status_text.clone(),
                            status_icon.clone(),
                            is_loading.clone(),
                        )
                        .await;
                    }
                    EventType::SourceUpdated => {
                        tracing::info!(
                            "SidebarViewModel received SourceUpdated event - reloading sources"
                        );
                        Self::reload_sources(
                            data_service.clone(),
                            sources.clone(),
                            is_connected.clone(),
                            status_text.clone(),
                            status_icon.clone(),
                            is_loading.clone(),
                        )
                        .await;
                    }
                    EventType::SourceRemoved => {
                        tracing::info!(
                            "SidebarViewModel received SourceRemoved event - reloading sources"
                        );
                        Self::reload_sources(
                            data_service.clone(),
                            sources.clone(),
                            is_connected.clone(),
                            status_text.clone(),
                            status_icon.clone(),
                            is_loading.clone(),
                        )
                        .await;
                    }
                    EventType::SourceOnlineStatusChanged => {
                        tracing::info!(
                            "SidebarViewModel received SourceOnlineStatusChanged event - reloading sources"
                        );
                        Self::reload_sources(
                            data_service.clone(),
                            sources.clone(),
                            is_connected.clone(),
                            status_text.clone(),
                            status_icon.clone(),
                            is_loading.clone(),
                        )
                        .await;
                    }
                    EventType::SyncStarted => {
                        let _ = show_spinner.set(true).await;
                        let _ = status_text.set("Syncing...".to_string()).await;
                        let _ = status_icon
                            .set("emblem-synchronizing-symbolic".to_string())
                            .await;
                    }
                    EventType::SyncCompleted => {
                        tracing::info!(
                            "SidebarViewModel received SyncCompleted event - reloading sources"
                        );
                        let _ = show_spinner.set(false).await;
                        Self::reload_sources(
                            data_service.clone(),
                            sources.clone(),
                            is_connected.clone(),
                            status_text.clone(),
                            status_icon.clone(),
                            is_loading.clone(),
                        )
                        .await;
                    }
                    _ => {}
                }
            }
        });
    }

    fn subscribe_to_property(&self, property_name: &str) -> Option<PropertySubscriber> {
        match property_name {
            "sources" => Some(self.sources.subscribe()),
            "is_loading" => Some(self.is_loading.subscribe()),
            "is_connected" => Some(self.is_connected.subscribe()),
            "status_text" => Some(self.status_text.subscribe()),
            "status_icon" => Some(self.status_icon.subscribe()),
            "show_spinner" => Some(self.show_spinner.subscribe()),
            _ => None,
        }
    }

    async fn refresh(&self) {
        self.load_sources().await;
    }
}
