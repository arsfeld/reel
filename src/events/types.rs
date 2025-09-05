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

    // Source events
    SourceAdded,
    SourceUpdated,
    SourceRemoved,
    SourceOnlineStatusChanged,
    SourcesCleanedUp,

    // Sync events
    SyncStarted,
    SyncProgress,
    SyncCompleted,
    SyncFailed,

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
            EventType::SyncStarted => "sync.started",
            EventType::SyncProgress => "sync.progress",
            EventType::SyncCompleted => "sync.completed",
            EventType::SyncFailed => "sync.failed",
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
        }
    }
}
