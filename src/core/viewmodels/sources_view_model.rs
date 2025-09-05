use super::{Property, PropertySubscriber, ViewModel};
use crate::db::entities::libraries::Model as Library;
use crate::db::entities::sources::Model as Source;
use crate::db::entities::sync_status::Model as SyncStatus;
use crate::events::{DatabaseEvent, EventBus, EventFilter, EventPayload, EventType};
use crate::services::DataService;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info};

#[derive(Debug, Clone)]
pub struct SourceInfo {
    pub source: Source,
    pub libraries: Vec<Library>,
    pub sync_status: Option<SyncStatus>,
    pub connection_status: ConnectionStatus,
    pub sync_progress: SyncProgressInfo,
    pub last_error: Option<String>,
}

impl SourceInfo {
    /// Create a friendly display name for this source
    pub fn friendly_name(&self) -> String {
        crate::models::source_utils::create_friendly_name(
            &self.source.name,
            &self.source.source_type,
        )
    }
}

#[derive(Debug, Clone)]
pub enum ConnectionStatus {
    Connected,
    Connecting,
    Disconnected,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct SyncProgressInfo {
    pub is_syncing: bool,
    pub overall_progress: f32,
    pub current_stage: SyncStage,
    pub stage_progress: f32,
    pub items_processed: usize,
    pub total_items: usize,
    pub estimated_time_remaining: Option<std::time::Duration>,
}

#[derive(Debug, Clone)]
pub enum SyncStage {
    Idle,
    ConnectingToServer,
    DiscoveringLibraries,
    LoadingMovies {
        library_name: String,
    },
    LoadingTVShows {
        library_name: String,
    },
    LoadingEpisodes {
        show_name: String,
        season: u32,
        current: usize,
        total: usize,
    },
    LoadingMusic {
        library_name: String,
    },
    ProcessingMetadata,
    Complete,
    Failed {
        error: String,
    },
}

impl Default for SyncProgressInfo {
    fn default() -> Self {
        Self {
            is_syncing: false,
            overall_progress: 0.0,
            current_stage: SyncStage::Idle,
            stage_progress: 0.0,
            items_processed: 0,
            total_items: 0,
            estimated_time_remaining: None,
        }
    }
}

impl Default for ConnectionStatus {
    fn default() -> Self {
        Self::Disconnected
    }
}

pub struct SourcesViewModel {
    data_service: Arc<DataService>,
    sources: Property<Vec<SourceInfo>>,
    selected_source: Property<Option<Source>>,
    is_loading: Property<bool>,
    error: Property<Option<String>>,
    sync_in_progress: Property<HashMap<String, f32>>,
    total_sources: Property<usize>,
    online_sources: Property<usize>,
    event_bus: Option<Arc<EventBus>>,
}

impl SourcesViewModel {
    pub fn new(data_service: Arc<DataService>) -> Self {
        Self {
            data_service,
            sources: Property::new(Vec::new(), "sources"),
            selected_source: Property::new(None, "selected_source"),
            is_loading: Property::new(false, "is_loading"),
            error: Property::new(None, "error"),
            sync_in_progress: Property::new(HashMap::new(), "sync_in_progress"),
            total_sources: Property::new(0, "total_sources"),
            online_sources: Property::new(0, "online_sources"),
            event_bus: None,
        }
    }

