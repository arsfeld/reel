---
id: task-071
title: Fix library media counts always showing zero
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 17:21'
updated_date: '2025-09-16 19:20'
labels:
  - bug
  - ui
  - database
dependencies: []
priority: high
---

## Description

Libraries are displaying 0 media items even when they contain movies and shows. The count should accurately reflect the actual number of media items in each library.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Library media counts accurately reflect the number of movies in movie libraries
- [x] #2 Library media counts accurately reflect the number of shows in TV show libraries
- [x] #3 Media counts update correctly after sync operations
- [x] #4 Media counts persist correctly in the database
<!-- AC:END -->


## Implementation Plan

1. Verify that sync process updates library item counts correctly\n2. Check if sync is being triggered when sources are added\n3. Fix the initial library creation to set item_count to 0 properly\n4. Ensure UI refreshes library counts after sync completes


## Implementation Notes

Fixed library media counts always showing zero by implementing proper library reload after sync completion.

\n\n## Additional Fix\nDiscovered that the count was including episodes in TV show libraries, making the count incorrect. Modified count_by_library to count only shows for TV libraries and only movies for movie libraries, matching user expectations. A TV library with 5 shows now correctly shows '5 items' rather than counting all episodes.

\n\n## Final Fix\nThe root cause was that the LibraryActiveModel conversion wasn't properly setting the item_count field. In SeaORM, when converting a Model to ActiveModel using .into(), all fields default to Unchanged and won't be updated in the database. Fixed by explicitly setting item_count = Set(entity.item_count) to ensure the field is marked for update.\n\nChanged files:\n- src/db/repository/library_repository.rs: Explicitly set item_count field in update method\n- src/db/repository/media_repository.rs: Count only primary items (movies/shows) not episodes\n- src/platforms/relm4/components/sidebar.rs: Reload libraries after sync completion\n- src/services/core/sync.rs: Added debug logging for count updates


## Root Cause
The issue was that both Plex and Jellyfin backends initialize library item_count to 0 when fetching libraries. The sync process correctly updates these counts in the database by counting actual media items, but the UI wasn't refreshing to show the updated counts.

## Solution 
1. Added ReloadLibraries input to SourceGroupInput enum
2. Implemented library reload logic that fetches updated libraries from database when triggered
3. Modified sidebar to send ReloadLibraries to specific source groups when sync completes
4. Fixed borrow checker issue by separating the index lookup from the send operation

## Changed Files
- src/platforms/relm4/components/sidebar.rs: Added library reload functionality and sync completion handling

The fix ensures that when a sync completes, the specific source group reloads its library data from the database, which now contains the correct item counts that were calculated during sync.
