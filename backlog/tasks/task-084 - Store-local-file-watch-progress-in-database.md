---
id: task-084
title: Store local file watch progress in database
status: To Do
assignee: []
created_date: '2025-09-16 17:40'
updated_date: '2025-09-16 17:51'
labels:
  - backend
  - local-files
  - database
dependencies: []
priority: low
---

## Description

Implement basic watch progress tracking for local files using the existing playback_progress table. Store and retrieve playback positions for resume functionality.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Implement update_progress to save position to database
- [ ] #2 Implement get_watch_status to retrieve saved progress
- [ ] #3 Use file path as unique identifier for progress tracking
- [ ] #4 Support mark_watched and mark_unwatched operations
<!-- AC:END -->
