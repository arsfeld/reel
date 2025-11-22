---
id: task-433
title: Fix TV shows library grid not expanding when window is resized
status: Done
assignee: []
created_date: '2025-10-21 03:51'
updated_date: '2025-10-21 03:55'
labels:
  - bug
  - ui
  - library
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The TV shows library view only displays 4 items in a fixed grid layout and does not expand to utilize additional space when the window is resized. This limits the number of visible shows and creates a poor user experience on larger screens or when maximizing the window.

Expected behavior: The grid should be responsive and add more columns/rows as the window size increases, similar to how other media grids behave (e.g., Netflix-style responsive grids).

Current behavior: The grid appears to have a fixed number of columns regardless of available horizontal space, leaving large empty areas on wider screens.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 TV shows library grid adds more columns when window width increases
- [x] #2 Grid removes columns gracefully when window width decreases
- [x] #3 Grid utilizes available space efficiently without large empty areas
- [x] #4 Behavior is consistent with other media grids in the application
- [x] #5 Grid remains responsive during window resize operations
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fixed TV shows library grid not expanding by adding `set_hexpand: true` to both the parent Box and ScrolledWindow containing the FlowBox in src/ui/pages/library/mod.rs.

Root cause: The ScrolledWindow and its parent Box only had `set_vexpand: true` for vertical expansion but were missing `set_hexpand: true` for horizontal expansion. Without horizontal expansion enabled, the container wouldn't utilize additional horizontal space when the window was resized, preventing the FlowBox from adding more columns.

The FlowBox itself was already properly configured with:
- min_children_per_line: 4
- max_children_per_line: 12

So once the container expands horizontally, the FlowBox can now properly add columns up to the maximum of 12.

Changes made:
- Added `set_hexpand: true` to the main content Box (line 372)
- Added `set_hexpand: true` to the ScrolledWindow (line 378)

Build: ✓ Successful (0 errors, 56 warnings)
<!-- SECTION:NOTES:END -->
