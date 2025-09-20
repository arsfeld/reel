---
id: task-105
title: Add horizontal scrolling for Home page sections
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 19:37'
updated_date: '2025-09-17 03:17'
labels:
  - feature
  - ui
dependencies: []
priority: high
---

## Description

Home page sections should support horizontal scrolling to show more items without taking up vertical space. Currently sections may be limited or not scrollable.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Each section has smooth horizontal scrolling
- [x] #2 Scroll indicators show when more content is available
- [x] #3 Keyboard navigation works for horizontal scrolling
- [x] #4 Touch/trackpad gestures supported for scrolling
- [x] #5 Items maintain aspect ratio during scroll
<!-- AC:END -->


## Implementation Plan

1. Analyze current FlowBox implementation and limitations
2. Replace FlowBox with a horizontal Box container for media cards
3. Implement proper horizontal scrolling with ScrolledWindow
4. Add scroll indicators (left/right arrows) that show when scrolling is possible
5. Implement keyboard navigation (left/right arrow keys)
6. Add touch/trackpad gesture support
7. Ensure media cards maintain fixed dimensions during scroll
8. Test with multiple backends to ensure sections display correctly
9. Verify image loading works during horizontal scrolling


## Implementation Notes

Implemented horizontal scrolling for Home page sections:

1. Modified home.rs to use FlowBox with single-row constraint (min/max children per line = 100)
2. Added ScrolledWindow with horizontal scrolling enabled and vertical disabled
3. Implemented scroll navigation buttons (left/right arrows) that enable/disable based on scroll position
4. Added keyboard navigation support (left/right arrow keys) for scrolling
5. Enabled kinetic scrolling for touch/trackpad gesture support
6. Fixed height to 290px to ensure media cards maintain aspect ratio
7. Scroll buttons use 80% page scrolling for smooth navigation
8. Keyboard scrolls by one card width (192px) for precise navigation

The implementation ensures smooth horizontal scrolling while maintaining card dimensions and providing multiple navigation methods (buttons, keyboard, touch/trackpad).
