---
id: task-358
title: Extract navigation logic to navigation.rs
status: Done
assignee:
  - '@claude'
created_date: '2025-10-03 14:31'
updated_date: '2025-10-03 14:47'
labels:
  - refactor
  - ui
dependencies: []
priority: medium
---

## Description

Move all navigation message handlers from update() to navigation.rs, including Navigate(page) match arms and navigation helper logic.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 navigation.rs file created in src/ui/main_window/
- [x] #2 All Navigate message handlers moved to navigation.rs
- [x] #3 Navigation helper functions moved to navigation.rs
- [x] #4 Navigation logic properly integrated with MainWindow
- [x] #5 Application compiles and navigation works correctly
<!-- AC:END -->


## Implementation Plan

1. Analyze navigation logic in main_window/mod.rs
2. Design navigation.rs module structure with public functions
3. Create navigation.rs file with extracted logic
4. Update mod.rs to import and use navigation module
5. Ensure all navigation handlers are properly migrated
6. Build and test navigation functionality


## Implementation Notes

Successfully extracted all navigation logic from main_window/mod.rs to navigation.rs module.

Changes made:
1. Created navigation.rs with all navigation handlers extracted from update()
2. Added module declaration to mod.rs
3. Replaced all navigation match arms in update() with function calls to navigation module
4. All navigation handlers now in dedicated functions for better organization
5. Project builds successfully with no new errors or warnings

The refactoring improves code organization by:
- Reducing main_window/mod.rs from 1673 to ~700 lines
- Separating navigation concerns into dedicated module
- Making navigation logic more maintainable and testable
- Following the same pattern as workers.rs module
