---
id: task-173
title: Refactor platform abstraction layer
status: In Progress
assignee:
  - '@assistant'
created_date: '2025-09-18 14:12'
updated_date: '2025-09-18 14:36'
labels:
  - refactoring
  - architecture
dependencies: []
priority: high
---

## Description

Move the core Relm4 application files (app.rs, main_window.rs) from src/platforms/relm4/ to a more integrated location while maintaining platform abstraction capability.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Move src/platforms/relm4/app.rs to src/app/relm4_app.rs
- [ ] #2 Move src/platforms/relm4/components/main_window.rs to src/ui/main_window.rs
- [ ] #3 Create proper platform abstraction in src/app/ for future platform support
- [ ] #4 Update main.rs to use new app structure
- [ ] #5 Ensure application still launches correctly
- [ ] #6 Verify platform detection still works
<!-- AC:END -->
