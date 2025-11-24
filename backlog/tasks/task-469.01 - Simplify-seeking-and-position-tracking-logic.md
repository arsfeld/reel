---
id: task-469.01
title: Simplify seeking and position tracking logic
status: Done
assignee: []
created_date: '2025-11-23 00:37'
updated_date: '2025-11-23 00:41'
labels: []
dependencies: []
parent_task_id: task-469
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Refactor the seeking implementation in gstreamer_player.rs (lines 639-797) to reduce complexity and eliminate workarounds.

Current issues:
- Dual position tracking with both `seek_pending` and `last_seek_target` 
- 100ms arbitrary delay after seeking (line 775)
- Complex position workaround in get_position() (lines 799-824)
- Manual state management during seeks

Changes needed:
1. Remove `seek_pending` and `last_seek_target` fields
2. Eliminate tokio::time::sleep(100ms) workaround
3. Wait for ASYNC_DONE message instead of arbitrary delays
4. Let pipeline manage state transitions during seeks automatically
5. Simplify get_position() to trust pipeline queries
6. Throttle UI position updates instead of tracking seek timestamps

Expected impact: Reduce seeking complexity by ~40%, improve accuracy

Reference: gstreamer-analysis.md section 6
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Remove seek_pending and last_seek_target fields
- [x] #2 Eliminate 100ms delay workaround
- [x] #3 Position tracking uses pipeline query as single source of truth
- [x] #4 Seeking completes based on ASYNC_DONE message not arbitrary delays
- [x] #5 Code complexity reduced by removing position tracking workarounds
<!-- AC:END -->
