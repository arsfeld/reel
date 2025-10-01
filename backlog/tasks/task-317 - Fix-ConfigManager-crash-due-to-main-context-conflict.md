---
id: task-317
title: Fix ConfigManager crash due to main context conflict
status: To Do
assignee: []
created_date: '2025-09-29 13:56'
labels:
  - bug
  - crash
  - worker
dependencies: []
priority: high
---

## Description

The ConfigManager worker is crashing with a panic when trying to acquire the main context that's already been acquired by another thread. This happens in config_manager.rs:234 when calling glib::source::timeout_add_local_once. The worker thread is trying to use GTK main context functions that should only be used from the main thread. Need to refactor the ConfigManager to use proper thread-safe communication or move the timeout to the main thread.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Analyze the ConfigManager worker implementation to understand the main context usage
- [ ] #2 Replace timeout_add_local_once with a thread-safe alternative
- [ ] #3 Use proper channel communication instead of main context timeouts in worker thread
- [ ] #4 Test that config file changes are still properly detected and handled
- [ ] #5 Verify no more crashes occur when config file changes
- [ ] #6 Ensure the fix doesn't break config reload functionality
<!-- AC:END -->
