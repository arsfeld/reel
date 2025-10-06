---
id: task-414
title: Fix sort order persisting when switching between library tabs
status: Done
assignee:
  - '@claude'
created_date: '2025-10-06 14:53'
updated_date: '2025-10-06 18:46'
labels:
  - ui
  - bug
dependencies: []
priority: high
---

## Description

When switching to the Recently Added tab, the sort order changes to 'Date Added'. This sort order incorrectly persists when switching back to other tabs (Movies, TV Shows, etc.) instead of maintaining each tab's independent sort preference.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Sort order in Recently Added tab defaults to Date Added
- [x] #2 Sort order does not persist when switching from Recently Added to other tabs
- [x] #3 Each library tab maintains its own independent sort order state
<!-- AC:END -->


## Implementation Plan

1. Analyze current filter state saving mechanism
2. Modify FilterState to store sort preferences per view mode
3. Update save_filter_state to preserve sort order per view mode
4. Update apply_filter_state to restore view mode-specific sort order
5. Update SetViewMode handler to restore sort order when switching views
6. Test switching between view modes maintains independent sort orders


## Implementation Notes

Fixed sort order persisting across library view tabs by implementing view mode-specific sort preferences.

Changes:

1. Modified FilterState structure (src/ui/pages/library/types.rs):
   - Added ViewModeSortPrefs struct to hold sort_by and sort_order per view mode
   - Changed FilterState to use HashMap<ViewMode, ViewModeSortPrefs> instead of single sort_by/sort_order fields
   - Updated Default implementation to set appropriate defaults for each view mode (Title/Ascending for All and Unwatched, DateAdded/Descending for Recently Added)
   - Added Eq and Hash traits to SortBy and ViewMode enums to enable HashMap usage

2. Updated LibraryPage (src/ui/pages/library/mod.rs):
   - Added view_mode_sort_prefs field to store per-view-mode sort preferences
   - Initialized field in init() with default preferences from FilterState

3. Enhanced filter state management (src/ui/pages/library/filters.rs):
   - Updated apply_filter_state() to restore view_mode_sort_prefs and apply sort settings for current view mode
   - Updated from_library_page() to save current view mode sort preferences

4. Modified SetViewMode handler (src/ui/pages/library/mod.rs):
   - Added logic to restore sort preferences when switching view modes
   - Ensures each view mode maintains independent sort order

Result: Each library tab (All, Unwatched, Recently Added) now maintains its own independent sort order. Switching between tabs restores the appropriate sort order for that tab, and changes to sort order in one tab do not affect other tabs.

Additional fix: Made Recently Added tab truly immutable
- Sort controls are now ignored when in Recently Added view mode
- SetSortBy and ToggleSortOrder handlers return early if in Recently Added mode
- SetViewMode always forces DateAdded/Descending when switching to Recently Added
- apply_filter_state enforces DateAdded/Descending for Recently Added
- from_library_page skips saving sort preferences for Recently Added mode

This ensures Recently Added tab ALWAYS shows DateAdded/Descending sort order, regardless of any user attempts to change it or saved preferences.
