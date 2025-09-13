# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Reel is a native, cross-platform media player application written in Rust that brings your Plex and Jellyfin libraries to the desktop with a premium, Netflix-like experience. The application features:

- **Multi-platform support**: GTK4/libadwaita for Linux/GNOME (primary), macOS support in development
- **Multiple backend support**: Simultaneous connections to Plex, Jellyfin, and local media libraries
- **Offline-first architecture**: Instant UI loading from SQLite cache with background synchronization
- **Reactive UI system**: Event-driven ViewModels with observable properties for responsive updates
- **Dual playback engines**: MPV (default, recommended) and GStreamer for maximum compatibility
- **Modern Rust architecture**: Type-safe database layer with SeaORM, async/await with Tokio, repository pattern

The project is transitioning to a fully Relm4-based UI implementation that leverages AsyncComponents, Tracker patterns, Factory patterns for collections, Worker components for background tasks, and Command patterns for structured async operations - abandoning ViewModels in favor of Relm4's native reactive architecture.

## Development Environment

This project uses Nix flakes for development environment management. Always enter the development shell before running commands:

```bash
nix develop
```

## Essential Commands

### Build and Run
```bash
# Enter development environment (REQUIRED FIRST)
nix develop

# Build the project
cargo build

# Run the application
cargo run

# Build release version
cargo build --release
```

### Code Quality
```bash
# Format code
cargo fmt

# Run linter
cargo clippy

# Check compilation without building
cargo check
```

### Testing
```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run tests with output
cargo test -- --nocapture
```

## Architecture Overview

### Module Structure
- `src/main.rs` - Entry point with platform detection
- `src/platforms/relm4/` - Relm4 platform implementation (target architecture)
  - `app.rs` - Relm4 application root AsyncComponent
  - `components/` - Pure Relm4 components with native patterns
    - `main_window.rs` - Root window AsyncComponent
    - `pages/` - Page AsyncComponents (home, library, player, sources, movie_details, show_details)
    - `factories/` - Factory components for collections (media cards, episode lists)
    - `workers/` - Background worker components (sync, search, image loading)
    - `shared/` - Common messages, commands, navigation
- `src/platforms/gtk/` - Legacy GTK4 platform implementation
  - `app.rs` - GTK application initialization
  - `ui/` - GTK4/libadwaita UI components
    - `main_window.rs` - Main application window
    - `pages/` - Different application views
    - `widgets/` - Reusable UI components
    - `viewmodels/` - Reactive ViewModels (being phased out)
- `src/core/` - Platform-agnostic core logic
  - `state.rs` - Application state management
  - `viewmodels/` - Core ViewModels (library, player, sources, sidebar, details, home)
  - `player_traits.rs` - Media player abstraction
  - `frontend.rs` - Frontend trait for platform abstraction
- `src/backends/` - Media server integrations
  - `traits.rs` - Common backend interface (`MediaBackend` trait)
  - `plex/` - Plex backend with OAuth authentication
  - `jellyfin/` - Jellyfin backend with username/password auth
  - `local/` - Local files backend (mostly unimplemented)
- `src/services/` - Core services
  - `data.rs` - DataService for database operations with caching
  - `sync.rs` - SyncManager for background synchronization
  - `auth_manager.rs` - Authentication and credential management
  - `source_coordinator.rs` - Multi-backend coordination
- `src/events/` - Event system
  - `event_bus.rs` - Central event broadcasting with Tokio
  - `types.rs` - Event type definitions (Media, Sync, Library, Playback, Source)
- `src/db/` - Database layer (SeaORM/SQLite)
  - `repository/` - Repository pattern implementations (media, library, source, playback, sync)
  - `entities/` - SeaORM entity definitions with relations
  - `migrations/` - Database schema migrations
  - `connection.rs` - Database connection management
- `src/player/` - Media playback
  - `mpv_player.rs` - MPV backend (default, no subtitle issues)
  - `gstreamer_player.rs` - GStreamer backend (has subtitle color artifacts)
  - `traits.rs` - Player interface definition
  - `factory.rs` - Player backend selection
- `src/models/` - Data models and auth providers
- `src/utils/` - Utilities (errors, image loading)

### Key Design Patterns

1. **Relm4 Reactive Architecture**: Pure component-based reactive system
   - **AsyncComponents**: Data-heavy pages with built-in loading states and command patterns
   - **Tracker Pattern**: Efficient change tracking for minimal re-renders (`#[tracker::track]`)
   - **Factory Pattern**: Dynamic collections with FactoryVecDeque for lists and grids
   - **Worker Pattern**: Background tasks isolated in worker components
   - **MessageBroker**: Inter-component communication replacing custom event bus
   - **Command Pattern**: Structured async operations with proper lifecycle management

