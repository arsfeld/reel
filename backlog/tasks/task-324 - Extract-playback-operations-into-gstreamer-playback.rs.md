---
id: task-324
title: Extract playback operations into gstreamer/playback.rs
status: To Do
assignee: []
created_date: '2025-10-01 15:22'
labels:
  - refactoring
  - player
  - gstreamer
dependencies: []
priority: medium
---

## Description

Playback control methods (play, pause, stop, seek, volume, position, duration) are cohesive operations that can be grouped in a dedicated module. This separates state transitions from initialization and configuration.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 New file src/player/gstreamer/playback.rs exists with playback control functions
- [ ] #2 Play, pause, stop, seek methods moved to playback module
- [ ] #3 Position, duration, volume query methods moved to playback module
- [ ] #4 All existing tests pass without modification
- [ ] #5 Code compiles without warnings
- [ ] #6 Playback controls work identically (state transitions, seeking, volume)
<!-- AC:END -->
