# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Reel is a native, cross-platform media player application written in Rust that brings your Plex and Jellyfin libraries to the desktop with a premium, Netflix-like experience. The application features:

- **Multi-platform support**: Relm4/libadwaita for Linux/GNOME (primary), macOS support in development
- **Multiple backend support**: Simultaneous connections to Plex, Jellyfin, and local media libraries
- **Offline-first architecture**: Instant UI loading from SQLite cache with background synchronization
- **Reactive UI system**: Relm4 components with AsyncComponents, Factory patterns, and Worker components
- **Dual playback engines**: MPV (Linux) and GStreamer (macOS) for cross-platform support
- **Modern Rust architecture**: Type-safe database layer with SeaORM, async/await with Tokio, repository pattern

The project uses a fully Relm4-based UI implementation that leverages AsyncComponents, Tracker patterns, Factory patterns for collections, Worker components for background tasks, and Command patterns for structured async operations.

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
- `src/main.rs` - Entry point and application initialization
- `src/app/` - Application core with Relm4 AsyncComponent
  - `mod.rs` - Application root component and state management
- `src/ui/` - Relm4 UI components
  - `main_window.rs` - Root window AsyncComponent
  - `sidebar.rs` - Navigation sidebar component
  - `pages/` - Page AsyncComponents (home, library, player, sources, movie_details, show_details, preferences)
  - `factories/` - Factory components for collections (media cards, episode lists)
  - `dialogs/` - Dialog components (auth, source management)
  - `shared/` - Common UI utilities, messages, navigation
- `src/workers/` - Background worker components
  - `connection_monitor.rs` - Connection health monitoring
  - `sync_worker.rs` - Background synchronization
  - `search_worker.rs` - Search functionality
  - `image_loader.rs` - Async image loading
- `src/services/` - Service layer architecture
  - `core/` - Core service implementations (auth, sync, media, source)
  - `brokers/` - MessageBroker components (connection, media, sync)
  - `commands/` - Command pattern implementations (auth, media, sync)
  - `types/` - Service type definitions
  - `initialization.rs` - Service initialization and setup
  - `cache_keys.rs` - Cache key management
- `src/backends/` - Media server integrations
  - `traits.rs` - Common backend interface (`MediaBackend` trait)
  - `plex/` - Plex backend with OAuth authentication
  - `jellyfin/` - Jellyfin backend with username/password auth
  - `local/` - Local files backend (mostly unimplemented)
  - `sync_strategy.rs` - Synchronization strategies
- `src/db/` - Database layer (SeaORM/SQLite)
  - `repository/` - Repository pattern implementations (media, library, source, playback, sync)
  - `entities/` - SeaORM entity definitions with relations
  - `migrations/` - Database schema migrations
  - `connection.rs` - Database connection management
- `src/player/` - Media playback
  - `mpv_player.rs` - MPV backend (Linux only - has OpenGL issues on macOS)
  - `gstreamer_player.rs` - GStreamer backend (required on macOS)
  - `controller.rs` - Player controller and state management
  - `factory.rs` - Player backend selection (forces GStreamer on macOS)
- `src/models/` - Data models and auth providers
  - `identifiers.rs` - Type-safe ID wrappers
  - `auth_provider.rs` - Authentication provider types
  - `connection.rs` - Connection state models
  - `playlist_context.rs` - Playlist and playback context
- `src/core/` - Core abstractions and traits
  - `mod.rs` - Core module exports and traits
- `src/mapper/` - Data mapping and transformations
- `src/styles/` - CSS and theming
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

6. **Platform Abstraction**: Frontend trait allows for multiple platform implementations (macOS planned)

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

Built with Relm4 and libadwaita:
- Follows GNOME Human Interface Guidelines
- Responsive design adapting to window size
- Hardware-accelerated video playback via MPV/GStreamer
- Dark mode support
- Reactive components with AsyncComponents, Factory patterns, and Worker components

## Event System

The application uses Relm4's MessageBroker for inter-component communication:
- **MessageBroker** for component communication (located in `src/services/brokers/`)
- **Commands** provide structured async operations with proper lifecycle (in `src/services/commands/`)
- **Worker Components** handle background tasks in isolation (in `src/workers/`)
- Service brokers manage specific domains: ConnectionBroker, MediaBroker, SyncBroker

## Dependencies

Key dependencies managed through Cargo.toml:
- **Relm4 Stack**: `relm4`, `relm4-components`, `relm4-icons`, `tracker` for reactive UI
- Relm4/libadwaita for UI foundation
- MPV (libmpv2) for primary video playback
- GStreamer for secondary playback option
- Tokio for async runtime
- SeaORM/SQLite for database operations
- Reqwest for HTTP requests
- LRU for memory caching
- Keyring for secure credential storage

## Development Notes

- The project uses Nix flakes - always work within `nix develop` shell
- GStreamer plugins are configured in the Nix environment
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
- When building / checking, always use the last line to get the proper number of errors
- IMPORTANT: Don't add backward compatibility code, tackle the root cause
- IMPORTANT: do not use fallbacks, they only introduce confusion and errors.
- never mention claude in commits

<!-- BACKLOG.MD MCP GUIDELINES START -->

<CRITICAL_INSTRUCTION>

## BACKLOG WORKFLOW INSTRUCTIONS

This project uses Backlog.md MCP for all task and project management activities.

**CRITICAL RESOURCE**: Read `backlog://workflow/overview` to understand when and how to use Backlog for this project.

- **First time working here?** Read the overview resource IMMEDIATELY to learn the workflow
- **Already familiar?** You should have the overview cached ("## Backlog.md Overview (MCP)")
- **When to read it**: BEFORE creating tasks, or when you're unsure whether to track work

The overview resource contains:
- Decision framework for when to create tasks
- Search-first workflow to avoid duplicates
- Links to detailed guides for task creation, execution, and completion
- MCP tools reference

You MUST read the overview resource to understand the complete workflow. The information is NOT summarized here.

</CRITICAL_INSTRUCTION>

<!-- BACKLOG.MD MCP GUIDELINES END -->
