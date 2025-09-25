# Relm4 Services Architecture Guide

## Overview

This document defines the service architecture patterns for Reel's Relm4 implementation, providing a type-safe, stateless service layer that integrates seamlessly with Relm4's reactive component model.

## Core Principles

### 1. Stateless Services
Services are implemented as structs with pure static functions. No internal state, no Arc<Self>. State belongs in Components, Workers, or the Database.

```rust
// ✅ IMPLEMENTED: Stateless service pattern (see src/services/core/media.rs)
pub struct MediaService;

impl MediaService {
    pub async fn get_libraries(db: &DatabaseConnection) -> Result<Vec<Library>> {
        let repo = LibraryRepositoryImpl::new(db.clone());
        let models = repo.find_all().await?;
        // Convert and return
    }
}
```

### 2. Type-Safe Identifiers
All identifiers use strongly-typed newtypes defined in `src/models/identifiers.rs` using the `impl_id_type!` macro.

```rust
// ✅ IMPLEMENTED: Type-safe identifiers (see src/models/identifiers.rs)
impl_id_type!(SourceId);
impl_id_type!(LibraryId);
impl_id_type!(MediaItemId);
impl_id_type!(ShowId);
impl_id_type!(UserId);
impl_id_type!(BackendId);
impl_id_type!(ProviderId);

// Usage in services
pub async fn get_media_item(db: &DatabaseConnection, item_id: &MediaItemId) -> Result<Option<MediaItem>>
```

### 3. Worker-Based Background Tasks
Use Relm4 Workers for all background operations. Workers handle sync, search, image loading, and connection monitoring.

```rust
// ✅ IMPLEMENTED: Worker pattern (see src/workers/sync_worker.rs)
impl Worker for SyncWorker {
    type Init = Arc<DatabaseConnection>;
    type Input = SyncWorkerInput;
    type Output = SyncWorkerOutput;

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            SyncWorkerInput::StartSync { source_id, library_id, force } => {
                sender.output(SyncWorkerOutput::SyncStarted { source_id, library_id });
                // Perform sync...
            }
        }
    }
}
```

## Actual Service Structure

```
src/services/
├── core/                     # Stateless business logic services
│   ├── auth.rs              # AuthService
│   ├── backend.rs           # BackendService - manages backend instances
│   ├── connection.rs        # ConnectionService - source connections
│   ├── connection_cache.rs  # Connection caching utilities
│   ├── media.rs             # MediaService - media operations
│   ├── playback.rs          # PlaybackService
│   ├── playlist.rs          # PlaylistService
│   ├── playqueue.rs         # PlayQueueService
│   └── sync.rs              # SyncService
├── commands/                 # Command pattern implementations
│   ├── auth_commands.rs     # Authentication commands
│   ├── media_commands.rs    # Media fetch commands (GetLibrariesCommand, etc.)
│   └── sync_commands.rs     # Sync commands
├── brokers/                  # Message brokers (currently stub implementations)
│   ├── connection_broker.rs # Connection status messages
│   ├── media_broker.rs      # Media update messages (uses logging, not full broker)
│   └── sync_broker.rs       # Sync status messages
├── cache_keys.rs            # Type-safe cache key system
└── initialization.rs        # Service initialization logic

src/workers/                  # Background workers (separate module)
├── connection_monitor.rs    # Connection health monitoring
├── image_loader.rs          # Async image loading
├── search_worker.rs         # Search operations
└── sync_worker.rs           # Synchronization worker

src/models/identifiers.rs    # Type-safe ID definitions (not in services/)
```

## Type-Safe Identifiers

