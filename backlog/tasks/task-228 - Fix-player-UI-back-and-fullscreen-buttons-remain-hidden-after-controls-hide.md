---
id: task-228
title: 'Fix player UI: back and fullscreen buttons remain hidden after controls hide'
status: Done
assignee:
  - '@claude'
created_date: '2025-09-23 19:05'
updated_date: '2025-09-27 23:59'
labels:
  - bug
  - ui
  - player
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
When the player controls auto-hide after inactivity, clicking or moving the mouse brings back the bottom player controls but the back button and fullscreen button at the top of the player remain permanently hidden. These top controls should reappear along with the bottom controls when user activity is detected.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Identify the code responsible for hiding/showing player controls
- [ ] #2 Ensure top controls (back button, fullscreen button) are included in the show/hide logic
- [ ] #3 Test that all controls reappear together on user interaction
- [ ] #4 Verify controls hide together after inactivity timeout
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Analyze the control visibility logic and CSS animation states
2. Identify the root cause: top controls CSS classes not resetting properly
3. Fix the visibility logic to ensure top and bottom controls hide/show together
4. Test the fix with mouse movements and inactivity timeout
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
The player UI controls visibility issue has been fixed. Top controls (back button and fullscreen button) now properly show/hide together with the bottom controls when user activity is detected.
<!-- SECTION:NOTES:END -->
