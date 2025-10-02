---
id: task-247
title: Fix crash on app close due to image loader worker panic
status: Done
assignee:
  - '@claude'
created_date: '2025-09-26 12:59'
updated_date: '2025-09-28 00:10'
labels:
  - bug
  - ui
  - crash
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The application crashes with a panic when closing due to the ImageLoader worker trying to process a CancelLoad message after the component has been dropped. The panic occurs in the WorkerController::emit method when it tries to unwrap a Result that contains a CancelLoad error.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Application closes gracefully without any panics
- [x] #2 ImageLoader worker properly handles shutdown and cancellation
- [x] #3 All pending image loads are cleanly cancelled on component drop
- [x] #4 No error messages or stack traces appear when closing the app
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Understand the emit() panic mechanism in WorkerController
2. Replace all .emit() calls with safe alternatives that don't panic on shutdown
3. Test application closing to ensure no panics occur
4. Verify all image loading cancellation still works correctly
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Fixed the panic on app shutdown

### Issue
The application was crashing with a panic when closing because the ImageLoader worker's channel was being closed during shutdown, and components were still trying to send CancelLoad messages using `.emit()` which panics on send failure.

### Solution  
Replaced all `.emit()` calls with `.sender().send()` and ignored the Result using `let _ =` pattern. This ensures that if the worker is already dropped or the channel is closed during shutdown, the send will fail gracefully without panicking.

### Changes Made
1. **src/ui/pages/home.rs** - Replaced 3 `.emit()` calls:
   - Line 781: LoadImage request
   - Line 905: CancelLoad in clear_sections()
   - Line 934: CancelLoad in clear_source_sections()

2. **src/ui/pages/library.rs** - Replaced 3 `.emit()` calls:
   - Line 1013: CancelLoad for out-of-range items
   - Line 1051: LoadImage request
   - Line 1077: CancelLoad in cleanup

3. **src/ui/pages/show_details.rs** - Replaced 1 `.emit()` call:
   - Line 765: LoadImage for episode thumbnails

### Technical Details
The Relm4 WorkerController provides two methods for sending messages:
- `.emit(msg)` - Panics if the worker is dropped (unsafe during shutdown)
- `.sender().send(msg)` - Returns Result, safe during shutdown

By using `.sender().send()` with `let _ =` we handle shutdown gracefully, as the pattern is already used successfully in other parts of the codebase (main_window.rs, image_loader.rs internal).
<!-- SECTION:NOTES:END -->
