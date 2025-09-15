---
id: task-011
title: Implement Relm4 player seek functionality
status: Done
assignee:
  - '@claude'
created_date: '2025-09-15 02:10'
updated_date: '2025-09-15 02:55'
labels:
  - player
  - relm4
  - controls
dependencies: []
priority: high
---

## Description

The seek bar in the Relm4 player doesn't work properly. Users should be able to drag the seek bar to jump to different positions in the video, and the seek bar should update as the video plays.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Fix seek bar drag detection (currently checking has_focus instead of drag state)
- [x] #2 Implement proper seek on value change during user interaction
- [x] #3 Update seek bar position during playback without interfering with user drag
- [x] #4 Add seek preview/tooltip showing time at cursor position
- [x] #5 Ensure seek commands are properly sent to player backend
<!-- AC:END -->


## Implementation Plan

1. Review current seek bar implementation
2. Add drag state tracking to PlayerPage struct
3. Implement proper drag detection using button press/release events
4. Update seek logic to only seek when user releases drag
5. Add position update timer that respects drag state
6. Test seeking functionality with a video


## Implementation Notes

Implemented complete seek functionality:
1. Replaced has_focus() check with proper drag state tracking (is_seeking field)
2. Added StartSeeking/StopSeeking/UpdateSeekPreview input variants
3. Used GestureClick controllers for press/release detection
4. Seek bar now updates position preview during drag
5. Actual seek only happens on mouse release
6. Position updates are blocked while user is dragging
7. Added tooltip showing time at hover position
8. All seek commands properly sent to player backend
