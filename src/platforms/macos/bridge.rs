// Swift-Rust bridge for macOS frontend
// This module provides FFI bindings for the Swift frontend to interact with the Rust core

use anyhow::Result;
use once_cell::sync::Lazy;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::RwLock;

use self::swift_bridge_ffi::{BackendBridge, EventBridge, LibraryBridge};
use crate::backends::traits::{BackendInfo, BackendType};
use crate::config::Config;
use crate::core::state::AppState;
use crate::events::{DatabaseEvent, EventType};
use crate::models::Library;

// swift-bridge exports for Swift interop
// The generated Swift files will be produced by build.rs via swift_bridge_build
#[allow(non_snake_case)]
#[swift_bridge::bridge]
mod swift_bridge_ffi {
    // Value types crossing the bridge
    #[swift_bridge(swift_repr = "struct")]
    struct BackendBridge {
        id: String,
        name: String,
        kind: String,
    }

    #[swift_bridge(swift_repr = "struct")]
    struct LibraryBridge {
        id: String,
        name: String,
        item_count: u32,
    }

    #[swift_bridge(swift_repr = "struct")]
    struct EventBridge {
        kind: String,
        entity_id: String,
        payload_json: String,
    }

    extern "Rust" {
        type MacOSCore;
        type EventSub;

        // Lifecycle
        fn make_core() -> MacOSCore;
        fn sb_initialize(self: &mut MacOSCore) -> bool;
        fn sb_is_initialized(self: &MacOSCore) -> bool;

        // Utilities
        fn get_build_info() -> String;

        // Data access
        fn list_backends(self: &MacOSCore) -> Vec<BackendBridge>;
        fn get_cached_libraries(self: &MacOSCore, backend_id: String) -> Vec<LibraryBridge>;

        // Events
        fn subscribe(self: &MacOSCore, event_kinds: Vec<String>) -> EventSub;
        fn next_event_blocking(self: &mut EventSub, timeout_ms: u32) -> Option<EventBridge>;
        fn unsubscribe(self: EventSub);
    }
}

// Core handle for Swift to hold
pub struct MacOSCore {
    state: Option<Arc<AppState>>,
}

impl MacOSCore {
    pub fn new() -> Self {
        Self { state: None }
    }

    pub async fn initialize(&mut self) -> Result<()> {
        let config = Arc::new(RwLock::new(Config::load()?));
        let state = AppState::new_async(config).await?;
        self.state = Some(Arc::new(state));
        Ok(())
    }

    pub fn is_initialized(&self) -> bool {
        self.state.is_some()
    }

    // swift-bridge-friendly wrappers
    pub fn sb_initialize(&mut self) -> bool {
        RUNTIME.block_on(async { self.initialize().await.is_ok() })
    }

    pub fn sb_is_initialized(&self) -> bool {
        self.is_initialized()
    }

    pub fn list_backends(&self) -> Vec<BackendBridge> {
        if let Some(state) = &self.state {
            RUNTIME.block_on(async {
                state
                    .get_all_backends()
                    .await
                    .into_iter()
                    .map(BackendBridge::from)
                    .collect()
            })
        } else {
            Vec::new()
        }
    }

    pub fn get_cached_libraries(&self, backend_id: String) -> Vec<LibraryBridge> {
        if let Some(state) = &self.state {
            RUNTIME.block_on(async {
                match state.get_cached_libraries(&backend_id).await {
                    Ok(libs) => libs.into_iter().map(LibraryBridge::from).collect(),
                    Err(_) => Vec::new(),
                }
            })
        } else {
            Vec::new()
        }
    }

    pub fn subscribe(&self, event_kinds: Vec<String>) -> EventSub {
        let mut types = Vec::new();
        for s in event_kinds {
            if let Some(t) = parse_event_type(&s) {
                types.push(t);
            }
        }

        if let Some(state) = &self.state {
            let mut sub = state.event_bus.subscribe();
            if !types.is_empty() {
                sub = state.event_bus.subscribe_to_types(types);
            }
            EventSub { inner: Some(sub) }
        } else {
            EventSub { inner: None }
        }
    }
}

// Free function constructor used by swift-bridge
pub fn make_core() -> MacOSCore {
    MacOSCore::new()
}

// Free function exposed via swift-bridge for a quick sanity check
pub fn get_build_info() -> String {
    format!(
        "Reel {} on {} ({})",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

// Global runtime for the bridge
static RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("reel-bridge")
        .enable_all()
        .build()
        .expect("Failed to create runtime")
});

impl From<(String, BackendInfo)> for BackendBridge {
    fn from((id, info): (String, BackendInfo)) -> Self {
        let kind = match info.backend_type {
            BackendType::Plex => "plex",
            BackendType::Jellyfin => "jellyfin",
            BackendType::Local => "local",
            BackendType::Generic => "generic",
        };
        Self {
            id,
            name: info.display_name,
            kind: kind.to_string(),
        }
    }
}

impl From<Library> for LibraryBridge {
    fn from(lib: Library) -> Self {
        Self {
            id: lib.id,
            name: lib.title,
            item_count: 0,
        }
    }
}

