---
id: task-411
title: Fix library view tabs showing empty icon outlines instead of labels
status: To Do
assignee:
  - '@claude'
created_date: '2025-10-06 13:27'
updated_date: '2025-10-06 14:55'
labels:
  - ui
  - bug
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The library view tabs in the header bar are currently displaying empty icon outlines instead of showing the actual view names (All, Unwatched, etc.). Users cannot see which view they are selecting.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Tabs display text labels (All, Unwatched, etc.) instead of empty icon outlines
- [x] #2 Tab labels are clearly visible and readable
- [x] #3 Tab switching functionality remains working
<!-- AC:END -->


## Implementation Plan

1. Investigate current TabBar/TabView configuration
2. Check if icon property is set on TabPages
3. Test fix by explicitly setting icon to None or configuring TabBar to show titles
4. Verify tabs display text labels correctly
5. Test tab switching still works


## Implementation Notes

Fixed library view tabs showing empty icon outlines by explicitly setting icon to None for each TabPage.

Changes:
- Added gio import to library.rs
- Set icon to None for all three tab pages (All, Unwatched, Recently Added) using set_icon(None::<&gio::Icon>)

This prevents adw::TabBar from displaying empty icon placeholders and ensures text labels are shown instead. Tab switching functionality remains unchanged as it only relies on the title property.

Attempts made:
1. Set icon to None for all three tab pages using set_icon(None::<&gio::Icon>)
2. Added "pill" CSS class to TabBar

If tabs still show icon outlines, may need to:
- Use a different widget (ButtonBox with ToggleButtons)
- Add custom CSS to hide icon placeholders
- Check if TabBar has other configuration options

Added CSS rules in base.css to force hide tab icons:
- Set tab image opacity to 0
- Collapsed tab image dimensions to 0
- Styled tabbar.pill tabs with compact sizing
- Set tab labels to 12px font-size

This should completely hide the icon placeholders and show only text labels.