2. **Backend Abstraction**: All media sources implement the `MediaBackend` trait, allowing uniform handling of different server types.

3. **Repository Pattern**: Each data entity has its own repository with consistent CRUD operations and type safety through SeaORM.

4. **Three-Tier Caching**: 
   - Memory Cache (LRU with 1000 item limit)
   - Database Cache (SQLite with SeaORM)
   - Backend API (source-specific optimization)

5. **Offline-First Architecture**: 
   - SQLite cache stores all metadata locally
   - UI loads instantly from cache
   - Background sync updates data without blocking UI
   - Offline fallback for all operations

6. **Platform Abstraction**: Frontend trait allows for multiple platform implementations (GTK currently, macOS planned)

## Backend System

The application supports multiple backends simultaneously through the `MediaBackend` trait:

```rust
#[async_trait]
pub trait MediaBackend: Send + Sync {
    async fn authenticate(&self, credentials: Credentials) -> Result<User>;
    async fn get_libraries(&self) -> Result<Vec<Library>>;
    async fn get_movies(&self, library_id: &str) -> Result<Vec<Movie>>;
    // ... other methods
}
```

Each backend (Plex, Jellyfin, Local) implements this trait independently.

## Database Schema

SQLite database with SeaORM manages:
- `sources` - Backend configurations and connection info
- `libraries` - Media libraries with type and item counts
- `media_items` - Movies, shows, episodes with metadata
- `playback_progress` - Resume positions and watch status
- `sync_status` - Sync state tracking per source
- `offline_content` - Downloaded media for offline playback

Key indexes for performance:
- `idx_media_items_library` - Fast library queries
- `idx_media_items_source` - Fast source filtering
- `idx_media_items_title` - Fast title sorting
- `idx_playback_media_user` - Fast progress lookups

## UI Framework

Built with GTK4 and libadwaita:
- Follows GNOME Human Interface Guidelines
- Responsive design adapting to window size
- Hardware-accelerated video playback via MPV/GStreamer
- Dark mode support
- Blueprint UI templates for declarative layouts

## ViewModels & Event System

The application uses a reactive ViewModel pattern:
- **ViewModels** manage UI state and react to data changes
- **Properties** provide observable data containers with change notifications
- **EventBus** broadcasts events system-wide using Tokio channels
- **Event types** include Media, Sync, Library, Playback, Source events

Current migration status (75% complete):
- ✅ Database infrastructure with SeaORM
- ✅ Repository pattern implementation
- ✅ Event system with 12/27 event types working
- ✅ LibraryViewModel and SidebarViewModel fully reactive
- 🟡 4 of 6 UI pages need ViewModel integration

## Dependencies

Key dependencies managed through Cargo.toml:
- **Relm4 Stack**: `relm4`, `relm4-components`, `relm4-icons`, `tracker` for reactive UI
- GTK4/libadwaita for UI foundation
- MPV (libmpv2) for primary video playback
- GStreamer for secondary playback option
- Tokio for async runtime
- SeaORM/SQLite for database operations
- Reqwest for HTTP requests
- LRU for memory caching
- Keyring for secure credential storage

## Development Notes

- The project uses Nix flakes - always work within `nix develop` shell
- GStreamer plugins and GTK schemas are configured in the Nix environment
- Database uses SeaORM with migrations in `src/db/migrations/`
- Use `cargo watch` for auto-rebuild during development
- Pre-commit hooks run `cargo fmt` automatically

## Package Building

Within the Nix development shell:
```bash
# Build Debian package
build-deb

# Build RPM package  
build-rpm

# Build AppImage
build-appimage

# Build all packages
build-all-packages
```

## Known Issues & TODOs

### Critical Issues
- Homepage sections randomly replace each other with multiple backends
- Horizontal scrolling on homepage doesn't load images
- GStreamer subtitle color artifacts (use MPV player instead)
- Main Window has hybrid status system creating race conditions between reactive and direct UI updates

### Architecture Gaps
- Repository layer has zero event integration (bypasses event system)
- PropertySubscriber uses panic! in Clone implementation
- Transaction support exists but not integrated into sync flow
- 4 UI pages still need ViewModel integration (MovieDetails, ShowDetails, Sources partial, Player)

### Backend Implementation Status
- **Plex**: 90% complete (missing proper cast/crew extraction)
- **Jellyfin**: 90% complete (cast/crew implemented but depends on server metadata)
- **Local Files**: 10% complete (mostly TODO stubs, basic structure only)