---
id: task-467.05
title: Extract volume controls to VolumeManager
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
Extract volume control widget and volume adjustment logic from player mod.rs into a dedicated `volume.rs` module with a VolumeManager struct.

State to extract:
- `volume: f64`
- `volume_slider: gtk::Scale`

Logic to extract:
- Volume slider widget creation and initialization
- Volume slider value change handler
- Volume up/down keyboard shortcuts (10% increments)
- Volume state synchronization with player backend
- Volume slider UI updates

VolumeManager API:
- `new(sender) -> Self`
- `get_volume_slider() -> &gtk::Scale`
- `get_volume() -> f64`
- `set_volume(volume: f64, player: &PlayerHandle)`
- `volume_up(sender)` - Increase by 10%
- `volume_down(sender)` - Decrease by 10%
- `sync_from_player(volume: f64)` - Update from player state
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Create src/ui/pages/player/volume.rs with VolumeManager struct
- [x] #2 Extract volume state and slider widget from PlayerPage
- [x] #3 Move volume adjustment logic to manager
- [x] #4 Move volume up/down handlers to manager
- [x] #5 Code compiles without errors
- [x] #6 Volume controls work correctly with slider and keyboard shortcuts
<!-- AC:END -->
