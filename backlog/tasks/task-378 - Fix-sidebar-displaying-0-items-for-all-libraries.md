---
id: task-378
title: Fix sidebar displaying 0 items for all libraries
status: Done
assignee:
  - '@claude'
created_date: '2025-10-03 17:32'
updated_date: '2025-10-03 17:43'
labels:
  - bug
  - regression
  - sidebar
  - ui
dependencies: []
priority: high
---

## Description

The sidebar is now showing 0 items for every library when it previously displayed the correct item counts. This appears to be a regression that broke the item count retrieval or display logic in the sidebar. Need to identify what changed and restore the correct item count display for libraries.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Identify where sidebar retrieves library item counts
- [x] #2 Determine what changed that caused item counts to show as 0
- [x] #3 Review database query for library item counts
- [x] #4 Fix the broken item count retrieval logic
- [x] #5 Verify item counts display correctly in sidebar for all libraries
- [x] #6 Test with both movie and TV libraries to confirm counts are accurate
<!-- AC:END -->


## Implementation Plan

1. Identify root cause: The sidebar shows 0 items because TV Shows and Movies libraries are failing to sync
2. Fix the media_people UNIQUE constraint error by using NotSet for auto-increment ID field
3. Test sync to verify all libraries sync correctly
4. Verify sidebar displays correct item counts


## Implementation Notes

Fixed the media_people UNIQUE constraint violation that was preventing TV Shows and Movies libraries from syncing.

Root cause: The save_media_people function was using Set(p.id) with ID=0 for all records, causing UNIQUE constraint violations on the auto-increment primary key.

Solution: Changed to use NotSet for the id field in src/db/repository/people_repository.rs:173, allowing the database to auto-generate unique IDs.

Added comprehensive debug logging for people operations:
- Cast/crew save operations in media service
- People upsert batch operations
- Media_people relationship insert/delete operations

Verified existing database has no corrupted data (2417 unique records). The fix allows all libraries to sync successfully and sidebar now displays correct item counts.
