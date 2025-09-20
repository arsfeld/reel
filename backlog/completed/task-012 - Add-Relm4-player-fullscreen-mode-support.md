---
id: task-012
title: Add Relm4 player fullscreen mode support
status: Done
assignee:
  - '@arosenfeld'
created_date: '2025-09-15 02:10'
updated_date: '2025-09-16 00:34'
labels:
  - player
  - relm4
  - fullscreen
dependencies: []
priority: medium
---

## Description

The Relm4 player has a fullscreen button but the fullscreen functionality needs proper implementation including keyboard shortcuts, proper video scaling, and cursor hiding in fullscreen mode.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Implement F11 and F key shortcuts for fullscreen toggle
- [x] #2 Properly scale video to fill screen in fullscreen mode
- [x] #3 Hide cursor automatically in fullscreen after 3 seconds
- [x] #4 Show cursor on mouse movement in fullscreen
- [x] #5 Handle window state changes for fullscreen transitions
- [x] #6 Ensure ESC key exits fullscreen mode
<!-- AC:END -->


## Implementation Plan

1. Review current fullscreen implementation
2. Add ESC key handling to exit fullscreen
3. Improve video scaling in fullscreen mode
4. Test keyboard shortcuts (F11, F, ESC)
5. Ensure cursor hiding works with 3-second timer
6. Test mouse movement shows cursor in fullscreen

## Implementation Notes

Fixed fullscreen mode functionality in the Relm4 player:

- Added proper ESC key handling to exit fullscreen when in fullscreen mode, otherwise navigate back
- F11 and F keys were already implemented for fullscreen toggle
- Cursor auto-hide after 3 seconds was already implemented
- Mouse movement properly shows cursor and resets timer
- Video widget already has proper expansion settings for fullscreen
- Window state transitions handled via fullscreen()/unfullscreen() methods

The implementation required adding a new EscapePressed input variant to handle the dual behavior of the ESC key based on fullscreen state.
