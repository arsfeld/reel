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
    pub is_syncing: bool,
    pub sync_progress: f32,
    pub last_error: Option<String>,
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

                    let sync_progress = self.sync_in_progress.get().await;
                    let is_syncing = sync_progress.contains_key(&source.id);
                    let progress = sync_progress.get(&source.id).copied().unwrap_or(0.0);

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
                        is_syncing,
                        sync_progress: progress,
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
                        info.sync_progress = progress;
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
                        info.is_syncing = false;
                        info.sync_progress = 0.0;
                        info.last_error = error;
                    }
                    self.sources.set(sources).await;

                    let _ = self.refresh_source(source_id).await;
                }
            }
            EventType::LibraryCreated | EventType::LibraryDeleted | EventType::LibraryUpdated => {
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
