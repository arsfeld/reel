use std::collections::HashMap;

// ViewModels disabled during Relm4 migration
// use crate::core::viewmodels::property::Property;

/// Multi-stage reactive initialization state for non-blocking app startup
/// Note: Simplified during Relm4 migration - Properties replaced with plain values
#[derive(Clone, Debug)]
pub struct AppInitializationState {
    // Stage 1: Instant (0ms) - UI can display immediately
    pub ui_ready: bool,
    pub cached_data_loaded: bool,

    // Stage 2: Background (100-500ms) - Configuration and credentials
    pub sources_discovered: Vec<SourceInfo>,
    pub playback_ready: bool,

    // Stage 3: Network-dependent (1-10s) - Active connections
    pub sources_connected: HashMap<String, SourceReadiness>,
    pub sync_ready: bool,
}

impl AppInitializationState {
    pub fn new() -> Self {
        Self {
            ui_ready: false,
            cached_data_loaded: false,
            sources_discovered: Vec::new(),
            playback_ready: false,
            sources_connected: HashMap::new(),
            sync_ready: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SourceInfo {
    pub id: String,
    pub name: String,
    pub backend_type: String,
    pub status: ConnectionStatus,
    pub source_type: String,
    pub libraries: Vec<String>,
    pub is_enabled: bool,
    pub connection_status: String,
}

#[derive(Debug, Clone)]
pub enum SourceReadiness {
    Discovering,
    Connected {
        server_name: String,
        api_client_status: ApiClientStatus,
        library_count: u32,
    },
    PlaybackReady {
        server_name: String,
        credentials_valid: bool,
        last_successful_connection: Option<String>,
    },
    Syncing {
        progress: SyncProgress,
    },
    Unavailable,
    Error(String),
}

impl SourceReadiness {
    pub fn is_playable(&self) -> bool {
        matches!(self, SourceReadiness::PlaybackReady { .. })
    }

    pub fn is_fully_connected(&self) -> bool {
        matches!(
            self,
            SourceReadiness::Connected { .. } | SourceReadiness::PlaybackReady { .. }
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    Unknown,
    Connecting,
    Connected,
    Error(String),
}

#[derive(Debug, Clone)]
pub enum ApiClientStatus {
    NotReady,
    Ready,
}

#[derive(Debug, Clone)]
pub struct SyncProgress {
    pub current_item: usize,
    pub total_items: usize,
    pub current_library: String,
}
