---
id: task-466.02
title: Extract menu builder logic to separate module
status: Done
assignee: []
created_date: '2025-11-22 18:33'
updated_date: '2025-11-22 18:46'
labels: []
dependencies:
  - task-466.07
parent_task_id: task-466
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Extract all popover menu building functions from player.rs into a dedicated `menu_builders.rs` module. These functions are self-contained and handle creating audio/subtitle/zoom/quality menus.

Current location: Lines 477-827 in player.rs
Functions to extract:
- `populate_audio_menu(&self, sender: AsyncComponentSender<Self>)`
- `populate_subtitle_menu(&self, sender: AsyncComponentSender<Self>)`
- `populate_zoom_menu(&self, sender: AsyncComponentSender<Self>)`
- `populate_quality_menu(&self, sender: AsyncComponentSender<Self>)`

Required state fields:
- `player: Option<PlayerHandle>`
- `audio_menu_button: gtk::MenuButton`
- `subtitle_menu_button: gtk::MenuButton`
- `quality_menu_button: gtk::MenuButton`
- `zoom_menu_button: gtk::MenuButton`
- `active_popover_count: Rc<RefCell<usize>>`
- `current_upscaling_mode: UpscalingMode`
- `current_zoom_mode: ZoomMode`
- `is_mpv_backend: bool`
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 New file created: src/ui/pages/player/menu_builders.rs
- [ ] #2 All menu population functions moved to new module
- [ ] #3 Functions remain methods or become standalone with state passed as parameters
- [ ] #4 Code compiles without errors
- [ ] #5 All menus (audio, subtitle, zoom, quality) continue to work correctly
<!-- AC:END -->
