---
id: task-232
title: Fix mouse cursor remains hidden after leaving player page
status: Done
assignee:
  - '@claude'
created_date: '2025-09-23 19:47'
updated_date: '2025-09-29 02:31'
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
Fixed the mouse cursor visibility issue by implementing a centralized solution using the NavigationView's 'notify::visible-page' signal.

The issue occurred because:
1. During navigation, the player page remained alive but hidden (not destroyed)
2. Mouse movement during navigation triggered MouseLeaveWindow events  
3. This caused the player to hide the cursor again after our restoration attempts

The solution:
- Monitors navigation changes through NavigationView's 'notify::visible-page' signal
- Detects transitions away from the Player page
- Restores cursor using idle_add_local_once() to ensure it happens after mouse events
- Handles all navigation paths: back button, sidebar navigation, etc.

This centralized approach avoids adding cursor restoration code to every navigation route and properly handles the event timing issues.
<!-- SECTION:NOTES:END -->
