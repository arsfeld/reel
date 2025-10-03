---
id: task-369
title: Fix sync status indicator for episode updates in TV libraries
status: To Do
assignee: []
created_date: '2025-10-03 16:50'
labels:
  - bug
  - sync
  - ui
  - sidebar
dependencies: []
priority: high
---

## Description

When syncing episode information, the TV show library loses its sync indicator in the sidebar even though sync progress messages are still being sent. The sync progress is not properly attached to the library, causing the UI to show the library as not syncing when it actually is. Need to ensure episode sync operations properly update the library sync status.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Identify where episode sync operations send progress messages
- [ ] #2 Verify library_id is properly included in sync progress for episode updates
- [ ] #3 Update sync progress messages to include parent library information
- [ ] #4 Sidebar sync indicator remains active during episode sync operations
- [ ] #5 Sync progress properly associated with TV show library throughout episode sync
- [ ] #6 Test with TV library to confirm indicator stays active during episode sync
<!-- AC:END -->
