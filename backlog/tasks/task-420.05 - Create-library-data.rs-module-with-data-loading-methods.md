---
id: task-420.05
title: Create library/data.rs module with data loading methods
status: Done
assignee:
  - '@claude-code'
created_date: '2025-10-06 17:36'
updated_date: '2025-10-06 17:45'
labels: []
dependencies: []
parent_task_id: task-420
---

## Description

Extract data loading and manipulation methods from library.rs into a new data.rs module. This includes database operations, image loading, and viewport management.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Create src/ui/pages/library/data.rs file
- [x] #2 Move data loading method (load_all_items with sorting logic)
- [x] #3 Move refresh method
- [x] #4 Move viewport methods (update_visible_range)
- [x] #5 Move image loading methods (load_images_for_visible_range, cancel_pending_images)
- [x] #6 Add necessary imports (db, chrono, etc.)
- [x] #7 Code compiles without errors
<!-- AC:END -->


## Implementation Plan

1. Search for all data loading methods in library.rs
2. Create data.rs with impl LibraryPage block
3. Extract load_all_items with all sorting logic
4. Extract refresh method
5. Extract viewport tracking methods
6. Extract image loading/canceling methods
7. Add necessary imports
8. Verify compilation


## Implementation Notes

Created src/ui/pages/library/data.rs with data loading methods:

- Moved load_all_items with complete sorting logic for all SortBy variants
- Moved refresh method that clears state and reloads
- Moved viewport tracking: update_visible_range
- Moved image management: load_images_for_visible_range, cancel_pending_images
- All methods kept as impl LibraryPage block with pub(super) visibility
- Added imports for GTK, Relm4, HashMap, tracing, messages, types, and workers
- Code compiles without errors
