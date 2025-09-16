---
id: task-085
title: Add file watcher for local media changes
status: To Do
assignee: []
created_date: '2025-09-16 17:40'
labels:
  - backend
  - local-files
dependencies: []
priority: low
---

## Description

Monitor configured directories for changes (new files, deletions, renames) and automatically update the library. Use filesystem events for real-time updates.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Implement file system watcher using notify crate
- [ ] #2 Detect new video files added to watched directories
- [ ] #3 Detect file deletions and remove from database
- [ ] #4 Detect file renames and update database entries
- [ ] #5 Throttle rapid changes to avoid excessive rescanning
<!-- AC:END -->
