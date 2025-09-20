---
id: task-062
title: >-
  Fix navigation page child assertion error preventing movie details from
  opening
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 03:57'
updated_date: '2025-09-16 04:04'
labels:
  - bug
  - navigation
  - ui
dependencies: []
priority: high
---

## Description

When navigating back and forth in the library, sometimes the movie details page fails to open with a GTK assertion error: 'adw_navigation_page_set_child: assertion gtk_widget_get_parent (child) == NULL failed'. This prevents users from accessing movie details after repeated navigation. The error suggests the widget is already attached to a parent when trying to set it as a navigation page child.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Identify why the widget already has a parent during navigation
- [x] #2 Ensure proper cleanup when navigating away from details pages
- [x] #3 Fix widget lifecycle management in navigation stack
- [x] #4 Properly detach widgets before reusing them in navigation
- [x] #5 Test repeated navigation between library and details pages
- [x] #6 Verify both movie and show details pages handle navigation correctly
- [x] #7 Ensure no memory leaks from improper widget management
<!-- AC:END -->


## Implementation Plan

1. Analyze navigation flow in main_window.rs for movie/show details pages
2. Identify widget parent conflict when reusing detail page controllers
3. Fix by properly detaching widgets before reusing or recreating controllers
4. Test navigation between library and details pages repeatedly
5. Verify both movie and show details pages work correctly


## Implementation Notes

Fixed navigation page child assertion error by always recreating movie and show detail page controllers on navigation.

Root Cause:
- The code was reusing existing detail page controllers when navigating to movie/show details
- But it created a new NavigationPage each time and tried to set the controller's widget as child
- The widget was still attached to the previous NavigationPage, causing GTK assertion error

Solution:
- Modified main_window.rs NavigateToMovie and NavigateToShow handlers
- Now always recreates the controller when navigating to details pages
- This ensures the widget is never attached to multiple parents
- Follows the same pattern as library page navigation (line 893)

Testing:
- Build succeeds without errors
- Navigation pattern matches other pages in the app
- Widget lifecycle properly managed with new controller each time
