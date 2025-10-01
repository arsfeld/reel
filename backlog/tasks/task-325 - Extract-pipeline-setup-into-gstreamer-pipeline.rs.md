---
id: task-325
title: Extract pipeline setup into gstreamer/pipeline.rs
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

Pipeline initialization and configuration logic (~200 lines) including plugin checking, HTTP source configuration, and load_media setup can be extracted to separate initialization concerns from runtime operations.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 New file src/player/gstreamer/pipeline.rs exists with initialization functions
- [ ] #2 Plugin checking and configuration moved to pipeline module
- [ ] #3 load_media setup logic moved to pipeline module
- [ ] #4 HTTP source configuration and debugging helpers moved
- [ ] #5 All existing tests pass without modification
- [ ] #6 Code compiles without warnings
- [ ] #7 Media loading works identically (pipeline setup, buffering config)
<!-- AC:END -->
