---
id: task-466.05
title: Extract skip intro/credits logic to separate module
status: Done
assignee: []
created_date: '2025-11-22 18:34'
updated_date: '2025-11-22 19:00'
labels: []
dependencies:
  - task-466.07
parent_task_id: task-466
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Extract the skip intro and skip credits functionality from player.rs into a dedicated `skip_markers.rs` module. This includes visibility logic, auto-skip behavior, and marker management.

Current location: Scattered throughout update() method (lines 2840-2976)
Code to extract:
- Skip button visibility update logic
- Auto-skip logic for intro and credits
- Manual skip button handlers
- Marker loading and storage
- Timer management for auto-hiding buttons

Required state fields:
- `intro_marker: Option<ChapterMarker>`
- `credits_marker: Option<ChapterMarker>`
- `skip_intro_visible: bool`
- `skip_credits_visible: bool`
- `skip_intro_hide_timer: Option<SourceId>`
- `skip_credits_hide_timer: Option<SourceId>`
- `config_skip_intro_enabled: bool`
- `config_skip_credits_enabled: bool`
- `config_auto_skip_intro: bool`
- `config_auto_skip_credits: bool`
- `config_minimum_marker_duration_seconds: u64`
- `position: Duration`
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 New file created: src/ui/pages/player/skip_markers.rs
- [x] #2 All skip intro/credits logic moved to new module
- [x] #3 Marker visibility and auto-skip behavior properly encapsulated
- [x] #4 Code compiles without errors
- [x] #5 Skip intro and skip credits buttons work correctly with proper auto-hide
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Successfully extracted skip intro/credits logic from player.rs into skip_markers.rs module:

**Module Created:**
- `src/ui/pages/player/skip_markers.rs` (266 lines)
- Encapsulates all skip marker functionality in SkipMarkerManager struct

**Extracted Functionality:**
- Marker data management (intro_marker, credits_marker)
- Visibility state tracking (skip_intro_visible, skip_credits_visible)
- Auto-hide timer management
- Config value caching for skip settings
- Auto-skip logic for both intro and credits
- Manual skip button handlers
- Visibility update logic based on playback position

**Integration:**
- PlayerPage now uses single `skip_marker_manager` field instead of 12 separate fields
- Config updates properly delegated to manager via update_config()
- All skip button visibility updates handled by manager.update_visibility()
- Button click handlers delegated to manager.skip_intro() and manager.skip_credits()

**Benefits:**
- Reduced PlayerPage struct from ~110 fields to ~98 fields (12 field reduction)
- Clear separation of concerns for skip marker functionality
- Easier to test and maintain skip marker logic
- Proper cleanup via Drop trait implementation

**Result:**
Code compiles successfully with all tests passing. Skip intro and skip credits buttons work correctly with proper auto-hide behavior and auto-skip functionality.
<!-- SECTION:NOTES:END -->
