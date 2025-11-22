---
id: task-466.03
title: Extract control visibility state machine to separate module
status: Done
assignee: []
created_date: '2025-11-22 18:34'
updated_date: '2025-11-22 18:48'
labels: []
dependencies:
  - task-466.07
parent_task_id: task-466
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Extract the control visibility state machine logic from player.rs into a dedicated `controls_visibility.rs` module. This includes the state machine, transitions, and helper methods.

Current location: Lines 29-443 in player.rs
Code to extract:
- `ControlState` enum (Hidden, Visible, Hovering)
- `transition_to_hidden()` method
- `transition_to_visible()` method
- `transition_to_hovering()` method
- `controls_visible()` method
- `mouse_movement_exceeds_threshold()` method
- `is_mouse_over_controls()` method

Required state fields:
- `control_state: ControlState`
- `last_mouse_position: Option<(f64, f64)>`
- `active_popover_count: Rc<RefCell<usize>>`
- `window: adw::ApplicationWindow`
- `controls_overlay: Option<gtk::Box>`
- `inactivity_timeout_secs: u64`
- `mouse_move_threshold: f64`
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 New file created: src/ui/pages/player/controls_visibility.rs
- [ ] #2 ControlState enum and all transition methods moved to new module
- [ ] #3 State machine encapsulated with clear public API
- [ ] #4 Code compiles without errors
- [ ] #5 Control visibility and cursor hiding behavior works correctly
<!-- AC:END -->
