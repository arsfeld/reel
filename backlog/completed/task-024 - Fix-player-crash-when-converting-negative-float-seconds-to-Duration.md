---
id: task-024
title: Fix player crash when converting negative float seconds to Duration
status: Done
assignee:
  - '@claude'
created_date: '2025-09-15 03:38'
updated_date: '2025-09-15 15:57'
labels:
  - bug
  - player
  - relm4
  - critical
dependencies: []
priority: high
---

## Description

The Relm4 player crashes with a panic when attempting to convert negative float seconds to Duration during playback. This occurs after media is successfully loaded and hardware decoding is initialized.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Identify the source of negative duration values in the player code
- [x] #2 Add validation to prevent negative values from being converted to Duration
- [x] #3 Handle edge cases where seek position or playback time might be negative
- [x] #4 Ensure player gracefully handles invalid time values without crashing
- [x] #5 Add error recovery mechanism for duration conversion failures
- [x] #6 Test with various media files to ensure no regressions
<!-- AC:END -->


## Implementation Plan

1. Search for all Duration::from_secs_f64() calls in player-related code
2. Add validation to ensure values are non-negative before conversion
3. Handle negative values by clamping to 0 or returning None/default
4. Test with various media files to verify no regressions


## Implementation Notes

Fixed player crash by adding .max(0.0) validation to all Duration::from_secs_f64() calls in:
- src/player/mpv_player.rs: Protected 3 locations where MPV returns position/duration values
- src/platforms/relm4/components/pages/player.rs: Protected 3 locations in seek bar handlers

The fix ensures negative values from MPV (which can occur during seeking or at playback start) are clamped to 0 before conversion to Duration, preventing panic.
