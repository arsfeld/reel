---
id: task-270
title: Add home sections progress to sync worker notifications
status: To Do
assignee: []
created_date: '2025-09-26 18:05'
labels:
  - sync
  - ui
dependencies: []
---

## Description

Update the sync worker's progress tracking and notifications to include home sections synchronization status, ensuring users see progress for all content types being synced.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Update SyncProgress struct to include home_sections_synced field
- [ ] #2 Modify SyncWorkerOutput to report home sections sync progress
- [ ] #3 Update BROKER notifications to include home sections counts
- [ ] #4 Add home sections to sync completion summary
<!-- AC:END -->
