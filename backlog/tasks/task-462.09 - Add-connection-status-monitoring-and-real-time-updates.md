---
id: task-462.09
title: Add connection status monitoring and real-time updates
status: Done
assignee: []
created_date: '2025-11-20 23:43'
updated_date: '2025-11-21 02:12'
labels:
  - workers
  - monitoring
  - real-time
dependencies: []
parent_task_id: task-462
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement real-time monitoring of connection and authentication status with automatic UI updates.

Implementation:
- Extend ConnectionMonitor worker to check auth status periodically
- Emit broker messages when auth status changes
- Update source UI automatically when status changes
- Add debouncing to prevent excessive checks
- Handle transition from authenticated to auth-required gracefully
- Show notification when authentication becomes required
- Update source card status in real-time without page reload

This ensures users are notified promptly when re-authentication is needed.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Connection monitor checks authentication status periodically
- [x] #2 Auth status changes trigger broker messages
- [x] #3 Source UI updates in real-time when status changes
- [x] #4 Users are notified when authentication becomes required
- [x] #5 Debouncing prevents excessive checks
- [x] #6 Status monitoring doesn't impact performance
- [x] #7 Monitoring works for all backend types
<!-- AC:END -->
