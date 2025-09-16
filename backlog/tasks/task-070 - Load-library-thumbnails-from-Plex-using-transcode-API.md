---
id: task-070
title: Load library thumbnails from Plex using transcode API
status: To Do
assignee: []
created_date: '2025-09-16 04:37'
labels:
  - plex
  - thumbnails
  - performance
dependencies: []
priority: high
---

## Description

Implement loading of library thumbnails from Plex using the transcoding API endpoint. The API provides resized thumbnails via the /photo/:/transcode endpoint with configurable width, height, and quality parameters. This will improve performance by loading appropriately sized images instead of full resolution thumbnails.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Implement thumbnail URL generation for Plex libraries using transcode endpoint
- [ ] #2 Add width and height parameters support (e.g., width=240&height=360)
- [ ] #3 Include minSize and upscale parameters in URL construction
- [ ] #4 Handle authentication token in transcoded URL requests
- [ ] #5 Update library page to use transcoded thumbnails
- [ ] #6 Test with various library types (movies, shows, music)
<!-- AC:END -->
