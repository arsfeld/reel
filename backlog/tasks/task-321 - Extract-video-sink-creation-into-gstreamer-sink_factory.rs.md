---
id: task-321
title: Extract video sink creation into gstreamer/sink_factory.rs
status: In Progress
assignee:
  - '@claude'
created_date: '2025-10-01 15:22'
updated_date: '2025-10-01 15:24'
labels:
  - refactoring
  - player
  - gstreamer
dependencies: []
priority: medium
---

## Description

The gstreamer_player.rs file is over 2000 lines. Video sink creation logic (~300 lines) is self-contained and can be extracted into a separate module to improve code organization and maintainability.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 New file src/player/gstreamer/sink_factory.rs exists with all sink creation functions
- [ ] #2 GStreamerPlayer delegates to sink_factory for all sink creation
- [ ] #3 All existing tests pass without modification
- [ ] #4 Code compiles without warnings
- [ ] #5 No behavioral changes - player works identically
<!-- AC:END -->

## Implementation Plan

1. Create src/player/gstreamer/ directory
2. Create src/player/gstreamer/sink_factory.rs with sink creation functions
3. Update src/player/gstreamer_player.rs to use the new module
4. Ensure all imports are correct
5. Build and test to verify no behavioral changes
