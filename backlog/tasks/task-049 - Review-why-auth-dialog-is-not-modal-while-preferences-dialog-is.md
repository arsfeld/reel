---
id: task-049
title: Review why auth dialog is not modal while preferences dialog is
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 02:31'
updated_date: '2025-09-16 02:37'
labels:
  - bug
dependencies: []
priority: high
---

## Description

The auth dialog should be modal like the preferences dialog, but it's not behaving as expected. Need to investigate why the auth dialog isn't properly modal despite being implemented as adw::Dialog, while the preferences dialog works correctly as a modal.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Compare auth dialog implementation with preferences dialog implementation
- [x] #2 Identify differences in how the dialogs are created and presented
- [x] #3 Check if auth dialog is using Dialog or Window
- [x] #4 Ensure auth dialog blocks interaction with main window
- [x] #5 Fix auth dialog to be properly modal
- [x] #6 Test that auth dialog prevents clicking on main window
<!-- AC:END -->


## Implementation Plan

1. Analyze how preferences dialog is presented as modal from main_window
2. Check how auth dialog is currently presented
3. Modify auth dialog presentation to match preferences approach
4. Test that auth dialog blocks interaction with main window


## Implementation Notes

Fixed auth dialog modality issue by:

1. Added parent_window field to AuthDialog struct to store parent window reference
2. Modified AuthDialog Init type to accept (DatabaseConnection, Option<gtk4::Window>) tuple
3. Updated auth dialog initialization in main_window to pass parent window: Some(root.clone().upcast())
4. Changed dialog.present() to use stored parent window reference: dialog.present(Some(parent))

The issue was that the auth dialog was trying to find the active window dynamically which was unreliable. Now it receives the parent window during initialization and uses it when presenting, matching how the preferences dialog works.
