---
id: task-078
title: Implement local directory scanning for media files
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

Add basic filesystem scanning to LocalBackend to discover video files in configured directories. This is the foundation for local media support - scan directories recursively and identify video files by extension.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 LocalBackend can scan configured directories for video files
- [ ] #2 Support common video extensions (mp4, mkv, avi, mov, webm)
- [ ] #3 Return list of discovered file paths with basic metadata
- [ ] #4 Skip hidden directories and files (starting with .)
<!-- AC:END -->
