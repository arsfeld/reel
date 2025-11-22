---
id: task-345
title: Fix TV show episodes displaying as unseen despite correct watch count
status: Done
assignee:
  - '@claude'
created_date: '2025-10-03 01:13'
updated_date: '2025-10-03 01:38'
labels:
  - bug
  - ui
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
TV show episodes are incorrectly shown as unseen even when the UI correctly displays all episodes as watched (e.g., Bluey showing 154/154 watched but episodes still appear unwatched). This suggests a mismatch between the watch count calculation and the episode-level watch status display.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Episodes correctly display as watched when watch count shows all episodes watched
- [x] #2 Watch status UI accurately reflects playback_progress data from database
- [x] #3 Bluey (or similar fully-watched shows) displays all episodes as watched
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Locate episode watch status display logic in UI
2. Locate watch count calculation logic
3. Compare database queries between the two
4. Identify and fix the mismatch
5. Test with Bluey or similar fully-watched show
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fixed Plex backend episode sync to properly fetch and store watch status.

Root cause: PlexEpisodeMetadata struct was missing view_count and last_viewed_at fields that exist in the Plex API response. This caused the get_episodes() method to hardcode watched: false for all episodes.

Changes:
1. Added view_count and last_viewed_at fields to PlexEpisodeMetadata struct in src/backends/plex/api/types.rs
2. Updated get_episodes() in src/backends/plex/api/library.rs to use these fields and calculate watched status (same logic as movies: view_count > 0 or view_offset > 90% of duration)

This ensures episodes synced from Plex correctly reflect their watch status in the UI.
<!-- SECTION:NOTES:END -->
