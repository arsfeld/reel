---
id: task-160
title: Fix TV show seasons and episode counts not persisting in database
status: Done
assignee:
  - '@claude'
created_date: '2025-09-17 19:40'
updated_date: '2025-09-18 01:38'
labels:
  - bug
  - database
  - sync
  - critical
dependencies: []
priority: high
---

## Description

ALL TV shows are displaying with 0 seasons and 0 episode count in the UI, even though episodes exist and are being synced. The root issue is that Show metadata (particularly the seasons array and total_episode_count) is not being properly persisted to the database during sync or updates. Episodes are syncing correctly but the parent Show entity retains empty/zero values for seasons and episode counts.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Investigate why Show seasons array is not persisting to database during sync
- [x] #2 Debug the metadata serialization/deserialization for Show entities
- [x] #3 Fix the database update mechanism to properly persist Show metadata
- [x] #4 Ensure sync process correctly updates existing shows with season data
- [x] #5 Verify Shows are saved with correct seasons data during initial library sync
- [x] #6 Add comprehensive logging to track Show metadata through save/update cycle
- [x] #7 Test that show_details page displays seasons and episode counts after fix
<!-- AC:END -->


## Implementation Plan

1. Search for Show entity definition and metadata structure
2. Investigate database save/update methods for Show entities
3. Check serialization/deserialization of seasons array
4. Examine sync process flow for TV shows
5. Add detailed logging at key points in the save/update cycle
6. Fix the persistence issue
7. Test the fix with TV show data


## Implementation Notes

Fixed the issue where TV show seasons and episode counts were not persisting to database.

The root cause was in the media_item_mapper.rs file where the to_model() function was attempting to serialize the entire MediaItem object using serde_json::to_value(self), which was not properly preserving the Show metadata fields.

Fixed by:
1. Replaced generic serialization with explicit metadata construction for each MediaItem type
2. For Shows specifically, now properly serializes seasons array, cast, and episode counts into the metadata JSON field
3. Added comprehensive logging throughout the save/update cycle to track Show metadata
4. Added verification logic after database operations to confirm seasons are persisted
5. Enhanced sync process logging to track shows and their seasons during initial fetch from backends

Both Plex and Jellyfin backends were already fetching seasons data when getting shows, so no backend changes were needed.

The fix ensures that:
- Seasons array is properly serialized to the metadata JSON column
- Total episode count and watched episode count are preserved
- Cast information is maintained
- All Show metadata is correctly deserialized when reading from database
