# Reel Architecture Documentation

**Version:** 1.0.0  
**Last Updated:** 2025-08-28  
**Migration Status:** 75% Complete - SeaORM Migration In Progress

## Overview

Reel is a modern GTK4/libadwaita media player application for GNOME, written in Rust. The application implements an innovative offline-first architecture with reactive UI updates through an event-driven system, currently undergoing a comprehensive migration from a basic cache system to a production-ready SeaORM/SQLite solution with reactive ViewModels.

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Application Layer                       │
│  ┌─────────────────┐    ┌──────────────────────────────────┐ │
│  │   GTK4/Adwaita  │    │        Main Window              │ │
│  │   UI Framework  │    │  ┌────────────┬────────────────┐ │ │
│  └─────────────────┘    │  │  Sidebar   │   Page Views   │ │ │
│                         │  │            │                │ │ │
│                         │  └────────────┴────────────────┘ │ │
│                         └──────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────┐
│                  Presentation Layer                         │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────────────┐ │
│  │  ViewModels  │ │  Properties  │ │   Event Subscribers  │ │
│  │              │ │              │ │                      │ │
│  │ • Library    │ │ • Reactive   │ │ • UI Change Events   │ │
│  │ • Player     │ │ • Observable │ │ • Data Invalidation  │ │
│  │ • Sources    │ │ • Bindable   │ │ • Sync Progress      │ │
│  │ • Sidebar    │ │              │ │                      │ │
│  └──────────────┘ └──────────────┘ └──────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────┐
│                   Service Layer                             │
│  ┌─────────────┐ ┌──────────────┐ ┌───────────────────────┐ │
│  │ DataService │ │ SyncManager  │ │   SourceCoordinator   │ │
│  │             │ │              │ │                       │ │
│  │ • CRUD Ops  │ │ • Background │ │ • Backend Management  │ │
│  │ • Caching   │ │   Sync       │ │ • Auth Coordination   │ │
│  │ • Events    │ │ • Progress   │ │ • Multi-source Ops    │ │
│  └─────────────┘ └──────────────┘ └───────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────┐
│                    Event System                             │
│  ┌─────────────────┐              ┌────────────────────────┐ │
│  │    EventBus     │◄────────────►│    Event Types         │ │
│  │                 │              │                        │ │
│  │ • Publish/Sub   │              │ • Media Events         │ │
│  │ • Filtering     │              │ • Sync Events          │ │
│  │ • History       │              │ • Library Events       │ │
│  │ • Broadcasting  │              │ • Playback Events      │ │
│  └─────────────────┘              └────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────┐
│                   Data Access Layer                         │
│  ┌─────────────┐ ┌──────────────┐ ┌───────────────────────┐ │
│  │ Repositories│ │  SeaORM      │ │     Memory Cache      │ │
│  │             │ │  Entities    │ │                       │ │
│  │ • Media     │ │              │ │ • LRU Cache          │ │
│  │ • Library   │ │ • Type-safe  │ │ • Write-through      │ │
│  │ • Source    │ │ • Relations  │ │ • Thread-safe        │ │
│  │ • Playback  │ │ • Migrations │ │                       │ │
│  └─────────────┘ └──────────────┘ └───────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────┐
│                  Backend Integration Layer                  │
│  ┌─────────────────┐ ┌────────────────┐ ┌──────────────────┐ │
│  │   Plex Backend  │ │ Jellyfin       │ │  Local Files     │ │
│  │                 │ │ Backend        │ │  Backend         │ │
│  │ • Auth & API    │ │                │ │                  │ │
│  │ • Media Fetch   │ │ • Auth & API   │ │ • File Scanning  │ │
│  │ • Streaming     │ │ • Media Fetch  │ │ • Metadata       │ │
│  └─────────────────┘ │ • Streaming    │ │   Extraction     │ │
│                       └────────────────┘ └──────────────────┘ │
└─────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────┐
│                    Storage Layer                            │
│  ┌─────────────────┐              ┌────────────────────────┐ │
│  │ SQLite Database │              │    Media Storage       │ │
│  │                 │              │                        │ │
│  │ • SeaORM Schema │              │ • Image Cache          │ │
│  │ • Migrations    │              │ • Offline Content      │ │
│  │ • Transactions  │              │ • GStreamer Pipeline   │ │
│  │ • Full-text     │              │                        │ │
│  │   Search (FTS5) │              │                        │ │
│  └─────────────────┘              └────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## Current Migration Status: SeaORM Integration (75% Complete)

