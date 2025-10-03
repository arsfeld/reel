---
id: task-392.05
title: Create menus.rs module for menu population methods
status: To Do
assignee: []
created_date: '2025-10-04 02:23'
labels:
  - refactor
  - ui
dependencies: []
parent_task_id: task-392
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Extract menu population methods into menus.rs as impl blocks for PlayerPage. These methods populate audio, subtitle, quality, and zoom menus.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Create src/ui/pages/player/menus.rs file
- [ ] #2 Move populate_audio_menu() method (lines 240-307) to menus.rs
- [ ] #3 Move populate_subtitle_menu() method (lines 309-376) to menus.rs
- [ ] #4 Move populate_zoom_menu() method (lines 378-500) to menus.rs
- [ ] #5 Move populate_quality_menu() method (lines 502-590) to menus.rs
- [ ] #6 Move update_playlist_position_label() method (lines 592-631) to menus.rs
- [ ] #7 All menus populate correctly and user can select options
<!-- AC:END -->
