---
id: task-392.01
title: Create types.rs module for player message types
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
Extract PlayerInput, PlayerOutput, and PlayerCommandOutput enums into a dedicated types.rs file. This is the safest first step with no dependencies on other modules.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Create src/ui/pages/player/types.rs file
- [ ] #2 Move PlayerInput enum (~80 lines, lines 646-725) to types.rs
- [ ] #3 Move PlayerOutput enum to types.rs
- [ ] #4 Move PlayerCommandOutput enum and Debug impl to types.rs
- [ ] #5 All type definitions compile correctly
<!-- AC:END -->