### ✅ Production-Ready Components

#### Database Infrastructure (100% Complete)
- **SeaORM Integration**: Full implementation with SQLite backend
- **Migration System**: Automated schema management with rollback support
- **Connection Pooling**: Production-ready connection management
- **Entity Definitions**: Complete type-safe entity layer with relations
- **Foreign Key Constraints**: Proper referential integrity with CASCADE deletes

#### Repository Layer (95% Complete) 
- **Repository Pattern**: Clean separation of data access and business logic
- **Type-Safe Queries**: Full SeaORM query builder implementation
- **CRUD Operations**: Complete Create, Read, Update, Delete operations
- **Advanced Queries**: Search, filtering, sorting, and bulk operations
- **Memory Cache**: Production-ready LRU cache with write-through pattern

#### Event System (65% Complete - Major Breakthrough)
- **EventBus Infrastructure**: Tokio broadcast-based event system
- **Core Event Types**: 12 of 27 event types fully implemented
- **SidebarViewModel Integration**: ✅ **FULLY REACTIVE** - Events properly reload data
- **End-to-End Reactivity**: Database changes → Events → UI updates working
- **Event Filtering**: Advanced subscription filtering by type, source, priority

### 🟡 Partially Complete Components

#### ViewModels & UI Integration (20% Complete)
- **ViewModel Infrastructure**: Property system with reactive change notifications
- **LibraryView**: ✅ **FULLY INTEGRATED** - Complete ViewModel integration with DB conversion
- **SidebarViewModel**: ✅ **FULLY REACTIVE** - Event-driven data reloading
- **Remaining Pages**: 4 of 6 pages still need ViewModel integration
- **PropertySubscriber**: Currently using panic! workaround for Clone implementation

#### Service Layer (80% Complete)
- **DataService**: Renamed from CacheManager, uses repository pattern
- **Event Emission**: Service layer properly emits events on CRUD operations
- **Transaction Support**: Methods exist but not integrated into sync flow
- **SyncManager**: Uses DataService but transactions not fully implemented

### 🔴 Critical Issues to Address

#### Main Window Status System Conflicts
- **Problem**: Hybrid status update system creates race conditions
- **Details**: SidebarViewModel has reactive properties, but old code directly manipulates UI
- **Impact**: Prevents fully consistent reactive status updates
- **Methods Needing Refactor**: `update_user_display()`, `show_sync_progress()`, auth flows

#### Repository Event Integration Gap
- **Problem**: Repository layer has ZERO event integration
- **Impact**: Direct repository calls bypass the entire event system
- **Solution**: Emit events from repository operations, not just service layer

## Core Architectural Patterns

### 1. Reactive Architecture Pattern

The application implements a reactive architecture where data flows unidirectionally:

```
Database → Repository → Service → Event → ViewModel → UI
```

**Key Components:**
- **Properties**: Observable data containers with change notifications
- **EventBus**: Central event broadcasting system
- **ViewModels**: Reactive data containers that automatically update UI
- **Event Subscribers**: Components that react to data changes

### 2. Repository Pattern

Each data entity has its own repository implementing a common interface:

```rust
#[async_trait]
pub trait Repository<T> {
    async fn find_by_id(&self, id: &str) -> Result<Option<T>>;
    async fn find_all(&self) -> Result<Vec<T>>;
    async fn insert(&self, entity: T) -> Result<T>;
    async fn update(&self, entity: T) -> Result<T>;
    async fn delete(&self, id: &str) -> Result<()>;
}
```

**Benefits:**
- Testable data layer
- Consistent API across entities
- Clean separation of concerns
- Type safety through SeaORM

### 3. Multi-Backend Abstraction

All media sources implement the `MediaBackend` trait:

```rust
#[async_trait]
pub trait MediaBackend: Send + Sync {
    async fn authenticate(&self, credentials: Credentials) -> Result<User>;
    async fn get_libraries(&self) -> Result<Vec<Library>>;
    async fn get_movies(&self, library_id: &str) -> Result<Vec<Movie>>;
    // ... other methods
}
```

**Current Backends:**
- **Plex**: Complete implementation with authentication and API integration
- **Jellyfin**: Complete implementation with authentication and API integration  
- **Local**: 90% TODO stubs, file scanning partially implemented

