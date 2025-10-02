---
id: task-322
title: Extract stream management into gstreamer/stream_manager.rs
status: Done
assignee:
  - '@claude'
created_date: '2025-10-01 15:22'
updated_date: '2025-10-02 23:35'
labels:
  - refactoring
  - player
  - gstreamer
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Stream collection processing and audio/subtitle track management (~200 lines) has a clear boundary and can be extracted into a dedicated module for better code organization.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 New file src/player/gstreamer/stream_manager.rs exists with all stream handling functions
- [x] #2 StreamInfo struct moved to stream_manager module
- [x] #3 GStreamerPlayer delegates to stream_manager for stream operations
- [x] #4 All existing tests pass without modification
- [x] #5 Code compiles without warnings
- [x] #6 Stream selection and switching works identically
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Create new src/player/gstreamer/stream_manager.rs module file
2. Move StreamInfo struct to stream_manager.rs
3. Create StreamManager struct to hold stream state (collections, current streams)
4. Move stream processing functions to StreamManager (process_stream_collection, send_default_selection, etc.)
5. Move track management functions to StreamManager (get/set audio/subtitle tracks, cycling)
6. Update GStreamerPlayer to use StreamManager instead of direct stream handling
7. Update src/player/gstreamer/mod.rs to include the new module
8. Build with `nix develop -c cargo build` and fix any compilation errors
9. Run tests with `nix develop -c cargo test` to ensure everything works
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Successfully extracted stream management functionality from gstreamer_player.rs into a dedicated stream_manager.rs module.

Key changes:
- Created new src/player/gstreamer/stream_manager.rs with StreamManager struct
- Moved StreamInfo struct and all stream-related functions (~400 lines) to the new module
- GStreamerPlayer now delegates all stream operations to StreamManager
- Added StreamManager::from_arcs() constructor for use in bus message handlers
- Updated mod.rs to export the new module
- All 233 tests pass without modification
- Code compiles successfully with no new warnings introduced
<!-- SECTION:NOTES:END -->