    pub async fn load_sources(&self) -> Result<()> {
        self.is_loading.set(true).await;
        self.error.set(None).await;

        match self.data_service.get_sources().await {
            Ok(sources) => {
                let mut source_infos = Vec::new();
                let mut online_count = 0;

                for source in sources {
                    if source.is_online {
                        online_count += 1;
                    }

                    let libraries = self
                        .data_service
                        .get_libraries(&source.id)
                        .await
                        .unwrap_or_default();

                    let sync_status = self
                        .data_service
                        .get_latest_sync_status(&source.id)
                        .await
                        .ok()
                        .flatten();

                    let sync_progress_map = self.sync_in_progress.get().await;
                    let is_syncing = sync_progress_map.contains_key(&source.id);
                    let overall_progress =
                        sync_progress_map.get(&source.id).copied().unwrap_or(0.0);

                    // Determine connection status
                    let connection_status = if source.is_online {
                        ConnectionStatus::Connected
                    } else {
                        ConnectionStatus::Disconnected
                    };

                    // Create sync progress info
                    let sync_progress = if is_syncing {
                        SyncProgressInfo {
                            is_syncing: true,
                            overall_progress,
                            current_stage: if overall_progress < 0.2 {
                                SyncStage::ConnectingToServer
                            } else if overall_progress < 0.4 {
                                SyncStage::DiscoveringLibraries
                            } else {
                                // Determine stage based on library types
                                if let Some(library) = libraries.first() {
                                    match library.library_type.as_str() {
                                        "movie" => SyncStage::LoadingMovies {
                                            library_name: library.title.clone(),
                                        },
                                        "show" => SyncStage::LoadingTVShows {
                                            library_name: library.title.clone(),
                                        },
                                        _ => SyncStage::ProcessingMetadata,
                                    }
                                } else {
                                    SyncStage::ProcessingMetadata
                                }
                            },
                            stage_progress: (overall_progress * 5.0) % 1.0, // Approximate stage progress
                            items_processed: (overall_progress * 100.0) as usize,
                            total_items: 100,               // Placeholder
                            estimated_time_remaining: None, // TODO: Calculate based on sync speed
                        }
                    } else {
                        SyncProgressInfo::default()
                    };

                    let last_error = sync_status.as_ref().and_then(|s| {
                        if s.status == "failed" {
                            s.error_message.clone()
                        } else {
                            None
                        }
                    });

                    source_infos.push(SourceInfo {
                        source,
                        libraries,
                        sync_status,
                        connection_status,
                        sync_progress,
                        last_error,
                    });
                }

                self.total_sources.set(source_infos.len()).await;
                self.online_sources.set(online_count).await;
                self.sources.set(source_infos).await;
                self.is_loading.set(false).await;

                Ok(())
            }
            Err(e) => {
                error!("Failed to load sources: {}", e);
                self.error.set(Some(e.to_string())).await;
                self.is_loading.set(false).await;
                Err(e)
            }
        }
    }

    pub async fn add_source(&self, source: Source) -> Result<()> {
        match self.data_service.add_source(source.clone()).await {
            Ok(_) => {
                info!("Source added: {}", source.name);
                self.load_sources().await
            }
            Err(e) => {
                error!("Failed to add source: {}", e);
                self.error.set(Some(e.to_string())).await;
                Err(e)
            }
        }
    }

    pub async fn remove_source(&self, source_id: String) -> Result<()> {
        match self.data_service.remove_source(&source_id).await {
            Ok(_) => {
                info!("Source removed: {}", source_id);
                if let Some(selected) = self.selected_source.get().await
                    && selected.id == source_id
                {
                    self.selected_source.set(None).await;
                }
                self.load_sources().await
            }
            Err(e) => {
                error!("Failed to remove source: {}", e);
                self.error.set(Some(e.to_string())).await;
                Err(e)
            }
        }
    }

    pub async fn sync_source(&self, source_id: String) -> Result<()> {
        self.sync_in_progress
            .update(|map| {
                map.insert(source_id.clone(), 0.0);
            })
            .await;

        let event = DatabaseEvent::new(
            EventType::SyncStarted,
            EventPayload::Sync {
                source_id: source_id.clone(),
                sync_type: "manual".to_string(),
                progress: Some(0.0),
                items_synced: None,
                error: None,
            },
        );

        if let Some(event_bus) = &self.event_bus {
            let _ = event_bus.publish(event).await;
        }

        self.load_sources().await
    }

    pub async fn cancel_sync(&self, source_id: String) {
        self.sync_in_progress
            .update(|map| {
                map.remove(&source_id);
            })
            .await;

        let _ = self.load_sources().await;
    }