### 4. Three-Tier Caching Strategy

```
UI ← Memory Cache ← Database Cache ← Backend API
```

1. **Memory Cache (LRU)**: Fast access for recently accessed items
2. **Database Cache (SQLite)**: Persistent offline storage with SeaORM
3. **Backend Cache**: Source-specific optimization and rate limiting

### 5. Event-Driven Synchronization

Background sync operates through event emission:

1. **SyncManager** starts background sync
2. **Events emitted** at each stage (started, progress, completed)
3. **ViewModels subscribe** to relevant events
4. **UI updates automatically** when data changes

## Database Schema

### Core Tables

#### Sources Table
```sql
CREATE TABLE sources (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    source_type TEXT NOT NULL, -- 'plex', 'jellyfin', 'local'
    connection_url TEXT,
    is_online BOOLEAN DEFAULT FALSE,
    last_sync TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

#### Libraries Table  
```sql
CREATE TABLE libraries (
    id TEXT PRIMARY KEY,
    source_id TEXT NOT NULL,
    title TEXT NOT NULL,
    library_type TEXT NOT NULL, -- 'movies', 'shows', 'music'
    item_count INTEGER DEFAULT 0,
    FOREIGN KEY (source_id) REFERENCES sources(id) ON DELETE CASCADE
);
```

#### Media Items Table
```sql
CREATE TABLE media_items (
    id TEXT PRIMARY KEY,
    library_id TEXT NOT NULL,
    source_id TEXT NOT NULL,
    media_type TEXT NOT NULL, -- 'movie', 'show', 'episode'
    title TEXT NOT NULL,
    year INTEGER,
    duration_ms INTEGER,
    rating REAL,
    poster_url TEXT,
    overview TEXT,
    metadata TEXT, -- JSON for type-specific fields
    FOREIGN KEY (library_id) REFERENCES libraries(id) ON DELETE CASCADE
);
```

#### Playback Progress Table
```sql
CREATE TABLE playback_progress (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    media_id TEXT NOT NULL,
    position_ms INTEGER NOT NULL,
    duration_ms INTEGER NOT NULL,
    watched BOOLEAN DEFAULT FALSE,
    last_watched_at TIMESTAMP,
    FOREIGN KEY (media_id) REFERENCES media_items(id) ON DELETE CASCADE
);
```

### Performance Optimization

**Indexes:**
```sql
CREATE INDEX idx_media_items_library ON media_items(library_id);
CREATE INDEX idx_media_items_source ON media_items(source_id);
CREATE INDEX idx_media_items_title ON media_items(sort_title);
CREATE INDEX idx_playback_media_user ON playback_progress(media_id);
```

**Full-Text Search (Planned):**
```sql
CREATE VIRTUAL TABLE media_search USING fts5(
    title, overview, genres, director, actors,
    content='media_items', content_rowid='rowid'
);
```

## Event System Architecture

### Event Types (27 Total, 12 Implemented)

#### ✅ Working Events (12/27)
- **Media Events**: MediaCreated, MediaUpdated (2/5)
- **Sync Events**: All 4 events (SyncStarted, SyncProgress, SyncCompleted, SyncFailed)
- **Source Events**: SourceAdded (1/4)
- **Library Events**: LibraryCreated, LibraryUpdated, LibraryItemCountChanged (3/4)
- **Playback Events**: 5/6 events working
- **Cache Events**: CacheCleared (1/3)

#### ❌ Missing Events (15/27)
- Media: MediaDeleted, MediaBatchCreated, MediaBatchUpdated
- Source: SourceUpdated, SourceRemoved, SourceOnlineStatusChanged
- Library: LibraryDeleted
- User: All 3 user events
- System: All 4 system events
- Cache: CacheInvalidated, CacheUpdated

### Event Flow Example

```rust
// 1. Service operation triggers event
data_service.create_media_item(item).await?;

// 2. Event emitted through EventBus
event_bus.emit_media_created(id, media_type, library_id, source_id).await?;

// 3. ViewModel receives event
sidebar_viewmodel.handle_library_updated(event).await;

// 4. ViewModel updates properties
self.library_items.set(updated_items).await;

