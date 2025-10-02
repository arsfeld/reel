---
id: task-163
title: Fix video player seek bar dragging and reactivity issues
status: Done
assignee:
  - '@claude'
created_date: '2025-09-18 01:59'
updated_date: '2025-09-29 02:11'
labels:
  - player
  - bug
  - ui
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The seek bar in the video player has two critical issues: 1) Dragging the seek bar doesn't work - users cannot scrub through the video by dragging. 2) The seek bar displays incorrect values and is not reactive to the actual video playback position. The bar appears to show random values instead of accurately tracking playback progress.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Seek bar accurately displays current playback position in real-time
- [x] #2 Users can drag the seek bar to seek to any position in the video
- [x] #3 Seek bar updates smoothly during playback without jumps or incorrect values
- [x] #4 Seek functionality works with both MPV and GStreamer backends
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Analyze current seek bar implementation issues:
   - Position update timer runs at 1Hz but seek bar value updates incorrectly
   - Drag-to-seek gesture handlers are set up but not working properly
   - is_seeking flag exists but may not be properly managed

2. Fix position tracking and display:
   - Ensure UpdatePosition command properly fetches position/duration
   - Fix seek bar range initialization when duration becomes available
   - Handle edge cases for position clamping

3. Fix drag-to-seek functionality:
   - Review GestureClick handlers for press/release events
   - Ensure is_seeking flag prevents position updates during drag
   - Fix value_changed handler to provide seek preview
   - Ensure seek command is sent on release

4. Test with both player backends:
   - Test MPV player seeking and position tracking
   - Test GStreamer player seeking and position tracking
   - Verify smooth updates without jumps
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation Summary

Fixed critical issues with the video player seek bar that prevented dragging and caused incorrect position display.

### Changes Made:

1. **Fixed Seek Bar Initialization (src/ui/pages/player.rs:1113-1116)**
   - Changed initial range from fixed 0-100 to 0-1 (will be updated when duration is known)
   - Set seek bar as initially disabled until content is loaded
   - Enabled seek bar when duration becomes available (line 2754)

2. **Implemented Proper Drag-to-Seek (src/ui/pages/player.rs:1288-1353)**
   - Replaced dual GestureClick handlers with proper GestureDrag for drag detection
   - Added separate GestureClick for immediate click-to-seek functionality
   - Implemented seeking state tracker using Rc<RefCell<bool>> to coordinate between handlers
   - Fixed value_changed handler to only send preview updates during active dragging

3. **Fixed Position Update Logic (src/ui/pages/player.rs:2139-2158)**
   - Updated StartSeeking/StopSeeking handlers to properly manage seeking state
   - Added unsafe blocks for GTK data storage/retrieval operations
   - Ensured is_seeking flag prevents position updates during user dragging

### Technical Details:

- Used GestureDrag for proper drag begin/end detection instead of GestureClick
- Implemented click position calculation for immediate seeking on single clicks
- Added proper state management to prevent position update conflicts during seeking
- Fixed seek bar range to match actual media duration instead of hardcoded 100 seconds

### Testing:

- Verified seek bar accurately displays current playback position
- Confirmed drag-to-seek works smoothly without jumps
- Tested click-to-seek for immediate position changes
- Ensured compatibility with both MPV and GStreamer backends
<!-- SECTION:NOTES:END -->
