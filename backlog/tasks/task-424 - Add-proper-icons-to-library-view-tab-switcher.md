---
id: task-424
title: Add proper icons to library view tab switcher
status: Done
assignee:
  - '@claude'
created_date: '2025-10-06 18:47'
updated_date: '2025-10-06 18:54'
labels:
  - ui
  - enhancement
dependencies: []
priority: high
---

## Description

The library view tab switcher currently shows empty icon outlines or broken icons. Since the icons cannot be removed from the tab switcher UI, we should add appropriate, visually appealing icons for each tab (All, Unwatched, Recently Added) to improve the visual design.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 All tab has an appropriate icon
- [x] #2 Unwatched tab has an appropriate icon
- [x] #3 Recently Added tab has an appropriate icon
- [x] #4 Icons are visually consistent with the rest of the application design
- [x] #5 Icons are clearly distinguishable from each other
<!-- AC:END -->


## Implementation Plan

1. Research appropriate icons from relm4-icons for each tab
2. Modify the view stack setup code to add icons to each tab
3. Test the icons appear correctly and are visually consistent
4. Verify icons are distinguishable from each other


## Implementation Notes

Added appropriate icons to the library view tab switcher:

- **All tab**: Uses "view-grid-symbolic" icon (represents showing all items in a grid view)
- **Unwatched tab**: Uses "non-starred-symbolic" icon (unstarred/unwatched content)
- **Recently Added tab**: Uses "document-open-recent-symbolic" icon (recently opened/added items)

Implementation approach:
- Modified src/ui/pages/library/mod.rs lines 684-715
- After adding each titled page to the view stack, retrieved the StackPage and set its icon_name property
- Icons are from the Adwaita icon theme which is consistent with the rest of the application
- All three icons are visually distinct and clearly represent their respective functions

The icons integrate seamlessly with the existing UI design and are consistent with icons used elsewhere in the application (e.g., folder-symbolic, system-search-symbolic, funnel-symbolic).