// 5. UI automatically updates through property subscription
```

## UI Architecture: ViewModels & Reactive Properties

### ViewModel Pattern Implementation

Each UI page has a corresponding ViewModel that manages its state and reacts to data changes:

```rust
#[async_trait]
pub trait ViewModel: Send + Sync {
    async fn initialize(&self, event_bus: Arc<EventBus>);
    fn subscribe_to_property(&self, property_name: &str) -> Option<PropertySubscriber>;
    async fn refresh(&self);
    fn dispose(&self);
}
```

### Property System

**Property<T>**: Reactive data container with change notifications

```rust
pub struct Property<T> {
    value: Arc<RwLock<T>>,
    sender: broadcast::Sender<T>,
}

impl<T: Clone> Property<T> {
    pub async fn set(&self, value: T) {
        *self.value.write().await = value.clone();
        let _ = self.sender.send(value);
    }
    
    pub async fn get(&self) -> T {
        self.value.read().await.clone()
    }
    
    pub fn subscribe(&self) -> PropertySubscriber {
        PropertySubscriber::new(self.sender.subscribe())
    }
}
```

### Current ViewModel Status

#### ✅ LibraryViewModel (Fully Integrated)
- Complete ViewModel integration with LibraryView
- DB entity to UI model conversion
- Reactive property bindings
- Filter/sort operations delegate to ViewModel
- All operations use ViewModel, no direct data access

#### ✅ SidebarViewModel (Fully Reactive) 
- Event handlers properly reload data from database
- Reactive status updates (status_text, status_icon, show_spinner)
- Eliminated competing cache loading systems
- End-to-end reactivity proven working

#### 🟡 Partially Integrated ViewModels
- **HomeViewModel**: Basic property subscriptions, needs completion
- **SourcesViewModel**: Partial integration, auth operations still direct
- **PlayerViewModel**: Integration status unknown, needs investigation
- **DetailsViewModel**: Created but no UI page integration

#### ❌ Missing Integration
- **MovieDetailsPage & ShowDetailsPage**: No ViewModel integration
- **Property binding**: Still using wait_for_change() loops instead of proper GTK binding

## Service Layer Architecture

### DataService (Formerly CacheManager)

The central service for all data operations:

```rust
pub struct DataService {
    db: DatabaseConnection,
    media_repo: MediaRepositoryImpl,
    library_repo: LibraryRepositoryImpl,
    source_repo: SourceRepositoryImpl,
    cache: Arc<RwLock<LruCache<String, CachedItem>>>,
    event_bus: Arc<EventBus>,
}
```

**Key Features:**
- Repository pattern usage
- Memory caching with LRU eviction
- Event emission on all CRUD operations
- Transaction support (partially implemented)
- Write-through caching strategy

### SyncManager

Handles background synchronization:

```rust
pub struct SyncManager {
    data_service: Arc<DataService>,
    event_bus: Arc<EventBus>,
    sync_status: Arc<RwLock<HashMap<String, SyncStatus>>>,
}
```

**Sync Types:**
- **Full Sync**: Complete refresh of all backend data
- **Incremental Sync**: Only changes since last sync
- **Library Sync**: Specific library update
- **Media Sync**: Individual item update

**Current Status:**
- Basic sync operations working
- Events emitted during sync process
- Transactions not fully integrated
- Progress tracking implemented

### SourceCoordinator

Manages multiple backend connections:

```rust
pub struct SourceCoordinator {
    auth_manager: Arc<AuthManager>,
    backend_manager: Arc<RwLock<BackendManager>>,
    sync_manager: Arc<SyncManager>,
    data_service: Arc<DataService>,
}
```

**Responsibilities:**
- Backend lifecycle management
- Authentication coordination
- Multi-source operations
- Health monitoring (planned)

## Backend Integration

### MediaBackend Trait

Common interface for all media sources:

```rust
#[async_trait]
pub trait MediaBackend: Send + Sync {
    async fn authenticate(&self, credentials: Credentials) -> Result<User>;
    async fn get_libraries(&self) -> Result<Vec<Library>>;
    async fn get_movies(&self, library_id: &str) -> Result<Vec<Movie>>;
    async fn get_shows(&self, library_id: &str) -> Result<Vec<Show>>;
    async fn get_episodes(&self, show_id: &str) -> Result<Vec<Episode>>;
    async fn get_media_info(&self, media_id: &str) -> Result<MediaInfo>;
    async fn get_stream_url(&self, media_id: &str, quality: Option<String>) -> Result<String>;
    async fn update_progress(&self, media_id: &str, position: Duration) -> Result<()>;
}
```

### Backend Status

#### Plex Backend (90% Complete)
- ✅ Authentication with PIN-based OAuth
- ✅ Library and media fetching
- ✅ Streaming URL generation
- ✅ Progress tracking
- ⚠️ Cast/crew extraction has TODO comments
- ✅ Error handling and retry logic

#### Jellyfin Backend (90% Complete)
- ✅ Authentication with username/password
- ✅ Library and media fetching  
- ✅ Streaming URL generation
- ✅ Progress tracking
- ⚠️ Cast/crew extraction has TODO comments
- ✅ Error handling

#### Local Backend (10% Complete)
- ❌ 25+ TODO comments throughout
- ❌ File scanning stub implementation
- ❌ Metadata extraction not implemented
- ❌ Watch folder monitoring not implemented
- ⚠️ Basic file listing only

## Critical Architecture Issues

### 1. Main Window Hybrid Status System (🚨 Critical)

**Problem:** Race conditions between reactive and direct UI updates

**Details:**
- SidebarViewModel has reactive properties (status_text, status_icon, show_spinner) 
- Old code directly manipulates the same UI elements
- Creates inconsistent state and bypasses reactive architecture

**Conflicting Methods:**
- `update_user_display()` - directly sets status label
- `show_sync_progress()` - directly manipulates sync spinner
- `update_connection_status()` - bypasses status_icon property
- Auth completion flows - mix direct and reactive updates

**Solution:** Eliminate all direct UI manipulation, force updates through SidebarViewModel properties

### 2. Repository Event Integration Gap (🔴 High Priority)

**Problem:** Repository layer has zero event integration

**Impact:**
- Direct repository calls bypass event system entirely
- ViewModels miss data changes from external operations
- Violates reactive architecture design

**Solution:** Add event emission to all repository CRUD operations

### 3. PropertySubscriber Clone Issue (🟡 Medium Priority)

**Problem:** broadcast::Receiver cannot implement Clone

**Current Workaround:** panic! in Clone implementation

**Impact:**
- Blocks advanced ViewModel composition patterns
- Potential runtime crashes if Clone is called
- Prevents proper property subscriber sharing

**Solution:** Redesign PropertySubscriber to work without Clone requirement

### 4. Transaction Integration Gap (🟡 Medium Priority)

**Problem:** Transaction support methods exist but unused

**Details:**
- `sync_libraries_transactional()` method implemented
- `execute_in_transaction()` wrapper added  
- Not integrated into actual sync flow
- Risk of data inconsistency during complex operations

**Solution:** Wire transaction methods into SyncManager operations

## Performance Considerations

### Memory Management

**LRU Cache Configuration:**
```rust
// Current configuration
let cache = LruCache::new(NonZeroUsize::new(1000).unwrap());
```

**Memory Usage Patterns:**
- UI model objects cached for instant access
- Image thumbnails cached with size limits
- Database connection pooling prevents connection overhead
- Background sync uses minimal memory footprint

### Database Performance

**Query Optimization:**
- Indexed columns for common queries (title, library_id, source_id)
- Foreign key constraints with CASCADE for efficient deletion
- Prepared statements through SeaORM query builder
- Connection pooling reduces overhead

**Planned Optimizations:**
- FTS5 full-text search for media queries
- Materialized views for complex aggregations
- Background vacuum scheduling
- Query performance monitoring

### UI Responsiveness

**Async Patterns:**
- All I/O operations use tokio async runtime
- UI never blocks on database or network operations
- Background sync with progress notifications
- Optimistic UI updates with rollback capability

## Security Considerations

### Authentication & Storage

**Credential Management:**
- Plex: PIN-based OAuth flow with token storage
- Jellyfin: Username/password with secure token storage
- Local: No authentication required

**Data Protection:**
- SQLite database stored in user's application data directory
- No credentials stored in plaintext
- Network requests use HTTPS when available
- Token refresh handled automatically

### Input Validation

**Database Layer:**
- SeaORM provides type safety and SQL injection protection
- Foreign key constraints prevent orphaned records
- Input sanitization for user-provided metadata

**Network Layer:**
- URL validation for backend connections
- Request timeout and retry logic
- CSRF protection for web-based authentication

## Development & Testing Strategy

### Code Quality

**Current Status:**
- ✅ Compiles successfully with warnings only
- ✅ Clippy linting passes
- ✅ Code formatting with rustfmt
- ❌ Zero unit tests for new architecture
- ❌ No integration tests for event system

**Testing Priorities:**
1. Repository layer integration tests
2. Event system unit tests  
3. ViewModel property binding tests
4. SyncManager transaction tests
5. Backend authentication tests

### Build System

**Nix Development Environment:**
- Reproducible development environment
- GTK4/GStreamer dependencies managed
- Database migration tools available
- Cargo tools pre-configured

**Essential Commands:**
```bash
nix develop          # Enter development shell
cargo build         # Build project
cargo run           # Run application
cargo test          # Run tests (when implemented)
cargo clippy        # Lint code
cargo fmt           # Format code
```

## Migration Timeline & Next Steps

### Immediate Priorities (Week 1)

1. **🚨 CRITICAL: Fix Main Window Status System**
   - Refactor `update_user_display()`, `show_sync_progress()`, auth completion flows
   - Force all status updates through SidebarViewModel reactive properties
   - Eliminate race conditions between reactive and direct updates

2. **🔴 HIGH: Complete UI Page ViewModel Integration**
   - MovieDetailsPage & ShowDetailsPage: Integrate DetailsViewModel
   - SourcesPage: Move auth operations to ViewModel
   - PlayerPage: Investigate and complete ViewModel usage
   - Replace wait_for_change() loops with proper GTK data binding

### Short-term Goals (Weeks 2-3)

3. **🔴 HIGH: Repository Event Integration**
   - Add event emission to all repository CRUD operations
   - Ensure all data changes trigger appropriate events
   - Test end-to-end reactivity for direct repository operations

4. **🟡 MEDIUM: Fix PropertySubscriber Clone Issue**
   - Redesign PropertySubscriber to avoid Clone requirement
   - Enable advanced ViewModel composition patterns
   - Remove panic! workaround

5. **🟡 MEDIUM: Complete Transaction Integration**
   - Wire up existing transaction methods into sync flow
   - Add transaction support to bulk operations
   - Ensure data consistency during complex operations

### Medium-term Goals (Weeks 4-6)

6. **🟡 MEDIUM: Comprehensive Testing Suite**
   - Unit tests for repository layer
   - Integration tests for event system
   - ViewModel property binding tests
   - Backend authentication tests

7. **🟡 MEDIUM: Complete Local Backend**
   - File scanning implementation
   - Metadata extraction from files
   - Watch folder monitoring
   - Integration with main architecture

8. **🟡 MEDIUM: Complete Missing Events**
   - Implement remaining 15 event types
   - Add User and System event categories
   - Complete Media batch operations events

### Long-term Goals (Weeks 7-8)

9. **Performance Optimization**
   - FTS5 full-text search implementation
   - Database vacuum scheduling
   - Query performance monitoring
   - Memory usage optimization

10. **Advanced Features**
    - Offline content management
    - Download queue functionality
    - Advanced search and filtering
    - Multi-user support

## Conclusion

Reel's architecture represents a modern, reactive approach to media player development. The current SeaORM migration has established a solid foundation with 75% completion, achieving major breakthroughs in event-driven reactivity. The core database infrastructure, repository pattern, and event system are production-ready.

Key architectural strengths:
- **Reactive Architecture**: Event-driven updates with proven end-to-end functionality
- **Type Safety**: SeaORM provides compile-time guarantees for database operations  
- **Clean Separation**: Repository pattern enables testable, maintainable data access
- **Multi-Backend Support**: Pluggable architecture supports multiple media sources
- **Offline-First**: Local cache with background sync ensures responsive UI

The remaining 25% focuses on completing UI integration, eliminating hybrid patterns, and adding comprehensive testing. With the core reactive architecture proven functional, the path to completion is clear and well-defined.

---

**For Developers:**
- Always work within `nix develop` environment
- Use repository pattern for all database access
- Emit events for all data changes
- Follow reactive architecture patterns
- Test ViewModels with property subscriptions
- Never bypass the event system

**For Contributors:**
- Focus on completing ViewModel integration first
- Ensure all new code includes appropriate tests
- Follow established patterns for consistency
- Document any architectural changes
- Maintain backwards compatibility during migration

This architecture documentation will be updated as the migration progresses and new patterns emerge.