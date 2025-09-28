---
id: task-248
title: Fix toast notifications not displaying in main window
status: Done
assignee:
  - '@claude'
created_date: '2025-09-26 13:02'
updated_date: '2025-09-26 13:09'
labels:
  - bug
  - ui
  - notifications
dependencies: []
priority: high
---

## Description

Toast notifications in the main window are not visible when triggered. The toast component exists but messages are not being displayed to the user, preventing important feedback and error messages from being shown.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Toast notifications display correctly when triggered
- [x] #2 Toast messages are visible with appropriate styling
- [x] #3 Toast messages auto-dismiss after appropriate timeout
- [x] #4 Error, success, and info toast variants all work correctly
- [x] #5 Toast notifications appear in correct position in the window
<!-- AC:END -->


## Implementation Plan

1. Investigate toast implementation in main_window.rs
2. Check if toast_overlay widget is properly connected
3. Identify that model.toast_overlay was not referencing the UI widget
4. Add toast_overlay to widget cloning list
5. Test that toasts display correctly


## Implementation Notes

Fixed toast notifications not displaying in main window by properly connecting the toast_overlay widget.

The issue was that the model.toast_overlay field was initialized with a new ToastOverlay instance that was not connected to the UI. When ShowToast messages were handled, toasts were being added to this disconnected overlay.

The fix adds a single line to clone the actual UI widget reference:
model.toast_overlay.clone_from(&widgets.toast_overlay);

This ensures that when ShowToast is handled, the toast is added to the actual overlay displayed in the window.
