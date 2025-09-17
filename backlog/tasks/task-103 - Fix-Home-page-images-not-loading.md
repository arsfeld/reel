---
id: task-103
title: Fix Home page images not loading
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 19:37'
updated_date: '2025-09-17 03:10'
labels:
  - bug
  - ui
dependencies: []
priority: high
---

## Description

Images are not loading on the Home page for media items. This affects poster images, backdrop images, and thumbnails for movies, shows, and episodes.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 All poster images load correctly for movies and shows
- [x] #2 Episode thumbnails load properly
- [x] #3 Images use correct Plex transcoding URLs with authentication
- [x] #4 Lazy loading works for off-screen images
- [x] #5 Fallback/placeholder shown while images load
<!-- AC:END -->


## Implementation Plan

1. Add ImageLoader worker to HomePage struct
2. Initialize ImageLoader in HomePage::init with proper forwarding
3. Add ImageLoaded and ImageLoadFailed inputs to HomePageInput enum
4. Track image requests similar to library page
5. Load images when HomeSectionsLoaded is processed
6. Handle ImageLoaded/ImageLoadFailed messages to update MediaCards
7. Ensure proper Plex/Jellyfin authentication tokens are used in URLs
8. Test with both Plex and Jellyfin backends

## Implementation Notes

Fixed image loading on the Home page by integrating the ImageLoader worker component. Added proper image request tracking and forwarding similar to the library page implementation. Images now load asynchronously with proper priority queuing and the UI shows loading indicators while images are being fetched.
