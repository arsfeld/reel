---
id: task-185
title: Update sidebar sync status to be source-based instead of library-based
status: To Do
assignee: []
created_date: '2025-09-18 16:46'
updated_date: '2025-10-02 14:56'
labels:
  - ui
  - sidebar
  - sync
dependencies: []
priority: high
---

## Description

The sidebar currently updates sync status on a per-library basis, but it should update on a per-source basis. Additionally, remove the green checkmark indicators and only show error states in the sidebar for a cleaner interface.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Refactor sidebar sync status updates to track at source level instead of library level
- [ ] #2 Remove green checkmark indicators from sidebar
- [ ] #3 Only display error states in sidebar sync status
- [ ] #4 Ensure sync progress and completion update the entire source section
<!-- AC:END -->
