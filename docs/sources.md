# Sources and Authentication Providers

This document provides comprehensive information about Reel's source management architecture, authentication providers, and the reactive Sources page implementation.

## Overview

Reel supports multiple media backends simultaneously through a unified source management system. Each backend (Plex, Jellyfin, Local) has different authentication patterns and source relationships, all managed through a consistent reactive architecture.

## Architecture Components

### Core Components

1. **AuthManager** (`src/services/auth_manager.rs`)
   - Manages authentication providers and credentials
   - Discovers available sources for each provider
   - Stores credentials securely in system keyring

2. **SourceCoordinator** (`src/services/source_coordinator.rs`)
   - Coordinates between authentication and source persistence
   - Syncs discovered sources to SQLite database
   - Manages source lifecycle (create, update, cleanup)

3. **DataService** (`src/services/data.rs`)
   - Provides database operations for sources
   - Implements upsert and cleanup operations
   - Serves as data layer for ViewModels

4. **SourcesViewModel** (`src/core/viewmodels/sources_view_model.rs`)
   - Reactive view model for Sources page
   - Tracks connection status and sync progress
   - Publishes events for UI updates

## Authentication Provider Types

### Plex Accounts
**Pattern**: One Account → Multiple Servers (Dynamic)

- **Authentication**: OAuth2 flow with Plex.tv
- **Server Discovery**: Dynamic via `PlexAuth::discover_servers()`
- **Source Relationship**: Fluid - servers can be added/removed from account
- **ID Generation**: Uses cached server information for consistency
- **Persistence**: All discovered servers synced to database with upsert/cleanup

```rust
// Plex servers change dynamically
let servers = auth_provider.discover_servers().await?;
for server in servers {
    data_service.upsert_source(server.into()).await?;
}
```

### Jellyfin Servers
**Pattern**: One Authentication → One Server (Static)

- **Authentication**: Username/password to specific server URL
- **Server Discovery**: Static - targets exactly one server
- **Source Relationship**: Fixed 1:1 relationship after setup
- **ID Generation**: Deterministic hash of `server_url + username`
- **Persistence**: Single source per authentication

```rust
// Jellyfin sources have stable, deterministic IDs
fn generate_stable_jellyfin_id(&self, server_url: &str, username: &str) -> String {
    let input = format!("{}:{}", server_url.trim_end_matches('/'), username);
    let hash = sha256_hash(&input)[..8];
    format!("jellyfin_{}", hash)
}
```

### Local Files
**Pattern**: Directory Paths → File System Sources

- **Authentication**: None required
- **Server Discovery**: File system scanning
- **Source Relationship**: Path-based
- **ID Generation**: Based on directory path
- **Persistence**: Stored as local source entries

## Source Lifecycle Management

### 1. Source Discovery
Sources are discovered during authentication:

```rust
// AuthManager discovers sources
let sources = self.discover_plex_sources(&provider_id).await?;

// Cache for offline scenarios
config.set_cached_sources(provider_id, sources.clone());

// Persist to database (NEW - fixes missing Plex sources)
self.data_service.sync_sources_to_database(&provider_id, &sources).await?;
```

### 2. Source Persistence
All sources are persisted to SQLite database:

- **Database Table**: `sources`
- **Upsert Logic**: Insert if new, update if exists
- **Cleanup Logic**: Remove sources that no longer exist
- **ID Stability**: Consistent IDs across application restarts

### 3. Source Updates
Sources are updated through reactive events:

```rust
// Event-driven updates
EventType::SourceDiscovered => self.load_sources().await,
EventType::UserAuthenticated => self.refresh_sources_for_provider(provider_id).await,
EventType::UserLoggedOut => self.remove_sources_for_provider(provider_id).await,
```

## Database Schema

