---
id: task-040
title: Implement MPV keyboard shortcuts in player page for both backends
status: To Do
assignee: []
created_date: '2025-09-16 00:34'
updated_date: '2025-09-16 04:34'
labels:
  - ui
  - player
  - enhancement
dependencies: []
priority: high
---

## Description

Add comprehensive MPV-style keyboard shortcuts to the player page that work with both GStreamer and MPV backends. The shortcuts should mimic MPV's default keybindings to provide a familiar experience for users accustomed to MPV's controls.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Implement seek shortcuts (arrow keys for 5s, Shift+arrows for 1s, Ctrl+arrows for 10s)
- [ ] #2 Implement playback speed controls ([ and ] for speed down/up, Backspace to reset)
- [ ] #3 Implement volume controls (9/0 or mouse wheel for volume, m for mute)
- [ ] #4 Implement frame stepping (. for next frame, , for previous frame when paused)
- [ ] #5 Implement subtitle controls (v for cycle subtitles, j/J for cycle subtitle tracks)
- [ ] #6 Implement audio track cycling (# key or Shift+3)
- [ ] #7 Implement fullscreen toggle (f key)
- [ ] #8 Implement on-screen controller visibility toggle (Tab key)
- [ ] #9 Implement quit/stop playback (q or ESC key)
- [ ] #10 Ensure all shortcuts work identically for both MPV and GStreamer backends
<!-- AC:END -->
