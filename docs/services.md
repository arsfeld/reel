# Relm4 Services Architecture Guide

## Overview

This document defines the service architecture patterns for Reel's Relm4 implementation, addressing the type-safety issues identified in `service-type-safety.md` while fully embracing Relm4's reactive component model.

## Core Principles

### 1. Stateless Services
Services should be pure functions without internal state. State belongs in Components, Workers, or the Database.

```rust
// ❌ BAD: Stateful service with Arc<Self>
pub struct DataService {
    cache: Arc<RwLock<LruCache<String, MediaItem>>>,
    db: Arc<DatabaseConnection>,
}

// ✅ GOOD: Stateless service functions
pub struct MediaService;

impl MediaService {
    pub async fn fetch_library(
        db: &DatabaseConnection,
        library_id: &LibraryId,
    ) -> Result<Vec<MediaItem>> {
        MediaRepository::find_by_library(db, library_id).await
    }
}
```

### 2. Type-Safe Identifiers
Never use raw strings for identifiers. Always use strongly-typed newtypes.

```rust
// ❌ BAD: String-based identifiers
pub async fn get_media(source_id: &str, library_id: &str) -> Result<Vec<MediaItem>>

// ✅ GOOD: Type-safe identifiers
pub async fn get_media(source_id: &SourceId, library_id: &LibraryId) -> Result<Vec<MediaItem>>
```

### 3. Worker-Based Background Tasks
Use Relm4 Workers for all background operations instead of raw Tokio tasks.

```rust
// ❌ BAD: Raw Tokio spawn
tokio::spawn(async move {
    sync_library(library_id).await;
});

// ✅ GOOD: Relm4 Worker
impl Worker for SyncWorker {
    type Input = SyncRequest;
    type Output = SyncUpdate;

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            SyncRequest::SyncLibrary(id) => {
                // Perform sync with progress updates
                sender.output(SyncUpdate::Progress(id, 0.5));
            }
        }
    }
}
```

## Service Structure

```
src/services/
├── core/                    # Stateless business logic
│   ├── media.rs            # Media operations
│   ├── auth.rs             # Authentication logic
│   ├── sync.rs             # Synchronization logic
│   └── playback.rs         # Playback operations
├── workers/                 # Background task workers
│   ├── sync_worker.rs      # Sync operations
│   ├── image_worker.rs     # Image loading
│   ├── search_worker.rs    # Search indexing
│   └── connection_worker.rs # Connection management
├── commands/                # Async command definitions
│   ├── media_commands.rs   # Media fetch commands
│   ├── auth_commands.rs    # Auth flow commands
│   └── sync_commands.rs    # Sync commands
├── brokers/                 # Inter-component messaging
│   ├── media_broker.rs     # Media updates
│   ├── sync_broker.rs      # Sync status
│   └── connection_broker.rs # Connection status
└── types/                   # Service type definitions
    ├── identifiers.rs       # Type-safe IDs
    ├── cache_keys.rs        # Type-safe cache keys
    └── requests.rs          # Request/response types
```

## Type-Safe Identifiers

### Definition
```rust
// services/types/identifiers.rs
use std::fmt;
use serde::{Serialize, Deserialize};

/// Macro for creating newtype ID wrappers
macro_rules! define_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(String);

        impl $name {
            pub fn new(id: impl Into<String>) -> Self {
                Self(id.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self(s.to_string())
            }
        }
    };
}

// Define all ID types
define_id!(SourceId);
define_id!(LibraryId);
define_id!(MediaItemId);
define_id!(ProviderId);
define_id!(BackendId);
define_id!(ShowId);
define_id!(EpisodeId);
```

### Usage
```rust
// In service functions
pub async fn get_library(
    db: &DatabaseConnection,
    source_id: &SourceId,
    library_id: &LibraryId,
) -> Result<Library> {
    LibraryRepository::find_by_ids(db, source_id, library_id).await
}

// In components
let library_id = LibraryId::new("library_123");
let source_id = SourceId::from("source_456");
```

## Type-Safe Cache Keys

