---
id: task-392.03
title: Create helpers.rs module for utility functions
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
Extract standalone helper functions into helpers.rs. These are pure utility functions with no dependencies.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Create src/ui/pages/player/helpers.rs file
- [ ] #2 Move format_duration() function (lines 16-27) to helpers.rs
- [ ] #3 Function is publicly accessible and works correctly
- [ ] #4 All usages of format_duration updated to use new module path
<!-- AC:END -->
