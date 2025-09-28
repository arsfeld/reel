---
id: task-290
title: Add config change subscriptions to remaining UI components
status: To Do
assignee: []
created_date: '2025-09-28 01:07'
labels:
  - config
  - ui
  - enhancement
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Update the remaining UI components (player page, home page, library views) to subscribe to config changes via MessageBroker and react appropriately when configuration is updated at runtime.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Player page subscribes to config changes and updates player if needed
- [ ] #2 Home page reacts to relevant config changes
- [ ] #3 Library views update based on config changes
- [ ] #4 Components properly unsubscribe when destroyed
<!-- AC:END -->
