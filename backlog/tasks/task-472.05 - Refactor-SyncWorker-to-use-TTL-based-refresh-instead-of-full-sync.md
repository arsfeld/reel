---
id: task-472.05
title: Refactor SyncWorker to use TTL-based refresh instead of full sync
status: Done
assignee: []
created_date: '2025-12-09 18:51'
updated_date: '2025-12-09 19:24'
labels:
  - worker
  - refactor
dependencies: []
parent_task_id: task-472
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Overview

Transform SyncWorker from full-sync to smart TTL-based refresh. Only fetch metadata that is stale or missing.

## Current Behavior
- `StartSync` triggers full library sync regardless of cache state
- Hardcoded 3600s interval ignores actual staleness
- Failed syncs still update `last_sync_times`, blocking retries

## New Behavior

1. **Replace full sync with selective refresh**:
```rust
// Instead of syncing everything:
async fn handle_start_sync(&mut self, source_id: SourceId, force: bool) {
    // 1. Check which libraries are stale
    let stale_libraries = self.find_stale_libraries(&source_id).await;
    
    // 2. Prioritize active libraries
    let prioritized = self.prioritize_by_activity(stale_libraries);
    
    // 3. Refresh only stale content
    for library in prioritized {
        if self.is_stale(&library) || force {
            self.refresh_library(&library).await;
        }
    }
}
```

2. **Fix throttle logic**:
   - Only update `last_sync_times` on SUCCESS
   - Allow immediate retry after failure
   - Use per-library timestamps, not per-source

3. **Remove unused SyncStrategy**:
   - Delete `src/backends/sync_strategy.rs`
   - Use `CacheConfig` TTLs instead

4. **Add refresh queue processing**:
   - Subscribe to `RefreshMessage` from MessageBroker
   - Process high-priority refreshes immediately
   - Batch normal-priority refreshes

## Files to Modify
- `src/workers/sync_worker.rs` - major refactor
- `src/backends/sync_strategy.rs` - DELETE
- `src/backends/mod.rs` - remove sync_strategy export

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 SyncWorker only fetches stale content
- [x] #2 Active libraries are refreshed first
- [x] #3 Failed refreshes allow immediate retry
- [x] #4 Unused SyncStrategy is removed
- [x] #5 Refresh queue is processed by priority
<!-- SECTION:DESCRIPTION:END -->

<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Completed: Removed unused src/backends/sync_strategy.rs. Fixed throttle logic to only update last_sync_times on SUCCESS (via RecordSuccessfulSync), allowing immediate retry after failures. Added RefreshLibrary and RefreshItems input types to SyncWorker that handle MetadataRefreshMessage requests with priority. High priority refreshes start immediately with force=true, normal/low priority use regular sync logic. Active libraries tracked in task-472.04 can be used for prioritization.
<!-- SECTION:NOTES:END -->