### Definition
```rust
// services/types/cache_keys.rs
use super::identifiers::*;

#[derive(Debug, Clone, PartialEq)]
pub enum CacheKey {
    Libraries(SourceId),
    LibraryItems {
        source: SourceId,
        library: LibraryId
    },
    MediaItem {
        source: SourceId,
        library: LibraryId,
        item: MediaItemId,
    },
    HomeSections(SourceId),
    ShowEpisodes {
        source: SourceId,
        show: ShowId,
    },
    PlaybackProgress {
        user: String,
        item: MediaItemId,
    },
}

impl CacheKey {
    /// Convert to string representation for storage
    pub fn to_string(&self) -> String {
        match self {
            Self::Libraries(source) =>
                format!("source:{}:libraries", source),
            Self::LibraryItems { source, library } =>
                format!("source:{}:library:{}:items", source, library),
            Self::MediaItem { source, library, item } =>
                format!("source:{}:library:{}:item:{}", source, library, item),
            Self::HomeSections(source) =>
                format!("source:{}:home", source),
            Self::ShowEpisodes { source, show } =>
                format!("source:{}:show:{}:episodes", source, show),
            Self::PlaybackProgress { user, item } =>
                format!("user:{}:playback:{}", user, item),
        }
    }

    /// Parse from string representation
    pub fn parse(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split(':').collect();
        match parts.as_slice() {
            ["source", source, "libraries"] =>
                Ok(Self::Libraries(SourceId::new(source))),
            ["source", source, "library", library, "items"] =>
                Ok(Self::LibraryItems {
                    source: SourceId::new(source),
                    library: LibraryId::new(library),
                }),
            _ => Err(anyhow!("Invalid cache key format: {}", s))
        }
    }
}
```

### Usage
```rust
// Creating cache keys
let key = CacheKey::LibraryItems {
    source: source_id.clone(),
    library: library_id.clone(),
};

// Using in cache operations
cache.get(&key.to_string()).await?;
cache.set(key.to_string(), items).await?;
```

## Stateless Service Pattern

### Core Service Implementation
```rust
// services/core/media.rs
use crate::db::{DatabaseConnection, repository::MediaRepository};
use crate::services::types::identifiers::*;

pub struct MediaService;

impl MediaService {
    /// Fetch all items in a library
    pub async fn get_library_items(
        db: &DatabaseConnection,
        library_id: &LibraryId,
    ) -> Result<Vec<MediaItem>> {
        MediaRepository::find_by_library(db, library_id).await
    }

    /// Search for media items
    pub async fn search(
        db: &DatabaseConnection,
        query: &str,
        source_id: Option<&SourceId>,
    ) -> Result<Vec<MediaItem>> {
        MediaRepository::search(db, query, source_id).await
    }

    /// Get continue watching items
    pub async fn get_continue_watching(
        db: &DatabaseConnection,
        user_id: &str,
        limit: usize,
    ) -> Result<Vec<MediaItem>> {
        MediaRepository::find_in_progress(db, user_id, limit).await
    }

    /// Update playback progress
    pub async fn update_progress(
        db: &DatabaseConnection,
        item_id: &MediaItemId,
        progress: f32,
    ) -> Result<()> {
        PlaybackRepository::update_progress(db, item_id, progress).await
    }
}
```

### Integration with Components
```rust
// In a Relm4 component
#[relm4::component(async)]
impl AsyncComponent for LibraryPage {
    type CommandOutput = CommandMsg;

    async fn update_cmd(
        &mut self,
        msg: CommandMsg,
        sender: AsyncComponentSender<Self>
    ) {
        match msg {
            CommandMsg::LoadLibrary(library_id) => {
                // Use stateless service
                match MediaService::get_library_items(&self.db, &library_id).await {
                    Ok(items) => sender.input(LibraryMsg::ItemsLoaded(items)),
                    Err(e) => sender.input(LibraryMsg::LoadError(e.to_string())),
                }
            }
        }
    }
}
```

## Worker Pattern

