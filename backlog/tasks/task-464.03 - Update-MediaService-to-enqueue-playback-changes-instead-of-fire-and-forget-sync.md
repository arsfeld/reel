---
id: task-464.03
title: >-
  Update MediaService to enqueue playback changes instead of fire-and-forget
  sync
status: Done
assignee: []
created_date: '2025-11-22 20:10'
updated_date: '2025-11-22 20:40'
labels:
  - service
  - refactor
  - sync
dependencies: []
parent_task_id: task-464
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Refactor the MediaService to enqueue playback progress and watch status changes in the sync queue instead of using fire-and-forget `tokio::spawn()` tasks.

**Current Fire-and-Forget Locations**:
- `src/services/core/media.rs:837` - `update_playback_progress`
- `src/services/core/media.rs:912` - `mark_as_watched`

**Changes Required**:
1. Add `PlaybackSyncRepository` to MediaService dependencies
2. Replace `tokio::spawn()` with queue enqueue operations
3. Update local database immediately (optimistic update)
4. Enqueue sync operation for background processing
5. Return success immediately to UI (don't block on sync)

**New Flow**:
```rust
// Old: Fire and forget
tokio::spawn(async move {
    backend.update_progress(...).await?;
});

// New: Queue for reliable sync
playback_sync_repo.enqueue_change(SyncChange {
    media_item_id,
    source_id,
    change_type: ChangeType::ProgressUpdate,
    position_ms,
    ...
})?;
```

**Additional Changes**:
- Add proper error handling for queue operations
- Add logging for enqueue operations
- Update existing tests to verify queue usage
- Add integration tests for the new flow
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 MediaService uses PlaybackSyncRepository for all playback changes
- [x] #2 Fire-and-forget tokio::spawn calls are removed
- [x] #3 Local database is updated immediately (optimistic)
- [x] #4 Changes are enqueued for background sync
- [x] #5 Error handling is implemented for queue operations
- [x] #6 Existing tests are updated to verify queue usage
- [ ] #7 Integration tests cover the new sync flow
<!-- AC:END -->
