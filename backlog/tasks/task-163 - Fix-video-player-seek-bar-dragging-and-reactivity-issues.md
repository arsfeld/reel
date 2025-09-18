---
id: task-163
title: Fix video player seek bar dragging and reactivity issues
status: To Do
assignee: []
created_date: '2025-09-18 01:59'
labels:
  - player
  - bug
  - ui
dependencies: []
priority: high
---

## Description

The seek bar in the video player has two critical issues: 1) Dragging the seek bar doesn't work - users cannot scrub through the video by dragging. 2) The seek bar displays incorrect values and is not reactive to the actual video playback position. The bar appears to show random values instead of accurately tracking playback progress.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Seek bar accurately displays current playback position in real-time
- [ ] #2 Users can drag the seek bar to seek to any position in the video
- [ ] #3 Seek bar updates smoothly during playback without jumps or incorrect values
- [ ] #4 Seek functionality works with both MPV and GStreamer backends
<!-- AC:END -->