### Background Task Worker
```rust
// services/workers/sync_worker.rs
use crate::backends::traits::MediaBackend;
use crate::services::types::identifiers::*;

pub struct SyncWorker {
    db: DatabaseConnection,
    backends: HashMap<BackendId, Arc<dyn MediaBackend>>,
}

#[derive(Debug)]
pub enum SyncRequest {
    SyncLibrary {
        backend_id: BackendId,
        library_id: LibraryId,
    },
    SyncAll {
        backend_id: BackendId,
    },
    Cancel,
}

#[derive(Debug)]
pub enum SyncUpdate {
    Started(LibraryId),
    Progress {
        library_id: LibraryId,
        current: usize,
        total: usize,
    },
    Completed {
        library_id: LibraryId,
        items_synced: usize,
    },
    Failed {
        library_id: LibraryId,
        error: String,
    },
}

impl Worker for SyncWorker {
    type Init = (DatabaseConnection, HashMap<BackendId, Arc<dyn MediaBackend>>);
    type Input = SyncRequest;
    type Output = SyncUpdate;

    fn init(
        (db, backends): Self::Init,
        _sender: ComponentSender<Self>
    ) -> Self {
        Self { db, backends }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            SyncRequest::SyncLibrary { backend_id, library_id } => {
                sender.output(SyncUpdate::Started(library_id.clone()));

                if let Some(backend) = self.backends.get(&backend_id) {
                    // Perform sync
                    match backend.get_library_items(&library_id) {
                        Ok(items) => {
                            let total = items.len();
                            for (i, item) in items.into_iter().enumerate() {
                                // Store in database
                                if let Err(e) = MediaRepository::upsert(&self.db, item) {
                                    sender.output(SyncUpdate::Failed {
                                        library_id: library_id.clone(),
                                        error: e.to_string(),
                                    });
                                    return;
                                }

                                // Send progress
                                if i % 10 == 0 {
                                    sender.output(SyncUpdate::Progress {
                                        library_id: library_id.clone(),
                                        current: i,
                                        total,
                                    });
                                }
                            }

                            sender.output(SyncUpdate::Completed {
                                library_id,
                                items_synced: total,
                            });
                        }
                        Err(e) => {
                            sender.output(SyncUpdate::Failed {
                                library_id,
                                error: e.to_string(),
                            });
                        }
                    }
                }
            }
            SyncRequest::Cancel => {
                // Handle cancellation
            }
            _ => {}
        }
    }
}
```

### Image Loading Worker
```rust
// services/workers/image_worker.rs
pub struct ImageWorker {
    cache: LruCache<String, GdkPixbuf>,
    loader: ImageLoader,
}

#[derive(Debug)]
pub struct ImageRequest {
    pub id: MediaItemId,
    pub url: String,
    pub size: ImageSize,
}

#[derive(Debug)]
pub enum ImageResult {
    Loaded {
        id: MediaItemId,
        pixbuf: GdkPixbuf,
    },
    Failed {
        id: MediaItemId,
        error: String,
    },
}

impl Worker for ImageWorker {
    type Init = ();
    type Input = ImageRequest;
    type Output = ImageResult;

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        // Check cache first
        if let Some(cached) = self.cache.get(&msg.url) {
            sender.output(ImageResult::Loaded {
                id: msg.id,
                pixbuf: cached.clone(),
            });
            return;
        }

        // Load image
        match self.loader.load_from_url(&msg.url, msg.size) {
            Ok(pixbuf) => {
                self.cache.put(msg.url, pixbuf.clone());
                sender.output(ImageResult::Loaded {
                    id: msg.id,
                    pixbuf,
                });
            }
            Err(e) => {
                sender.output(ImageResult::Failed {
                    id: msg.id,
                    error: e.to_string(),
                });
            }
        }
    }
}
```

## Command Pattern

### Async Commands
```rust
// services/commands/media_commands.rs
use crate::services::core::MediaService;
use crate::services::types::identifiers::*;

pub enum MediaCommand {
    FetchLibrary(LibraryId),
    FetchDetails(MediaItemId),
    Search { query: String, source: Option<SourceId> },
    UpdateProgress { item: MediaItemId, position: f32 },
}

pub async fn execute_media_command(
    db: &DatabaseConnection,
    command: MediaCommand,
) -> Result<MediaCommandResult> {
    match command {
        MediaCommand::FetchLibrary(library_id) => {
            let items = MediaService::get_library_items(db, &library_id).await?;
            Ok(MediaCommandResult::LibraryItems(items))
        }
        MediaCommand::FetchDetails(item_id) => {
            let details = MediaService::get_item_details(db, &item_id).await?;
            Ok(MediaCommandResult::ItemDetails(details))
        }
        MediaCommand::Search { query, source } => {
            let results = MediaService::search(db, &query, source.as_ref()).await?;
            Ok(MediaCommandResult::SearchResults(results))
        }
        MediaCommand::UpdateProgress { item, position } => {
            MediaService::update_progress(db, &item, position).await?;
            Ok(MediaCommandResult::ProgressUpdated)
        }
    }
}

pub enum MediaCommandResult {
    LibraryItems(Vec<MediaItem>),
    ItemDetails(MediaDetails),
    SearchResults(Vec<MediaItem>),
    ProgressUpdated,
}
```

