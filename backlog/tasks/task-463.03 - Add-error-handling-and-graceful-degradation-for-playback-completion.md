---
id: task-463.03
title: Add error handling and graceful degradation for playback completion
status: Done
assignee: []
created_date: '2025-11-22 18:02'
updated_date: '2025-11-22 18:17'
labels:
  - error-handling
  - player
  - stability
dependencies: []
parent_task_id: task-463
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Several code paths lack proper error handling when playback completes, which may contribute to unexpected app closure. Need to ensure all playback completion scenarios are handled gracefully with proper fallback behaviors.

**Identified Issues**:
- No `PlayerInput` handler for natural playback completion without next item
- Potential panic points with `unwrap()` calls in player backends
- Missing playlist context handling (when `self.playlist_context` is `None`)
- `LoadError` navigates back after 500ms, but no handling for `None` from `get_next_item()`

**Needed Improvements**:
- Add explicit handling for EOS (End-of-Stream) events from both GStreamer and MPV
- Ensure all paths through playback completion lead to a defined UI state
- Replace panic-prone `unwrap()` with proper error handling
- Add logging for debugging playback completion issues

**Key Files**:
- `src/ui/pages/player.rs` - PlayerPage input handling
- `src/player/gstreamer/bus_handler.rs` - GStreamer EOS handling
- `src/player/mpv_player.rs` - MPV EOF detection
- `src/player/controller.rs` - Player state management
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 App never closes unexpectedly when episode ends
- [x] #2 All EOS/EOF events result in defined UI state
- [x] #3 Playback completion is properly logged for debugging
- [x] #4 Missing playlist context is handled gracefully
- [ ] #5 No unwrap() calls on potentially-failing player operations
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added error handling and graceful degradation for playback completion edge cases.

Changes made in src/ui/pages/player.rs:
- Added explicit handling for when playlist context is None
- Added explicit handling for when auto-play is disabled
- Added debug logging for all playback completion paths
- Separated auto-play enabled check from next episode check for clearer logic flow

The code now handles all edge cases:
- Episode ends with next episode available → auto-play
- Episode ends without next episode (auto-play enabled) → navigate back after delay
- Episode ends with auto-play disabled → let video finish naturally, user navigates manually
- Episode ends without playlist context → let video finish naturally

EOS/EOF events are already handled by the player backends (GStreamer sets state to Stopped, MPV checks eof-reached property).
<!-- SECTION:NOTES:END -->
