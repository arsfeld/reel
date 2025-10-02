---
id: task-278
title: Fix HiDPI video rendering on macOS - video renders in quarter of window
status: Done
assignee: []
created_date: '2025-09-27 01:32'
updated_date: '2025-10-02 14:57'
labels:
  - player
  - macos
  - mpv
  - hidpi
  - bug
dependencies: []
priority: high
---

## Description

Video playback on macOS with Retina/HiDPI displays is rendering in only the bottom-left quarter of the window. The video is scaled to fit that quarter but should fill the entire player area. This is likely due to incorrect handling of the backing scale factor between logical and physical pixels in the OpenGL rendering pipeline.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Investigate GTK4 scale factor handling for OpenGL on macOS
- [ ] #2 Implement proper conversion between logical and physical pixels for MPV rendering
- [ ] #3 Test video fills entire player window on Retina displays
- [ ] #4 Verify correct aspect ratio is maintained
- [ ] #5 Ensure no performance degradation from scaling
<!-- AC:END -->
