---
id: task-166
title: Add video zoom controls to hide black borders
status: Done
assignee:
  - '@claude-code'
created_date: '2025-09-18 02:48'
updated_date: '2025-10-04 21:50'
labels:
  - player
  - ui
  - feature
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement zoom controls in the video player to allow users to hide black borders (letterboxing/pillarboxing) that appear when video aspect ratio doesn't match the display. This feature should allow users to zoom in/crop the video to fill the screen, removing unwanted black bars while maintaining video quality.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Add zoom control buttons or slider to video player UI
- [x] #2 Implement zoom levels (e.g., Fit, Fill, 16:9, 4:3, Custom zoom)
- [x] #3 Zoom setting persists during playback session
- [x] #4 Zoom controls work with both MPV and GStreamer backends
- [x] #5 Video remains centered when zoomed
- [x] #6 Keyboard shortcuts for zoom control (e.g., Z key to cycle zoom modes)
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation Summary

This feature was already fully implemented in the codebase. The zoom functionality allows users to hide black borders by adjusting video zoom and aspect ratio.

## What Was Found

### UI Components (src/ui/pages/player.rs)
- **Zoom Menu Button**: MenuButton with dropdown menu showing all zoom modes (lines 98, 1112-1113)
- **Zoom Label**: Label displaying current zoom mode (e.g., "Fit", "16:9", "150%")
- **populate_zoom_menu()**: Method that creates the zoom menu with all preset and custom zoom levels (lines 408-530)

### Zoom Modes (src/player/types.rs:13-68)
- **Fit**: Default mode - fits entire video in window (may show black bars)
- **Fill**: Fills window completely (may crop video)
- **16:9**: Force 16:9 aspect ratio
- **4:3**: Force 4:3 aspect ratio  
- **2.35:1**: Cinematic aspect ratio
- **Custom(f64)**: Arbitrary zoom levels (e.g., 1.1 = 110%, 1.5 = 150%)
- Preset custom levels available: 110%, 120%, 130%, 150%

### Backend Implementation

**MPV Backend** (src/player/mpv_player.rs:1371-1444):
- Uses MPV properties: video-zoom, video-pan-x, video-pan-y, video-aspect-override
- Fit mode: Resets all zoom/pan values to 0
- Fill mode: Applies positive zoom to fill screen
- Aspect modes: Override video aspect ratio
- Custom mode: Converts zoom level to log2 scale for MPV

**GStreamer Backend** (src/player/gstreamer_player.rs:1154-1206):
- Uses CSS transforms on the GTK widget
- Adds CSS classes: zoom-fit, zoom-fill, zoom-16-9, zoom-4-3, zoom-2-35, zoom-custom
- Custom mode applies inline CSS transform with scale()

### Keyboard Shortcuts (src/ui/pages/player.rs:1617-1636)
- `z`: Cycle through zoom modes (Fit → Fill → 16:9 → 4:3 → 2.35:1 → Fit)
- `Shift+Z`: Zoom out (decrease by 0.1)
- `+` or `=`: Zoom in (increase by 0.1)
- `-` or `_`: Zoom out (decrease by 0.1)
- `Ctrl+0`: Reset to Fit mode

### Input Handlers (src/ui/pages/player.rs:2819-2864)
- **SetZoomMode**: Sets zoom mode and updates backend + UI
- **CycleZoom**: Cycles through preset modes
- **ZoomIn**: Increases custom zoom level by 0.1 (max 3.0)
- **ZoomOut**: Decreases custom zoom level by 0.1 (min 0.5)
- **ZoomReset**: Resets to Fit mode

### Session Persistence
Zoom mode is stored in `current_zoom_mode` field and persists throughout playback session. Does not persist across app restarts (by design).

## Files Modified
No files were modified. This task discovered that the feature was already fully implemented.

## Testing Notes
All acceptance criteria are met:
1. ✅ UI controls present (zoom menu button)
2. ✅ Multiple zoom modes implemented (Fit, Fill, aspect ratios, custom)
3. ✅ Zoom persists during playback session
4. ✅ Works with both MPV and GStreamer backends
5. ✅ Video centering handled by backends (MPV: video-pan, GStreamer: CSS)
6. ✅ Comprehensive keyboard shortcuts implemented
<!-- SECTION:NOTES:END -->
