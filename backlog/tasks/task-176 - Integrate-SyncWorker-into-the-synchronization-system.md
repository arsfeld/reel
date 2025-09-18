---
id: task-176
title: Integrate SyncWorker into the synchronization system
status: To Do
assignee: []
created_date: '2025-09-18 14:27'
labels:
  - feature
  - workers
  - high-priority
  - sync
dependencies: []
priority: high
---

## Description

The SyncWorker was moved from src/platforms/relm4/components/workers/ to src/workers/ but is not being used according to task 169. This worker needs to be properly integrated with the sync service to handle background synchronization of media libraries from multiple backends.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Review SyncWorker implementation and capabilities
- [ ] #2 Integrate SyncWorker with src/services/core/sync.rs
- [ ] #3 Replace or augment existing sync logic with SyncWorker
- [ ] #4 Implement proper sync scheduling and progress reporting
- [ ] #5 Connect SyncWorker to UI components for sync status display
- [ ] #6 Handle concurrent syncs from multiple backends properly
- [ ] #7 Add sync progress indicators to the sources page
- [ ] #8 Test synchronization with large libraries (1000+ items)
- [ ] #9 Ensure sync state persists across application restarts
<!-- AC:END -->
