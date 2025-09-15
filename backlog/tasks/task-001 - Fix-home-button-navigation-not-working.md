---
id: task-001
title: Fix home button navigation not working
status: To Do
assignee: []
created_date: '2025-09-15 01:39'
labels:
  - ui
  - navigation
  - bug
dependencies: []
priority: high
---

## Description

The home button in the sidebar sends the correct navigation event but the home page doesn't display. The navigation logic in MainWindow may have race conditions or state issues preventing proper home page display.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Home button click navigates to home page successfully
- [ ] #2 Navigation stack is properly reset when navigating home
- [ ] #3 Home page content loads and displays after navigation
<!-- AC:END -->