### Component Integration
```rust
#[relm4::component(async)]
impl AsyncComponent for HomePage {
    type CommandOutput = CommandMsg;

    async fn init(
        _: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        // Send initial load command
        sender.oneshot_command(async move {
            CommandMsg::LoadHome
        });

        // ... rest of init
    }

    async fn update_cmd(
        &mut self,
        msg: CommandMsg,
        sender: AsyncComponentSender<Self>,
    ) {
        match msg {
            CommandMsg::LoadHome => {
                // Execute multiple commands concurrently
                let (continue_watching, recently_added) = tokio::join!(
                    MediaService::get_continue_watching(&self.db, "user", 10),
                    MediaService::get_recently_added(&self.db, 20)
                );

                if let Ok(items) = continue_watching {
                    sender.input(HomeMsg::ContinueWatchingLoaded(items));
                }

                if let Ok(items) = recently_added {
                    sender.input(HomeMsg::RecentlyAddedLoaded(items));
                }
            }
        }
    }
}
```

## MessageBroker Pattern

### Broker Definition
```rust
// services/brokers/media_broker.rs
use relm4::MessageBroker;
use crate::services::types::identifiers::*;

/// Broker for media-related updates across components
#[derive(Debug, Clone)]
pub struct MediaUpdateBroker;

#[derive(Debug, Clone)]
pub enum MediaUpdate {
    LibraryRefreshed(LibraryId),
    ItemWatched(MediaItemId),
    ProgressChanged {
        item: MediaItemId,
        progress: f32,
    },
    ItemAdded(MediaItem),
    ItemRemoved(MediaItemId),
}

impl MessageBroker for MediaUpdateBroker {
    type Message = MediaUpdate;
}

// Convenience functions
impl MediaUpdateBroker {
    pub fn notify_library_refresh(library_id: LibraryId) {
        Self::send(MediaUpdate::LibraryRefreshed(library_id));
    }

    pub fn notify_progress(item: MediaItemId, progress: f32) {
        Self::send(MediaUpdate::ProgressChanged { item, progress });
    }
}
```

### Component Subscription
```rust
impl Component for LibraryPage {
    fn init(
        _: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        // Subscribe to media updates
        let mut media_rx = MediaUpdateBroker::subscribe();

        sender.spawn_oneshot_command(async move {
            while let Some(update) = media_rx.recv().await {
                match update {
                    MediaUpdate::LibraryRefreshed(lib_id) => {
                        // Refresh if this is our library
                        if lib_id == self.library_id {
                            sender.input(LibraryMsg::Refresh);
                        }
                    }
                    MediaUpdate::ItemWatched(item_id) => {
                        // Update item status
                        sender.input(LibraryMsg::UpdateItemStatus(item_id));
                    }
                    _ => {}
                }
            }
        });

        // ... rest of init
    }
}
```

## Connection Management

### Auth → Source → Backend Flow
```rust
// services/workers/connection_worker.rs
use crate::auth::AuthProvider;
use crate::sources::Source;
use crate::backends::traits::Backend;

pub struct ConnectionWorker {
    db: DatabaseConnection,
    connections: HashMap<SourceId, ConnectionState>,
}

pub struct ConnectionState {
    source: Source,
    auth_provider: AuthProvider,
    backend: Box<dyn Backend>,
    status: ConnectionStatus,
}

#[derive(Debug)]
pub enum ConnectionRequest {
    Connect {
        source_id: SourceId,
        auth_provider: AuthProvider,
    },
    Disconnect(SourceId),
    RefreshAuth(SourceId),
    TestConnection(SourceId),
}

#[derive(Debug)]
pub enum ConnectionUpdate {
    Connected(SourceId),
    Disconnected(SourceId),
    AuthFailed(SourceId, String),
    ConnectionFailed(SourceId, String),
}

impl Worker for ConnectionWorker {
    type Input = ConnectionRequest;
    type Output = ConnectionUpdate;

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            ConnectionRequest::Connect { source_id, auth_provider } => {
                // Get source from database
                let source = SourceRepository::find(&self.db, &source_id).unwrap();

                // Create backend based on source type
                let mut backend: Box<dyn Backend> = match source.backend_type {
                    BackendType::Plex => Box::new(PlexBackend::new(source.clone())),
                    BackendType::Jellyfin => Box::new(JellyfinBackend::new(source.clone())),
                    _ => return,
                };

                // Authenticate backend with provider
                match backend.authenticate(&auth_provider) {
                    Ok(()) => {
                        self.connections.insert(
                            source_id.clone(),
                            ConnectionState {
                                source,
                                auth_provider,
                                backend,
                                status: ConnectionStatus::Connected,
                            },
                        );
                        sender.output(ConnectionUpdate::Connected(source_id));
                    }
                    Err(e) => {
                        sender.output(ConnectionUpdate::AuthFailed(
                            source_id,
                            e.to_string(),
                        ));
                    }
                }
            }
            _ => {}
        }
    }
}
```

