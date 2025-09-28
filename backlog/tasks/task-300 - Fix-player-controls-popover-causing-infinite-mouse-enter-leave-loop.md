---
id: task-300
title: Fix player controls popover causing infinite mouse enter/leave loop
status: Done
assignee:
  - '@claude'
created_date: '2025-09-28 02:58'
updated_date: '2025-09-28 03:07'
labels:
  - bug
  - player
  - ui
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
When using popovers in the player controls, the application enters a bad state with thousands of rapidly repeating log lines showing mouse enter/leave events and Gdk-CRITICAL assertions about surface freeze counts. This creates a feedback loop that floods the logs and likely causes performance issues.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Identify root cause of the mouse enter/leave event loop when popovers are shown
- [x] #2 Fix the Gdk surface thaw_updates assertion failures
- [x] #3 Ensure popovers in player controls work without triggering event loops
- [x] #4 Verify no performance degradation or log flooding occurs
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add popover state tracking to PlayerPage struct
2. Connect to popover show/hide signals on all MenuButtons
3. Block control hiding when any popover is visible
4. Implement proper popover lifecycle management
5. Test all popover interactions
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation Summary

Fixed the infinite mouse enter/leave event loop that occurred when popovers were shown in the player controls.

### Root Cause

The issue was caused by a feedback loop:
1. When a popover opened from a MenuButton in the controls overlay
2. The popover appeared on top of the controls, triggering a mouse leave event
3. The controls would hide due to the leave event
4. The popover would close because its parent was hidden
5. This triggered a mouse enter event on the controls
6. The controls would show again, restarting the cycle

### Solution

Implemented popover state tracking to prevent control hiding when popovers are open:

1. **Added popover counter** - Added `active_popover_count: Rc<RefCell<usize>>` to track open popovers

2. **Modified control hiding logic** - Updated `transition_to_hidden()` to check popover count before hiding

3. **Connected popover signals** - Added show/hide signal handlers to all menu popovers:
   - Audio track menu
   - Subtitle track menu
   - Zoom mode menu
   - Quality/upscaling menu

4. **Updated mouse leave handler** - Modified `MouseLeaveWindow` input handler to respect popover state

### Files Modified

- `src/ui/pages/player.rs`:
  - Added popover state tracking field
  - Modified all populate_*_menu functions
  - Updated control hiding logic
  - Fixed async future warning in upscaling mode update

### Result

Popovers now work correctly without triggering event loops. Controls remain visible while any popover is open, preventing the feedback loop that caused log flooding and Gdk surface assertion failures.
<!-- SECTION:NOTES:END -->
