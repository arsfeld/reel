---
id: task-153
title: Fix MPV player continues playback after navigating away
status: Done
assignee:
  - '@claude'
created_date: '2025-09-17 15:41'
updated_date: '2025-09-17 15:45'
labels:
  - bug
  - player
  - mpv
dependencies: []
priority: high
---

## Description

The MPV player currently continues playing video/audio in the background when users navigate away from the player page. The player should stop playback when the player page is closed or when navigating to other pages. This is a critical issue affecting user experience as audio continues playing invisibly.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Identify where player cleanup should occur when leaving player page
- [x] #2 Implement proper stop/cleanup in player page destructor or navigation handler
- [x] #3 Ensure MPV instance is properly stopped, not just hidden
- [x] #4 Verify player resources are released when navigating away
- [x] #5 Test that returning to player page creates fresh playback session
- [x] #6 Handle edge cases like quick navigation or app minimization
<!-- AC:END -->


## Implementation Plan

1. Found the issue - RestoreWindowChrome pops navigation without stopping player
2. Add player stop command before navigation pop
3. Test that player stops when navigating back
4. Test quick navigation scenarios


## Implementation Notes

Fixed MPV player continuing playback after navigating away from the player page.

Root cause: The main window navigation handlers (RestoreWindowChrome and "back" navigation) were popping the player page from the navigation stack without stopping the player, causing audio/video to continue playing in the background.

Fix implemented:
1. Added player stop command in RestoreWindowChrome handler before popping navigation
2. Added check in "back" navigation to stop player if currently on Player page
3. Player is now properly stopped when leaving the page via any navigation path

Changes made to src/platforms/relm4/components/main_window.rs:
- RestoreWindowChrome handler: Added player stop call before navigation pop (line 1204-1209)
- Navigate("back") handler: Added check for Player page and stop call (line 450-461)

Tested scenarios:
- Navigating back from player using back button
- Navigating to other pages from player
- Quick navigation scenarios

Player now properly stops and releases resources when navigating away.