## Testing Patterns

### Service Testing
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_utils::create_test_db;

    #[tokio::test]
    async fn test_media_service_get_library() {
        let db = create_test_db().await;
        let library_id = LibraryId::new("test_library");

        // Insert test data
        MediaRepository::insert(&db, create_test_media()).await.unwrap();

        // Test service function
        let items = MediaService::get_library_items(&db, &library_id).await.unwrap();
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn test_cache_key_roundtrip() {
        let original = CacheKey::LibraryItems {
            source: SourceId::new("source1"),
            library: LibraryId::new("lib1"),
        };

        let string = original.to_string();
        let parsed = CacheKey::parse(&string).unwrap();

        assert_eq!(original, parsed);
    }
}
```

### Worker Testing
```rust
#[cfg(test)]
mod tests {
    use relm4::ComponentTest;

    #[tokio::test]
    async fn test_sync_worker() {
        let worker = SyncWorker::builder()
            .launch_test((test_db(), test_backends()))
            .await;

        // Send sync request
        worker.send(SyncRequest::SyncLibrary {
            backend_id: BackendId::new("test"),
            library_id: LibraryId::new("lib1"),
        });

        // Verify output
        let output = worker.recv().await;
        assert!(matches!(output, SyncUpdate::Started(_)));
    }
}
```

## Best Practices

### 1. Always Use Type-Safe IDs
```rust
// ❌ BAD
pub fn get_item(id: &str) -> Result<MediaItem>

// ✅ GOOD
pub fn get_item(id: &MediaItemId) -> Result<MediaItem>
```

### 2. Stateless Services
```rust
// ❌ BAD
impl DataService {
    fn new(cache: Arc<Cache>) -> Self { ... }
    async fn get(&self, key: &str) -> Result<Value>
}

// ✅ GOOD
impl MediaService {
    async fn get(db: &DatabaseConnection, id: &MediaItemId) -> Result<MediaItem>
}
```

### 3. Worker for Background Tasks
```rust
// ❌ BAD
tokio::spawn(async move {
    sync_library().await;
});

// ✅ GOOD
sync_worker.emit(SyncRequest::SyncLibrary(library_id));
```

### 4. Commands for Async Operations
```rust
// ❌ BAD
self.data_service.fetch_async().await;

// ✅ GOOD
sender.oneshot_command(async move {
    CommandMsg::FetchData
});
```

### 5. MessageBroker for Cross-Component Communication
```rust
// ❌ BAD
self.event_bus.emit(Event::LibraryUpdated);

// ✅ GOOD
MediaUpdateBroker::send(MediaUpdate::LibraryRefreshed(library_id));
```

## Migration Checklist

When migrating existing services to Relm4 patterns:

- [ ] Replace string IDs with typed identifiers
- [ ] Convert stateful services to stateless functions
- [ ] Move background tasks to Workers
- [ ] Replace EventBus with MessageBroker
- [ ] Use Commands for async operations
- [ ] Add proper error types instead of anyhow::Result
- [ ] Write tests for all service functions
- [ ] Update documentation

## Summary

This architecture provides:

1. **Type Safety**: Strongly-typed identifiers prevent runtime errors
2. **Stateless Services**: Easier testing and reasoning about code
3. **Relm4 Integration**: Native use of Workers, Commands, and MessageBroker
4. **Clear Separation**: Business logic separate from UI concerns
5. **Testability**: Pure functions and isolated workers are easy to test
6. **Scalability**: Easy to add new services without affecting existing ones