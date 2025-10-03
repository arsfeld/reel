---
id: task-392.06
title: Update mod.rs to wire all player modules together
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
Convert player.rs to a module directory structure with mod.rs as the entry point. Add re-exports and ensure the AsyncComponent implementation uses all the new modules correctly.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Create src/ui/pages/player/mod.rs with re-exports
- [ ] #2 Add 'pub use types::*;' to re-export message types
- [ ] #3 Add 'pub use state::*;' to re-export PlayerPage and ControlState
- [ ] #4 Add 'pub use helpers::*;' to re-export utility functions
- [ ] #5 Declare controls and menus as modules
- [ ] #6 Remove original src/ui/pages/player.rs file
- [ ] #7 AsyncComponent implementation compiles with new module structure
- [ ] #8 All imports in other files updated to use player module correctly
<!-- AC:END -->
