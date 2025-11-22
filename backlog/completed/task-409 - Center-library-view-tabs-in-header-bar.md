---
id: task-409
title: Center library view tabs in header bar
status: Done
assignee:
  - '@arosenfeld'
created_date: '2025-10-06 03:19'
updated_date: '2025-10-06 12:01'
labels:
  - library
  - ui
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The TabBar (All/Unwatched/Recently Added) in the library header is positioned too close to the search bar on the right. It should be properly centered in the header title area for better visual balance.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 View tabs are visually centered in the header bar
- [x] #2 Proper spacing from both search bar (right) and sidebar toggle (left)
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Wrap tab_bar in a centered Box widget
2. Set proper halign and hexpand properties for centering
3. Test visual centering with different window sizes
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Wrapped the TabBar widget in a centered gtk::Box container with halign set to Center and hexpand set to true. This ensures the tab bar is properly centered in the header title area with balanced spacing from both the sidebar toggle on the left and the search bar on the right.

Modified: src/ui/pages/library.rs:946-954

The centered box is created when sending the header widget to the main window, allowing GTK to properly distribute space and center the tabs visually in the header bar.
<!-- SECTION:NOTES:END -->
