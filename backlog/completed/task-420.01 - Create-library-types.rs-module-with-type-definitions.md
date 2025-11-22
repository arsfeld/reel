---
id: task-420.01
title: Create library/types.rs module with type definitions
status: Done
assignee:
  - '@claude-code'
created_date: '2025-10-06 17:36'
updated_date: '2025-10-06 17:39'
labels: []
dependencies: []
parent_task_id: task-420
---

## Description

Extract type definitions from library.rs into a new types.rs module. This includes enums (SortBy, SortOrder, WatchStatus, ViewMode) and structs (FilterStatistics, ActiveFilter, ActiveFilterType, FilterState).

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Create src/ui/pages/library/types.rs file
- [x] #2 Move all enum definitions (SortBy, SortOrder, WatchStatus, ViewMode)
- [x] #3 Move all struct definitions (FilterStatistics, ActiveFilter, ActiveFilterType, FilterState)
- [x] #4 Keep all implementations with their types
- [x] #5 Add necessary imports (serde, etc.)
- [x] #6 Code compiles without errors
<!-- AC:END -->


## Implementation Plan

1. Create library directory: mkdir -p src/ui/pages/library
2. Extract type definitions from library.rs
3. Create types.rs with all enums and structs
4. Keep all Default implementations
5. Remove from_library_page method (will be added back in 420.06)
6. Verify compilation with cargo check


## Implementation Notes

Created src/ui/pages/library/types.rs with all type definitions from library.rs:

- Moved SortBy, SortOrder, WatchStatus, ViewMode enums with Default impls
- Moved FilterStatistics, ActiveFilter, ActiveFilterType, FilterState structs
- Kept FilterState URL serialization methods (to_url_params, from_url_params)
- Temporarily removed from_library_page method to avoid circular dependency (will be restored in task 420.06)
- All necessary imports added (serde)
- Code compiles without errors (verified with cargo check)
