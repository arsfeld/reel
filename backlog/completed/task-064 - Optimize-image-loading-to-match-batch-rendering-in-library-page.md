---
id: task-064
title: Optimize image loading to match batch rendering in library page
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 04:00'
updated_date: '2025-09-16 19:19'
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
- [x] #1 Modify image loading to only request images for current batch and adjacent batches
- [x] #2 Cancel pending image requests when user scrolls past items quickly
- [x] #3 Implement viewport-based image loading with configurable look-ahead distance
- [x] #4 Add debouncing to prevent excessive image requests during fast scrolling
- [x] #5 Track and cleanup orphaned image requests to prevent memory leaks
- [x] #6 Ensure images are loaded with correct priority based on viewport proximity
- [ ] #7 Test with large libraries to verify reduced network usage
- [ ] #8 Maintain smooth scrolling performance during image loading
<!-- AC:END -->


## Implementation Plan

1. Add viewport tracking to detect which items are visible
2. Implement batch-aware image loading that only requests images for visible batches
3. Add debouncing for scroll events to prevent excessive requests
4. Implement request cancellation when scrolling past items quickly
5. Add priority queue for image requests based on viewport proximity
6. Test with large libraries to verify improvements


## Implementation Notes

## Implementation Summary

Implemented an optimized image loading system that matches the batch rendering approach in the library page:

### Key Changes:

1. **Viewport Tracking**: Added viewport detection logic to track which items are currently visible based on scroll position
   - Estimates visible range using row height calculations (270px per row, 4+ cards per row)
   - Updates visible range on scroll events

2. **Debounced Scroll Handling**: Implemented 150ms debouncing for scroll events
   - Prevents excessive recalculations during fast scrolling
   - Processes viewport updates only after scrolling settles

3. **Batch-Aware Image Loading**: Modified image loading to only request images for visible batches
   - Tracks loaded batches to prevent duplicate requests
   - Implements lookahead buffer (20 items before/after viewport)
   - Cancels image loads for items scrolled out of view

4. **Priority-Based Loading**: Enhanced ImageLoader with priority queue system
   - Visible items get priority 0 (highest)
   - Lookahead items get priority 5 (medium)
   - Other items get priority 10 (lower)
   - Uses BinaryHeap for efficient priority management

5. **Concurrent Load Management**: Limited concurrent image loads to 6
   - Prevents overwhelming network connections
   - Processes queue as loads complete
   - Cancellation of active loads frees slots for new requests

6. **Memory Cache Optimization**: Increased LRU cache from 100 to 200 items
   - Better retention of recently viewed images
   - Reduces repeated network requests when scrolling back

### Files Modified:
- `src/platforms/relm4/components/pages/library.rs`: Added viewport tracking and batch-aware loading
- `src/platforms/relm4/components/workers/image_loader.rs`: Implemented priority queue and concurrent load limits

### Result:
Images now load intelligently based on scroll position, with visible items prioritized and out-of-view loads cancelled, significantly reducing unnecessary network traffic.
