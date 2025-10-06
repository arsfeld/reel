---
id: task-412
title: Redefine library view tabs as built-in immutable filters
status: Done
assignee:
  - '@claude'
created_date: '2025-10-06 13:29'
updated_date: '2025-10-06 14:11'
labels:
  - ui
  - refactor
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Library view tabs (All, Unwatched, etc.) currently add filters to the UI, which is incorrect. Instead, they should represent pre-determined built-in views that pre-filter all items but don't show the filter in the UI. Each view is immutable and built into the system, but users can add additional filters on top of these base views.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Library view tabs pre-filter items without showing the filter in the UI
- [x] #2 Switching to 'Unwatched' view filters to unwatched items without displaying an 'unwatched' filter pill
- [x] #3 Each library view acts as an immutable built-in base filter
- [x] #4 Users can still add additional filters on top of the active library view
- [x] #5 Filter UI only shows user-added filters, not the built-in view filter
<!-- AC:END -->


## Implementation Plan

1. Remove code that adds Tab filter pills in get_active_filters method
2. Remove Tab filter removal handler in RemoveFilter match statement
3. Remove Tab variant from ActiveFilterType enum
4. Verify view tabs still filter items correctly
5. Verify filter pills only show user-added filters
6. Test that additional filters can be added on top of view filters


## Implementation Notes

Redefined library view tabs as built-in immutable filters that pre-filter items without showing in the UI.

Changes:
1. Removed Tab variant from ActiveFilterType enum (library.rs:165-171)
2. Removed Tab filter removal handler from RemoveFilter match statement (library.rs:1623-1629)
3. Removed code that adds Tab filter pills in get_active_filters method (library.rs:2610-2623)
4. Refactored filtering logic to separate view mode filtering from user-added filters:
   - Extracted is_watched computation outside match arms for reuse
   - Modified tab_match to handle Unwatched view filtering directly
   - Removed code that set watch_status_filter when view mode changed
5. Updated SetViewMode handler to not modify watch_status_filter

Now view tabs (All, Unwatched, Recently Added) act as immutable built-in filters that:
- Pre-filter items in the background without showing filter pills
- Allow users to add additional filters on top of the base view
- Keep user-added filters independent from view selection
