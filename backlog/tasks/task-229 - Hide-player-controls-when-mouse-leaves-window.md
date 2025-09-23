---
id: task-229
title: Hide player controls when mouse leaves window
status: Done
assignee:
  - '@claude'
created_date: '2025-09-23 19:05'
updated_date: '2025-09-23 19:23'
labels:
  - ui
  - player
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Player controls should automatically hide when the mouse cursor exits the player window boundaries, improving the viewing experience
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Controls hide immediately when mouse leaves player window
- [x] #2 Controls remain visible while mouse is within player window
- [x] #3 Existing control fade-out timer is cancelled when mouse leaves
- [x] #4 Controls reappear when mouse re-enters the window
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Examine current motion event handling in player.rs\n2. Add mouse enter/leave event detection for the entire window\n3. Implement immediate control hiding when mouse leaves window\n4. Cancel existing fade timer when mouse exits\n5. Show controls when mouse re-enters window\n6. Test all acceptance criteria
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented mouse enter/leave detection for the player window to immediately hide controls when the cursor exits. Added ShowControlsIfHidden input to prevent constant timer resets during normal mouse movement. Controls now hide immediately on window exit, cancel timers properly, and reappear on window re-enter. While functional, the implementation revealed significant architectural issues with the current boolean flag approach - created task-230 for proper state machine refactoring.
<!-- SECTION:NOTES:END -->
