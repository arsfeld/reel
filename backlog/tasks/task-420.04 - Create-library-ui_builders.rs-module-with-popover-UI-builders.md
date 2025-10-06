---
id: task-420.04
title: Create library/ui_builders.rs module with popover UI builders
status: Done
assignee:
  - '@claude-code'
created_date: '2025-10-06 17:36'
updated_date: '2025-10-06 17:43'
labels: []
dependencies: []
parent_task_id: task-420
---

## Description

Extract UI building methods from library.rs into a new ui_builders.rs module. This includes all methods that create and update popover UIs for filters.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Create src/ui/pages/library/ui_builders.rs file
- [x] #2 Move popover update methods (update_genre_popover, update_year_popover, update_rating_popover, update_watch_status_popover)
- [x] #3 Move unified filters popover method (update_unified_filters_popover)
- [x] #4 Move section builder methods (build_genre_filter_section, build_year_filter_section, build_rating_filter_section, build_watch_status_filter_section)
- [x] #5 Add necessary imports (gtk, relm4)
- [x] #6 Code compiles without errors
<!-- AC:END -->


## Implementation Plan

1. Search for all UI builder methods in library.rs
2. Create ui_builders.rs with impl LibraryPage block
3. Extract popover update methods (genre, year, rating, watch status)
4. Extract unified filters popover method
5. Extract section builder methods
6. Add necessary imports
7. Verify compilation


## Implementation Notes

Created src/ui/pages/library/ui_builders.rs with UI building methods:

- Moved 4 popover update methods: update_genre_popover, update_year_popover, update_rating_popover, update_watch_status_popover
- Moved unified filters popover: update_unified_filters_popover
- Moved 4 section builder methods: build_genre_filter_section, build_year_filter_section, build_rating_filter_section, build_watch_status_filter_section
- All methods kept as impl LibraryPage block with pub(super) visibility
- Added imports for GTK, Relm4, messages, and types
- Code compiles without errors
