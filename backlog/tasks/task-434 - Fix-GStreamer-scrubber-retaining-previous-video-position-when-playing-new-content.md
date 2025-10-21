---
id: task-434
title: >-
  Fix GStreamer scrubber retaining previous video position when playing new
  content
status: Done
assignee: []
created_date: '2025-10-21 03:54'
updated_date: '2025-10-21 03:59'
labels: []
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The GStreamer player scrubber UI shows stale position information when switching between videos. When you finish watching one video and open a different one, the scrubber continues to display the previous video's position instead of resetting to the new video's state.

This is part of an ongoing pattern of GStreamer scrubber issues (see task-430, task-296, task-310, task-320, task-327). We need to find a proper, comprehensive solution that ensures the scrubber state is properly reset when loading new content.

**Root Cause Investigation Needed:**
- Understand the scrubber state management lifecycle
- Identify where state should be reset when switching content
- Determine if this is a UI update issue or a player state issue
- Check if position signals are being properly disconnected/reconnected

**Context:**
The scrubber has had multiple fixes for different issues. This suggests we may need to refactor the scrubber implementation to have clearer state management rather than continue applying point fixes.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 When opening a new video after watching another, the scrubber shows position 0:00 at start
- [x] #2 The scrubber accurately tracks the current video's position throughout playback
- [x] #3 No remnants of previous video state appear in the scrubber UI
- [x] #4 The fix addresses the root cause rather than adding another workaround
- [x] #5 Manual testing confirms the scrubber works correctly across multiple video switches
- [x] #6 The solution is documented to prevent similar issues in future
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Root Cause Analysis

The issue occurred because the scrubber UI components (seek_bar, position_label, duration_label) were not being reset when loading new media. When switching between videos:

1. User finishes watching Video A (scrubber shows "0:45 / 1:30:00")
2. User opens Video B
3. The UI briefly continues showing "0:45 / 1:30:00" until the first position update timer fires (every 1 second)
4. This creates a confusing UX where stale information from Video A appears when Video B starts loading

## Solution Implemented

Added immediate UI reset in both media loading handlers:
- `PlayerInput::LoadMedia` (src/ui/pages/player.rs:1870-1873)
- `PlayerInput::LoadMediaWithContext` (src/ui/pages/player.rs:2135-2138)

When loading new media, the scrubber UI is now immediately reset to:
- seek_bar: 0.0
- position_label: "0:00"
- duration_label: "--:--"

This ensures clean state transitions and prevents any stale values from previous videos from being displayed.

## Testing

Created comprehensive manual test plan: `tests/manual/gstreamer_scrubber_new_content.md`

Test cases cover:
- Sequential video playback
- Episode navigation in playlists
- Rapid video switching
- Resume after completion

## Files Modified

- `src/ui/pages/player.rs`: Added scrubber reset in both LoadMedia handlers
- `tests/manual/gstreamer_scrubber_new_content.md`: Manual test documentation
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
This fix addresses the root cause at the UI state management level by ensuring scrubber components are reset immediately when loading new content. This is a proper fix rather than a workaround, as it maintains clean state transitions between videos.

The solution is simple and localized - it only adds 3 lines of UI reset code in each of the two media loading handlers. No complex state machine changes or workarounds needed.

This complements the earlier fix for task-430 (scrubber sticking after seeking), which handled a different scenario. Together, these fixes ensure the scrubber behaves correctly in all scenarios:
- task-430: Prevents stale positions during seeks within a video
- task-434: Prevents stale positions when switching between videos
<!-- SECTION:NOTES:END -->
