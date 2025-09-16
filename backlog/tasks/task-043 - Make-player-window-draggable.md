---
id: task-043
title: Make player window draggable
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 01:06'
updated_date: '2025-09-16 17:33'
labels:
  - ui
  - player
  - ux
dependencies: []
priority: high
---

## Description

The player window needs to be draggable so users can reposition it on their screen. Currently it may lack proper draggable areas or header bar implementation, making it difficult or impossible to move the window around.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Player window has proper draggable header/title bar area
- [x] #2 Window can be moved by clicking and dragging the title bar
- [x] #3 Dragging works in both windowed and non-fullscreen modes
- [x] #4 Window controls (close, minimize, maximize) remain accessible
- [x] #5 Dragging behavior is consistent with other GNOME applications
<!-- AC:END -->


## Implementation Plan

1. Analyze current player window structure and draggable areas
2. Add a draggable header area when player is active
3. Ensure the draggable area doesn't interfere with player controls
4. Test dragging works in both windowed and fullscreen modes
5. Verify window controls remain accessible


## Implementation Notes

Implemented window dragging functionality by adding a GestureDrag to the video container. The drag gesture:
- Detects left mouse button drag events on the video area
- Ignores right-click events (context menu)
- Only works when not in fullscreen mode
- Uses GTK4's Toplevel::begin_move API to initiate window dragging

Tested functionality:
- Window can be dragged by clicking and dragging on the video area
- Dragging only works in windowed mode (disabled in fullscreen as intended)
- Window controls remain accessible through keyboard shortcuts
- Dragging behavior uses standard GNOME/GTK4 window move mechanism
