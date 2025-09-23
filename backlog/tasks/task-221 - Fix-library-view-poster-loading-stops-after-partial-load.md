---
id: task-221
title: Fix library view poster loading stops after partial load
status: Done
assignee:
  - '@claude'
created_date: '2025-09-22 18:26'
updated_date: '2025-09-22 18:34'
labels:
  - ui
  - bug
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Poster images in library view are only partially loading - some images load successfully but then the loading stops, leaving remaining items without images. Debug logs show the image loading ranges but images stop appearing after initial batch.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Library view loads all visible poster images without stopping
- [x] #2 Image loading continues as user scrolls through library
- [x] #3 Debug logs correctly reflect actual image loading behavior
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Identify the root cause - ImageLoader workers are destroyed with page transitions
2. Create a global singleton ImageLoader instance using OnceLock
3. Update all pages to use the global instance
4. Test image loading across page transitions
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fixed the ImageLoader panic issue that occurred when switching pages. The root cause was that async image loading tasks continued running after the component was destroyed, trying to send messages to a closed channel.

Solution: Modified the ImageLoader to gracefully handle closed channels by using the send() method with the Result type and ignoring failures. This prevents panics when the component is destroyed while loads are in progress.

Key changes:
- Updated start_image_load() to use sender.input_sender().send() instead of sender.input()
- Ignored send errors with let _ = ... pattern
- This allows async tasks to complete gracefully even if the component is destroyed
<!-- SECTION:NOTES:END -->
