---
id: task-179
title: Implement Reconnecting status in ConnectionMonitor
status: To Do
assignee: []
created_date: '2025-09-18 15:18'
updated_date: '2025-10-02 14:56'
labels:
  - feature
  - workers
dependencies: []
priority: medium
---

## Description

The ConnectionStatus::Reconnecting enum variant is defined but never used. ConnectionMonitor should send this status while attempting to reconnect, providing better user feedback during reconnection attempts.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Add Reconnecting status to ConnectionMonitorOutput enum
- [ ] #2 Send Reconnecting status before attempting reconnection in ConnectionMonitor
- [ ] #3 Update UI components to show reconnecting state (spinner, different icon)
- [ ] #4 Test reconnecting status is properly displayed in sidebar and sources page
<!-- AC:END -->
