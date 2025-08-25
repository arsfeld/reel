# Backend Architecture in GNOME Reel

## Overview

The backend system in GNOME Reel provides a unified abstraction layer for different media sources (Plex, Jellyfin, Local Files). The architecture follows a three-tier model: Authentication Providers → Sources → Backends.

## Key Components

### 1. Authentication Providers (`AuthProvider`)
Location: `src/models/auth.rs`

Authentication providers store credentials for accessing media services:
- **PlexAccount**: Stores Plex authentication token and user info
- **JellyfinAuth**: Stores Jellyfin server URL, username, and access token  
- **LocalFiles**: Placeholder for local file access
- **NetworkCredentials**: For SMB/NFS/WebDAV access (future)

### 2. Sources (`Source`)
Location: `src/models/source.rs`

Sources represent discovered media servers:
- One AuthProvider can have multiple Sources (e.g., one Plex account → multiple Plex servers)
- Contains metadata like server name, type, and connection info
- Links back to its parent AuthProvider via `auth_provider_id`

### 3. Media Backends (`MediaBackend` trait)
Location: `src/backends/traits.rs`

The `MediaBackend` trait defines the interface all backends must implement:
- **Core Operations**: authenticate, get_libraries, get_movies, get_shows, get_episodes
- **Playback**: get_stream_url, update_progress, mark_watched/unwatched
- **Search**: search across content
- **Sync Support**: get_backend_id, get_last_sync_time, supports_offline

## Initialization Flow

Based on the logs, here's the startup sequence:

1. **AuthManager loads saved providers** (`src/services/auth_manager.rs:28-69`)
   - Reads from config file
   - Loads tokens from system keyring
   - Migrates legacy backends if needed

2. **SourceCoordinator initializes sources** (`src/services/source_coordinator.rs:145-243`)
   - For each AuthProvider:
     - Checks for cached sources (offline-first)
     - Shows cached data immediately
     - Triggers background refresh to discover/update sources
   - Creates status tracking for each source

3. **Backend creation** (`src/services/source_coordinator.rs:453-506`)
   - For each discovered Source:
     - Creates appropriate backend (PlexBackend, JellyfinBackend, etc.)
     - Registers with BackendManager
     - Tests connection with `backend.initialize()`
     - Updates connection status (Connected/Offline/Error)

## Data Flow

```
User adds account → AuthManager
                    ├─ Stores credentials in keyring
                    ├─ Saves provider to config
                    └─ Discovers sources
                        └─ SourceCoordinator
                            ├─ Creates backend instances
                            ├─ Registers with BackendManager
                            └─ Manages sync and status
```

## Offline-First Architecture

The system prioritizes showing cached data:

1. **Cache Loading**: On startup, loads cached sources immediately
2. **UI Display**: Shows content from SQLite cache without waiting for network
3. **Background Refresh**: Asynchronously updates from servers
4. **Status Tracking**: Maintains connection status (Connected/Offline/NeedsAuth/Error)

## Backend Manager

The `BackendManager` (`src/backends/manager.rs`) maintains:
- Registry of all active backends
- Backend ordering for priority
- Unified access to all backends

## Sync System

The `SyncManager` (`src/services/sync_manager.rs`) handles:
- Full sync: Complete refresh of all data
- Incremental sync: Only changes since last sync  
- Library sync: Specific library update
- Background sync coordination

## Key Design Decisions

1. **Separation of Auth and Sources**: Allows one account to manage multiple servers
2. **Trait-based Backend Interface**: Enables uniform handling of different server types
3. **Offline-First with Cache**: Ensures instant UI loading regardless of connectivity
4. **Background Sync**: Keeps data fresh without blocking the UI
5. **Keyring Storage**: Secure credential storage outside of config files

## Current Issue from Logs

The logs show:
- Plex provider exists (`plex_2ddefe5b`)
- Token is loaded successfully
- But no backends are initialized: "No backends were successfully initialized"
- This suggests the Plex source discovery or backend creation is failing

The system correctly falls back to showing "no valid credentials" message when backends fail to initialize.