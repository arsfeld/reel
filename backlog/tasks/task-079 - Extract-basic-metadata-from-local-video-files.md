---
id: task-079
title: Extract basic metadata from local video files
status: To Do
assignee: []
created_date: '2025-09-16 17:39'
updated_date: '2025-09-16 17:50'
labels:
  - backend
  - local-files
dependencies: []
priority: medium
---

## Description

Use filesystem metadata and simple filename parsing to extract basic movie information like title, year, and file size. This provides minimal metadata without external dependencies.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Extract title from filename (remove extension and clean up)
- [ ] #2 Parse year from filename if present (e.g., 'Movie (2023).mp4')
- [ ] #3 Get file size and last modified date
- [ ] #4 Generate unique media ID from file path
<!-- AC:END -->
