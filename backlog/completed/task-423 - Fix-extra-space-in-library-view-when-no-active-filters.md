---
id: task-423
title: Fix extra space in library view when no active filters
status: Done
assignee:
  - '@claude'
created_date: '2025-10-06 18:45'
updated_date: '2025-10-06 18:49'
labels: []
dependencies: []
priority: high
---

## Description

When switching to Recently Added or Unwatched tabs in library view, the active filters component becomes visible but displays no filters, creating noticeable extra space between the filters toolbar and the media grid. The filters container should be hidden when there are no active filters to display.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Active filters container is hidden when empty
- [x] #2 No extra space appears between toolbar and library content
- [x] #3 Behavior is consistent across all view modes (All, Unwatched, Recently Added)
- [x] #4 Visual regression testing shows no unwanted spacing
<!-- AC:END -->


## Implementation Plan

1. Analyze the issue: `has_active_filters()` returns true for ViewMode changes, but `get_active_filters_list()` returns empty list
2. Change the visibility condition to check if filter pills list is non-empty instead of `has_active_filters()`
3. Test that container is hidden when on Unwatched/Recently Added tabs with no other filters
4. Test that container appears when actual filters are added
5. Verify no regressions in other view modes


## Implementation Notes

Fixed the extra space in library view when no active filters are present.


## Root Cause
The visibility condition for the active filters container was checking `has_active_filters()`, which returns `true` when ViewMode is not "All" (i.e., when on Unwatched or Recently Added tabs). However, `get_active_filters_list()` doesn't include ViewMode in the filter pills list, resulting in an empty visible container.

## Solution
Changed the visibility condition in `src/ui/pages/library/mod.rs:400` from:
```rust
set_visible: model.has_active_filters()
```
to:
```rust
set_visible: !model.get_active_filters_list().is_empty()
```

This ensures the active filters container is only visible when there are actual filter pills to display, eliminating the extra space when switching to Unwatched or Recently Added tabs with no other active filters.

## Files Modified
- `src/ui/pages/library/mod.rs` - Updated visibility condition for active filters container
