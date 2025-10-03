---
id: task-388
title: Skip cast/crew loading during initial sync to avoid incomplete data
status: In Progress
assignee:
  - '@claude'
created_date: '2025-10-03 19:13'
updated_date: '2025-10-03 19:15'
labels:
  - backend
  - sync
  - cast-crew
  - performance
dependencies: []
priority: high
---

## Description

During initial sync, backends return truncated cast/crew data (only 3 members from preview endpoints). This incomplete data gets stored in people tables, requiring lazy loading to fetch complete data later. Skip cast/crew entirely during sync and only fetch them when user views detail pages, ensuring we always get complete data on first load.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Plex get_movies() and get_shows() do not extract cast/crew from bulk API responses
- [ ] #2 Jellyfin get_movies() and get_shows() do not extract cast/crew from list endpoints
- [ ] #3 Initial sync does not call save_people_for_media for movies/shows
- [ ] #4 Detail pages always trigger full metadata fetch on first view to load cast/crew
- [ ] #5 People tables remain empty for cast/crew until detail page viewed
- [ ] #6 Sync performance improves by skipping unnecessary cast/crew processing
<!-- AC:END -->

## Implementation Plan

1. Find where Plex get_movies/get_shows extract cast/crew during sync
2. Find where Jellyfin get_movies/get_shows extract cast/crew during sync
3. Locate save_people_for_media calls during sync
4. Remove cast/crew extraction from bulk sync endpoints
5. Verify detail pages trigger full metadata fetch for cast/crew
6. Test sync performance improvement
