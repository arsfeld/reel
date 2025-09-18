---
id: task-169
title: Investigate why SyncWorker is not being used
status: To Do
assignee: []
created_date: '2025-09-18 14:05'
labels: []
dependencies: []
priority: high
---

## Description

The SyncWorker component exists for background synchronization but is not being used. Currently, sync operations are being called directly from UI components. Need to understand the architectural decision and determine if SyncWorker should be integrated or removed.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Analyze current sync implementation in main_window.rs and sources.rs
- [ ] #2 Review SyncWorker capabilities and design
- [ ] #3 Determine if SyncWorker should be integrated for better architecture
- [ ] #4 Document findings and recommendation
- [ ] #5 Implement chosen solution (integrate SyncWorker or remove it)
<!-- AC:END -->
