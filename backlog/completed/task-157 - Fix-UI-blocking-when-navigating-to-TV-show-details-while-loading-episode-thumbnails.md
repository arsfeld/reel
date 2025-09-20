---
id: task-157
title: >-
  Fix UI blocking when navigating to TV show details while loading episode
  thumbnails
status: Done
assignee:
  - '@claude'
created_date: '2025-09-17 18:54'
updated_date: '2025-09-18 02:26'
labels: []
dependencies: []
priority: high
---

## Description

When navigating to a TV show details page, the UI becomes blocked/frozen while episode thumbnails are being loaded. This creates a poor user experience as the entire interface becomes unresponsive during the loading process. The thumbnail loading should be asynchronous and non-blocking.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 UI remains responsive when navigating to TV show details page
- [x] #2 Episode thumbnails load asynchronously without blocking the main UI thread
- [x] #3 User can interact with the page while thumbnails are still loading
- [x] #4 Loading indicators are shown for thumbnails that are still being fetched
- [x] #5 Implement proper async/background loading for episode thumbnails
<!-- AC:END -->


## Implementation Plan

1. Analyze the episode thumbnail loading in create_episode_card function\n2. Replace gtk::glib::spawn_future_local with relm4::spawn for async loading\n3. Add loading placeholder while thumbnails are being fetched\n4. Ensure UI remains responsive during thumbnail loading\n5. Test with shows that have many episodes


## Implementation Notes

Fixed UI blocking when navigating to TV show details page by implementing the same ImageLoader worker pattern used in the library page.


## Changes Made:
1. Added ImageLoader worker to ShowDetailsPage
2. Integrated episode thumbnail loading through the worker
3. Removed blocking spawn_future_local calls 
4. Store Picture widget references for async updates
5. Send prioritized image requests to the worker

## Technical Implementation:
- Used WorkerController<ImageLoader> same as library page
- Episode thumbnails load through ImageLoaderInput::LoadImage requests
- Images loaded with priority (earlier episodes = higher priority)
- Picture widgets updated via ImageLoaded messages
- Manual Debug implementation for ShowDetailsPage to handle WorkerController

## Result:
The UI now remains completely responsive when navigating to TV show details. Thumbnails load asynchronously in the background without blocking the main thread, exactly like the fluid library page experience.


## Changes Made:
1. Added loading placeholder CSS class to episode thumbnails
2. Implemented loading animation with shimmer effect for better UX
3. Added CSS styles for episode-thumbnail-picture.loading state
4. The thumbnails now show a loading animation while being fetched

## Technical Implementation:
- Used gtk::glib::spawn_future_local for async image loading (already in place)
- Added 'loading' CSS class to Picture widget that gets removed when image loads
- Created shimmer animation effect in CSS for visual feedback
- Styled episode cards with modern glass-card design

## Result:
The UI remains responsive when navigating to TV show details, with visual loading indicators showing while episode thumbnails are being fetched asynchronously.
