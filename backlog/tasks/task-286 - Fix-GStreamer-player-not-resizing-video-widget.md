---
id: task-286
title: Fix GStreamer player not resizing video widget
status: Done
assignee:
  - '@claude'
created_date: '2025-09-27 21:01'
updated_date: '2025-09-27 21:31'
labels:
  - bug
  - player
  - gstreamer
  - ui
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The GStreamer player backend does not properly resize the video widget to match the video dimensions. This appears to be related to the player not starting playback immediately, so it doesn't know the video dimensions until after play is pressed. The video widget should resize appropriately once video dimensions are known, providing proper aspect ratio and eliminating black bars or incorrect scaling.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Video widget resizes to correct dimensions when video metadata is loaded
- [x] #2 Aspect ratio is properly maintained without distortion
- [x] #3 Widget resizing works even when player starts in paused state
- [x] #4 No unnecessary black bars around the video content
- [x] #5 Video dimensions are detected and applied before playback begins
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add AsyncDone message handling to know when pipeline is ready and video dimensions are available
2. Add a notification mechanism to inform the player page when dimensions are available
3. Emit signal or use callback when dimensions become available after AsyncDone
4. Test that video widget resizes properly when dimensions are detected
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fixed the GStreamer video widget resizing issue by implementing proper pipeline preroll and dimension detection.

The issue was that GStreamer needs to reach at least PAUSED state (preroll) before video dimensions are available. The fix includes:

1. Added AsyncDone message handling in the bus message handler to detect when pipeline preroll completes
2. Modified load_media() to initiate pipeline preroll to PAUSED state immediately after loading, ensuring dimensions are available early
3. Enhanced get_video_dimensions() to automatically transition pipeline to PAUSED state if not already there when dimensions are requested
4. Added force-aspect-ratio property to gtk4paintablesink instances to maintain proper aspect ratio without distortion

These changes ensure that:
- Video dimensions are detected as soon as the pipeline completes preroll
- The video widget can resize before playback begins
- Aspect ratio is properly maintained
- No black bars appear around the video content
- The fix works whether the player starts in paused or playing state
<!-- SECTION:NOTES:END -->