### Implementation
```rust
// src/models/identifiers.rs
macro_rules! impl_id_type {
    ($name:ident) => {
        #[derive(Clone, Debug, Serialize, Deserialize)]
        pub struct $name(String);

        impl $name {
            pub fn new(id: impl Into<String>) -> Self {
                Self(id.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                self.0 == other.0
            }
        }

        impl Eq for $name {}

        impl Hash for $name {
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.0.hash(state);
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

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }
    };
}

// All ID types are defined in src/models/identifiers.rs
impl_id_type!(SourceId);
impl_id_type!(BackendId);
impl_id_type!(ProviderId);
impl_id_type!(LibraryId);
impl_id_type!(MediaItemId);
impl_id_type!(ShowId);
impl_id_type!(UserId);
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

### Implementation
```rust
// src/services/cache_keys.rs
use crate::models::{LibraryId, MediaItemId, ShowId, SourceId};
use crate::db::entities::media_items::MediaType;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CacheKey {
    /// Simple cache key for any media item by ID (memory cache)
    Media(String),

    /// Cache key for list of libraries for a source
    Libraries(SourceId),

    /// Cache key for list of items in a library
    LibraryItems(SourceId, LibraryId),

    /// Cache key for a specific media item
    MediaItem {
        source: SourceId,
        library: LibraryId,
        media_type: MediaType,
        item_id: MediaItemId,
    },

    /// Cache key for home sections of a source
    HomeSections(SourceId),

    /// Cache key for episodes of a show
    ShowEpisodes(SourceId, LibraryId, ShowId),

    /// Cache key for a specific episode
    Episode(SourceId, LibraryId, MediaItemId),

    /// Cache key for a show
    Show(SourceId, LibraryId, ShowId),

    /// Cache key for a movie
    Movie(SourceId, LibraryId, MediaItemId),
}

impl CacheKey {
    /// Convert the cache key to its string representation
    pub fn to_string(&self) -> String {
        match self {
            CacheKey::Media(id) => format!("media:{}", id),
            CacheKey::Libraries(source) => 
                format!("{}:libraries", source.as_str()),
            CacheKey::LibraryItems(source, library) => 
                format!("{}:library:{}:items", source.as_str(), library.as_str()),
            CacheKey::MediaItem { source, library, media_type, item_id } => {
                let type_str = match media_type {
                    MediaType::Movie => "movie",
                    MediaType::Show => "show",
                    MediaType::Episode => "episode",
                    MediaType::Album => "album",
                    MediaType::Track => "track",
                    MediaType::Photo => "photo",
                };
                format!("{}:{}:{}:{}", source.as_str(), library.as_str(), type_str, item_id.as_str())
            }
            CacheKey::HomeSections(source) => 
                format!("{}:home_sections", source.as_str()),
            CacheKey::ShowEpisodes(source, library, show) => 
                format!("{}:{}:show:{}:episodes", source.as_str(), library.as_str(), show.as_str()),
            // ... other variants
        }
    }
}
```

### Usage
```rust
// Creating cache keys
let key = CacheKey::LibraryItems(source_id, library_id);

// Using in cache operations
cache.get(&key.to_string()).await?;
cache.set(key.to_string(), items).await?;
```

## Stateless Service Pattern

### Core Service Implementation
```rust
// src/services/core/media.rs
pub struct MediaService;

impl MediaService {
    /// Get all libraries for a specific source
    pub async fn get_libraries_for_source(
        db: &DatabaseConnection,
        source_id: &SourceId,
    ) -> Result<Vec<Library>> {
        let repo = LibraryRepositoryImpl::new(db.clone());
        let models = repo
            .find_by_source(source_id.as_ref())
            .await
            .context("Failed to get libraries from database")?;

        models
            .into_iter()
            .map(|m| m.try_into())
            .collect::<Result<Vec<Library>, _>>()
            .context("Failed to convert library models")
    }

