---
id: task-284
title: Fix preferences dialog showing wrong video backend selected
status: Done
assignee:
  - '@claude'
created_date: '2025-09-27 19:53'
updated_date: '2025-09-28 00:50'
labels:
  - bug
  - ui
  - preferences
  - player
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The preferences dialog incorrectly displays which video backend is currently selected. While toggling between backends works correctly and saves to the configuration file, the UI does not accurately reflect the current backend selection when the dialog is opened.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Preferences dialog correctly shows the currently active video backend on open
- [x] #2 Selected backend in UI matches the backend from configuration file
- [x] #3 Backend selection persists correctly across application restarts
- [x] #4 UI updates immediately when backend is changed
- [x] #5 Verify both MPV and GStreamer selections display correctly
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Analyze current implementation to understand the issue
2. Identify that the problem is in line 76 - it uses model.default_player which is set during init
3. The view macro creates a static view that doesn't update when model changes
4. Need to make the dropdown reactive to model changes
5. Test with both MPV and GStreamer to verify fix works
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Fix Summary

The preferences dialog was not correctly showing the currently selected video backend when opened. The issue was that the dialog loaded the configuration only once during initialization and did not refresh when reopened.

## Changes Made

1. **Added tracker pattern to PreferencesDialog**: Implemented the tracker::track attribute on the PreferencesDialog struct to enable reactive UI updates when model fields change.

2. **Added ReloadConfig message**: Created a new PreferencesDialogInput::ReloadConfig message that reloads the configuration from file and updates the model fields.

3. **Trigger config reload on dialog presentation**: Modified main_window.rs to send the ReloadConfig message whenever an existing preferences dialog is presented, ensuring it shows the current configuration values.

4. **Made dropdown reactive**: Added the #[track] attribute to the dropdown's set_selected property to make it update when the default_player field changes.

## Files Modified

- src/ui/dialogs/preferences_dialog.rs: Added tracker pattern, ReloadConfig handling, and reactive dropdown
- src/ui/dialogs/mod.rs: Exported PreferencesDialogInput
- src/ui/main_window.rs: Send ReloadConfig when presenting existing dialog

The fix ensures that the preferences dialog always displays the correct video backend selection that matches the configuration file, and updates immediately when changes are made.
<!-- SECTION:NOTES:END -->
