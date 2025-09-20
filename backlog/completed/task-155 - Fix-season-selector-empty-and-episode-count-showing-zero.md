---
id: task-155
title: Fix season selector empty and episode count showing zero
status: Done
assignee:
  - '@claude'
created_date: '2025-09-17 15:45'
updated_date: '2025-09-17 18:41'
labels:
  - bug
  - ui
  - frontend
dependencies: []
priority: high
---

## Description

The season selector dropdown in the TV show details page always shows '(None)' by default and is empty, preventing users from selecting different seasons. Additionally, the episode count always displays '0 episodes' regardless of the actual number of episodes in the show. These bugs prevent proper navigation and display of TV show content.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Debug why seasons are not populating in the dropdown selector
- [x] #2 Ensure seasons are properly fetched from backend and stored
- [x] #3 Fix season dropdown to show available seasons with proper labels (Season 1, Season 2, etc.)
- [x] #4 Set first season as default selection when page loads
- [x] #5 Fix episode count calculation to show correct number
- [x] #6 Update episode count when switching between seasons
- [x] #7 Handle special seasons (Season 0/Specials) appropriately
- [x] #8 Test with shows having multiple seasons and varying episode counts
<!-- AC:END -->


## Implementation Plan

1. Examine the show_details.rs component to understand season/episode handling
2. Check how seasons are fetched from backends and stored in database
3. Debug the season dropdown selector implementation
4. Fix the data flow from backend to UI for seasons
5. Fix episode count calculation logic
6. Test with multiple shows with varying season structures


## Implementation Notes

Found root cause: Shows fetched from Plex hub APIs (recently added, etc.) were stored with empty seasons array.

Implemented fix:
1. Added debug logging to track seasons data when loading shows
2. Created FetchSeasonsFromBackend command to retrieve seasons when missing
3. Added BackendService::get_backend_for_item() to create backend for a specific media item
4. Modified show_details.rs to detect empty seasons and fetch from backend dynamically
5. Falls back to "Season 1" placeholder if backend fetch fails
6. Updates database with fetched seasons data for future loads

The fix ensures seasons are populated either from cache or fetched on-demand from the backend.

Additional fixes:
- Episode count now calculated from sum of all seasons when fetched
- Season 0 displayed as "Specials" instead of "Season 0" for better UX
- Episode count updates correctly from fetched season data

Summary:
Successfully fixed the TV show details page issues where season selector was empty and episode count showed zero. The root cause was that shows loaded from Plex hub APIs (recently added, etc.) were stored without season metadata. Implemented dynamic fetching of seasons when missing, with proper fallbacks and database updates for caching.
