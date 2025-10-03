---
id: task-392
title: Break down player.rs into smaller modules
status: To Do
assignee: []
created_date: '2025-10-04 02:22'
labels:
  - refactor
  - ui
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split the large player.rs file (~2800 lines) into smaller, focused modules for better maintainability and navigation. This is a pure organizational split with no logic changes - all behavior must remain identical.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 All new module files created in src/ui/pages/player/ directory
- [ ] #2 All imports and re-exports working correctly
- [ ] #3 Application compiles without errors
- [ ] #4 All existing tests pass
- [ ] #5 No behavioral changes - player functionality works identically
<!-- AC:END -->
