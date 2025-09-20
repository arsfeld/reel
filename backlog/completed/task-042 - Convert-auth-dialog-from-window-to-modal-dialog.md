---
id: task-042
title: Convert auth dialog from window to modal dialog
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 00:40'
updated_date: '2025-09-16 00:53'
labels:
  - ui
  - auth
  - refactor
dependencies: []
priority: high
---

## Description

The authentication dialog is currently implemented as a standalone window but should be a modal dialog attached to the main window. This follows GNOME HIG guidelines and prevents issues with window management and focus.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Convert auth dialog from standalone window to modal dialog
- [x] #2 Dialog properly attaches to main window as transient
- [x] #3 Dialog blocks interaction with main window while open
- [x] #4 Dialog centers correctly over parent window
- [x] #5 Escape key and close button properly dismiss the dialog
<!-- AC:END -->


## Implementation Plan

1. Study current auth_dialog implementation to understand widget structure
2. Verify adw::Dialog supports modal mode and transient parent settings
3. Modify auth_dialog to properly set transient parent from main window
4. Ensure dialog properly blocks interaction with main window
5. Test escape key and close button behavior
6. Verify dialog centering over parent window


## Implementation Notes

Converted auth dialog from standalone window to modal dialog:

1. Updated AuthDialogInput::Show handler to get the active window from the application
2. Used adw::Dialog's present() method with parent window parameter to make it modal
3. Dialog now properly attaches to main window as transient and blocks interaction
4. Dialog centers over parent window automatically when presented with parent
5. Escape key and close button work by default with adw::Dialog

The implementation uses relm4::main_application() to get the active window dynamically, avoiding Send trait issues with passing window references through the message enum.
