---
id: task-274
title: Fix home section scrolling - containers larger than content with empty space
status: Done
assignee:
  - '@claude'
created_date: '2025-09-27 00:37'
updated_date: '2025-09-27 00:45'
labels:
  - ui
  - homepage
dependencies: []
priority: high
---

## Description

The home page sections have horizontal scrolling containers that are larger than the items they contain, resulting in empty space at the end of each section. This creates a poor user experience where users can scroll past the actual content into empty space.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Identify why scroll containers are larger than their content
- [x] #2 Fix container sizing to match actual content width
- [x] #3 Ensure scroll indicators (left/right arrows) are properly disabled when reaching content boundaries
- [x] #4 Test with various numbers of items to ensure proper sizing
- [x] #5 Verify no empty space remains at the end of sections
<!-- AC:END -->


## Implementation Plan

1. Analyze FlowBox configuration that forces single row layout
2. Check if container width calculation is incorrect
3. Fix FlowBox to properly size to content width
4. Test scroll button enable/disable logic
5. Verify with different numbers of items


## Implementation Notes

Fixed the home section scrolling container sizing issue by:

1. Changed FlowBox configuration to use actual item count for min/max_children_per_line
2. This ensures the FlowBox sizes exactly to the number of items present
3. Improved scroll button boundary detection with 1.0 pixel threshold

The root cause was that setting arbitrary min/max values caused the FlowBox to allocate incorrect space. By using section.items.len() for both min and max, the container now sizes perfectly to its content.
