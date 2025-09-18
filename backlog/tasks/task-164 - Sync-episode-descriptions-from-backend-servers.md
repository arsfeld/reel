---
id: task-164
title: Sync episode descriptions from backend servers
status: To Do
assignee: []
created_date: '2025-09-18 02:32'
labels:
  - sync
  - backend
  - metadata
dependencies: []
priority: medium
---

## Description

Episode descriptions are currently empty in the application. Need to implement proper syncing of episode descriptions from Plex and Jellyfin backends during the sync process. This metadata is available from the servers but not being properly extracted and stored in the database.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Episode descriptions are fetched from Plex backend during sync
- [ ] #2 Episode descriptions are fetched from Jellyfin backend during sync
- [ ] #3 Descriptions are properly stored in the database media_items table
- [ ] #4 Episode descriptions display correctly in the UI episode lists
- [ ] #5 Sync process updates existing empty descriptions without duplicating episodes
<!-- AC:END -->
