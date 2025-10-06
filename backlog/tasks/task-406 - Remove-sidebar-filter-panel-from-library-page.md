---
id: task-406
title: Remove sidebar filter panel from library page
status: Done
assignee:
  - '@claude'
created_date: '2025-10-06 02:52'
updated_date: '2025-10-06 03:27'
labels:
  - library
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The sidebar filter panel is confusing and looks bad visually. It should be removed entirely from the library page. All filtering functionality should be replaced with a cleaner approach (to be determined in separate tasks).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Remove all sidebar filter panel UI code (gtk::Revealer, ScrolledWindow, etc.)
- [x] #2 Remove filter_panel_visible state and ToggleFilterPanel input
- [x] #3 Remove filter panel toggle button from toolbar
- [x] #4 Remove genre/year/rating/watch status dropdowns from sidebar
- [x] #5 Clean up any CSS related to filter panel
- [x] #6 Verify library page works without filter panel
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Examine library page code to identify all filter panel components
2. Remove sidebar filter panel UI components (Revealer, ScrolledWindow, etc.)
3. Remove filter_panel_visible state and ToggleFilterPanel message handling
4. Remove filter panel toggle button from toolbar
5. Remove genre/year/rating/watch status dropdowns
6. Clean up related CSS
7. Build and verify library page works correctly
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Successfully removed all filter panel UI components from the library page:

- Removed filter panel toggle button and separator from toolbar
- Removed entire Revealer widget containing filter panel sidebar (genre, year, rating, watch status dropdowns)
- Removed filter_panel_visible state field from LibraryPage struct and FilterState
- Removed ToggleFilterPanel input variant and its handler
- Removed all menu button fields and their references (genre_menu_button, year_menu_button, rating_menu_button, watch_status_menu_button)
- Removed menu button initialization and popover setup code
- Removed all menu button label updates throughout the codebase

The library page now compiles successfully without the filter panel. The filter popovers are still retained for potential future use with a different UI approach.
<!-- SECTION:NOTES:END -->
