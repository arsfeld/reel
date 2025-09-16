---
id: task-061
title: Fix movie poster not loading on movie details page
status: In Progress
assignee:
  - '@claude'
created_date: '2025-09-16 03:37'
updated_date: '2025-09-16 03:53'
labels:
  - bug
  - ui
  - media
dependencies: []
priority: high
---

## Description

The movie details page fails to display the movie poster image. While other metadata and information may be shown, the poster image is missing or not loading properly. This impacts the visual presentation of the movie details page and user experience.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Identify why movie poster is not loading on details page
- [x] #2 Check if poster URL is being correctly retrieved from backend
- [x] #3 Verify image loading mechanism for movie details page
- [x] #4 Ensure poster image component is properly initialized
- [x] #5 Fix any issues with image path or loading logic
- [ ] #6 Test poster loading with movies from different backends (Plex/Jellyfin)
- [ ] #7 Verify poster displays at correct size and aspect ratio
<!-- AC:END -->


## Implementation Plan

1. Locate movie details page component and analyze current image loading code
2. Trace data flow from backend to UI for poster URLs
3. Check image loading utility functions and components
4. Verify poster URL format and accessibility
5. Fix any identified issues
6. Test with multiple movies from different backends
