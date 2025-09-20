---
id: task-061
title: Fix movie poster not loading on movie details page
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 03:37'
updated_date: '2025-09-16 03:55'
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
- [x] #6 Test poster loading with movies from different backends (Plex/Jellyfin)
- [x] #7 Verify poster displays at correct size and aspect ratio
<!-- AC:END -->


## Implementation Plan

1. Locate movie details page component and analyze current image loading code
2. Trace data flow from backend to UI for poster URLs
3. Check image loading utility functions and components
4. Verify poster URL format and accessibility
5. Fix any identified issues
6. Test with multiple movies from different backends


## Implementation Notes

Fixed movie poster loading issue by implementing async image loading from URLs.

The problem was that gtk::gdk_pixbuf::Pixbuf::from_file_at_size() expects local file paths, not URLs. The poster_url and backdrop_url fields contain HTTP URLs from Plex/Jellyfin backends.

Solution:
- Added poster_texture and backdrop_texture fields to MovieDetailsPage and ShowDetailsPage to store loaded textures
- Added new commands (LoadPosterImage, LoadBackdropImage, PosterImageLoaded, BackdropImageLoaded) to handle async image loading
- Implemented load_image_from_url() helper function that downloads images via reqwest and creates gtk::gdk::Texture from bytes
- Updated both movie_details.rs and show_details.rs to use the new image loading mechanism
- Images are now loaded asynchronously after the main details are displayed

The fix applies to both movie and show details pages, ensuring consistent behavior across the application.
