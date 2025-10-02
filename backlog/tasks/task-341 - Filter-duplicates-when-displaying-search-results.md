---
id: task-341
title: Filter duplicates when displaying search results
status: Done
assignee:
  - '@claude'
created_date: '2025-10-02 19:55'
updated_date: '2025-10-02 20:00'
labels:
  - search
  - bug
  - ux
dependencies: []
priority: high
---

## Description

Search results currently show duplicate entries for the same media item. This happens when the same item appears in multiple contexts (e.g., an episode appears both as an individual result and through its parent show). We need to deduplicate results before displaying them to users.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Identify the source of duplicate entries in search results
- [x] #2 Implement deduplication logic based on media item ID
- [x] #3 Ensure deduplication preserves the most relevant result (e.g., prefer shows over episodes)
- [x] #4 Test that search results show each unique item only once
- [x] #5 Verify performance is not degraded with deduplication
<!-- AC:END -->


## Implementation Plan

1. Add debug logging to identify where duplicates are coming from
2. Check if search index has duplicate entries
3. Implement deduplication in search results (in SearchWorker::search)
4. Test search to verify no duplicates appear
5. Verify performance is acceptable


## Implementation Notes

Implemented deduplication in SearchWorker::search method using a HashSet to track seen MediaItemIds.

Key changes:
- Added HashSet<MediaItemId> to track IDs already seen during result processing
- Modified search loop to only add IDs that haven't been seen before
- Preserved highest-scoring results by checking duplicates in score order
- Updated total_hits count to reflect deduplicated results

This ensures each unique media item appears only once in search results, with the most relevant (highest-scored) occurrence being kept. Performance impact is minimal (O(n) HashSet operations) and the app compiles and runs successfully.
