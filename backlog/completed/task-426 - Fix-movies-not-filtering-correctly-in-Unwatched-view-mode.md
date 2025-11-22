---
id: task-426
title: Fix movies not filtering correctly in Unwatched view mode
status: Done
assignee:
  - '@claude'
created_date: '2025-10-06 19:37'
updated_date: '2025-10-06 19:38'
labels:
  - ui
  - bug
  - critical
dependencies: []
priority: high
---

## Description

Movies are not being filtered properly in the Unwatched view mode. Both 'All' and 'Unwatched' tabs show the same number of movies, even though some movies are clearly watched (no unseen icon). TV Shows filter correctly. The issue is that playback_progress_map is only fetched when watch_status_filter \!= All, but the Unwatched view mode uses selected_view_mode, so the map is empty for movies when checking is_watched status.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Unwatched view mode shows only unwatched movies
- [x] #2 Watched movies are excluded from Unwatched view
- [x] #3 Movie count in Unwatched view is less than All view when watched movies exist
- [x] #4 TV Shows continue to work correctly
<!-- AC:END -->


## Implementation Plan

1. Analyze AllItemsLoaded handler to understand when playback_progress_map is fetched
2. Identify the bug: map only fetched when watch_status_filter != All
3. Fix: Also fetch map when selected_view_mode == Unwatched
4. Update condition at line 867 to check both watch_status_filter and view_mode
5. Test by viewing Unwatched tab with mixed watched/unwatched movies


## Implementation Notes

Fixed movies not filtering by watched status in Unwatched view mode.

Root Cause:
- playback_progress_map was only fetched when watch_status_filter != All (line 886)
- Unwatched view mode uses selected_view_mode == ViewMode::Unwatched
- For movies, is_watched check requires playback_progress_map to determine status
- When viewing Unwatched tab with watch_status_filter == All, map was empty
- Empty map caused is_watched to always be false for movies
- All movies passed the !is_watched filter, showing watched and unwatched together
- TV shows worked because they use metadata fields, not playback_progress table

Changes:
1. src/ui/pages/library/mod.rs:887-888 - Added OR condition to fetch playback_progress_map when selected_view_mode == Unwatched

Result:
- Unwatched view mode now correctly filters movies by watched status
- Only unwatched movies appear in Unwatched tab
- TV shows continue to work as before
- Watch status filter continues to work independently
