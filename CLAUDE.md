# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Reel is a modern GTK4/libadwaita media player application for GNOME, written in Rust. It provides a premium media consumption experience with support for multiple backends (Plex, Jellyfin, local files) and features an innovative offline-first architecture with seamless background synchronization.

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
- `src/app.rs` - Main application struct and initialization
- `src/ui/` - GTK4/libadwaita UI components
  - `window.rs` - Main application window
  - `pages/` - Different application views (home, library, player, settings)
  - `widgets/` - Reusable UI components
- `src/backends/` - Media server integrations
  - `traits.rs` - Common backend interface (`MediaBackend` trait)
  - `plex/`, `jellyfin/`, `local/` - Backend implementations
- `src/services/` - Core services (cache, sync, authentication)
- `src/state/` - Application state management
- `src/player/` - GStreamer-based media playback
- `src/models/` - Data models for media items

### Key Design Patterns

1. **Backend Abstraction**: All media sources implement the `MediaBackend` trait, allowing uniform handling of different server types.

2. **Offline-First Architecture**: 
   - SQLite cache stores all metadata locally
   - UI loads instantly from cache
   - Background sync updates data without blocking UI
   - Offline fallback for all operations

3. **State Management**: Centralized `AppState` with subscription system for reactive UI updates.

4. **Async Operations**: Uses Tokio for all I/O operations, keeping the UI responsive.

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

## Sync Strategy

The sync system operates in the background with these key components:

1. **SyncManager** - Coordinates sync operations across backends
2. **CacheManager** - Handles local SQLite database and offline storage
3. **Sync Types**:
   - Full sync: Complete refresh of all data
   - Incremental sync: Only changes since last sync
   - Library sync: Specific library update
   - Media sync: Individual item update

## Database Schema

SQLite database stores:
- `media_cache` - Cached media metadata
- `offline_store` - Data for offline access
- `sync_metadata` - Sync status tracking
- `backend_config` - Backend configurations
- `playback_progress` - Resume positions
- `download_queue` - Offline download management

## UI Framework

Built with GTK4 and libadwaita:
- Follows GNOME Human Interface Guidelines
- Responsive design adapting to window size
- Hardware-accelerated video playback via GStreamer
- Dark mode support

## Dependencies

Key dependencies managed through Cargo.toml:
- GTK4/libadwaita for UI
- GStreamer for media playback
- Tokio for async runtime
- SQLx for database operations
- Reqwest for HTTP requests

## Development Notes

- The project uses Nix flakes - always work within `nix develop` shell
- GStreamer plugins and GTK schemas are configured in the Nix environment
- Database migrations use SQLx offline mode (SQLX_OFFLINE=true)
- Use `cargo watch` for auto-rebuild during development