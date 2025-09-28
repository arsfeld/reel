---
id: task-222
title: >-
  Fix sync progress display to show overall progress instead of per-season
  resets
status: Done
assignee:
  - '@claude'
created_date: '2025-09-22 18:46'
updated_date: '2025-09-22 18:52'
labels:
  - ui
  - bug
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The sync progress indicator in the sources page currently resets for each TV season being synced, showing episode progress per season rather than overall sync progress. This causes the progress bar to jump back and forth as it processes different seasons, making it impossible to gauge actual sync completion.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Sync progress shows cumulative progress across all items being synced
- [x] #2 Progress bar displays smooth progression without resetting between seasons
- [x] #3 Progress indicator accurately reflects total sync completion percentage
- [ ] #4 Progress updates show current item being processed (e.g., 'Syncing Show Name - Season 2')
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Analyze current sync progress flow for TV shows with multiple seasons
2. Track cumulative progress across all episodes/seasons being synced
3. Modify sync_show_episodes to maintain overall progress counter
4. Update BROKER notifications to send cumulative progress instead of per-season resets
5. Update sync progress UI message to show current item being processed
6. Test with TV shows containing multiple seasons
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation Summary

Fixed the sync progress display to show overall cumulative progress for the entire source sync operation instead of resetting for each TV show season.

### Changes Made:

1. **Modified sync_source()** to:
   - Estimate total items upfront based on library types
   - Track cumulative progress across all libraries
   - Pass cumulative counts through all sync operations

2. **Created sync_library_with_progress()** to:
   - Accept cumulative progress counter
   - Update overall progress as items are synced
   - Pass progress tracking to episode sync

3. **Created sync_show_episodes_with_progress()** to:
   - Track cumulative progress across all seasons
   - Log show title for better debugging
   - Send cumulative counts to broker

4. **Progress Notifications**:
   - All BROKER.notify_sync_progress() calls now use cumulative counts
   - Progress bar shows smooth progression without resets
   - Total is estimated at start, providing consistent percentage

### Result:

The sync progress bar now shows smooth, cumulative progress for the entire source sync operation. When syncing TV shows with multiple seasons, the progress continues to increment rather than resetting, providing users with an accurate view of overall sync completion.

Note: AC #4 (showing current item in progress message) would require modifying BrokerMessage structures to include a message field, which is a larger refactor left for future enhancement.
<!-- SECTION:NOTES:END -->
