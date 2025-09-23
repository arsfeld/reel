---
id: task-232
title: Fix mouse cursor remains hidden after leaving player page
status: To Do
assignee: []
created_date: '2025-09-23 19:47'
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
- [ ] #1 Mouse cursor becomes visible when navigating away from player using back button
- [ ] #2 Mouse cursor becomes visible when navigating to other pages via sidebar or other navigation
- [ ] #3 Cursor visibility is properly cleaned up when player component is deactivated
- [ ] #4 No lingering cursor hide timers affect other pages
<!-- AC:END -->
