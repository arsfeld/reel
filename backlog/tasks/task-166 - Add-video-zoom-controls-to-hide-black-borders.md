---
id: task-166
title: Add video zoom controls to hide black borders
status: To Do
assignee: []
created_date: '2025-09-18 02:48'
labels:
  - player
  - ui
  - feature
dependencies: []
priority: high
---

## Description

Implement zoom controls in the video player to allow users to hide black borders (letterboxing/pillarboxing) that appear when video aspect ratio doesn't match the display. This feature should allow users to zoom in/crop the video to fill the screen, removing unwanted black bars while maintaining video quality.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Add zoom control buttons or slider to video player UI
- [ ] #2 Implement zoom levels (e.g., Fit, Fill, 16:9, 4:3, Custom zoom)
- [ ] #3 Zoom setting persists during playback session
- [ ] #4 Zoom controls work with both MPV and GStreamer backends
- [ ] #5 Video remains centered when zoomed
- [ ] #6 Keyboard shortcuts for zoom control (e.g., Z key to cycle zoom modes)
<!-- AC:END -->
