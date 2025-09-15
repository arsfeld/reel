---
id: task-002
title: Fix sync initialization only loading libraries without content
status: To Do
assignee: []
created_date: '2025-09-15 01:40'
labels:
  - sync
  - backend
  - bug
dependencies: []
---

## Description

During initialization sync, libraries are loaded successfully but the actual content (movies/shows/episodes) within each library is not being synced. Investigation shows the sync service only syncs season 1 episodes for TV shows (hardcoded in sync.rs line 236-241) and may have other content loading issues.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 All movies in each library are synced during initialization
- [ ] #2 All TV show seasons and episodes are synced (not just season 1)
- [ ] #3 Sync progress is properly reported during content loading
- [ ] #4 Content appears in UI after sync completes
<!-- AC:END -->
