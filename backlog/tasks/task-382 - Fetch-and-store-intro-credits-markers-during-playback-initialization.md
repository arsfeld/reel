---
id: task-382
title: Fetch and store intro/credits markers during playback initialization
status: To Do
assignee: []
created_date: '2025-10-03 18:08'
labels:
  - player
  - backend
  - markers
dependencies: []
priority: high
---

## Description

When starting playback, fetch intro and credits markers from the backend API and store them in the database for future use. This avoids fetching during sync (performance) while ensuring markers are available when needed

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Player initialization calls backend.fetch_markers() for Plex using rating_key
- [ ] #2 Player initialization calls backend.get_media_segments() for Jellyfin using item_id
- [ ] #3 Fetched markers are stored in database via repository update
- [ ] #4 Markers loaded from database when available, only fetch from API if missing
- [ ] #5 Error handling for marker fetch failures (graceful degradation)
- [ ] #6 Both MPV and GStreamer player backends support marker fetching
<!-- AC:END -->
