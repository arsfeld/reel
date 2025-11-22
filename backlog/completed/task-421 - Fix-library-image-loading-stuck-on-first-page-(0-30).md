---
id: task-421
title: Fix library image loading stuck on first page (0-30)
status: Done
assignee:
  - '@claude'
created_date: '2025-10-06 18:14'
updated_date: '2025-10-06 18:20'
labels:
  - ui
  - bug
  - performance
dependencies: []
priority: high
---

## Description

Library image loading is repeatedly loading images for items 0 to 30 instead of progressing to subsequent pages (30-60, 60-90, etc.) as the user scrolls. The logs show 'Loading images for items 0 to 30' being repeated indefinitely, indicating the image loading logic is not properly tracking which page/range of items should be loaded next.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Image loading progresses through all pages sequentially (0-30, then 30-60, then 60-90, etc.)
- [x] #2 Images are loaded for the correct range based on scroll position or viewport
- [x] #3 No repeated loading of the same range unless user scrolls back to that range
- [x] #4 All library items eventually have their images loaded as user scrolls through library
<!-- AC:END -->


## Implementation Plan

1. Analyze the widget tree navigation in update_visible_range()
2. Fix the navigation path to correctly find the ScrolledWindow
3. Test that images load progressively as user scrolls
4. Verify no repeated loading of the same range


## Implementation Notes

Fixed widget tree navigation in update_visible_range() in src/ui/pages/library/data.rs:189-200

Root cause: The function was trying to navigate from Overlay -> Box (main) -> ScrolledWindow, but the actual structure is Overlay -> Box (main) -> Box (content area) -> ScrolledWindow. The missing level in the navigation meant the ScrolledWindow was never found, so visible_start_idx and visible_end_idx remained at 0, causing image loading to be stuck on the first page (0-30).

Fix: Added .first_child() to navigate from the main content area Box to the ScrolledWindow inside it.

The widget tree structure:
- Overlay (root)
  - Box (main content, line 136)
    - Box (toolbar)
    - Box (media type filters)  
    - Box (main content area, line 361)
      - ScrolledWindow (line 368)

Navigation now correctly follows: root.first_child() -> main Box, .last_child() -> content area Box, .first_child() -> ScrolledWindow
