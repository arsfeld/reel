---
id: task-080
title: Implement get_libraries for LocalBackend
status: To Do
assignee: []
created_date: '2025-09-16 17:40'
updated_date: '2025-09-16 17:50'
labels:
  - backend
  - local-files
dependencies: []
priority: medium
---

## Description

Create virtual libraries for local media by categorizing the configured directories. Each top-level directory becomes a library for simple organization.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Return one library per configured media directory
- [ ] #2 Set library type as 'Movies' for MVP (no TV show detection yet)
- [ ] #3 Include directory path in library name
- [ ] #4 Calculate and return item count for each library
<!-- AC:END -->
