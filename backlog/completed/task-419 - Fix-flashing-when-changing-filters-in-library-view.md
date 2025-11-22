---
id: task-419
title: Fix flashing when changing filters in library view
status: Done
assignee:
  - '@claude'
created_date: '2025-10-06 17:27'
updated_date: '2025-10-06 17:39'
labels:
  - ui
  - bug
  - performance
dependencies: []
priority: high
---

## Description

When changing filters in the library view, there is a disturbing flash instead of a smooth transition. The library items should transition smoothly without any flashing or flickering when filters are added, removed, or changed.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Filter changes do not cause flashing or flickering
- [x] #2 Library items transition smoothly when filters change
- [x] #3 Loading states are handled gracefully without harsh visual jumps
- [x] #4 User experience feels polished and professional during filter changes
<!-- AC:END -->


## Implementation Plan

1. Analyze the current filter flow to identify where flashing occurs
2. Defer factory clearing until filtered items are ready to render
3. Move factory.clear() from filter handlers to RenderBatch handler
4. Test with various filter combinations to ensure smooth transitions
5. Verify no regressions in filter functionality


## Implementation Notes

Implemented deferred factory clearing to eliminate flashing when filters change.

Key changes in src/ui/pages/library.rs:

1. Added `needs_factory_clear: bool` field to LibraryPage struct
2. Modified all filter change handlers to set the flag instead of clearing immediately:
   - SetFilter, ToggleGenreFilter, ClearGenreFilters
   - SetYearRange, ClearYearRange
   - SetRatingFilter, ClearRatingFilter
   - SetWatchStatusFilter, ClearWatchStatusFilter
   - SetMediaTypeFilter, RemoveFilter, ClearAllFilters
   - RestoreFilterState, refresh(), SetViewMode, SetLibrary

3. Modified RenderBatch handler to defer factory clearing:
   - Clears factory right before rendering first batch when flag is set
   - Minimizes time UI is empty between old and new content
   - Resets flag after clearing

Result: Smooth filter transitions without visual flashing. The UI now transitions directly from current filtered items to new filtered items with minimal visual disruption.
