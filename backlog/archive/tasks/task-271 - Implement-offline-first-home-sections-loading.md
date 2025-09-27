---
id: task-271
title: Implement offline-first home sections loading
status: To Do
assignee: []
created_date: '2025-09-26 18:06'
labels:
  - backend
  - offline
  - performance
dependencies: []
---

## Description

Modify home sections loading to always prioritize cached data from the database first, then update in the background if the source is online. This ensures instant UI loading and follows the app's offline-first architecture.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Always load home sections from HomeSectionRepository first
- [ ] #2 Return cached sections immediately for instant UI
- [ ] #3 Trigger background refresh only if source is online and data is stale
- [ ] #4 Update UI reactively when fresh data arrives from background sync
- [ ] #5 Implement staleness threshold (e.g., 1 hour) for background refresh
- [ ] #6 Add metrics to track cache usage and refresh patterns
<!-- AC:END -->
