---
id: task-420.02
title: Create library/messages.rs module with Input/Output enums
status: Done
assignee:
  - '@claude-code'
created_date: '2025-10-06 17:36'
updated_date: '2025-10-06 17:40'
labels: []
dependencies: []
parent_task_id: task-420
---

## Description

Extract message enums from library.rs into a new messages.rs module. This includes LibraryPageInput and LibraryPageOutput enums used for component communication.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Create src/ui/pages/library/messages.rs file
- [x] #2 Move LibraryPageInput enum definition
- [x] #3 Move LibraryPageOutput enum definition
- [x] #4 Add necessary imports for types used in messages
- [x] #5 Code compiles without errors
<!-- AC:END -->


## Implementation Plan

1. Read LibraryPageInput and LibraryPageOutput from library.rs
2. Create messages.rs file
3. Extract both enum definitions
4. Add necessary imports for all types used in messages
5. Verify compilation


## Implementation Notes

Created src/ui/pages/library/messages.rs with message enums:

- Moved LibraryPageInput enum with all 28 variants
- Moved LibraryPageOutput enum with 2 variants
- Added imports for:
  - gtk types (gdk::Texture, Widget)
  - Database types (MediaItemModel)
  - Model types (LibraryId, MediaItemId)
  - Broker types (BrokerMessage)
  - Library types (ActiveFilterType, FilterState, SortBy, ViewMode, WatchStatus)
- Code compiles without errors