### Sources Table
```sql
CREATE TABLE sources (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    source_type TEXT NOT NULL,  -- 'plex', 'jellyfin', 'local', 'network'
    backend_id TEXT NOT NULL,
    host_url TEXT,
    is_online BOOLEAN DEFAULT true,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

### Key Indexes
- `idx_sources_backend_id` - Fast provider lookups
- `idx_sources_type` - Source type filtering
- `idx_sources_online` - Online status queries

## Reactive Sources Page

### UI Components (`src/platforms/gtk/ui/pages/sources.rs`)

The Sources page provides a Netflix-like interface showing:

#### Source Display
- **Grouped by type**: Plex Servers, Jellyfin Servers, Local Files
- **Connection status**: ✅ Connected, 🔄 Connecting, ❌ Disconnected, ⚠️ Error
- **Friendly names**: Clean display names instead of technical IDs
- **Real-time updates**: Automatic refresh when sources change

#### Interactive Controls
- **Add Account**: Dropdown with Plex/Jellyfin options
- **Remove Account**: Removes auth provider and all associated sources
- **Sync Controls**: Start/stop sync operations per source

### ViewModel Integration (`src/core/viewmodels/sources_view_model.rs`)

Enhanced `SourceInfo` structure for detailed tracking:

```rust
#[derive(Debug, Clone)]
pub struct SourceInfo {
    pub source: Source,
    pub libraries: Vec<Library>,
    pub sync_status: Option<SyncStatus>,
    pub connection_status: ConnectionStatus,
    pub sync_progress: SyncProgressInfo,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ConnectionStatus {
    Connected,
    Connecting,
    Disconnected,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct SyncProgressInfo {
    pub is_syncing: bool,
    pub overall_progress: f32,           // 0.0 to 1.0
    pub current_stage: SyncStage,
    pub stage_progress: f32,
    pub items_processed: usize,
    pub total_items: usize,
    pub estimated_time_remaining: Option<Duration>,
}
```

### Sync Progress Tracking

The Sources page shows detailed sync progress:

#### Library Sync (0-80%)
```rust
SyncStage::LoadingMovies { library_name } => 
    format!("Loading movies from {}", library_name),
SyncStage::LoadingTVShows { library_name } => 
    format!("Loading TV shows from {}", library_name),
```

#### Episode Sync (80-100%)
```rust
SyncStage::LoadingEpisodes { show_name, season, current, total } => 
    format!("Loading episodes from {} S{:02} ({}/{})", 
            show_name, season, current, total),
```

## Recent Fixes and Improvements

### Source Persistence Fix (2025-09-05)
**Problem**: Plex sources not appearing in Sources page
**Solution**: Added database persistence to `SourceCoordinator.add_plex_account()`

```rust
// OLD: Sources only cached, not persisted
config.set_cached_sources(provider_id, sources.clone());

// NEW: Sources cached AND persisted to database
config.set_cached_sources(provider_id, sources.clone());
self.data_service.sync_sources_to_database(&provider_id, &sources).await?;
```

### Jellyfin ID Stability Fix (2025-09-05)
**Problem**: Random UUID generation caused source archival
**Solution**: Deterministic ID generation based on server URL + username

```rust
// Stable, deterministic ID generation
fn generate_stable_jellyfin_id(&self, server_url: &str, username: &str) -> String {
    let input = format!("{}:{}", server_url.trim_end_matches('/'), username);
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    let hash = result[..8].iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();
    format!("jellyfin_{}", hash)
}
```

### Sync Progress Accuracy (2025-09-05)
**Problem**: Sync showed 100% but episodes continued downloading
**Solution**: Two-phase progress calculation (80% libraries + 20% episodes)

### UI Improvements (2025-09-05)
**Enhancements**:
- Fixed remove functionality to remove auth providers instead of individual sources
- Added Plex/Jellyfin dropdown for account selection
- Improved progress bar alignment and visual indicators
- Enhanced account removal with proper auth provider cleanup

## Event System Integration

### Source-Related Events
```rust
EventType::SourceDiscovered,    // New source found
EventType::SourceUpdated,       // Source metadata changed
EventType::SourceRemoved,       // Source no longer available
EventType::SourceStatusChanged, // Connection status change
EventType::UserAuthenticated,   // New auth provider added
EventType::UserLoggedOut,       // Auth provider removed
```

### Event Flow
```
Authentication → Source Discovery → Database Persistence → Event Emission → ViewModel Update → UI Refresh
```

## Storage Strategy

### Three-Tier Architecture
1. **SQLite Database** - Primary source of truth (persistent, searchable)
2. **Memory Cache** - Fast access for UI operations (LRU cache)
3. **Config Cache** - Bootstrap cache for offline scenarios only

### Data Flow
```
Discovery → Cache → Database → Repository → Service → Event → ViewModel → UI
```

## Best Practices

### Source Creation
- Only create sources during authentication via `SourceCoordinator`
- Never create sources as side effects of sync operations
- Use proper source types: `plex`, `jellyfin`, `local`, `network`
- Ensure stable ID generation for consistency

### Error Handling
- Validate source existence before sync operations
- Provide clear error messages with context
- Handle connection failures gracefully
- Never silently fail source operations

### UI Updates
- Use reactive ViewModels for all UI updates
- Subscribe to Properties for automatic change notifications
- Avoid direct database access from UI layer
- Handle loading states and error conditions

## Troubleshooting

### Common Issues

#### Sources Not Appearing
- Check if authentication completed successfully
- Verify database persistence in `SourceCoordinator`
- Ensure reactive ViewModel updates are working
- Check event system for proper event emission

#### Duplicate Sources
- Verify ID generation is deterministic
- Check upsert logic in database operations
- Ensure cleanup operations are working
- Look for multiple authentication attempts

#### Sync Progress Issues
- Verify two-phase progress calculation (libraries + episodes)
- Check event emission during sync stages
- Ensure progress values are clamped (0.0-1.0)
- Monitor SyncStage transitions

### Debug Information
Enable debug logging for source management:
```rust
RUST_LOG=reel::services::source_coordinator=debug,reel::services::auth_manager=debug cargo run
```

This will show source discovery, persistence, and lifecycle events in detail.