    pub async fn select_source(&self, source_id: String) {
        let sources = self.sources.get().await;
        if let Some(info) = sources.iter().find(|s| s.source.id == source_id) {
            self.selected_source.set(Some(info.source.clone())).await;
        }
    }

    pub async fn refresh_source(&self, source_id: String) -> Result<()> {
        match self.data_service.get_source(&source_id).await {
            Ok(Some(source)) => {
                let mut sources = self.sources.get().await;
                if let Some(info) = sources.iter_mut().find(|s| s.source.id == source_id) {
                    info.source = source;

                    info.libraries = self
                        .data_service
                        .get_libraries(&source_id)
                        .await
                        .unwrap_or_default();

                    info.sync_status = self
                        .data_service
                        .get_latest_sync_status(&source_id)
                        .await
                        .ok()
                        .flatten();
                }
                self.sources.set(sources).await;
                Ok(())
            }
            Ok(None) => {
                let msg = format!("Source {} not found", source_id);
                Err(anyhow::anyhow!(msg))
            }
            Err(e) => {
                error!("Failed to refresh source: {}", e);
                Err(e)
            }
        }
    }

    async fn handle_event(&self, event: DatabaseEvent) {
        match event.event_type {
            EventType::SourceAdded | EventType::SourceRemoved => {
                let _ = self.load_sources().await;
            }
            EventType::SourceUpdated | EventType::SourceOnlineStatusChanged => {
                if let EventPayload::Source { id, .. } = event.payload {
                    let _ = self.refresh_source(id).await;
                }
            }
            EventType::SyncStarted => {
                if let EventPayload::Sync { source_id, .. } = event.payload {
                    self.sync_in_progress
                        .update(|map| {
                            map.insert(source_id.clone(), 0.0);
                        })
                        .await;
                    let _ = self.refresh_source(source_id).await;
                }
            }
            EventType::SyncProgress => {
                if let EventPayload::Sync {
                    source_id,
                    progress,
                    ..
                } = event.payload
                    && let Some(progress) = progress
                {
                    self.sync_in_progress
                        .update(|map| {
                            map.insert(source_id.clone(), progress);
                        })
                        .await;

                    let mut sources = self.sources.get().await;
                    if let Some(info) = sources.iter_mut().find(|s| s.source.id == source_id) {
                        info!(
                            "Updating sync progress for source '{}': {} -> {}",
                            info.source.name, info.sync_progress.overall_progress, progress
                        );
                        info.sync_progress.overall_progress = progress;
                        info.sync_progress.is_syncing = true;

                        // Update stage based on progress
                        info.sync_progress.current_stage = if progress < 0.2 {
                            SyncStage::ConnectingToServer
                        } else if progress < 0.4 {
                            SyncStage::DiscoveringLibraries
                        } else if progress < 0.8 {
                            // Determine stage based on library types
                            if let Some(library) = info.libraries.first() {
                                match library.library_type.as_str() {
                                    "movie" => SyncStage::LoadingMovies {
                                        library_name: library.title.clone(),
                                    },
                                    "show" => SyncStage::LoadingTVShows {
                                        library_name: library.title.clone(),
                                    },
                                    _ => SyncStage::ProcessingMetadata,
                                }
                            } else {
                                SyncStage::ProcessingMetadata
                            }
                        } else {
                            SyncStage::Complete
                        };

                        info.sync_progress.stage_progress = (progress * 5.0) % 1.0;
                        info.sync_progress.items_processed = (progress * 100.0) as usize;
                    }
                    self.sources.set(sources).await;
                }
            }
            EventType::SyncCompleted | EventType::SyncFailed => {
                if let EventPayload::Sync {
                    source_id, error, ..
                } = event.payload
                {
                    self.sync_in_progress
                        .update(|map| {
                            map.remove(&source_id);
                        })
                        .await;

                    let mut sources = self.sources.get().await;
                    if let Some(info) = sources.iter_mut().find(|s| s.source.id == source_id) {
                        info.sync_progress.is_syncing = false;
                        info.sync_progress.overall_progress =
                            if error.is_none() { 1.0 } else { 0.0 };
                        info.sync_progress.current_stage = if let Some(error_msg) = &error {
                            SyncStage::Failed {
                                error: error_msg.clone(),
                            }
                        } else {
                            SyncStage::Complete
                        };
                        info.last_error = error.clone();

                        // Update connection status based on sync result
                        if error.is_none() {
                            info.connection_status = ConnectionStatus::Connected;
                        }
                    }
                    self.sources.set(sources).await;

                    let _ = self.refresh_source(source_id).await;
                }
            }
            EventType::LibraryCreated | EventType::LibraryDeleted | EventType::LibraryUpdated => {
                let _ = self.load_sources().await;
            }
            EventType::UserAuthenticated => {
                info!("User authenticated, refreshing sources");
                let _ = self.load_sources().await;
            }
            EventType::UserLoggedOut => {
                info!("User logged out, refreshing sources");
                let _ = self.load_sources().await;
            }
            _ => {}
        }
    }

