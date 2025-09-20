---
id: task-070
title: Load library thumbnails from Plex using transcode API
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 04:37'
updated_date: '2025-09-16 19:57'
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
- [x] #1 Implement thumbnail URL generation for Plex libraries using transcode endpoint
- [x] #2 Add width and height parameters support (e.g., width=240&height=360)
- [x] #3 Include minSize and upscale parameters in URL construction
- [x] #4 Handle authentication token in transcoded URL requests
- [x] #5 Update library page to use transcoded thumbnails
- [x] #6 Test with various library types (movies, shows, music)
<!-- AC:END -->


## Implementation Notes

Task was already implemented in src/backends/plex/api.rs. The build_image_url method uses Plex's /photo/:/transcode endpoint with width=320&height=480&minSize=1&upscale=1 parameters and proper X-Plex-Token authentication. All image URLs throughout the app (posters, thumbnails, backdrops) for movies, shows, seasons, and episodes use this transcoded endpoint for improved performance.
