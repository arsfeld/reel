---
id: task-418
title: Move media summary/stats to right side of filters toolbar
status: Done
assignee:
  - '@claude'
created_date: '2025-10-06 17:25'
updated_date: '2025-10-06 17:32'
labels:
  - design
  - ui
dependencies: []
priority: high
---

## Description

The media summary/stats (showing counts like '1,234 movies') should be moved from its current position to the right side of the filters toolbar. This will create a better visual balance with filters on the left and stats on the right.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Media summary/stats is positioned on the right side of the filters toolbar
- [x] #2 Stats remain visible and properly aligned
- [x] #3 Layout is responsive and handles different window sizes
- [x] #4 Visual hierarchy is clear with filters left, stats right
<!-- AC:END -->


## Implementation Plan

1. Analyze current layout: toolbar at top (left-aligned), stats below in content area
2. Move stats widgets from filter summary section into toolbar
3. Restructure toolbar with left box (controls) and right box (stats)
4. Test responsive layout at different window sizes
5. Verify visual hierarchy is clear


## Implementation Notes

Moved media summary/stats from the content area to the right side of the filters toolbar.

Changes:
- Restructured toolbar with three horizontal sections: left controls, spacer, and right stats
- Left section contains sort controls (dropdown, toggle), search and filters buttons
- Spacer box with hexpand pushes stats to the right
- Right section contains result count, average rating, and year range labels
- Removed duplicate stats section from filter summary area below toolbar
- Stats visibility logic preserved: shown when not loading and either filters active or items present

The layout is responsive - the spacer expands to fill available space, keeping controls left and stats right at all window sizes.
