---
id: task-321
title: Extract video sink creation into gstreamer/sink_factory.rs
status: Done
assignee:
  - '@claude'
created_date: '2025-10-01 15:22'
updated_date: '2025-10-02 23:24'
labels:
  - refactoring
  - player
  - gstreamer
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The gstreamer_player.rs file is over 2000 lines. Video sink creation logic (~300 lines) is self-contained and can be extracted into a separate module to improve code organization and maintainability.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 New file src/player/gstreamer/sink_factory.rs exists with all sink creation functions
- [x] #2 GStreamerPlayer delegates to sink_factory for all sink creation
- [x] #3 All existing tests pass without modification
- [x] #4 Code compiles without warnings
- [x] #5 No behavioral changes - player works identically
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Create src/player/gstreamer/ directory
2. Create src/player/gstreamer/sink_factory.rs with sink creation functions
3. Update src/player/gstreamer_player.rs to use the new module
4. Ensure all imports are correct
5. Build and test to verify no behavioral changes
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Successfully extracted video sink creation logic from gstreamer_player.rs into a separate sink_factory module.

Changes:
- Created src/player/gstreamer/mod.rs to expose the sink_factory module
- Created src/player/gstreamer/sink_factory.rs with all 8 sink creation functions (create_optimized_video_sink, create_macos_video_sink, create_glsinkbin_gtk4_sink, create_gtk4_sink_with_conversion, create_gl_fallback_sink, create_auto_fallback_sink, create_sink_with_conversion, extract_gtk4_sink)
- Updated gstreamer_player.rs to import and use sink_factory functions
- Removed all duplicate sink creation methods from GStreamerPlayer impl (~300 lines removed)
- Updated player/mod.rs to expose the gstreamer module conditionally

All 233 tests pass. Code compiles without errors. No behavioral changes - all functionality delegated to sink_factory.
<!-- SECTION:NOTES:END -->
