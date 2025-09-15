---
id: task-035
title: Remove machine_id from Sources page UI
status: To Do
assignee: []
created_date: '2025-09-15 15:36'
labels:
  - ui
  - sources
  - cleanup
dependencies: []
priority: medium
---

## Description

The Sources page currently displays the machine_id for connected servers, which is a long, technical identifier that provides no value to users and clutters the interface. This internal identifier should be hidden from the user-facing UI.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Remove machine_id display from Sources page server cards
- [ ] #2 Ensure machine_id is still stored internally for backend operations
- [ ] #3 Verify no other UI components display machine_id
<!-- AC:END -->
