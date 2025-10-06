---
id: task-420.03
title: Create library/filters.rs module with filter helper methods
status: Done
assignee:
  - '@claude-code'
created_date: '2025-10-06 17:36'
updated_date: '2025-10-06 17:41'
labels: []
dependencies: []
parent_task_id: task-420
---

## Description

Extract filter-related helper methods from library.rs into a new filters.rs module. This includes methods for filter state management, statistics, and display helpers.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Create src/ui/pages/library/filters.rs file
- [x] #2 Move filter state methods (apply_filter_state, save_filter_state)
- [x] #3 Move filter statistics methods (get_filter_statistics, has_active_filters, get_active_filters_list, get_filter_suggestions)
- [x] #4 Move filter display methods (update_active_filters_display, get_active_filter_count)
- [x] #5 Move label generation methods (get_genre_label, get_year_label, get_rating_label, get_watch_status_label)
- [x] #6 Add necessary imports and make methods available to main component
- [x] #7 Code compiles without errors
<!-- AC:END -->


## Implementation Plan

1. Search for all filter-related methods in library.rs
2. Create filters.rs with impl LibraryPage block
3. Extract all filter state methods (apply, save)
4. Extract statistics methods
5. Extract display helper methods (get_active_filter_count, label generators)
6. Extract filter list methods
7. Add FilterState::from_library_page back to types.rs
8. Verify compilation


## Implementation Notes

Created src/ui/pages/library/filters.rs with filter helper methods:

- Moved filter state management: apply_filter_state, save_filter_state
- Moved filter statistics: get_filter_statistics, has_active_filters, get_active_filters_list, get_filter_suggestions
- Moved display methods: update_active_filters_display, get_active_filter_count
- Moved label generators: get_genre_label, get_year_label, get_rating_label, get_watch_status_label
- All methods kept as impl LibraryPage block with pub(super) visibility
- Added necessary imports for types, messages, and GTK
- Restored FilterState::from_library_page to types.rs (needed by save_filter_state)
- Code compiles without errors
