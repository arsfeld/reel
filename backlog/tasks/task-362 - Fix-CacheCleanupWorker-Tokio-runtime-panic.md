---
id: task-362
title: Fix CacheCleanupWorker Tokio runtime panic
status: Done
assignee:
  - '@assistant'
created_date: '2025-10-03 14:45'
updated_date: '2025-10-03 14:49'
labels:
  - bug
  - workers
  - critical
dependencies: []
priority: high
---

## Description

The CacheCleanupWorker crashes with 'there is no reactor running, must be called from the context of a Tokio 1.x runtime' at line 352 in start_timer(). Relm4 workers run in glib context, not Tokio runtime, so tokio::time::interval cannot be used directly. Need to use glib timers or spawn the timer in a Tokio context.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Identify the problematic tokio::time::interval call in cache_cleanup_worker.rs:352
- [x] #2 Replace Tokio interval with glib timer or proper async spawning
- [x] #3 Worker starts without panicking
- [x] #4 Cleanup timer functionality works correctly
- [x] #5 Application runs without crash on startup
<!-- AC:END -->


## Implementation Plan

1. Identify the problematic tokio::time::interval at line 352 in start_timer()
2. Remove the unused cleanup_timer field and related interval code
3. Clean up the Interval import that's no longer needed
4. Build and test that worker starts without panicking
5. Verify cleanup functionality still works correctly


## Implementation Notes

Fixed the CacheCleanupWorker Tokio runtime panic by removing the unused cleanup_timer field and problematic tokio::time::interval call.

Root cause: Line 352 in start_timer() was creating a Tokio interval outside of a Tokio runtime context. Relm4 workers run in glib context, not Tokio runtime.

Solution: The cleanup_timer field was completely unused - the actual periodic cleanup was already properly implemented using relm4::spawn with tokio::time::sleep (lines 387-392). Removed:
- Unused Interval import
- cleanup_timer field from struct
- interval() call in start_timer()
- Related cleanup_timer assignments

The worker now starts successfully without panicking, and cleanup functionality continues to work as the actual timer mechanism was already correct.
