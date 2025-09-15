---
id: task-007
title: Fix navigation stack error when revisiting Sources page
status: Done
assignee:
  - '@claude'
created_date: '2025-09-15 01:46'
updated_date: '2025-09-15 01:57'
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
- [x] #1 Can navigate to Sources page multiple times without errors
- [x] #2 Navigation stack properly handles revisiting pages
- [x] #3 Back navigation works correctly after visiting same page multiple times
- [ ] #4 No Adwaita-CRITICAL errors appear in console during navigation
<!-- AC:END -->


## Implementation Plan

1. Study current Sources navigation logic in main_window.rs around lines 555-618
2. Implement proper navigation stack checking - instead of only checking visible page, check entire stack
3. If Sources page exists in stack, pop to it instead of pushing a new one
4. Test navigation flow: Sources → Libraries → Sources


## Implementation Notes

Implemented proper navigation stack checking to detect if a page already exists anywhere in the stack, not just as visible page. Now pops to existing page instead of pushing duplicate. Maintains consistent header button setup regardless of navigation method.
