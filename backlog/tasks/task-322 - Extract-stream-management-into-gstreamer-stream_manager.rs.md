---
id: task-322
title: Extract stream management into gstreamer/stream_manager.rs
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

Stream collection processing and audio/subtitle track management (~200 lines) has a clear boundary and can be extracted into a dedicated module for better code organization.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 New file src/player/gstreamer/stream_manager.rs exists with all stream handling functions
- [ ] #2 StreamInfo struct moved to stream_manager module
- [ ] #3 GStreamerPlayer delegates to stream_manager for stream operations
- [ ] #4 All existing tests pass without modification
- [ ] #5 Code compiles without warnings
- [ ] #6 Stream selection and switching works identically
<!-- AC:END -->
