# Backend Architecture: Consolidated Source Management

## Executive Summary

The backend architecture has been successfully consolidated around SourceCoordinator as the single interface for all backend operations. The migration from a dual-system architecture (BackendManager + SourceCoordinator) to a unified system is complete, eliminating architectural confusion and establishing clear ownership patterns.

## Current Architecture (Post-Migration)

### Unified System: SourceCoordinator

**SourceCoordinator** (`src/services/source_coordinator.rs`) is now the single interface for all backend operations:
- Orchestrates AuthManager, BackendManager, CacheManager, and SyncManager
- Manages source lifecycle (discovery, initialization, removal)
- Tracks connection status for each source
- Provides offline-first initialization
- Encapsulates BackendManager as an internal implementation detail

## Key Components

### SourceCoordinator (Primary Interface)

**Location**: `src/services/source_coordinator.rs`

**Responsibilities**:
- Single interface for all backend operations
- Coordinates AuthManager, BackendManager, CacheManager, and SyncManager
- Handles authentication flow for new sources
- Manages source status tracking (Connected/Offline/NeedsAuth/Error)
- Provides offline-first initialization
- Encapsulates backend lifecycle management

**Key Methods**:
- `add_plex_account()` - Add Plex account and discover servers
- `add_jellyfin_source()` - Add Jellyfin server
- `initialize_all_sources()` - Offline-first startup initialization
- `sync_source()` - Sync specific source
- `get_backend()` - Get backend by ID (explicit backend selection)
- `get_source_status()` - Get connection status

### BackendManager (Internal)

**Location**: `src/backends/mod.rs`

**Status**: Now an internal implementation detail of SourceCoordinator
- Not directly accessible from UI or AppState
- Manages backend registry and ordering
- All access routed through SourceCoordinator

### No Active Backend Concept

**Critical Design Decision**: The application no longer assumes an "active" or "default" backend:
- Every operation explicitly specifies backend_id
- Media items store their backend_id
- UI tracks which backend each view uses
- Multiple backends coexist without conflict

## Resolved Issues (Migration Complete)

All critical issues have been resolved through the consolidation:

### ✅ Single SyncManager Implementation
- Removed stub SyncManager from `src/backends/mod.rs`
- Only the full implementation in `src/services/sync.rs` remains
- Consistent sync behavior across the application

### ✅ Unified Backend Registration
- All backend registration goes through SourceCoordinator
- Single source of truth for backend lifecycle
- Proper initialization and status tracking guaranteed

### ✅ Consistent Access Pattern
```rust
// All access now through SourceCoordinator
let backend = state.source_coordinator.get_backend(backend_id).await;
let status = state.source_coordinator.get_source_status(backend_id).await;
```

### ✅ Unified State Management
- SourceCoordinator manages all backend state
- Connection status, ordering, and metadata in one place
- BackendManager is now internal to SourceCoordinator

### ✅ No Circular Dependencies
- SourceCoordinator constructor takes only required services
- No back-reference to AppState
- Single-phase initialization
- SourceCoordinator is non-optional in AppState

## Architecture Hierarchy

The consolidated architecture follows a clear, unidirectional flow:

```
UI Layer
    ↓
AppState
    ↓
SourceCoordinator ← AuthManager
    ↓               ↓
BackendManager   CacheManager
    ↓               ↓
Backends        SyncManager
```

### Key Principles

1. **Single Interface**: All backend operations go through SourceCoordinator
2. **Explicit Backend Selection**: Every operation specifies backend_id
3. **No Default Backend**: No assumptions about "active" or "first" backend
4. **Offline-First**: Cache loads immediately, background refresh follows
5. **Clean Dependencies**: No circular references, single-phase initialization

## Migration Status: ✅ COMPLETE

The backend architecture consolidation has been successfully completed through the following phases:

### Completed Migration Phases

1. **Circular Dependency Resolution**: Removed AppState dependency from SourceCoordinator
2. **Method Consolidation**: Added all necessary methods to SourceCoordinator
3. **UI Code Update**: Replaced all direct BackendManager access
4. **AppState Refactoring**: Made BackendManager private, routed through SourceCoordinator
5. **Code Cleanup**: Removed stub implementations and deprecated concepts
6. **Backend ID Integration**: Added backend_id to all media items for explicit backend tracking

### Production-Ready State

The architecture is now production-ready with:
- **Single Interface**: SourceCoordinator handles all backend operations
- **Explicit Backend Selection**: No assumptions about default backends
- **Clean Dependencies**: No circular references or optional wrappers
- **Type Safety**: Simpler types without Option layers
- **Offline-First**: Instant cache loading with background refresh

## Benefits Achieved

1. **Single Source of Truth**: All backend operations through SourceCoordinator
2. **Consistent State**: No duplicate or conflicting state representations
3. **Clear Architecture**: Obvious hierarchy and responsibilities
4. **Reduced Maintenance**: No code duplication, clear boundaries
5. **Better Testability**: Single interface to mock/test
6. **No Optional Checks**: SourceCoordinator always exists
7. **Type Safety**: Simpler types without Option wrapper
8. **Predictable Initialization**: Single-phase startup without race conditions
9. **Multi-Source Support**: Proper handling of multiple backends simultaneously

## Implementation Guidelines

### Backend Selection
Every operation must explicitly specify which backend to use:
- Pass `backend_id` through the call chain
- Store `backend_id` with cached/persisted data
- Display backend information in UI
- Never assume a default backend

### Adding New Backends
To add support for a new media source:
1. Implement the `MediaBackend` trait
2. Add new `AuthProvider` variant
3. Update `SourceCoordinator::create_and_register_backend()`
4. Backend will automatically integrate with existing infrastructure

### Offline-First Pattern
1. Load cached data immediately on startup
2. Display with "Offline" status
3. Trigger background refresh
4. Update UI when connection established

## Conclusion

The backend architecture consolidation is complete and production-ready. The system now provides a clean, maintainable foundation for multi-source media management with proper separation of concerns and no architectural ambiguity.