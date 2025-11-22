---
id: task-466.04
title: Extract backend management logic to separate module
status: Done
assignee: []
created_date: '2025-11-22 18:34'
updated_date: '2025-11-22 18:50'
labels: []
dependencies:
  - task-466.07
parent_task_id: task-466
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Extract the player backend management and lifecycle logic from player.rs into a dedicated `backend_manager.rs` module. This handles switching between MPV and GStreamer backends.

Current location: Lines 134-332 in player.rs
Code to extract:
- `backend_prefers_mpv()` method
- `mpv_upscaling_mode_from_config()` method
- `attach_player_controller()` method
- `rebuild_player_backend()` method
- `handle_config_update()` method
- `ensure_backend_alignment()` method

Required state fields:
- `player: Option<PlayerHandle>`
- `is_mpv_backend: bool`
- `current_upscaling_mode: UpscalingMode`
- `video_container: gtk::Box`
- `video_placeholder: Option<gtk::Label>`
- `error_message: Option<String>`
- `player_state: PlayerState`
- Access to config values
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 New file created: src/ui/pages/player/backend_manager.rs
- [ ] #2 All backend lifecycle methods moved to new module
- [ ] #3 Backend switching logic properly encapsulated
- [ ] #4 Code compiles without errors
- [ ] #5 Backend switching and video widget attachment works correctly
<!-- AC:END -->
