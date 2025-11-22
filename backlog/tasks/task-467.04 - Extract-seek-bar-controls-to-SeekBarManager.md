---
id: task-467.04
title: Extract seek bar controls to SeekBarManager
status: Done
assignee: []
created_date: '2025-11-22 19:06'
updated_date: '2025-11-22 22:03'
labels:
  - refactoring
  - player
  - ui
dependencies: []
parent_task_id: task-467
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Extract seek bar widget and position/duration display from player mod.rs into a dedicated `seek_bar.rs` module with a SeekBarManager struct.

State to extract:
- `seek_bar: gtk::Scale`
- `position_label: gtk::Label`
- `duration_label: gtk::Label`
- `is_seeking: bool`

Logic to extract:
- Seek bar widget creation and initialization
- Click and drag gesture handling
- Position/duration label formatting
- Tooltip with time preview on hover
- Seek bar value updates during playback
- Drag state management to prevent position flicker

SeekBarManager API:
- `new(sender) -> Self`
- `get_seek_bar() -> &gtk::Scale`
- `get_position_label() -> &gtk::Label`
- `get_duration_label() -> &gtk::Label`
- `update_position(position: Duration, is_seeking: bool)`
- `update_duration(duration: Duration)`
- `set_seeking(seeking: bool)`
- `reset()` - Clear to initial state
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Create src/ui/pages/player/seek_bar.rs with SeekBarManager struct
- [x] #2 Extract seek bar widget and label widgets from PlayerPage
- [x] #3 Move click/drag gesture handling to manager
- [x] #4 Move position/duration formatting to manager
- [x] #5 Code compiles without errors
- [x] #6 Seek bar works correctly with click, drag, and keyboard shortcuts
<!-- AC:END -->
