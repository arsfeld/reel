---
id: task-357
title: Extract component initialization to components.rs
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

Move sidebar, home page, and auth dialog initialization from main_window init() to a separate components.rs file.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 components.rs file created in src/ui/main_window/
- [ ] #2 Sidebar initialization moved to components.rs
- [ ] #3 Home page initialization moved to components.rs
- [ ] #4 Auth dialog initialization moved to components.rs
- [ ] #5 Components properly returned and stored in MainWindow struct
- [ ] #6 Application compiles and components function correctly
<!-- AC:END -->
