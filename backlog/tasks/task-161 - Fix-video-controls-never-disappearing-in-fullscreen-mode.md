---
id: task-161
title: Fix video controls never disappearing in fullscreen mode
status: To Do
assignee: []
created_date: '2025-09-18 01:39'
labels:
  - bug
  - player
  - ui
dependencies: []
priority: high
---

## Description

When the video player enters fullscreen mode, the video controls (play/pause, timeline, etc.) remain visible and never auto-hide after user inactivity. The controls should automatically hide after a few seconds of no mouse/keyboard activity and reappear when the user moves the mouse or presses a key.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Implement auto-hide timer for video controls in fullscreen mode
- [ ] #2 Controls should hide after 3 seconds of inactivity
- [ ] #3 Controls should reappear on mouse movement
- [ ] #4 Controls should reappear on keyboard input
- [ ] #5 Ensure controls remain accessible when mouse hovers over them
- [ ] #6 Add smooth fade in/out animation for control visibility
- [ ] #7 Test auto-hide behavior works correctly in fullscreen mode
<!-- AC:END -->
