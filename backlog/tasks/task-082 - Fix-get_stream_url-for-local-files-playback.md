---
id: task-082
title: Fix get_stream_url for local files playback
status: To Do
assignee: []
created_date: '2025-09-16 17:40'
updated_date: '2025-09-16 17:50'
labels:
  - backend
  - local-files
  - player
dependencies: []
priority: medium
---

## Description

Update the existing get_stream_url implementation to properly handle local file paths and ensure they work with both MPV and GStreamer players.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Validate that file path exists before returning StreamInfo
- [ ] #2 Properly escape file paths with spaces and special characters
- [ ] #3 Return absolute file paths with correct file:// URL format
- [ ] #4 Test playback with both MPV and GStreamer backends
<!-- AC:END -->
