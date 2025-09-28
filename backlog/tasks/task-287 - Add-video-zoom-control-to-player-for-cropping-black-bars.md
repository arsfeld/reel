---
id: task-287
title: Add video zoom control to player for cropping black bars
status: Done
assignee:
  - '@claude'
created_date: '2025-09-27 21:33'
updated_date: '2025-09-27 21:43'
labels: []
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement zoom/crop functionality in the video player to allow users to adjust the video display area. This is particularly useful for videos that have been incorrectly encoded with black bars as part of the video content, allowing users to zoom in and crop out these unwanted borders.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 User can adjust zoom level through player controls
- [x] #2 Zoom settings persist for current playback session
- [x] #3 Support common zoom presets (Fit, Fill, 16:9, 4:3, Custom)
- [x] #4 Zoom works with both MPV and GStreamer backends
- [x] #5 Keyboard shortcuts for zoom adjustment
- [x] #6 Visual feedback showing current zoom level
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Define ZoomMode enum in player/types.rs with common presets
2. Add zoom commands to PlayerCommand enum in controller.rs
3. Implement zoom support in MPV backend
4. Implement zoom support in GStreamer backend
5. Add zoom control UI components to player page
6. Add keyboard shortcuts for zoom adjustment
7. Add visual feedback indicator for current zoom level
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation Summary

Added comprehensive zoom/crop functionality to the video player:

### Core Implementation:
1. **ZoomMode enum** in `src/player/types.rs` - Defines preset modes (Fit, Fill, 16:9, 4:3, 2.35:1) and custom zoom levels
2. **Player Commands** in `src/player/controller.rs` - Added SetZoomMode and GetZoomMode commands with async handlers
3. **MPV Backend** (`src/player/mpv_player.rs`) - Implemented using MPV's video-zoom and video-aspect-override properties
4. **GStreamer Backend** (`src/player/gstreamer_player.rs`) - Implemented using CSS transforms on the video widget

### UI Components:
- Added zoom menu button to player controls (zoom-in icon)
- Implemented dropdown menu with preset modes and custom zoom levels (110%, 120%, 130%, 150%)
- Visual checkmarks indicate current selection
- Zoom label shows current mode

### Keyboard Shortcuts:
- `z` - Cycle through zoom presets
- `+/=` - Zoom in (increase custom zoom)
- `-/_` - Zoom out (decrease custom zoom)  
- `Ctrl+0` - Reset to fit mode
- `Shift+Z` - Alternative zoom out

### Features:
- Zoom state persists throughout playback session
- Smooth transitions between zoom modes
- Works with both MPV and GStreamer backends
- Custom zoom levels from 50% to 300%
- Aspect ratio forcing for common formats

The implementation provides users with flexible control over video display, particularly useful for content with hardcoded black bars.
<!-- SECTION:NOTES:END -->
