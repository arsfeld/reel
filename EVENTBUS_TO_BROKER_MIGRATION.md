# EventBus to MessageBroker Migration Analysis

## Current State

### EventBus is Still Actively Used Throughout the Codebase

The EventBus system is **NOT** deprecated yet - it's still the primary event system. The MessageBroker exists but is **completely unused**.

## Where Events Are Published

### 1. Repository Layer (Most Active Publisher)
All repository operations publish events via EventBus:

- **SourceRepository** (`src/db/repository/source_repository.rs`)
  - `SourceAdded` - when source is created
  - `SourceUpdated` - when source is modified
  - `SourceRemoved` - when source is deleted
  - `SourceOnlineStatusChanged` - when online status changes

- **LibraryRepository** (`src/db/repository/library_repository.rs`)
  - `LibraryDeleted` - when library is deleted
  - `LibraryItemCountChanged` - when item count updates

- **MediaRepository** (`src/db/repository/media_repository.rs`)
  - `MediaCreated` - when media item is created
  - `MediaUpdated` - when media item is updated
  - `MediaDeleted` - when media item is deleted
  - `MediaBatchCreated` - when batch of media is created

- **PlaybackRepository** (`src/db/repository/playback_repository.rs`)
  - Likely publishes playback events (need to check)

- **SyncRepository** (`src/db/repository/sync_repository.rs`)
  - Sync-related events

### 2. ViewModels (Heavy EventBus Usage)
All ViewModels use EventBus for state changes:

- **SourcesViewModel** - source management events
- **PreferencesViewModel** - preference change events
- **PlayerViewModel** - playback events
- **NavigationViewModel** - navigation events
- **AuthenticationViewModel** - auth events
- **LibraryViewModel** - library events
- **SidebarViewModel** - sidebar state events
- **HomeViewModel** - home page events

### 3. GTK UI Components
- **Sidebar Widget** - publishes navigation events
- **Main Window** - subscribes to various events
- **Pages** (Home, Library, MovieDetails, Player, Sources) - subscribe to events
- **Auth Dialog** - auth events
- **Preferences Window** - preference events

### 4. Database Layer
- **Database Connection** (`src/db/connection.rs`)
  - `DatabaseMigrated` event after migrations

### 5. Backend Implementations
- **Plex** (`src/backends/plex/mod.rs`)
- **Jellyfin** (`src/backends/jellyfin/mod.rs`)
- **Local** (`src/backends/local/mod.rs`)

## Critical Finding: SyncService Bypasses Events Entirely!

The SyncService is creating repositories with `new_without_events()`:
```rust
let repo = SyncRepositoryImpl::new_without_events(db.clone());
```

This means **sync operations generate NO events**, causing:
- No UI updates during sync
- No progress reporting
- Complete UI/data disconnection

## MessageBroker Status

### What's Done ✅
1. BROKER static is initialized
2. Helper methods added for common patterns
3. Logging/debugging added
4. Service bridge created for conversions
5. New message types defined (DataMessage, SourceMessage, etc.)

### What's NOT Done ❌
1. **Zero components subscribe to MessageBroker**
2. **No events are published to MessageBroker**
3. **SyncService doesn't use ANY event system**
4. **Repositories still use EventBus exclusively**
5. **ViewModels still use EventBus exclusively**

## Migration Path

### Phase 1: Fix SyncService (CRITICAL)
1. Stop using `new_without_events()`
2. Add MessageBroker integration to SyncService
3. Publish sync events to BOTH systems during transition

### Phase 2: Repository Integration
1. Add MessageBroker to repositories alongside EventBus
2. Publish to both systems temporarily
3. Repositories are the main event source - fixing them fixes most issues

### Phase 3: UI Component Subscription
1. Subscribe Relm4 components to MessageBroker
2. Keep GTK components on EventBus for now
3. Gradually migrate GTK components

### Phase 4: Remove EventBus
1. Once all publishers use MessageBroker
2. Once all subscribers use MessageBroker
3. Remove EventBus completely

## Current Event Flow

```
Repository Operation
    ↓
EventBus.publish()  [if not using new_without_events]
    ↓
ViewModels subscribe & update
    ↓
UI components bind to ViewModel properties
    ↓
UI updates
```

## Problem: SyncService Flow is Broken

```
SyncService Operation
    ↓
new_without_events()  [NO EVENTS!]
    ↓
Database updates
    ↓
❌ No events published
❌ No ViewModels notified
❌ No UI updates
```

## Recommendations

1. **IMMEDIATE**: Fix SyncService to use events
2. **HIGH PRIORITY**: Integrate MessageBroker into repositories
3. **MEDIUM**: Subscribe UI components to MessageBroker
4. **LOW**: Gradually phase out EventBus

The sync UI issues are directly caused by SyncService bypassing the event system entirely. This must be fixed first.