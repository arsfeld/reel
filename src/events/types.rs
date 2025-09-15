use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Main database event type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseEvent {
    pub id: String,
    pub event_type: EventType,
    pub payload: EventPayload,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub source: EventSource,
    pub priority: EventPriority,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl DatabaseEvent {
    pub fn new(event_type: EventType, payload: EventPayload) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            event_type,
            payload,
            timestamp: chrono::Utc::now(),
            source: EventSource::System,
            priority: EventPriority::Normal,
            metadata: HashMap::new(),
        }
    }

    pub fn with_source(mut self, source: EventSource) -> Self {
        self.source = source;
        self
    }

    pub fn with_priority(mut self, priority: EventPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_metadata(mut self, key: String, value: serde_json::Value) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Event types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EventType {
    // Media events
    MediaCreated,
    MediaUpdated,
    MediaDeleted,
    MediaBatchCreated,
    MediaBatchUpdated,

    // Library events
    LibraryCreated,
    LibraryUpdated,
    LibraryDeleted,
    LibraryItemCountChanged,

    // Home sections events
    HomeSectionsUpdated,

    // Source events
    SourceAdded,
    SourceUpdated,
    SourceRemoved,
    SourceOnlineStatusChanged,
    SourcesCleanedUp,

    // Initialization events
    SourceDiscovered,
    SourcePlaybackReady,
    SourceConnected,
    SourceConnectionFailed,
    AllSourcesDiscovered,
    FirstSourceReady,
    AllSourcesConnected,
    InitializationStageCompleted,

    // Sync events
    SyncStarted,
    SyncProgress,
    SyncCompleted,
    SyncFailed,

    // Navigation events
    NavigationRequested,
    NavigationCompleted,
    NavigationFailed,
    NavigationHistoryChanged,
    PageTitleChanged,
    HeaderConfigChanged,

    // Sidebar navigation events
    LibraryNavigationRequested,
    HomeNavigationRequested,

    // Playback events
    PlaybackStarted,
    PlaybackPaused,
    PlaybackResumed,
    PlaybackStopped,
    PlaybackPositionUpdated,
    PlaybackCompleted,

    // Cache events
    CacheInvalidated,
    CacheUpdated,
    CacheCleared,

    // User events
    UserAuthenticated,
    UserLoggedOut,
    UserPreferencesChanged,

    // System events
    DatabaseMigrated,
    BackgroundTaskStarted,
    BackgroundTaskCompleted,
    ErrorOccurred,
}

/// Event payload containing specific data for each event type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventPayload {
    Media {
        id: String,
        media_type: String,
        library_id: String,
        source_id: String,
    },
    MediaBatch {
        ids: Vec<String>,
        library_id: String,
        source_id: String,
    },
    Library {
        id: String,
        source_id: String,
        item_count: Option<i32>,
    },
    Source {
        id: String,
        source_type: String,
        is_online: Option<bool>,
    },
    Sync {
        source_id: String,
        sync_type: String,
        progress: Option<f32>,
        items_synced: Option<usize>,
        error: Option<String>,
    },
    Navigation {
        page_name: String,
        page_title: Option<String>,
        can_go_back: Option<bool>,
        error: Option<String>,
    },
    LibraryNavigation {
        source_id: String,
        library_id: String,
        library_title: String,
        library_type: String,
    },
    HomeNavigation {
        source_id: Option<String>,
    },
    Playback {
        media_id: String,
        position: Option<Duration>,
        duration: Option<Duration>,
    },
    Cache {
        cache_key: Option<String>,
        cache_type: String,
    },
    User {
        user_id: String,
        action: String,
    },
    System {
        message: String,
        details: Option<serde_json::Value>,
    },
    Initialization {
        source_id: Option<String>,
        stage: Option<String>,
        readiness: Option<String>,
        error: Option<String>,
    },
}

/// Event source indicating where the event originated
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventSource {
    System,
    Repository(String),
    Service(String),
    UI(String),
    Backend(String),
    User(String),
}

/// Event priority for processing order
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, PartialOrd)]
pub enum EventPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

impl EventType {
    /// Get a string representation for filtering/routing
    pub fn as_str(&self) -> &'static str {
        match self {
            EventType::MediaCreated => "media.created",
            EventType::MediaUpdated => "media.updated",
            EventType::MediaDeleted => "media.deleted",
            EventType::MediaBatchCreated => "media.batch_created",
            EventType::MediaBatchUpdated => "media.batch_updated",
            EventType::LibraryCreated => "library.created",
            EventType::LibraryUpdated => "library.updated",
            EventType::LibraryDeleted => "library.deleted",
            EventType::LibraryItemCountChanged => "library.item_count_changed",
            EventType::SourceAdded => "source.added",
            EventType::SourceUpdated => "source.updated",
            EventType::SourceRemoved => "source.removed",
            EventType::SourceOnlineStatusChanged => "source.online_status_changed",
            EventType::SourcesCleanedUp => "sources.cleaned_up",
            EventType::SourceDiscovered => "source.discovered",
            EventType::SourcePlaybackReady => "source.playback_ready",
            EventType::SourceConnected => "source.connected",
            EventType::SourceConnectionFailed => "source.connection_failed",
            EventType::AllSourcesDiscovered => "sources.all_discovered",
            EventType::FirstSourceReady => "sources.first_ready",
            EventType::AllSourcesConnected => "sources.all_connected",
            EventType::InitializationStageCompleted => "initialization.stage_completed",
            EventType::SyncStarted => "sync.started",
            EventType::SyncProgress => "sync.progress",
            EventType::SyncCompleted => "sync.completed",
            EventType::SyncFailed => "sync.failed",
            EventType::NavigationRequested => "navigation.requested",
            EventType::NavigationCompleted => "navigation.completed",
            EventType::NavigationFailed => "navigation.failed",
            EventType::NavigationHistoryChanged => "navigation.history_changed",
            EventType::PageTitleChanged => "navigation.page_title_changed",
            EventType::HeaderConfigChanged => "navigation.header_config_changed",
            EventType::LibraryNavigationRequested => "sidebar.library_navigation_requested",
            EventType::HomeNavigationRequested => "sidebar.home_navigation_requested",
            EventType::PlaybackStarted => "playback.started",
            EventType::PlaybackPaused => "playback.paused",
            EventType::PlaybackResumed => "playback.resumed",
            EventType::PlaybackStopped => "playback.stopped",
            EventType::PlaybackPositionUpdated => "playback.position_updated",
            EventType::PlaybackCompleted => "playback.completed",
            EventType::CacheInvalidated => "cache.invalidated",
            EventType::CacheUpdated => "cache.updated",
            EventType::CacheCleared => "cache.cleared",
            EventType::UserAuthenticated => "user.authenticated",
            EventType::UserLoggedOut => "user.logged_out",
            EventType::UserPreferencesChanged => "user.preferences_changed",
            EventType::DatabaseMigrated => "system.database_migrated",
            EventType::BackgroundTaskStarted => "system.task_started",
            EventType::BackgroundTaskCompleted => "system.task_completed",
            EventType::ErrorOccurred => "system.error",
            EventType::HomeSectionsUpdated => "home.sections_updated",
        }
    }
}
