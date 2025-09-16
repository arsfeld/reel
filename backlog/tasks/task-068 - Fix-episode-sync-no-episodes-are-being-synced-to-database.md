---
id: task-068
title: Fix episode sync - no episodes are being synced to database
status: To Do
assignee: []
created_date: '2025-09-16 04:18'
labels:
  - bug
  - sync
  - critical
dependencies: []
priority: high
---

## Description

Episodes are not being synced to the database during the sync process. While the episode display functionality is now working, there are no episodes in the database to display. The sync process needs to be fixed to properly fetch and store episodes from both Plex and Jellyfin backends.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Investigate why episodes are not being synced during library sync
- [ ] #2 Check if sync_show_episodes is being called correctly
- [ ] #3 Verify backend get_episodes methods are returning data
- [ ] #4 Fix episode storage in database during sync
- [ ] #5 Test episode sync with Plex backend
- [ ] #6 Test episode sync with Jellyfin backend
- [ ] #7 Verify episodes appear in database after sync
- [ ] #8 Ensure episodes display in UI after successful sync
<!-- AC:END -->
