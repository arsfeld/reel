use chrono::{DateTime, Utc};
use std::collections::HashMap;

use crate::core::viewmodels::property::Property;

/// Multi-stage reactive initialization state for non-blocking app startup
pub struct AppInitializationState {
    // Stage 1: Instant (0ms) - UI can display immediately
    pub ui_ready: Property<bool>,
    pub cached_data_loaded: Property<bool>,

    // Stage 2: Background (100-500ms) - Configuration and credentials
    pub sources_discovered: Property<Vec<SourceInfo>>,
    pub playback_ready: Property<bool>,

    // Stage 3: Network-dependent (1-10s) - Active connections
    pub sources_connected: Property<HashMap<String, SourceReadiness>>,
    pub sync_ready: Property<bool>,
    // TODO: Add computed properties later when needed for UI binding
}

impl AppInitializationState {
    pub fn new() -> Self {
        let ui_ready = Property::new(false, "ui_ready");
        let cached_data_loaded = Property::new(false, "cached_data_loaded");
        let sources_discovered = Property::new(Vec::new(), "sources_discovered");
        let playback_ready = Property::new(false, "playback_ready");
        let sources_connected = Property::new(HashMap::new(), "sources_connected");
        let sync_ready = Property::new(false, "sync_ready");

        Self {
            ui_ready,
            cached_data_loaded,
            sources_discovered,
            playback_ready,
            sources_connected,
            sync_ready,
        }
    }
}

impl Clone for AppInitializationState {
    fn clone(&self) -> Self {
        Self {
            ui_ready: self.ui_ready.clone(),
            cached_data_loaded: self.cached_data_loaded.clone(),
            sources_discovered: self.sources_discovered.clone(),
            playback_ready: self.playback_ready.clone(),
            sources_connected: self.sources_connected.clone(),
            sync_ready: self.sync_ready.clone(),
        }
    }
}

/// Granular source readiness states for progressive enhancement
#[derive(Debug, Clone)]
pub enum SourceReadiness {
    /// No credentials or configuration available
    Unavailable,

    /// Has credentials and can attempt playback, but not fully connected
    PlaybackReady {
        credentials_valid: bool,
        last_successful_connection: Option<DateTime<Utc>>,
    },

    /// Full API access available - can sync metadata and browse
    Connected {
        api_client_status: ApiClientStatus,
        library_count: usize,
    },

    /// Connected and actively syncing metadata
    Syncing { progress: SyncProgress },
}

impl SourceReadiness {
    /// Check if this source can be used for media playback
    pub fn is_playable(&self) -> bool {
        matches!(
            self,
            SourceReadiness::PlaybackReady { .. }
                | SourceReadiness::Connected { .. }
                | SourceReadiness::Syncing { .. }
        )
    }

    /// Check if this source has full API connectivity
    pub fn is_fully_connected(&self) -> bool {
        matches!(
            self,
            SourceReadiness::Connected { .. } | SourceReadiness::Syncing { .. }
        )
    }

    /// Check if this source is actively syncing
    pub fn is_syncing(&self) -> bool {
        matches!(self, SourceReadiness::Syncing { .. })
    }
}

#[derive(Debug, Clone)]
pub enum ApiClientStatus {
    Ready,
    Authenticated,
    TestingConnection,
    Limited, // Some API features unavailable
}

#[derive(Debug, Clone)]
pub struct SyncProgress {
    pub current_library: String,
    pub libraries_completed: usize,
    pub total_libraries: usize,
    pub items_synced: usize,
    pub estimated_total_items: Option<usize>,
}

/// Information about a discovered source for UI display
#[derive(Debug, Clone)]
pub struct SourceInfo {
    pub id: String,
    pub name: String,
    pub source_type: String,
    pub libraries: Vec<LibraryInfo>,
    pub is_enabled: bool,
    pub connection_status: String,
}

#[derive(Debug, Clone)]
pub struct LibraryInfo {
    pub id: String,
    pub title: String,
    pub library_type: String,
    pub item_count: u32,
    pub icon: Option<String>,
}

/// Initialization events for the EventBus
#[derive(Debug, Clone)]
pub enum InitializationEvent {
    /// A source was discovered from configuration/cache
    SourceDiscovered { source_id: String, info: SourceInfo },

    /// A source is ready for playback (has credentials)
    SourcePlaybackReady { source_id: String },

    /// A source established full API connection
    SourceConnected {
        source_id: String,
        details: ConnectionDetails,
    },

    /// A source connection failed
    SourceConnectionFailed { source_id: String, error: String },

    /// All configured sources have been discovered
    AllSourcesDiscovered,

    /// At least one source is ready for playback
    FirstSourceReady,

    /// All sources are fully connected
    AllSourcesConnected,

    /// Initialization stage completed
    StageCompleted {
        stage: InitializationStage,
        duration_ms: u64,
    },
}

#[derive(Debug, Clone)]
pub enum InitializationStage {
    InstantUI,
    BackgroundDiscovery,
    NetworkConnections,
}

#[derive(Debug, Clone)]
pub struct ConnectionDetails {
    pub server_name: Option<String>,
    pub server_version: Option<String>,
    pub connection_type: crate::backends::traits::ConnectionType,
    pub library_count: usize,
}

impl ConnectionDetails {
    pub async fn from_backend(backend: &dyn crate::backends::MediaBackend) -> Self {
        let info = backend.get_backend_info().await;
        let library_count = backend
            .get_libraries()
            .await
            .map(|libs| libs.len())
            .unwrap_or(0);

        Self {
            server_name: info.server_name,
            server_version: info.server_version,
            connection_type: info.connection_type,
            library_count,
        }
    }
}
