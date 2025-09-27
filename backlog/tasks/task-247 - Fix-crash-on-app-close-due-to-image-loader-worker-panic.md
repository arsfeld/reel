---
id: task-247
title: Fix crash on app close due to image loader worker panic
status: In Progress
assignee:
  - '@claude'
created_date: '2025-09-26 12:59'
updated_date: '2025-09-26 13:11'
labels:
  - bug
  - ui
  - crash
dependencies: []
priority: high
---

## Description

The application crashes with a panic when closing due to the ImageLoader worker trying to process a CancelLoad message after the component has been dropped. The panic occurs in the WorkerController::emit method when it tries to unwrap a Result that contains a CancelLoad error.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Application closes gracefully without any panics
- [ ] #2 ImageLoader worker properly handles shutdown and cancellation
- [ ] #3 All pending image loads are cleanly cancelled on component drop
- [ ] #4 No error messages or stack traces appear when closing the app
<!-- AC:END -->
