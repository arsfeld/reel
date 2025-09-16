---
id: task-081
title: Implement get_movies for LocalBackend
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

Return discovered video files as Movie objects for display in the UI. Convert the scanned files into the Movie model used by the application.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Convert scanned video files to Movie objects
- [ ] #2 Use extracted metadata (title, year, file path)
- [ ] #3 Generate placeholder poster and backdrop URLs
- [ ] #4 Set runtime to 0 (will be detected during playback)
- [ ] #5 Return empty arrays for genres, cast, and crew
<!-- AC:END -->