    pub fn sources(&self) -> &Property<Vec<SourceInfo>> {
        &self.sources
    }

    pub fn selected_source(&self) -> &Property<Option<Source>> {
        &self.selected_source
    }

    pub fn is_loading(&self) -> &Property<bool> {
        &self.is_loading
    }

    pub fn error(&self) -> &Property<Option<String>> {
        &self.error
    }

    pub fn total_sources(&self) -> &Property<usize> {
        &self.total_sources
    }

    pub fn online_sources(&self) -> &Property<usize> {
        &self.online_sources
    }

    pub async fn refresh(&self) -> anyhow::Result<()> {
        // Load sources already handles loading state
        self.load_sources().await
    }
}

#[async_trait::async_trait]
impl ViewModel for SourcesViewModel {
    async fn initialize(&self, event_bus: Arc<EventBus>) {
        let filter = EventFilter::new().with_types(vec![
            EventType::SourceAdded,
            EventType::SourceUpdated,
            EventType::SourceRemoved,
            EventType::SourceOnlineStatusChanged,
            EventType::SyncStarted,
            EventType::SyncProgress,
            EventType::SyncCompleted,
            EventType::SyncFailed,
            EventType::LibraryCreated,
            EventType::LibraryUpdated,
            EventType::LibraryDeleted,
            EventType::UserAuthenticated,
            EventType::UserLoggedOut,
        ]);

        let mut subscriber = event_bus.subscribe_filtered(filter);
        let self_clone = self.clone();

        tokio::spawn(async move {
            while let Ok(event) = subscriber.recv().await {
                self_clone.handle_event(event).await;
            }
        });

        let _ = self.load_sources().await;
    }

    fn subscribe_to_property(&self, property_name: &str) -> Option<PropertySubscriber> {
        match property_name {
            "sources" => Some(self.sources.subscribe()),
            "selected_source" => Some(self.selected_source.subscribe()),
            "is_loading" => Some(self.is_loading.subscribe()),
            "error" => Some(self.error.subscribe()),
            "total_sources" => Some(self.total_sources.subscribe()),
            "online_sources" => Some(self.online_sources.subscribe()),
            _ => None,
        }
    }

    async fn refresh(&self) {
        let _ = self.load_sources().await;
    }

    fn dispose(&self) {}
}

impl Clone for SourcesViewModel {
    fn clone(&self) -> Self {
        Self {
            data_service: self.data_service.clone(),
            sources: self.sources.clone(),
            selected_source: self.selected_source.clone(),
            is_loading: self.is_loading.clone(),
            error: self.error.clone(),
            sync_in_progress: self.sync_in_progress.clone(),
            total_sources: self.total_sources.clone(),
            online_sources: self.online_sources.clone(),
            event_bus: self.event_bus.clone(),
        }
    }
}
