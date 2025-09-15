---
id: task-007
title: Fix navigation stack error when revisiting Sources page
status: To Do
assignee: []
created_date: '2025-09-15 01:46'
labels:
  - ui
  - navigation
  - bug
dependencies: []
priority: high
---

## Description

Navigating to Sources, then Libraries, then Sources again causes a critical error: 'Page 'Sources' is already in navigation stack'. The navigation logic in MainWindow incorrectly tries to push a page that's already in the stack instead of popping back to it or replacing the current page.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Can navigate to Sources page multiple times without errors
- [ ] #2 Navigation stack properly handles revisiting pages
- [ ] #3 Back navigation works correctly after visiting same page multiple times
- [ ] #4 No Adwaita-CRITICAL errors appear in console during navigation
<!-- AC:END -->
