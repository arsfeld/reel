---
id: task-356
title: Extract window setup to window_setup.rs
status: To Do
assignee: []
created_date: '2025-10-03 14:31'
labels:
  - refactor
  - ui
dependencies: []
priority: medium
---

## Description

Move window actions (preferences, about, quit), platform-specific styling, keyboard shortcuts, and primary menu creation to window_setup.rs.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 window_setup.rs file created in src/ui/main_window/
- [ ] #2 Window actions code moved from init() to window_setup.rs
- [ ] #3 Platform-specific styling code moved to window_setup.rs
- [ ] #4 Primary menu creation moved to window_setup.rs
- [ ] #5 Application compiles and window setup works correctly
<!-- AC:END -->
