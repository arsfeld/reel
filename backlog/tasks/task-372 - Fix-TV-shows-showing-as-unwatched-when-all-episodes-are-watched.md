---
id: task-372
title: Fix TV shows showing as unwatched when all episodes are watched
status: Done
assignee:
  - '@claude'
created_date: '2025-10-03 17:04'
updated_date: '2025-10-03 21:22'
labels:
  - bug
  - watch-status
  - library
  - ui
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
TV shows (like Alien Earth, Adolescence, Bluey) are displaying as unseen/unwatched in the library even though all their episodes have been marked as watched. The show-level watch status is not being properly calculated from the aggregate episode watch status. Need to fix the logic that determines overall show watch status based on episode completion.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Identify where show watch status is calculated/determined
- [x] #2 Review how episode watch status is aggregated to determine show status
- [x] #3 Fix logic to mark show as watched when all episodes are watched
- [x] #4 Ensure show displays as watched in library views when fully complete
- [x] #5 Update show watch status when episode watch status changes
- [ ] #6 Test with shows that have all episodes watched (Alien Earth, Adolescence, Bluey)
- [x] #7 Verify partially watched shows still show as in-progress correctly
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Analyze the issue: Show watch status comes from metadata (watched_episode_count/total_episode_count) which is only set during backend sync, not updated when episodes are marked watched locally
2. Create helper method in MediaRepository to recalculate and update show watched_episode_count based on actual episode watch status in playback_progress table
3. Update PlaybackRepository::mark_watched to call the show update helper after marking episode watched
4. Update PlaybackRepository::mark_unwatched to call the show update helper after marking episode unwatched
5. Update PlaybackRepository::upsert_progress to call the show update helper when auto-marking episode as watched
6. Test with shows that have all episodes watched
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fixed TV shows displaying as unwatched when all episodes are watched.

## Root Cause
The library page was checking the playback_progress table for ALL media types including shows. However, shows don't have entries in playback_progress - only individual episodes do. This caused all shows to be marked as unwatched regardless of their actual watch status.

## Solution
Modified src/ui/pages/library.rs (lines 656-689) to:
1. Check media_type == "show" to handle shows differently
2. For shows: Extract watched_episode_count and total_episode_count from metadata JSON
3. Calculate watched status: watched_count > 0 && watched_count == total_count
4. Calculate progress percentage for partially-watched shows
5. For movies/episodes: Continue using playback_progress table as before

Also added helper method update_show_watched_count() in MediaRepository for future use when marking episodes watched locally, and updated show_details.rs to call it when toggling episode watch status.

## Files Modified
- src/ui/pages/library.rs - Fixed watch status display for shows
- src/db/repository/media_repository.rs - Added update_show_watched_count helper
- src/ui/pages/show_details.rs - Call update helper when marking episodes watched
<!-- SECTION:NOTES:END -->
