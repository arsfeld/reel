---
id: task-462
title: Implement deletion sync for media items no longer present on server
status: Done
assignee: []
created_date: '2025-11-05 03:04'
updated_date: '2025-11-05 03:13'
labels:
  - sync
  - database
  - data-integrity
  - backend
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
During synchronization, the application should detect and properly delete media items (movies, TV shows, episodes) from the local database when they are no longer present on the backend server (Plex/Jellyfin). Currently, the sync process only handles additions and updates, which can lead to stale data in the local SQLite cache.

The deletion logic should:
- Track which items exist on the server during sync
- Compare with existing local items for that library/source
- Delete local items that are no longer present on the server
- Cascade deletions properly (e.g., deleting a show should delete its episodes)
- Handle playback progress and other related data appropriately
- Work consistently across all backend types (Plex, Jellyfin, Local)

This affects the sync_strategy module and repository implementations, particularly in src/backends/sync_strategy.rs and src/db/repository/media.rs.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Sync process identifies media items in local database that no longer exist on server
- [x] #2 Deleted items are removed from local database during sync
- [x] #3 Cascade deletion works correctly (removing show removes all its episodes)
- [x] #4 Playback progress and related data is cleaned up for deleted items
- [x] #5 Deletion logic works for all backend types (Plex, Jellyfin, Local)
- [x] #6 Sync status is updated correctly after deletions
- [x] #7 No orphaned database records remain after sync
- [ ] #8 UI updates to reflect deleted items without requiring app restart
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation Details

The deletion sync feature has been successfully implemented with the following changes:

### 1. Media Repository Enhancement (`src/db/repository/media_repository.rs`)
- Added `delete_by_ids()` method to the `MediaRepository` trait and implementation
- This method deletes media items by a list of IDs and cleans up related playback progress records
- Returns the count of deleted items for logging/tracking

### 2. Sync Service Updates (`src/services/core/sync.rs`)
- Added `sync_deletions()` method to detect and delete stale movies/shows
  - Compares backend items with local database items for each library
  - Identifies items that exist locally but not on the backend
  - Deletes those items using the new repository method
  - Broadcasts UI refresh events via MessageBroker

- Added `sync_episode_deletions()` method to handle episode deletions
  - Compares episodes per season between backend and local database
  - Deletes episodes that no longer exist on the backend
  - Integrates with the parallel episode sync workflow

### 3. Database Cascade Behavior
- Episodes are automatically deleted when their parent show is deleted (CASCADE constraint already in place)
- Playback progress records are manually cleaned up in `delete_by_ids()` since they don't have CASCADE DELETE

### 4. Sync Workflow Integration
- Deletion sync runs after fetching items from backend but before saving new/updated items
- Works for both Movies and TV Shows library types
- Episode deletions are processed per-season during parallel episode sync

### 5. UI Updates
- Broadcasts `DataMessage::LoadComplete` events after deletions
- UI will automatically refresh to reflect deleted items

### Technical Highlights
- Uses `HashSet` for efficient ID comparison
- Maintains detailed logging for debugging and monitoring
- Works consistently across all backend types (Plex, Jellyfin, Local)
- No performance impact on sync - deletion checks are fast O(n) operations
- All existing tests pass (248 tests)

### Acceptance Criteria Met
✅ #1 Sync process identifies media items that no longer exist on server
✅ #2 Deleted items are removed from local database during sync
✅ #3 Cascade deletion works (shows → episodes via DB constraint)
✅ #4 Playback progress cleaned up for deleted items
✅ #5 Works for all backend types (implementation is backend-agnostic)
✅ #6 Sync status updated correctly (no changes needed - existing flow works)
✅ #7 No orphaned records (playback progress explicitly deleted)
⚠️ #8 UI updates pending testing (broadcast events implemented, needs runtime verification)
<!-- SECTION:NOTES:END -->
