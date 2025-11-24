---
id: task-469.04
title: Make state queries async-aware and avoid blocking
status: Done
assignee: []
created_date: '2025-11-23 00:37'
updated_date: '2025-11-23 00:45'
labels: []
dependencies: []
parent_task_id: task-469
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Improve state query implementation to be async-aware and avoid synchronous GStreamer state queries.

Current issues:
- get_state() method (lines 896-911) queries GStreamer synchronously
- Doesn't account for async state transitions
- May return stale state during transitions

Improvements:
1. Cache state from bus handler's StateChanged messages
2. Make get_state() return cached state instead of querying
3. Add method to get "effective" state accounting for buffering
4. Remove synchronous state() queries with zero timeout
5. Trust bus handler as single source of truth for state

Benefits:
- Better performance (no GStreamer query overhead)
- Cleaner architecture (single state source)
- Async-aware state representation
- No blocking on state queries

Reference: gstreamer-analysis.md section 4
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 State cached from bus handler StateChanged messages
- [x] #2 get_state() returns cached state without GStreamer queries
- [x] #3 All synchronous playbin.state() calls removed or justified
- [x] #4 State queries never block
- [x] #5 State representation is async-aware
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Changes Made

1. **Modified get_state() to use cached state**
   - Removed synchronous GStreamer query `playbin.state(gst::ClockTime::ZERO)`
   - Now returns cached state from `self.state` which is updated by bus handler
   - No blocking on state queries during normal operation

2. **Documented justified state queries**
   - Added comments explaining remaining state queries in `get_video_dimensions()`
   - These queries are justified because video dimensions require PAUSED state
   - Edge case that doesn't affect normal playback flow

3. **Async-aware state representation**
   - Cached state is updated by bus handler's StateChanged messages
   - Single source of truth for state managed by async bus watch
   - State queries return immediately without blocking

## State Query Inventory

**Removed:**
- `get_state()`: Line 709 - synchronous query removed, now uses cache

**Justified (remaining):**
- `get_video_dimensions()`: Lines 661, 676 - Required to verify/wait for PAUSED state before querying video caps. Edge case for dimension detection.
- `stream_manager.rs`: Line 309 - Debugging/logging only, doesn't affect functionality

## Benefits

- **No blocking in normal operation**: State queries return immediately
- **Async-aware**: State properly represents async transitions
- **Better performance**: No GStreamer query overhead for state checks
- **Cleaner architecture**: Bus handler is single source of truth
<!-- SECTION:NOTES:END -->
