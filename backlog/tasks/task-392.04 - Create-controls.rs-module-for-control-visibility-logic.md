---
id: task-392.04
title: Create controls.rs module for control visibility logic
status: To Do
assignee: []
created_date: '2025-10-04 02:22'
labels:
  - refactor
  - ui
dependencies: []
parent_task_id: task-392
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Extract control visibility state machine methods into controls.rs as impl blocks for PlayerPage. These handle showing/hiding controls and cursor management.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Create src/ui/pages/player/controls.rs file
- [ ] #2 Move transition_to_hidden() method (lines 129-154) to controls.rs
- [ ] #3 Move transition_to_visible() method (lines 156-183) to controls.rs
- [ ] #4 Move transition_to_hovering() method (lines 185-202) to controls.rs
- [ ] #5 Move controls_visible() method (lines 204-207) to controls.rs
- [ ] #6 Move mouse_movement_exceeds_threshold() method (lines 209-219) to controls.rs
- [ ] #7 Move is_mouse_over_controls() method (lines 221-238) to controls.rs
- [ ] #8 All methods accessible and control visibility works correctly
<!-- AC:END -->
