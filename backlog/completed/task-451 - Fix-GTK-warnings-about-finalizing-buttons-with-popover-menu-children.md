---
id: task-451
title: Fix GTK warnings about finalizing buttons with popover menu children
status: Done
assignee: []
created_date: '2025-10-23 02:09'
updated_date: '2025-10-23 02:14'
labels: []
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The application generates numerous GTK warnings about improper widget cleanup when UI sections are cleared and rebuilt. The warnings indicate that GtkButton widgets are being finalized while they still have GtkPopoverMenu children attached, which violates GTK widget lifecycle rules.

Example warning:
```
Gtk-WARNING **: Finalizing GtkButton 0x55fac353f0d0, but it still has children left:
   - GtkPopoverMenu 0x55fac3655dc0
```

These warnings appear primarily when:
- Home page sections are cleared during refresh cycles
- Episode grids are rebuilt in show details page
- Media cards with context menus are removed from the UI

This indicates that popover menus are not being properly detached or destroyed before their parent buttons are finalized. While these are warnings rather than errors, they indicate incorrect widget management that could lead to memory leaks or undefined behavior.

The expected behavior is that all child widgets should be properly cleaned up before parent widgets are finalized, with no GTK warnings during normal UI operations.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 No GTK warnings about finalizing buttons with children during home page section refreshes
- [ ] #2 No GTK warnings about finalizing buttons with children during show details episode grid updates
- [ ] #3 Popover menus are properly detached or destroyed before their parent buttons are finalized
- [ ] #4 Widget cleanup follows GTK best practices for parent-child widget relationships
- [ ] #5 Application runs without GTK widget lifecycle warnings during normal operations
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Related to task-449 (UI continuously refreshing during playback) - that task prevents unnecessary refreshes, while this task ensures proper widget cleanup when refreshes legitimately occur (e.g., navigation between pages)

Fixed by implementing proper widget cleanup lifecycle:
- Added popover field to MediaCard struct to store reference
- Implemented shutdown method to unparent popover before button finalization
- This ensures GTK widget parent-child relationships are properly cleaned up

The shutdown method is called by Relm4 when factory components are removed, ensuring popovers are detached before their parent buttons are finalized.
<!-- SECTION:NOTES:END -->
