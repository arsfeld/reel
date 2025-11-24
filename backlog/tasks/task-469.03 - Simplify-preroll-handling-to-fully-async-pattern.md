---
id: task-469.03
title: Simplify preroll handling to fully async pattern
status: Done
assignee: []
created_date: '2025-11-23 00:37'
updated_date: '2025-11-23 00:43'
labels: []
dependencies: []
parent_task_id: task-469
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Refactor the load_media() preroll handling (lines 371-466 in gstreamer_player.rs) to use fully async message handling instead of mixing synchronous and asynchronous approaches.

Current approach:
1. Set pipeline to PAUSED
2. Synchronously poll bus messages with timeout
3. Wait for StreamCollection during preroll
4. Then set up async bus watch

Issues:
- Mixing synchronous polling with async bus watch increases complexity
- Synchronous polling during preroll goes against playbin3 async design
- 500ms timeout is arbitrary

Best practice:
"Applications using playbin3 should ideally be written to deal with things completely asynchronously" - playbin3 documentation

Refactor approach:
1. Set up async bus watch FIRST (before any state changes)
2. Set pipeline to PAUSED asynchronously  
3. Wait for ASYNC_DONE message via bus watch
4. Proceed with playback

Expected impact: Cleaner async architecture, reduced complexity

Reference: gstreamer-analysis.md section 4
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Async bus watch set up before any state changes
- [x] #2 Eliminate synchronous bus message polling loop
- [x] #3 Preroll completion detected via ASYNC_DONE message
- [x] #4 load_media() simplified by removing sync/async mixing
- [x] #5 All message handling follows consistent async pattern
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Changes Made

1. **Removed synchronous bus polling loop** (previously lines 392-444)
   - Eliminated 500ms timeout window for message polling
   - Removed manual processing of initial messages during preroll
   - No longer tracking stream_collection_received flag

2. **Moved async bus watch setup before state changes**
   - Bus watch is now set up FIRST, before any pipeline state transitions
   - Follows playbin3 best practice: "deal with things completely asynchronously"

3. **Simplified state transition**
   - Just set pipeline to PAUSED and return
   - No synchronous waiting for preroll completion
   - All messages (StreamCollection, ASYNC_DONE, errors) handled by bus watch

## Benefits

- **Cleaner async architecture**: Single consistent pattern for all message handling
- **Reduced complexity**: Removed ~50 lines of synchronous polling code
- **Better performance**: No blocking during preroll
- **Proper playbin3 usage**: Aligns with official documentation guidelines

## Why This Works

- The async bus watch (using glib main loop integration) processes all messages
- StreamCollection messages are handled whenever they arrive
- ASYNC_DONE signals when pipeline is ready for seeking
- Errors are caught and reported through the bus watch
- No need for manual state polling or timeouts
<!-- SECTION:NOTES:END -->
