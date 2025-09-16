---
id: task-059
title: Fix player controls not syncing with actual playback state
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 03:30'
updated_date: '2025-09-16 03:43'
labels:
  - bug
  - player
  - ui
dependencies: []
priority: high
---

## Description

The player control buttons (play/pause) do not reflect the actual playback state of the video. When the video is playing, the button might show play instead of pause, and vice versa. The timer controls appear to work correctly, but the play/pause button state is not synchronized with the actual media playback state.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Identify where play/pause button state is managed
- [x] #2 Ensure button state updates when playback state changes
- [x] #3 Handle state changes from both user actions and player events
- [x] #4 Sync button state when video starts/stops/pauses automatically
- [x] #5 Fix state synchronization for both MPV and GStreamer backends
- [x] #6 Ensure button reflects correct state after seeking or buffering
- [x] #7 Test with various playback scenarios (autoplay, manual control, errors)
<!-- AC:END -->


## Implementation Plan

1. Analyze player state updates in PlayPause handler
2. Check state updates in PositionUpdate handler
3. Verify player backend state retrieval
4. Add proper state synchronization after player commands
5. Test with both MPV and GStreamer backends


## Implementation Notes

Fixed player control synchronization issues by:

1. Modified PlayPause handler to query actual player state after executing commands instead of optimistically setting state
2. Updated Stop, Seek, and SetVolume handlers to fetch actual state from player after operations
3. Fixed LoadMedia handlers to get real state after media loading completes
4. Refactored MPV backend get_state() to query MPV properties (pause, idle-active) for real-time state
5. Refactored GStreamer backend get_state() to query GStreamer pipeline state directly

The root cause was that the UI was relying on optimistically set states rather than querying the actual player backend state. Now all state changes properly sync with the underlying player implementation.
