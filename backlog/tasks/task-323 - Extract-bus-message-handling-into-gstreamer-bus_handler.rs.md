---
id: task-323
title: Extract bus message handling into gstreamer/bus_handler.rs
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

Bus message processing logic (~300 lines) contains a large switch statement handling different GStreamer message types. Extracting it will make the main player file more focused on coordination rather than message details.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 New file src/player/gstreamer/bus_handler.rs exists with message handling logic
- [ ] #2 handle_bus_message_sync function moved to bus_handler module
- [ ] #3 Message routing and error handling preserved
- [ ] #4 All existing tests pass without modification
- [ ] #5 Code compiles without warnings
- [ ] #6 Bus messages processed identically (EOS, errors, state changes, etc.)
<!-- AC:END -->