    /// Get media items with pagination
    pub async fn get_media_items(
        db: &DatabaseConnection,
        library_id: &LibraryId,
        media_type: Option<MediaType>,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<MediaItem>> {
        let repo = MediaRepositoryImpl::new(db.clone());
        let models = repo
            .find_by_library_paginated(library_id.as_ref(), offset, limit)
            .await?;

        models
            .into_iter()
            .map(|m| m.try_into())
            .collect::<Result<Vec<MediaItem>>>()
    }

    /// Get continue watching items
    pub async fn get_continue_watching(
        db: &DatabaseConnection,
        limit: Option<i64>,
    ) -> Result<Vec<MediaItem>> {
        let repo = MediaRepositoryImpl::new(db.clone());
        let models = repo.find_continue_watching(limit).await?;

        models
            .into_iter()
            .map(|m| m.try_into())
            .collect::<Result<Vec<MediaItem>>>()
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

### Sync Worker Implementation
```rust
// src/workers/sync_worker.rs
pub struct SyncWorker {
    db: Arc<DatabaseConnection>,
    active_syncs: HashMap<SourceId, relm4::JoinHandle<()>>,
    sync_interval: Duration,
    auto_sync_enabled: bool,
    last_sync_times: HashMap<SourceId, Instant>,
}

#[derive(Debug, Clone)]
pub enum SyncWorkerInput {
    StartSync {
        source_id: SourceId,
        library_id: Option<LibraryId>,
        force: bool,
    },
    StopSync { source_id: SourceId },
    StopAllSyncs,
    SetSyncInterval(Duration),
    EnableAutoSync(bool),
    RecordSuccessfulSync { source_id: SourceId },
}

#[derive(Debug, Clone)]
pub enum SyncWorkerOutput {
    SyncStarted {
        source_id: SourceId,
        library_id: Option<LibraryId>,
    },
    SyncProgress(SyncProgress),
    SyncCompleted {
        source_id: SourceId,
        library_id: Option<LibraryId>,
        items_synced: usize,
        duration: Duration,
    },
    SyncFailed {
        source_id: SourceId,
        library_id: Option<LibraryId>,
        error: String,
    },
    SyncCancelled { source_id: SourceId },
}

impl Worker for SyncWorker {
    type Init = Arc<DatabaseConnection>;
    type Input = SyncWorkerInput;
    type Output = SyncWorkerOutput;

    fn init(db: Self::Init, _sender: ComponentSender<Self>) -> Self {
        Self {
            db,
            active_syncs: HashMap::new(),
            sync_interval: Duration::from_secs(3600),
            auto_sync_enabled: true,
            last_sync_times: HashMap::new(),
        }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            SyncWorkerInput::StartSync { source_id, library_id, force } => {
                // Check if sync already running
                if self.active_syncs.contains_key(&source_id) && !force {
                    return;
                }

                // Cancel existing sync if forcing
                if force {
                    if let Some(handle) = self.active_syncs.remove(&source_id) {
                        handle.abort();
                    }
                }

                // Start new sync
                let db = self.db.clone();
                let source_id_clone = source_id.clone();
                let library_id_clone = library_id.clone();
                let sender_clone = sender.clone();

                let handle = relm4::spawn(async move {
                    // Perform sync operation
                    let result = BackendService::sync_source(
                        &db,
                        &source_id_clone,
                        library_id_clone.as_ref(),
                    ).await;
                    
                    // Send completion message
                    match result {
                        Ok(count) => {
                            sender_clone.output(SyncWorkerOutput::SyncCompleted {
                                source_id: source_id_clone,
                                library_id: library_id_clone,
                                items_synced: count,
                                duration: Duration::from_secs(0), // TODO: track actual duration
                            });
                        }
                        Err(e) => {
                            sender_clone.output(SyncWorkerOutput::SyncFailed {
                                source_id: source_id_clone,
                                library_id: library_id_clone,
                                error: e.to_string(),
                            });
                        }
                    }
                });

                self.active_syncs.insert(source_id.clone(), handle);
                sender.output(SyncWorkerOutput::SyncStarted { source_id, library_id });
            }
            // ... other message handlers
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

### Command Implementation
```rust
// src/services/commands/media_commands.rs
use async_trait::async_trait;
use crate::services::commands::Command;

/// Base Command trait
#[async_trait]
pub trait Command<T> {
    async fn execute(&self) -> Result<T>;
}

/// Get all libraries command
pub struct GetLibrariesCommand {
    pub db: DatabaseConnection,
}

#[async_trait]
impl Command<Vec<Library>> for GetLibrariesCommand {
    async fn execute(&self) -> Result<Vec<Library>> {
        MediaService::get_libraries(&self.db).await
    }
}

/// Get media items for a library with pagination
pub struct GetMediaItemsCommand {
    pub db: DatabaseConnection,
    pub library_id: LibraryId,
    pub media_type: Option<MediaType>,
    pub offset: u32,
    pub limit: u32,
}

#[async_trait]
impl Command<Vec<MediaItem>> for GetMediaItemsCommand {
    async fn execute(&self) -> Result<Vec<MediaItem>> {
        MediaService::get_media_items(
            &self.db,
            &self.library_id,
            self.media_type,
            self.offset,
            self.limit,
        )
        .await
    }
}

/// Search media items
pub struct SearchMediaCommand {
    pub db: DatabaseConnection,
    pub query: String,
    pub source_id: Option<SourceId>,
    pub media_type: Option<MediaType>,
    pub limit: Option<u32>,
}

#[async_trait]
impl Command<Vec<MediaItem>> for SearchMediaCommand {
    async fn execute(&self) -> Result<Vec<MediaItem>> {
        MediaService::search(
            &self.db,
            &self.query,
            self.source_id.as_ref(),
            self.media_type,
            self.limit,
        )
        .await
    }
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

## Message Broker Pattern (Current Implementation)

### Broker Message Definitions
```rust
// src/services/brokers/media_broker.rs
// NOTE: Currently using logging functions instead of full MessageBroker implementation
// Components create their own Relm4 Sender/Receiver channels as needed

#[derive(Debug, Clone)]
pub enum MediaMessage {
    /// Library added or updated
    LibraryUpdated {
        source_id: SourceId,
        library_id: LibraryId,
        item_count: usize,
    },
    /// Media item added or updated
    ItemUpdated {
        source_id: SourceId,
        library_id: LibraryId,
        item: MediaItem,
    },
    /// Multiple items updated (bulk operation)
    ItemsBulkUpdated {
        source_id: SourceId,
        library_id: LibraryId,
        count: usize,
    },
    /// Item removed
    ItemRemoved {
        source_id: SourceId,
        library_id: LibraryId,
        item_id: MediaItemId,
    },
}

/// Current implementation uses logging functions
pub fn log_library_updated(source_id: SourceId, library_id: LibraryId, item_count: usize) {
    debug!(
        "Library updated: source={}, library={}, items={}",
        source_id, library_id, item_count
    );
}

pub fn log_item_updated(source_id: SourceId, library_id: LibraryId, item: &MediaItem) {
    debug!(
        "Item updated: source={}, library={}, item={}",
        source_id,
        library_id,
        item.id()
    );
}
```

### Component Communication Pattern
```rust
// Components currently use direct Relm4 channels
impl AsyncComponent for LibraryPage {
    type CommandOutput = LibraryCommand;
    
    async fn update_cmd(
        &mut self,
        msg: LibraryCommand,
        sender: AsyncComponentSender<Self>,
    ) {
        match msg {
            LibraryCommand::RefreshLibrary => {
                // Fetch updated data
                let items = MediaService::get_media_items(
                    &self.db,
                    &self.library_id,
                    None,
                    0,
                    100
                ).await.unwrap();
                
                // Send to component
                sender.input(LibraryMsg::ItemsLoaded(items));
            }
        }
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
sync_worker.emit(SyncWorkerInput::StartSync { 
    source_id, 
    library_id, 
    force: false 
});
```

### 4. Commands for Async Operations
```rust
// ✅ GOOD: Command pattern with trait
let cmd = GetMediaItemsCommand {
    db: self.db.clone(),
    library_id: library_id.clone(),
    media_type: None,
    offset: 0,
    limit: 100,
};
let items = cmd.execute().await?;
```

### 5. Direct Component Communication
```rust
// Current pattern: Components use Relm4 channels directly
// MessageBroker infrastructure exists but uses logging currently
sender.input(LibraryMsg::ItemsLoaded(items));
```

## Current Architecture Status

### Implemented ✅
- Type-safe identifiers via `impl_id_type!` macro
- Stateless service pattern with pure functions
- Worker-based background tasks (sync, search, image loading)
- Command pattern with async trait
- Cache key system with type safety
- Repository pattern for database operations

### Partially Implemented ⚠️
- Message brokers defined but using logging instead of full broker
- Connection monitoring worker exists but not fully integrated
- Some services still need complete conversions

### Known Gaps ❌
- Repository layer has zero event integration
- Transaction support exists but not integrated into sync flow
- Some UI pages still need full Relm4 component integration

## Migration Guidelines

When working with services:

1. **Use existing patterns**: Follow the stateless service pattern in `src/services/core/`
2. **Type-safe IDs**: Always use the identifier types from `src/models/identifiers.rs`
3. **Workers for background**: Put long-running tasks in `src/workers/`
4. **Commands for async**: Define commands in `src/services/commands/`
5. **Repository for DB**: Use repository pattern in `src/db/repository/`

## Summary

The services architecture provides:

1. **Type Safety**: Strongly-typed identifiers throughout
2. **Stateless Services**: Pure functions for easier testing
3. **Worker Integration**: Background tasks via Relm4 Workers
4. **Command Pattern**: Structured async operations
5. **Clear Separation**: Business logic separate from UI
6. **Repository Pattern**: Consistent database access