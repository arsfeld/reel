# Reel Application Architecture

## Overview

Reel is a modern GTK4/libadwaita media player application for GNOME, written in Rust. It provides a premium media consumption experience with support for multiple media server backends (Plex, Jellyfin, local files) and features an innovative offline-first architecture with seamless background synchronization.

## Core Architecture Principles

### 1. Offline-First Design
- All metadata is cached locally in SQLite database
- UI loads instantly from cached data
- Background sync updates data without blocking the UI
- Graceful fallback for offline operation

### 2. Multi-Backend Support
- Unified abstraction through the `MediaBackend` trait
- Support for simultaneous multiple sources
- Backend-agnostic UI components
- Extensible plugin-like architecture for new backends

### 3. Reactive State Management
- Centralized `AppState` with subscription system
- Arc/RwLock-based thread-safe state sharing
- Event-driven UI updates
- Separation of concerns between state and presentation

### 4. Asynchronous Operations
- Tokio runtime for all I/O operations
- Non-blocking UI with async/await patterns
- Background task management
- Parallel backend operations

## System Architecture Layers

```
┌─────────────────────────────────────────────────────────┐
│                     UI Layer (GTK4)                      │
│  ┌─────────────┬──────────────┬────────────────────┐   │
│  │   Pages     │   Widgets    │    Components      │   │
│  └─────────────┴──────────────┴────────────────────┘   │
├─────────────────────────────────────────────────────────┤
│                   State Management                       │
│  ┌──────────────────────────────────────────────────┐   │
│  │              AppState (Arc<RwLock>)              │   │
│  └──────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────┤
│                    Services Layer                        │
│  ┌──────────┬──────────┬──────────┬───────────────┐    │
│  │  Source  │   Sync   │  Cache   │     Auth      │    │
│  │  Coord.  │  Manager │ Manager  │    Manager    │    │
│  └──────────┴──────────┴──────────┴───────────────┘    │
├─────────────────────────────────────────────────────────┤
│                   Backend Abstraction                    │
│  ┌──────────┬──────────┬──────────┬───────────────┐    │
│  │   Plex   │ Jellyfin │  Local   │   Future...   │    │
│  └──────────┴──────────┴──────────┴───────────────┘    │
├─────────────────────────────────────────────────────────┤
│                    Data Layer                            │
│  ┌──────────────────┬─────────────────────────────┐     │
│  │   SQLite Cache   │    Configuration (TOML)     │     │
│  └──────────────────┴─────────────────────────────┘     │
└─────────────────────────────────────────────────────────┘
```

## Component Architecture

### 1. Application Initialization (`main.rs`, `app.rs`)

The application follows a multi-stage initialization process:

1. **Runtime Setup**: Tokio runtime initialization for async operations
2. **Framework Init**: GTK4, libadwaita, and GStreamer initialization
3. **Resource Loading**: Compiled UI resources and stylesheets
4. **State Creation**: AppState initialization with configuration
5. **Service Bootstrap**: SourceCoordinator and background services
6. **UI Creation**: Main window and component initialization

### 2. State Management (`state/app_state.rs`)

The `AppState` struct serves as the central state container:

- **Thread-Safe Design**: Uses `Arc<RwLock<T>>` for concurrent access
- **Component State**:
  - `backend_manager`: Manages all media backends
  - `auth_manager`: Handles authentication and credentials
  - `source_coordinator`: Orchestrates source operations
  - `current_user`: Active user session
  - `current_library`: Selected media library
  - `libraries`: Map of backend_id → libraries
  - `library_items`: Map of library_id → media items
  - `cache_manager`: Local data persistence
  - `sync_manager`: Background synchronization
  - `playback_state`: Current player state
  - `config`: Application configuration

### 3. Backend System (`backends/`)

#### Backend Trait (`traits.rs`)
The `MediaBackend` trait defines the interface all backends must implement:

