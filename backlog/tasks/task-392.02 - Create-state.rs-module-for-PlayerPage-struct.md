---
id: task-392.02
title: Create state.rs module for PlayerPage struct
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
Extract PlayerPage struct definition, ControlState enum, and related implementations into state.rs. Depends only on types.rs.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Create src/ui/pages/player/state.rs file
- [ ] #2 Move ControlState enum (lines 29-38) to state.rs
- [ ] #3 Move PlayerPage struct (lines 40-120) to state.rs
- [ ] #4 Move Debug impl for PlayerPage to state.rs
- [ ] #5 Move configuration constants (DEFAULT_INACTIVITY_TIMEOUT_SECS, etc.) to state.rs
- [ ] #6 All struct definitions compile with proper imports
<!-- AC:END -->
