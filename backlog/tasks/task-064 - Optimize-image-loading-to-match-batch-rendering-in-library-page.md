---
id: task-064
title: Optimize image loading to match batch rendering in library page
status: To Do
assignee: []
created_date: '2025-09-16 04:00'
updated_date: '2025-09-16 04:35'
labels:
  - performance
  - optimization
  - ui
dependencies: []
priority: high
---

## Description

Currently, image loading requests are sent immediately when items are rendered in batches. This can cause unnecessary network requests for items that may never be scrolled into view. Implement a smarter image loading strategy that only loads images for the current batch and nearby batches, with proper cleanup of pending requests when scrolling quickly.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Modify image loading to only request images for current batch and adjacent batches
- [ ] #2 Cancel pending image requests when user scrolls past items quickly
- [ ] #3 Implement viewport-based image loading with configurable look-ahead distance
- [ ] #4 Add debouncing to prevent excessive image requests during fast scrolling
- [ ] #5 Track and cleanup orphaned image requests to prevent memory leaks
- [ ] #6 Ensure images are loaded with correct priority based on viewport proximity
- [ ] #7 Test with large libraries to verify reduced network usage
- [ ] #8 Maintain smooth scrolling performance during image loading
<!-- AC:END -->