```rust
#[async_trait]
pub trait MediaBackend: Send + Sync + Debug {
    async fn initialize(&self) -> Result<Option<User>>;
    async fn authenticate(&self, credentials: Credentials) -> Result<User>;
    async fn get_libraries(&self) -> Result<Vec<Library>>;
    async fn get_movies(&self, library_id: &str) -> Result<Vec<Movie>>;
    async fn get_shows(&self, library_id: &str) -> Result<Vec<Show>>;
    async fn get_episodes(&self, show_id: &str, season: u32) -> Result<Vec<Episode>>;
    async fn get_stream_url(&self, media_id: &str) -> Result<StreamInfo>;
    async fn update_progress(&self, media_id: &str, position: Duration, duration: Duration) -> Result<()>;
    // ... additional methods
}
```

#### Backend Manager (`mod.rs`)
- Manages multiple backend instances
- Handles backend registration and lifecycle
- Maintains backend ordering for UI display
- Provides unified access to all backends

#### Backend Implementations
- **Plex Backend** (`plex/`): Full Plex Media Server integration
- **Jellyfin Backend** (`jellyfin/`): Jellyfin server support
- **Local Backend** (`local/`): Local file system media

### 4. Services Layer (`services/`)

#### Source Coordinator (`source_coordinator.rs`)
Central orchestrator for all source-related operations:
- Backend lifecycle management
- Authentication flow coordination
- Source discovery and registration
- Status tracking and monitoring
- Cross-backend operations

#### Auth Manager (`auth_manager.rs`)
Handles all authentication concerns:
- Credential storage and retrieval
- Provider management (Plex, Jellyfin, etc.)
- Token refresh and validation
- Secure credential persistence

#### Cache Manager (`cache.rs`)
Local data persistence layer:
- SQLite database management
- Media metadata caching
- Playback progress tracking
- Preference storage
- Offline data availability

#### Sync Manager (`sync.rs`)
Background synchronization service:
- Periodic library updates
- Incremental sync strategies
- Conflict resolution
- Offline queue management

### 5. UI Layer (`ui/`)

#### Main Window (`main_window.rs`)
Central UI container and navigation controller:
- Navigation stack management
- Page lifecycle coordination
- Global UI state management
- Event routing and handling

#### Pages (`pages/`)
Individual application screens:
- **Home Page**: Dashboard with recent content
- **Library View**: Media grid with filtering
- **Movie Details**: Movie information and playback
- **Show Details**: Series and episode navigation
- **Player Page**: Video playback interface
- **Sources Page**: Backend management

#### Widgets (`widgets/`)
Reusable UI components used across pages

#### Components (`components/`)
Complex UI modules with business logic

### 6. Player System (`player/`)

#### Player Trait (`traits.rs`)
Defines the media player interface:
```rust
#[async_trait]
pub trait MediaPlayer: Send {
    fn create_video_widget(&self) -> gtk4::Widget;
    async fn load_media(&self, url: &str) -> Result<()>;
    async fn play(&self) -> Result<()>;
    async fn pause(&self) -> Result<()>;
    async fn seek(&self, position: Duration) -> Result<()>;
    // ... additional methods
}
```

#### Player Implementations
- **GStreamer Player** (`gstreamer_player.rs`): Primary player using GStreamer
- **MPV Player** (`mpv_player.rs`): Alternative player using libmpv
- **Player Factory** (`factory.rs`): Creates appropriate player instance

### 7. Data Models (`models/`)

Core data structures used throughout the application:

- **User**: User account information
- **Library**: Media library metadata
- **MediaItem**: Polymorphic media container (Movie, Show, Episode, etc.)
- **Movie/Show/Episode**: Specific media types
- **StreamInfo**: Streaming URL and quality information
- **ChapterMarker**: Intro/credits skip markers
- **AuthProvider**: Authentication provider configuration
- **Source**: Media source configuration

## Data Flow Patterns

### 1. Authentication Flow
```
User Input → AuthDialog → AuthManager → Backend → SourceCoordinator → AppState → UI Update
```

### 2. Library Loading Flow
```
UI Request → AppState → CacheManager (fast path) → UI Update
                    ↓
              SyncManager → Backend → CacheManager → AppState → UI Update (background)
```

