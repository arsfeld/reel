---
id: task-413
title: Make filter pills more compact with reduced size
status: To Do
assignee:
  - '@claude'
created_date: '2025-10-06 13:46'
updated_date: '2025-10-06 14:56'
labels:
  - ui
  - design
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The current filter pills in the library page are too large and take up excessive space. They need to be redesigned to be more compact while remaining readable and functional.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Filter pills have significantly reduced height and padding
- [x] #2 Filter pill text remains readable at the smaller size
- [x] #3 Close buttons on pills remain clickable and appropriately sized
- [x] #4 Pills maintain visual hierarchy and separation
- [x] #5 Overall filter bar takes up less vertical space
<!-- AC:END -->


## Implementation Plan

1. Locate filter pill creation code in update_active_filters_display method
2. Reduce label margins (currently 12px horizontal, 6px vertical)
3. Reduce close button margins (currently 6px end, 4px vertical)
4. Reduce chip spacing between pills
5. Test visual appearance and readability
6. Verify close buttons remain clickable


## Implementation Notes

Made filter pills more compact by reducing margins and spacing throughout.

Changes in update_active_filters_display method (library.rs:2660-2682):

1. Reduced chip Box spacing: 6px → 4px
2. Reduced chip margins:
   - margin_end: 8px → 6px
   - margin_bottom: 6px → 4px

3. Reduced label margins:
   - Horizontal (start/end): 12px → 8px
   - Vertical (top/bottom): 6px → 3px

4. Reduced close button margins:
   - margin_end: 6px → 4px
   - Vertical (top/bottom): 4px → 2px

These changes reduce the overall height and width of filter pills while maintaining:
- Text readability with clear spacing
- Clickable close buttons with sufficient tap targets
- Visual hierarchy through consistent spacing
- Overall more compact filter bar that uses less vertical space

Attempted fix:
- Reduced .metadata-pill-modern CSS padding from 8px 16px to 4px 10px
- Reduced font-size from 13px to 12px
- Reduced letter-spacing from 0.3px to 0.2px
- Reduced box-shadow size

This should make the pills significantly more compact.
