---
id: task-232
title: Fix mouse cursor remains hidden after leaving player page
status: Done
assignee:
  - '@claude'
created_date: '2025-09-23 19:47'
updated_date: '2025-09-28 00:04'
labels:
  - player
  - ui
  - cursor
  - bug
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
When navigating away from the video player page (e.g., using the back button or navigation), the mouse cursor remains hidden. The cursor should be restored to its normal visible state when leaving the player. This is likely because the player's cursor hiding mechanism doesn't properly clean up when the component is deactivated or navigated away from.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Mouse cursor becomes visible when navigating away from player using back button
- [x] #2 Mouse cursor becomes visible when navigating to other pages via sidebar or other navigation
- [x] #3 Cursor visibility is properly cleaned up when player component is deactivated
- [x] #4 No lingering cursor hide timers affect other pages
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Identify all paths that can navigate away from player page
2. Add cursor restoration logic to player's shutdown or cleanup method
3. Ensure cursor is always restored when player page is deactivated
4. Test all navigation paths from player page
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fixed the mouse cursor visibility issue by implementing a shutdown method for the PlayerPage AsyncComponent.

The issue occurred because the player hides the cursor during playback but didn't restore it when the component was destroyed through non-NavigateBack paths (like sidebar navigation or StopPlayer command).

Solution:
- Added a shutdown() method to PlayerPage that is automatically called when the component is destroyed
- The shutdown method restores the default cursor and cleans up all active timers
- This ensures cursor visibility is properly restored regardless of how the user leaves the player page

The fix handles all navigation paths:
1. Back button navigation (was already working)
2. Sidebar navigation to other pages (now fixed)
3. StopPlayer command (now fixed)
4. Any other component destruction path (now fixed)
<!-- SECTION:NOTES:END -->
