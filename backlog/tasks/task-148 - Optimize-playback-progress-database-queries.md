---
id: task-148
title: Optimize playback progress database queries
status: To Do
assignee:
  - ''
created_date: '2025-09-17 15:31'
updated_date: '2025-09-17 15:37'
labels: []
dependencies: []
priority: high
---

## Description

The current implementation of save_media_item performs individual database queries for each media item's playback progress. This should be optimized to use batch operations for better performance during sync.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Create batch upsert method for playback progress in PlaybackRepository
- [ ] #2 Modify save_media_items_batch to collect all playback progress updates
- [ ] #3 Perform a single batch database operation for all playback progress records
- [ ] #4 Ensure transaction safety for batch operations
- [ ] #5 Add performance logging to measure improvement
<!-- AC:END -->


## Implementation Notes

This optimization is still needed but not critical for functionality. The current implementation works correctly but makes individual database queries for each media item during sync.

The batch optimization would:
- Collect all playback progress updates during sync
- Perform a single batch upsert operation
- Improve sync performance for large libraries

However, the current implementation is functional and the unwatched indicators work correctly.
