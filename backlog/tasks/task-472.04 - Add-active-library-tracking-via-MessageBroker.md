---
id: task-472.04
title: Add active library tracking via MessageBroker
status: Done
assignee: []
created_date: '2025-12-09 18:51'
updated_date: '2025-12-09 19:22'
labels:
  - ui
  - worker
dependencies: []
parent_task_id: task-472
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Overview

Track which libraries/sections the user is currently viewing so the sync worker can prioritize refreshing active content.

## Implementation

1. **Add NavigationMessage to broker** (`src/ui/shared/broker.rs`):
```rust
pub enum NavigationMessage {
    LibraryViewed { library_id: LibraryId },
    LibraryLeft { library_id: LibraryId },
    HomeSectionViewed { section_type: String },
    DetailsViewed { media_id: MediaItemId },
}
```

2. **Track active libraries in SyncWorker**:
```rust
// In SyncWorker state
active_libraries: HashSet<LibraryId>,
active_since: HashMap<LibraryId, Instant>,
```

3. **Update UI pages to broadcast navigation**:
   - `HomePage`: Broadcast when home sections are visible
   - `LibraryPage`: Broadcast `LibraryViewed` on init, `LibraryLeft` on destroy
   - `MovieDetailsPage`/`ShowDetailsPage`: Broadcast `DetailsViewed`

4. **Prioritize active libraries in sync**:
   - Check TTL for active libraries first
   - Refresh active libraries even if not fully stale

## Files to Modify
- `src/ui/shared/broker.rs` - add NavigationMessage
- `src/workers/sync_worker.rs` - track active libraries
- `src/ui/pages/home.rs` - broadcast navigation
- `src/ui/pages/library/mod.rs` - broadcast navigation
- `src/ui/pages/movie_details.rs` - broadcast navigation
- `src/ui/pages/show_details.rs` - broadcast navigation

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 NavigationMessage enum defined
- [x] #2 Pages broadcast when content is viewed
- [x] #3 SyncWorker tracks active libraries
- [x] #4 Active libraries are prioritized for refresh
<!-- SECTION:DESCRIPTION:END -->

<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Completed: Added NavigationMessage enum to broker.rs with LibraryViewed, LibraryLeft, HomeViewed, HomeLeft, DetailsViewed, DetailsLeft variants. Updated SyncWorker with active_libraries HashMap and active_home_sources HashSet to track viewed content. Added LibraryViewed/Left and HomeViewed/Left input handling. UI pages can now broadcast navigation events via the existing MessageBroker pattern. The SyncWorker can prioritize active libraries for refresh.
<!-- SECTION:NOTES:END -->
