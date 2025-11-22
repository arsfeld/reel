---
id: task-317
title: Fix ConfigManager crash due to main context conflict
status: Done
assignee:
  - '@claude'
created_date: '2025-09-29 13:56'
updated_date: '2025-10-02 15:02'
labels:
  - bug
  - crash
  - worker
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The ConfigManager worker is crashing with a panic when trying to acquire the main context that's already been acquired by another thread. This happens in config_manager.rs:234 when calling glib::source::timeout_add_local_once. The worker thread is trying to use GTK main context functions that should only be used from the main thread. Need to refactor the ConfigManager to use proper thread-safe communication or move the timeout to the main thread.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Analyze the ConfigManager worker implementation to understand the main context usage
- [x] #2 Replace timeout_add_local_once with a thread-safe alternative
- [x] #3 Use proper channel communication instead of main context timeouts in worker thread
- [x] #4 Test that config file changes are still properly detected and handled
- [x] #5 Verify no more crashes occur when config file changes
- [x] #6 Ensure the fix doesn't break config reload functionality
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Analyze current implementation - the issue is at line 233 using gtk::glib::timeout_add_local_once which requires main context
2. Replace timeout_add_local_once with tokio::time::sleep for debouncing
3. Move debouncing logic into the spawned async task
4. Test config reload still works with debouncing
5. Build and verify no crashes occur
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fixed ConfigManager crash by replacing gtk::glib::timeout_add_local_once with tokio::time::sleep.

## Changes
- Removed relm4::gtk import (no longer needed)
- Replaced gtk::glib::timeout_add_local_once wrapper with direct relm4::spawn_local call
- Added tokio::time::sleep(Duration::from_millis(100)).await for debouncing inside the async block

## Why
The ConfigManager worker was crashing because timeout_add_local_once requires the GLib main context, which can only be acquired from the main thread. Since this is a Worker running in a separate thread, using main context functions caused a panic.

## Solution
Used tokio::time::sleep instead, which is thread-safe and works correctly from worker threads. The debouncing behavior (100ms delay) is preserved, and all tests pass.

Modified file: src/workers/config_manager.rs:234
Build: ✅ Success (0 errors)
Tests: ✅ All 236 tests passed
<!-- SECTION:NOTES:END -->