### 3. Playback Flow
```
Media Selection → AppState → Backend.get_stream_url() → PlayerFactory → MediaPlayer → UI
                         ↓
                   Progress Updates → Backend → CacheManager
```

### 4. Sync Flow
```
Timer/Trigger → SyncManager → Backend.get_libraries() → CacheManager
                          ↓
                    Backend.get_items() → Diff/Merge → CacheManager → AppState
```

## Configuration Management

### Application Configuration (`config.rs`)
- TOML-based configuration file
- User preferences and settings
- Backend configurations
- UI customization options

### Storage Locations
- **Config**: `~/.config/reel/config.toml`
- **Cache**: `~/.cache/reel/cache.db`
- **Data**: `~/.local/share/reel/`

## Concurrency Model

### Thread Architecture
1. **Main Thread**: GTK UI and event handling
2. **Tokio Runtime**: Async I/O and network operations
3. **GStreamer Threads**: Media pipeline processing
4. **Background Workers**: Sync and maintenance tasks

### Synchronization Primitives
- `Arc<T>`: Shared ownership across threads
- `RwLock<T>`: Read-write locking for state
- `RefCell<T>`: Interior mutability for GTK objects
- `Channel`: Message passing between components

## Error Handling Strategy

### Error Types
- **Backend Errors**: Network, authentication, API failures
- **Player Errors**: Codec, streaming, hardware issues
- **Storage Errors**: Database, file system problems
- **UI Errors**: Resource loading, rendering issues

### Error Propagation
- `Result<T, anyhow::Error>` for fallible operations
- Graceful degradation for non-critical failures
- User-friendly error messages in UI
- Detailed logging for debugging

## Performance Optimizations

### 1. Lazy Loading
- Media items loaded on-demand
- Thumbnail loading with priority queue
- Incremental list rendering

### 2. Caching Strategy
- Multi-level cache (memory → SQLite → network)
- Adaptive cache expiration
- Predictive prefetching

### 3. UI Optimizations
- Virtual scrolling for large lists
- Image downscaling and caching
- Debounced search and filtering

### 4. Network Optimization
- Connection pooling
- Request batching
- Parallel backend queries

## Security Considerations

### 1. Credential Management
- Encrypted storage using system keyring
- Token-based authentication
- No plaintext password storage

### 2. Network Security
- HTTPS enforcement for remote connections
- Certificate validation
- Secure token transmission

### 3. Local Security
- File system permissions
- SQLite database encryption support
- Secure temporary file handling

## Extensibility Points

### 1. Backend Plugins
New backends can be added by:
1. Implementing the `MediaBackend` trait
2. Registering with `BackendManager`
3. Adding authentication support in `AuthManager`

### 2. Player Backends
New players can be added by:
1. Implementing the `MediaPlayer` trait
2. Updating `PlayerFactory` selection logic
3. Adding configuration options

### 3. UI Themes
- CSS-based theming through GTK4
- Resource override mechanism
- Dark/light mode support

## Testing Strategy

### Unit Tests
- Model serialization/deserialization
- Backend API parsing
- State management logic

### Integration Tests
- Backend connectivity
- Database operations
- Player functionality

### UI Tests
- Component rendering
- Navigation flows
- User interactions

## Future Architecture Considerations

### Planned Enhancements
1. **Plugin System**: Dynamic backend loading
2. **Transcoding Service**: On-device media conversion
3. **Recommendation Engine**: ML-based content suggestions
4. **Multi-User Support**: Profile management
5. **Cloud Sync**: Cross-device synchronization

### Scalability Considerations
- Microservice extraction for heavy operations
- Distributed caching for large libraries
- WebAssembly support for web deployment
- Mobile platform adaptation

## Conclusion

Reel's architecture emphasizes modularity, performance, and user experience. The clear separation of concerns, async-first design, and offline-capable architecture provide a solid foundation for a modern media player application. The trait-based backend system ensures extensibility while the reactive state management enables responsive UI updates.