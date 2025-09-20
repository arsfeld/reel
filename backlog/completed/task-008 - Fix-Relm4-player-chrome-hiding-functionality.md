---
id: task-008
title: Fix Relm4 player chrome hiding functionality
status: Done
assignee:
  - '@claude'
created_date: '2025-09-15 02:10'
updated_date: '2025-09-15 02:22'
labels:
  - ui
  - player
  - relm4
dependencies: []
priority: high
---

## Description

The Relm4 player does not hide the main window chrome (sidebar, header bar) when entering player mode. In the GTK implementation, the player enters a fullscreen-like immersive mode that hides all chrome. The Relm4 player should have the same behavior.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Hide sidebar when entering player mode
- [x] #2 Hide header bar/title bar when entering player mode
- [x] #3 Store window state before hiding chrome
- [x] #4 Restore window chrome when exiting player mode
- [x] #5 Handle ESC key to exit player and restore chrome
<!-- AC:END -->


## Implementation Plan

1. Analyze current main window navigation implementation
2. Add chrome visibility state to main window
3. Hide sidebar and header bar when navigating to player
4. Store previous chrome state before hiding
5. Restore chrome when exiting player
6. Handle ESC key to restore chrome and exit player


## Implementation Notes

Fixed chrome hiding functionality by:
1. Added hiding of both sidebar and content headers when entering player
2. Used split_view.set_collapsed(true) to hide the sidebar
3. Set toolbar styles to Flat for immersive viewing
4. Window state already being saved (size, maximized, fullscreen)
5. RestoreWindowChrome now restores all chrome elements
6. ESC key handler already implemented in player to navigate back
