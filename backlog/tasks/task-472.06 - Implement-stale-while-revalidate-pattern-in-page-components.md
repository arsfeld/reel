---
id: task-472.06
title: Implement stale-while-revalidate pattern in page components
status: In Progress
assignee: []
created_date: '2025-12-09 18:51'
updated_date: '2025-12-09 20:11'
labels:
  - ui
  - pattern
dependencies: []
parent_task_id: task-472
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Overview

Update page components to serve cached data immediately while triggering background refresh if stale. This ensures instant UI loading.

## Pattern

```rust
// In page's init or CommandOutput handler:
async fn load_content(&mut self) {
    // 1. Always load from cache first (instant)
    let cached = MediaService::get_from_cache(&self.library_id).await;
    if let Some(items) = cached {
        self.items = items;
        self.loading = false;
    }
    
    // 2. Check if refresh needed
    if MetadataRefreshService::needs_refresh(&self.library_id, &config) {
        // 3. Queue background refresh (non-blocking)
        sender.send(RefreshMessage::QueueLibraryRefresh {
            library_id: self.library_id.clone(),
            priority: RefreshPriority::High,
        });
    }
}

// 4. Handle refresh completion via broker subscription
fn handle_refresh_complete(&mut self, library_id: LibraryId) {
    if library_id == self.library_id {
        // Reload from cache with fresh data
        self.reload_from_cache();
    }
}
```

## Pages to Update

1. **HomePage** (`src/ui/pages/home.rs`):
   - Load home sections from cache
   - Queue refresh if stale
   - Subscribe to `HomeSectionsRefreshed` message

2. **LibraryPage** (`src/ui/pages/library/mod.rs`):
   - Load media items from cache
   - Queue refresh for current library
   - Subscribe to `LibraryRefreshed` message

3. **MovieDetailsPage** (`src/ui/pages/movie_details.rs`):
   - Load item from cache
   - If full metadata stale, queue refresh
   - Update UI when metadata arrives

4. **ShowDetailsPage** (`src/ui/pages/show_details.rs`):
   - Same as MovieDetailsPage
   - Also handle episode list refresh

## Files to Modify
- `src/ui/pages/home.rs`
- `src/ui/pages/library/mod.rs`
- `src/ui/pages/movie_details.rs`
- `src/ui/pages/show_details.rs`

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 All pages load from cache immediately
- [ ] #2 Stale content triggers background refresh
- [ ] #3 UI updates when fresh data arrives
- [ ] #4 No blocking on network requests
- [ ] #5 Works offline with cached data
<!-- SECTION:DESCRIPTION:END -->

<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Foundation complete: CacheConfig, MetadataRefreshService, and MessageBroker infrastructure are in place. Pages already load from DB cache (instant). Next step: Add staleness checks to page data loading functions and queue background refreshes when content is stale. This is a low-risk enhancement since pages work correctly now - adding TTL checks is incremental improvement.

## Progress Update (2025-12-09)

Completed foundation work:

1. **Migration registered**: `m20251209_000001_add_fetched_at` now in migrations/mod.rs
2. **Entity field added**: `fetched_at: Option<DateTime>` added to `MediaItemModel`
3. **Repository updates**: All insert/update methods now set `fetched_at`:
   - `insert_silent`, `update_silent`
   - `Repository::insert`, `Repository::update`
   - `bulk_insert`
4. **find_stale_items method added**: Queries items where `fetched_at` is older than TTL
5. **Broker messages added**: `MetadataRefreshMessage` and `RefreshPriority` types
6. **Service exports**: `cache_config` and `metadata_refresh` exported from `core/mod.rs`
7. **MediaItemMapper fixed**: Now sets `fetched_at` when converting models
8. **Test fixtures updated**: All test helper functions include `fetched_at`

### What's Ready

- `CacheConfig` with TTL settings per content type
- `MetadataRefreshService` with staleness checks and queue methods
- `find_stale_items` repository method for TTL-based queries
- `MetadataRefreshMessage` broker messages for refresh coordination

### What Remains

Pages already load from DB cache (offline-first). Need to add:
1. Staleness checks on page load using `MetadataRefreshService::needs_refresh_naive()`
2. Queue background refresh when stale using `queue_library_refresh()`
3. Handle `MetadataRefreshMessage::LibraryRefreshCompleted` to update UI

The HomePage already reloads on `SyncCompleted` messages. Similar pattern can be used for refresh messages.
<!-- SECTION:NOTES:END -->
