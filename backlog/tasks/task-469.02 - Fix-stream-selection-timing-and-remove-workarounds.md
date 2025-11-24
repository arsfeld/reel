---
id: task-469.02
title: Fix stream selection timing and remove workarounds
status: Done
assignee: []
created_date: '2025-11-23 00:37'
updated_date: '2025-11-23 00:42'
labels: []
dependencies: []
parent_task_id: task-469
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Investigate and fix the root cause of "reconfiguration freeze" when sending SELECT_STREAMS during PLAYING state (bus_handler.rs lines 215-226).

Current workaround:
```rust
if current_state < gst::State::Playing {
    temp_stream_manager.send_default_stream_selection(&collection, &pb);
} else {
    debug!("Pipeline already in PLAYING state, skipping SELECT_STREAMS to avoid reconfiguration freeze");
}
```

This conditional logic should not be necessary. Per playbin3 spec, stream selection should work at any time.

Investigation needed:
1. Root cause of freeze during PLAYING state stream selection
2. Check if missing ASYNC_DONE handling after SELECT_STREAMS
3. Verify proper async state change handling
4. Check for UI blocking during reconfiguration

Expected outcome: Remove conditional logic and allow stream selection at any time

Reference: gstreamer-analysis.md section 3
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Root cause of reconfiguration freeze identified
- [x] #2 Stream selection works reliably in PLAYING state
- [x] #3 Conditional SELECT_STREAMS logic removed
- [x] #4 Stream switching during playback works smoothly
- [x] #5 No UI freezes during stream selection
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Root Cause Analysis

The "reconfiguration freeze" was caused by a combination of factors:

1. **Mixed sync/async handling**: The preroll phase used synchronous bus polling while also having an async bus watch
2. **State management complexity**: Manual state checking and conditional logic around SELECT_STREAMS
3. **Timing assumptions**: The code assumed SELECT_STREAMS during PLAYING would cause problems

## Solution

1. Removed the conditional `if current_state < gst::State::Playing` check
2. Now SELECT_STREAMS is sent for all StreamCollection messages regardless of pipeline state
3. The async bus watch (using glib main loop integration) ensures non-blocking operation
4. Per playbin3 specification, SELECT_STREAMS should work at any pipeline state

## Why This Works

- playbin3 is designed to handle stream selection at any time
- The async bus watch prevents blocking the UI during reconfiguration
- Pipeline automatically handles necessary state transitions for stream changes
- ASYNC_DONE messages signal completion of reconfiguration

The workaround was unnecessary - the real issue was improper async handling, which has been addressed by the overall simplification.
<!-- SECTION:NOTES:END -->
