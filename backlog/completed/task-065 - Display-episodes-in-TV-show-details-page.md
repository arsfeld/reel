---
id: task-065
title: Display episodes in TV show details page
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 04:03'
updated_date: '2025-09-16 04:13'
labels:
  - bug
  - ui
  - tv-shows
  - critical
dependencies: []
priority: high
---

## Description

The TV show details page currently does not display episodes properly. Episodes should be shown in a grid or list format, organized by season, allowing users to browse and select episodes for playback. This is a critical feature for TV show navigation and playback.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Investigate current episode display implementation in show_details.rs
- [x] #2 Identify why episodes are not being displayed or loaded
- [x] #3 Implement episode grid/list display with proper layout
- [x] #4 Ensure episodes are grouped and filtered by selected season
- [x] #5 Add episode metadata display (title, episode number, thumbnail, duration)
- [x] #6 Implement episode selection and playback functionality
- [x] #7 Add visual indicators for watched/unwatched episodes
- [x] #8 Test episode display with shows from different backends (Plex/Jellyfin)
- [x] #9 Ensure proper handling of shows with multiple seasons
<!-- AC:END -->


## Implementation Plan

1. Fix GetEpisodesCommand to use MediaRepository properly
2. Add MediaService method for fetching episodes by show and season
3. Update episode card to load thumbnail images properly
4. Test episode display with different shows
5. Ensure episode click handler works correctly


## Implementation Notes

Fixed episode display issues in TV show details page:

1. Fixed GetEpisodesCommand to use proper MediaService method instead of hardcoded library ID
2. Added MediaService::get_episodes_for_show() method that correctly queries episodes from database
3. Fixed Plex backend to ensure show_id is always set for episodes (was sometimes missing)
4. Fixed Jellyfin backend to ensure show_id is always set for episodes
5. Episode grid, playback, and watched indicators were already implemented and working

The core issue was that episodes were not being fetched correctly from the database due to a wrong implementation in GetEpisodesCommand that used a non-existent library ID.
