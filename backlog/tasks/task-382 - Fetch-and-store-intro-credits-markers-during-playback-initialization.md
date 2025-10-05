---
id: task-382
title: Fetch and store intro/credits markers during playback initialization
status: In Progress
assignee:
  - '@assistant'
created_date: '2025-10-03 18:08'
updated_date: '2025-10-05 21:36'
labels:
  - player
  - backend
  - markers
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
When starting playback, fetch intro and credits markers from the backend API and store them in the database for future use. This avoids fetching during sync (performance) while ensuring markers are available when needed
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Player initialization calls backend.fetch_markers() for Plex using rating_key
- [ ] #2 Player initialization calls backend.get_media_segments() for Jellyfin using item_id
- [ ] #3 Fetched markers are stored in database via repository update
- [ ] #4 Markers loaded from database when available, only fetch from API if missing
- [ ] #5 Error handling for marker fetch failures (graceful degradation)
- [ ] #6 Both MPV and GStreamer player backends support marker fetching
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add fetch_markers() method to MediaBackend trait
2. Implement fetch_markers() for PlexBackend (using existing fetch_episode_markers)
3. Implement fetch_markers() for JellyfinBackend (using existing get_media_segments)
4. Add update_markers() method to MediaRepository
5. Modify player initialization to check DB markers first
6. If markers missing, fetch from backend and store in DB
7. Add error handling with graceful degradation
8. Test with both MPV and GStreamer players
<!-- SECTION:PLAN:END -->