// (C FFI removed; swift-bridge is the sole interop path.)

// Event subscription wrapper exposed to Swift
pub struct EventSub {
    inner: Option<crate::events::event_bus::EventSubscriber>,
}

impl EventSub {
    pub fn next_event_blocking(&mut self, timeout_ms: u32) -> Option<EventBridge> {
        if let Some(sub) = &mut self.inner {
            let timeout = std::time::Duration::from_millis(timeout_ms as u64);
            let fut = async { sub.recv().await.ok() };
            let evt_opt = RUNTIME.block_on(async {
                match tokio::time::timeout(timeout, fut).await {
                    Ok(evt) => evt,
                    Err(_) => None,
                }
            });
            if let Some(evt) = evt_opt {
                return Some(convert_event(evt));
            }
        }
        None
    }

    pub fn unsubscribe(self) {
        // Dropping self will drop the receiver
    }
}

fn convert_event(ev: DatabaseEvent) -> EventBridge {
    let kind = ev.event_type.as_str().to_string();
    let entity_id = match &ev.payload {
        crate::events::types::EventPayload::Media { id, .. } => id.clone(),
        crate::events::types::EventPayload::MediaBatch { ids, .. } => {
            ids.get(0).cloned().unwrap_or_default()
        }
        crate::events::types::EventPayload::Library { id, .. } => id.clone(),
        crate::events::types::EventPayload::Source { id, .. } => id.clone(),
        crate::events::types::EventPayload::Playback { media_id, .. } => media_id.clone(),
        _ => String::new(),
    };
    let payload_json = serde_json::to_string(&ev.payload).unwrap_or_default();
    EventBridge {
        kind,
        entity_id,
        payload_json,
    }
}

fn parse_event_type(s: &str) -> Option<EventType> {
    match s {
        // Dot-style and CamelCase
        "media.created" | "MediaCreated" => Some(EventType::MediaCreated),
        "media.updated" | "MediaUpdated" => Some(EventType::MediaUpdated),
        "media.deleted" | "MediaDeleted" => Some(EventType::MediaDeleted),
        "media.batch_created" | "MediaBatchCreated" => Some(EventType::MediaBatchCreated),
        "media.batch_updated" | "MediaBatchUpdated" => Some(EventType::MediaBatchUpdated),
        "library.created" | "LibraryCreated" => Some(EventType::LibraryCreated),
        "library.updated" | "LibraryUpdated" => Some(EventType::LibraryUpdated),
        "library.deleted" | "LibraryDeleted" => Some(EventType::LibraryDeleted),
        "library.item_count_changed" | "LibraryItemCountChanged" => {
            Some(EventType::LibraryItemCountChanged)
        }
        "source.added" | "SourceAdded" => Some(EventType::SourceAdded),
        "source.updated" | "SourceUpdated" => Some(EventType::SourceUpdated),
        "source.removed" | "SourceRemoved" => Some(EventType::SourceRemoved),
        "source.online_status_changed" | "SourceOnlineStatusChanged" => {
            Some(EventType::SourceOnlineStatusChanged)
        }
        "sync.started" | "SyncStarted" => Some(EventType::SyncStarted),
        "sync.progress" | "SyncProgress" => Some(EventType::SyncProgress),
        "sync.completed" | "SyncCompleted" => Some(EventType::SyncCompleted),
        "sync.failed" | "SyncFailed" => Some(EventType::SyncFailed),
        "playback.started" | "PlaybackStarted" => Some(EventType::PlaybackStarted),
        "playback.paused" | "PlaybackPaused" => Some(EventType::PlaybackPaused),
        "playback.resumed" | "PlaybackResumed" => Some(EventType::PlaybackResumed),
        "playback.stopped" | "PlaybackStopped" => Some(EventType::PlaybackStopped),
        "playback.position_updated" | "PlaybackPositionUpdated" => {
            Some(EventType::PlaybackPositionUpdated)
        }
        "playback.completed" | "PlaybackCompleted" => Some(EventType::PlaybackCompleted),
        "cache.invalidated" | "CacheInvalidated" => Some(EventType::CacheInvalidated),
        "cache.updated" | "CacheUpdated" => Some(EventType::CacheUpdated),
        "cache.cleared" | "CacheCleared" => Some(EventType::CacheCleared),
        "user.authenticated" | "UserAuthenticated" => Some(EventType::UserAuthenticated),
        "user.logged_out" | "UserLoggedOut" => Some(EventType::UserLoggedOut),
        "user.preferences_changed" | "UserPreferencesChanged" => {
            Some(EventType::UserPreferencesChanged)
        }
        "system.database_migrated" | "DatabaseMigrated" => Some(EventType::DatabaseMigrated),
        "system.task_started" | "BackgroundTaskStarted" => Some(EventType::BackgroundTaskStarted),
        "system.task_completed" | "BackgroundTaskCompleted" => {
            Some(EventType::BackgroundTaskCompleted)
        }
        "system.error" | "ErrorOccurred" => Some(EventType::ErrorOccurred),
        _ => None,
    }
}
